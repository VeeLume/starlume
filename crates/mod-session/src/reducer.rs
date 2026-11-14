//! The reducer: folds `Game.log` events and telemetry samples into the
//! current [`Session`].
//!
//! Pure: every time comes in with its input, nothing reads a clock, a file
//! or a socket. The shell owns one [`Reducer`] and calls it from the two
//! places that already hear the sources (the log watcher's signal callback
//! and the telemetry read loop).
//!
//! # What a session is
//!
//! **With the log:** one `Game.log`, from its first line to its quit line
//! (or to the moment the watcher saw the game gone). The id is the first
//! line's timestamp, so reading the same log again (a replay after an app
//! restart, a log picked up again) rebuilds the *same* session instead of a
//! second one.
//!
//! **Without the log:** from the first telemetry sample to the moment the
//! shell says the game is closed.
//!
//! **Both:** the log defines the session. Telemetry that arrived before the
//! log's first batch (or that was kept from before a rebuild) is taken into
//! it: its entries, and its wallet track.
//!
//! # Replay
//!
//! Replayed batches build the session exactly like live ones; the reducer
//! never *does* anything on an event, so there is nothing to suppress. The
//! flag only decides where a session without a quit line ends: one that was
//! only ever replayed ends at its last recorded moment, not "now".

use std::collections::HashMap;

use svc_log::{ContractChange, LogEvent, LogEventKind, MoneyChange, TradeSide};

use crate::model::{
    Entry, EntryKind, MAX_TIMELINE, MissionOutcome, RequestOutcome, Session, ShardStay, Source,
};
use crate::time::{Millis, parse_ts, session_id};
use crate::wallet::Ledger;

/// With telemetry alone, a session interrupted by an app restart carries on
/// when the stream is back within this long of its last recorded moment.
/// Past it the stream opens a new session. (Nothing but the log can tell a
/// game that kept running from one that was restarted, so a game restarted
/// inside this window, while Starlume was closed too, joins the old session.)
pub const RESUME_WITHIN: Millis = 10 * 60_000;
/// Most missions remembered at once (titles, and which already ended).
const MAX_MISSIONS: usize = 512;

/// What the stream said, reduced to what the session keeps. The shell maps
/// svc-telemetry's snapshot onto this.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TelemetrySample {
    /// When the frame arrived.
    pub at: Millis,
    /// aUEC; `None` when the frame said nothing about it.
    pub balance: Option<i64>,
    /// The vehicle the player is seated in.
    pub vehicle: Option<Vehicle>,
    /// The events of this frame.
    pub events: Vec<TelemetryEvent>,
}

/// A vehicle as the stream names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vehicle {
    /// The class key (`KRIG_S65_Stingray`).
    pub class_name: String,
    /// The entity name: tells two ships of one class apart.
    pub name: String,
}

/// The stream's events the session records. Seat events are not among
/// them: the game's are incomplete, so vehicles come from
/// [`TelemetrySample::vehicle`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelemetryEvent {
    Spawn,
    Dead,
    Downed,
    Respawn,
}

/// What a call did.
#[derive(Debug, Default, PartialEq)]
pub struct Outcome {
    /// The current session differs from before the call.
    pub changed: bool,
    /// A session ended with this call: file it. It stays the current
    /// session (not live) until the next one starts.
    pub finished: Option<Session>,
}

impl Outcome {
    fn changed(changed: bool) -> Self {
        Self {
            changed,
            finished: None,
        }
    }
}

#[derive(Debug, Default)]
struct Mission {
    title: Option<String>,
    contract: Option<String>,
    /// The `seq` of its `MissionEnded` entry.
    ended: Option<u32>,
}

/// Folds both sources into the current session. See the module docs.
#[derive(Debug, Default)]
pub struct Reducer {
    /// The live session, or the last one that ended.
    current: Option<Session>,
    /// An ended, log-built session set aside when telemetry began a new
    /// one: taken back in if the log then turns out to be the same game
    /// run. One slot, emptied when a log session begins and when the
    /// stream stops, so it never outlives the game it could belong to.
    shelved: Option<Session>,
    /// `current` was closed because the app stopped, not the game
    /// ([`Reducer::restore`]): the one case telemetry may reopen it.
    interrupted: bool,
    /// A sample arrived since the stream last stopped.
    stream_live: bool,
    /// A logged session ended while the stream was live: that stream is
    /// the game shutting down (or idling after its quit line), and opens
    /// no session of its own until it has stopped.
    await_stream_end: bool,
    /// A log session was announced (its channel inside); its first batch
    /// starts it.
    announced: Option<Option<String>>,
    ledger: Ledger,
    missions: HashMap<String, Mission>,
    /// The `seq` of the request the next shop answer belongs to.
    last_request: Option<u32>,
    last_location: Option<String>,
    last_jurisdiction: Option<String>,
    /// The vehicle of the last sample: stream state, kept across sessions
    /// so a new one does not re-enter the ship the player sits in.
    vehicle: Option<Vehicle>,
    /// The quit line's time.
    quit_at: Option<Millis>,
    /// Every batch of this log session so far was a replay.
    replay_only: bool,
}

/// Add an entry at its place in time (after entries of the same instant).
fn push(session: &mut Session, at: Millis, source: Source, locale: bool, kind: EntryKind) -> u32 {
    let seq = session.next_seq;
    session.next_seq += 1;
    let pos = session
        .timeline
        .iter()
        .rposition(|e| e.at <= at)
        .map_or(0, |i| i + 1);
    session.timeline.insert(
        pos,
        Entry {
            seq,
            at,
            source,
            locale_dependent: locale,
            kind,
        },
    );
    if session.timeline.len() > MAX_TIMELINE {
        session.timeline.remove(0);
        session.timeline_dropped = session.timeline_dropped.saturating_add(1);
    }
    session.last_activity_at = session.last_activity_at.max(at);
    seq
}

/// Take the telemetry half of `old` (the same game run, seen earlier) into
/// `session`: its entries under new `seq`s, and its wallet track.
fn adopt(session: &mut Session, old: &Session) {
    if !old.sources.telemetry {
        return;
    }
    session.sources.telemetry = true;
    for entry in old
        .timeline
        .iter()
        .filter(|e| e.source == Source::Telemetry)
    {
        push(
            session,
            entry.at,
            Source::Telemetry,
            false,
            entry.kind.clone(),
        );
    }
    session.wallet.append(&old.wallet);
    session.last_activity_at = session.last_activity_at.max(old.last_activity_at);
}

/// The vehicle the session's timeline leaves the player seated in.
fn seated_in(session: &Session) -> Option<Vehicle> {
    session
        .timeline
        .iter()
        .rev()
        .find_map(|e| match &e.kind {
            EntryKind::VehicleEntered { class_name, name } => Some(Some(Vehicle {
                class_name: class_name.clone(),
                name: name.clone(),
            })),
            EntryKind::VehicleLeft { .. } => Some(None),
            _ => None,
        })
        .flatten()
}

fn finish(session: &mut Session, ended_at: Millis) -> Session {
    let ended_at = ended_at.max(session.started_at);
    session.live = false;
    session.ended_at = Some(ended_at);
    if let Some(stay) = session.shards.last_mut()
        && stay.left_at.is_none()
    {
        stay.left_at = Some(ended_at);
    }
    session.clone()
}

fn outcome_of(completion: &str) -> MissionOutcome {
    match completion {
        "Complete" => MissionOutcome::Completed,
        "Fail" => MissionOutcome::Failed,
        "Abandon" => MissionOutcome::Abandoned,
        _ => MissionOutcome::Ended,
    }
}

impl Reducer {
    /// The live session, or the last one that ended; `None` when none was
    /// seen.
    pub fn current(&self) -> Option<&Session> {
        self.current.as_ref()
    }

    /// Put back the session kept from the last run of the app. It was live
    /// when the app stopped, so it is closed at its last recorded moment
    /// and returned for filing. If the game still runs, the log's replay
    /// rebuilds it under the same id and takes its telemetry half back in;
    /// with telemetry alone, the stream reopens it if it is back within
    /// [`RESUME_WITHIN`].
    pub fn restore(&mut self, mut session: Session) -> Session {
        if session.live || session.ended_at.is_none() {
            let at = session.last_activity_at;
            finish(&mut session, at);
        }
        self.current = Some(session.clone());
        self.interrupted = true;
        session
    }

    /// Forget everything: both sources are off, or the module is. Nothing
    /// stays in memory; what was finished is already filed.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    // ── Game.log ────────────────────────────────────────────────────────

    /// The watcher follows a log (svc-log's `SessionStarted` signal).
    /// `channel` is the install the log belongs to. The session itself
    /// starts with the first batch, which must begin at the log's first
    /// line: the log's own start line dates the session and so names it.
    /// (A caller holding only a later part of the log reads the whole log
    /// first; a batch without the start line is dated by its first event,
    /// which makes a different id.)
    pub fn log_session_started(&mut self, channel: Option<String>) {
        self.announced = Some(channel);
    }

    /// A batch of the followed log, oldest first.
    pub fn log_events(&mut self, events: &[LogEvent], replay: bool) -> Outcome {
        let mut changed = false;
        if let Some(channel) = self.announced.take() {
            let start_line = events
                .iter()
                .filter(|e| matches!(e.event, LogEventKind::SessionStarted));
            let Some(started_at) = start_line.chain(events).find_map(|e| parse_ts(&e.ts)) else {
                // Nothing datable yet: wait for a batch that is.
                self.announced = Some(channel);
                return Outcome::default();
            };
            self.begin_log_session(started_at, channel);
            changed = true;
        }
        self.replay_only &= replay;
        if !self
            .current
            .as_ref()
            .is_some_and(|s| s.live && s.sources.log)
        {
            // Events without a session being followed: not ours to place.
            return Outcome::changed(changed);
        }
        let mut session = self.current.take().expect("checked above");
        for event in events {
            let at = parse_ts(&event.ts).unwrap_or(session.last_activity_at);
            changed |= self.log_event(&mut session, at, &event.event);
        }
        self.current = Some(session);
        Outcome::changed(changed)
    }

    fn begin_log_session(&mut self, started_at: Millis, channel: Option<String>) {
        let mut session = Session {
            id: session_id(started_at),
            started_at,
            last_activity_at: started_at,
            live: true,
            channel,
            ..Session::default()
        };
        session.sources.log = true;

        // The same game run, seen earlier: a telemetry-only session that is
        // live right now, or an ended one this very log already built.
        let same_run = |old: &Session| (old.live && !old.sources.log) || old.id == session.id;
        let earlier: Vec<Session> = [self.shelved.take(), self.current.take()]
            .into_iter()
            .flatten()
            .filter(same_run)
            .collect();
        for old in &earlier {
            adopt(&mut session, old);
        }

        // Seated when this run was last seen (an app restart, a log picked
        // up again): the next sample must not enter the same ship twice.
        if self.vehicle.is_none() {
            self.vehicle = seated_in(&session);
        }
        self.current = Some(session);
        self.interrupted = false;
        self.await_stream_end = false;
        self.ledger = Ledger::default();
        self.missions.clear();
        self.last_request = None;
        self.last_location = None;
        self.last_jurisdiction = None;
        self.quit_at = None;
        self.replay_only = true;
    }

    /// The followed log's session is over (svc-log's `SessionEnded` signal,
    /// or the log opt-in went off). `now` dates the end when the log has no
    /// quit line and the session was followed live.
    pub fn log_session_ended(&mut self, now: Millis) -> Outcome {
        self.announced = None;
        let Some(session) = self.current.as_mut().filter(|s| s.live && s.sources.log) else {
            return Outcome::default();
        };
        let ended_at = match self.quit_at {
            Some(quit) => quit,
            None if self.replay_only => session.last_activity_at,
            None => now.max(session.last_activity_at),
        };
        let finished = finish(session, ended_at);
        self.vehicle = None;
        self.interrupted = false;
        self.await_stream_end = self.stream_live;
        Outcome {
            changed: true,
            finished: Some(finished),
        }
    }

    fn log_event(&mut self, session: &mut Session, at: Millis, event: &LogEventKind) -> bool {
        use LogEventKind as E;
        let log = |session: &mut Session, locale: bool, kind: EntryKind| {
            push(session, at, Source::Log, locale, kind)
        };
        match event {
            // The handle and the account id are not kept: nothing here
            // needs them, and a session file is easier to share without.
            E::SessionStarted | E::SessionHandle { .. } | E::SessionAccountId { .. } => {
                return false;
            }
            E::Branch { name } => session.branch = Some(name.clone()),
            E::Changelist { number } => session.build = Some(number.clone()),
            E::SessionPlatform { env_tag } => session.env_tag = Some(env_tag.clone()),
            E::SessionEnded { reason } => {
                self.quit_at = Some(at);
                session.end_reason = Some(reason.clone());
                session.last_activity_at = session.last_activity_at.max(at);
            }
            E::ShardJoined { shard } => {
                if let Some(stay) = session.shards.last_mut()
                    && stay.left_at.is_none()
                {
                    stay.left_at = Some(at);
                }
                session.shards.push(ShardStay {
                    shard: shard.clone(),
                    joined_at: at,
                    left_at: None,
                });
                log(
                    session,
                    false,
                    EntryKind::ShardJoined {
                        shard: shard.clone(),
                    },
                );
            }
            E::LocationInventoryRequested { location } => {
                // Also logged on opening the inventory where one already is.
                if self.last_location.as_ref() == Some(location) {
                    return false;
                }
                self.last_location = Some(location.clone());
                log(
                    session,
                    false,
                    EntryKind::Location {
                        location: location.clone(),
                    },
                );
            }
            E::JurisdictionEntered { jurisdiction } => {
                if self.last_jurisdiction.as_ref() == Some(jurisdiction) {
                    return false;
                }
                self.last_jurisdiction = Some(jurisdiction.clone());
                log(
                    session,
                    true,
                    EntryKind::Jurisdiction {
                        jurisdiction: jurisdiction.clone(),
                    },
                );
            }
            E::Contract {
                change,
                mission_id,
                title,
            } => match change {
                // Offers and shares are not something the player did.
                ContractChange::Available | ContractChange::Shared => return false,
                ContractChange::Accepted => {
                    if let Some(id) = mission_id {
                        self.mission(id).title = Some(title.clone());
                    }
                    log(
                        session,
                        true,
                        EntryKind::MissionAccepted {
                            mission_id: mission_id.clone(),
                            title: title.clone(),
                        },
                    );
                }
                ContractChange::Completed => {
                    return self.mission_ended(
                        session,
                        at,
                        mission_id.as_deref(),
                        Some(title),
                        MissionOutcome::Completed,
                        true,
                    );
                }
                ContractChange::Failed => {
                    return self.mission_ended(
                        session,
                        at,
                        mission_id.as_deref(),
                        Some(title),
                        MissionOutcome::Failed,
                        true,
                    );
                }
                ContractChange::Withdrawn => {
                    return self.mission_ended(
                        session,
                        at,
                        mission_id.as_deref(),
                        Some(title),
                        MissionOutcome::Withdrawn,
                        true,
                    );
                }
            },
            E::Objective {
                change,
                mission_id,
                text,
                ..
            } => {
                log(
                    session,
                    true,
                    EntryKind::Objective {
                        change: *change,
                        mission_id: mission_id.clone(),
                        text: text.clone(),
                    },
                );
            }
            // The server's objective pushes repeat the HUD's and add the
            // hidden ones: too many for a timeline.
            E::ObjectiveUpserted { .. } => return false,
            E::MissionMarkerCreated {
                mission_id,
                contract,
                ..
            } => {
                self.mission(mission_id).contract = Some(contract.clone());
                return false;
            }
            E::MissionEnded {
                mission_id,
                completion,
                ..
            } => {
                return self.mission_ended(
                    session,
                    at,
                    Some(mission_id),
                    None,
                    outcome_of(completion),
                    false,
                );
            }
            E::Money { change, amount } => {
                let amount = amount.round() as i64;
                let seq = log(
                    session,
                    true,
                    EntryKind::Money {
                        change: *change,
                        amount,
                    },
                );
                let signed = match change {
                    MoneyChange::Awarded => amount,
                    MoneyChange::Fined => -amount,
                };
                self.ledger
                    .explain(&mut session.wallet, at, seq, Some(signed));
            }
            E::CommodityTradeRequested {
                side,
                shop,
                resource_guid,
                price,
                quantity,
                quantity_unit,
            } => {
                let price = price.round() as i64;
                let seq = log(
                    session,
                    false,
                    EntryKind::Trade {
                        side: *side,
                        shop: shop.clone(),
                        resource_guid: resource_guid.clone(),
                        price,
                        quantity: *quantity,
                        quantity_unit: quantity_unit.clone(),
                        outcome: RequestOutcome::Requested,
                    },
                );
                self.last_request = Some(seq);
                let signed = match side {
                    TradeSide::Buy => -price,
                    TradeSide::Sell => price,
                };
                if signed != 0 {
                    self.ledger
                        .explain(&mut session.wallet, at, seq, Some(signed));
                }
            }
            E::ShopBuyRequested {
                shop,
                item_guid,
                item_name,
                price,
                quantity,
            } => self.shop_request(
                session, at, shop, item_guid, item_name, *price, *quantity, false,
            ),
            E::ShopRentalRequested {
                shop,
                item_guid,
                item_name,
                price,
            } => self.shop_request(session, at, shop, item_guid, item_name, *price, 1, true),
            E::ShopFlowResponse { success, .. } => {
                // The line does not say which request it answers: the one
                // just before it (svc-log's pairing rule).
                let Some(seq) = self.last_request.take() else {
                    return false;
                };
                let answer = if *success {
                    RequestOutcome::Confirmed
                } else {
                    self.ledger.retract(&mut session.wallet, seq);
                    RequestOutcome::Rejected
                };
                match session.timeline.iter_mut().rev().find(|e| e.seq == seq) {
                    Some(Entry {
                        kind:
                            EntryKind::Trade { outcome, .. } | EntryKind::ShopPurchase { outcome, .. },
                        ..
                    }) => *outcome = answer,
                    _ => return false,
                }
            }
            E::RefineryOrderCompleted { location } => {
                log(
                    session,
                    true,
                    EntryKind::RefineryOrderCompleted {
                        location: location.clone(),
                    },
                );
            }
            E::BlueprintReceived { name } => {
                log(
                    session,
                    true,
                    EntryKind::BlueprintReceived { name: name.clone() },
                );
            }
        }
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn shop_request(
        &mut self,
        session: &mut Session,
        at: Millis,
        shop: &str,
        item_guid: &str,
        item_name: &str,
        price: f64,
        quantity: u32,
        rental: bool,
    ) {
        let price = price.round() as i64;
        let seq = push(
            session,
            at,
            Source::Log,
            false,
            EntryKind::ShopPurchase {
                shop: shop.to_owned(),
                item_guid: item_guid.to_owned(),
                item_name: item_name.to_owned(),
                price,
                quantity,
                rental,
                outcome: RequestOutcome::Requested,
            },
        );
        self.last_request = Some(seq);
        // A free rental moves no money.
        if price > 0 {
            self.ledger
                .explain(&mut session.wallet, at, seq, Some(-price));
        }
    }

    fn mission(&mut self, id: &str) -> &mut Mission {
        if self.missions.len() >= MAX_MISSIONS && !self.missions.contains_key(id) {
            self.missions.retain(|_, m| m.ended.is_none());
            if self.missions.len() >= MAX_MISSIONS {
                self.missions.clear();
            }
        }
        self.missions.entry(id.to_owned()).or_default()
    }

    /// A mission is over, said by the HUD (`title`, locale-dependent) or by
    /// the engine. Both usually say it: the second one completes the entry
    /// of the first instead of adding another.
    fn mission_ended(
        &mut self,
        session: &mut Session,
        at: Millis,
        mission_id: Option<&str>,
        title: Option<&String>,
        outcome: MissionOutcome,
        locale: bool,
    ) -> bool {
        let known = mission_id.and_then(|id| self.missions.get(id));
        if let Some(seq) = known.and_then(|m| m.ended) {
            let Some(entry) = session.timeline.iter_mut().rev().find(|e| e.seq == seq) else {
                // Its entry fell off the capped timeline: said already.
                return false;
            };
            let before = entry.clone();
            if let EntryKind::MissionEnded {
                title: had_title,
                outcome: had_outcome,
                ..
            } = &mut entry.kind
            {
                if had_title.is_none() {
                    *had_title = title.cloned();
                }
                if *had_outcome == MissionOutcome::Ended {
                    *had_outcome = outcome;
                }
            }
            // One engine line is enough to make it locale-proof.
            entry.locale_dependent &= locale;
            return *entry != before;
        }

        let kind = EntryKind::MissionEnded {
            mission_id: mission_id.map(str::to_owned),
            title: title
                .cloned()
                .or_else(|| known.and_then(|m| m.title.clone())),
            contract: known.and_then(|m| m.contract.clone()),
            outcome,
        };
        let seq = push(session, at, Source::Log, locale, kind);
        if let Some(id) = mission_id {
            self.mission(id).ended = Some(seq);
        }
        if outcome == MissionOutcome::Completed {
            // The log never says what a mission paid.
            self.ledger.explain(&mut session.wallet, at, seq, None);
        }
        true
    }

    // ── Telemetry ───────────────────────────────────────────────────────

    /// A telemetry sample. With no session live it starts a telemetry-only
    /// one, or reopens the one an app restart interrupted; right after a
    /// logged session ended it does neither until the stream has stopped.
    pub fn telemetry(&mut self, sample: &TelemetrySample) -> Outcome {
        let mut changed = false;
        self.stream_live = true;
        if !self.current.as_ref().is_some_and(|s| s.live) {
            if self.await_stream_end {
                return Outcome::default();
            }
            let resumable = self.interrupted
                && self.current.as_ref().is_some_and(|s| {
                    !s.sources.log
                        && (0..=RESUME_WITHIN).contains(&(sample.at - s.last_activity_at))
                });
            self.interrupted = false;
            if resumable {
                let session = self.current.as_mut().expect("checked above");
                session.live = true;
                session.ended_at = None;
                if self.vehicle.is_none() {
                    self.vehicle = seated_in(session);
                }
            } else {
                let mut session = Session {
                    id: session_id(sample.at),
                    started_at: sample.at,
                    last_activity_at: sample.at,
                    live: true,
                    ..Session::default()
                };
                session.sources.telemetry = true;
                // An ended session the log built may be this same game run
                // (the app restarted): the log decides, if it comes.
                self.shelved = self.current.replace(session).filter(|s| s.sources.log);
            }
            changed = true;
        }
        let session = self.current.as_mut().expect("a live session");
        if !session.sources.telemetry {
            session.sources.telemetry = true;
            changed = true;
        }

        let seated = |v: &Option<Vehicle>| v.as_ref().map(|v| v.name.clone());
        if seated(&sample.vehicle) != seated(&self.vehicle) {
            if let Some(left) = self.vehicle.take() {
                push(
                    session,
                    sample.at,
                    Source::Telemetry,
                    false,
                    EntryKind::VehicleLeft {
                        class_name: left.class_name,
                        name: left.name,
                    },
                );
            }
            if let Some(entered) = &sample.vehicle {
                push(
                    session,
                    sample.at,
                    Source::Telemetry,
                    false,
                    EntryKind::VehicleEntered {
                        class_name: entered.class_name.clone(),
                        name: entered.name.clone(),
                    },
                );
            }
            self.vehicle = sample.vehicle.clone();
            changed = true;
        }

        for event in &sample.events {
            let kind = match event {
                TelemetryEvent::Spawn => EntryKind::Spawned,
                TelemetryEvent::Dead => EntryKind::Died,
                TelemetryEvent::Downed => EntryKind::Downed,
                TelemetryEvent::Respawn => EntryKind::Respawned,
            };
            push(session, sample.at, Source::Telemetry, false, kind);
            changed = true;
        }

        if let Some(balance) = sample.balance {
            changed |= session.wallet.observe(sample.at, balance, &mut self.ledger);
        }
        if changed {
            session.last_activity_at = session.last_activity_at.max(sample.at);
        }
        Outcome::changed(changed)
    }

    /// The game is closed, or the telemetry opt-in went off. Ends a
    /// telemetry-only session; one the log defines ends with its log.
    pub fn telemetry_stopped(&mut self, now: Millis) -> Outcome {
        self.vehicle = None;
        self.stream_live = false;
        self.await_stream_end = false;
        // No stream, so no log replay of the same run is still to come.
        self.shelved = None;
        let Some(session) = self.current.as_mut().filter(|s| s.live && !s.sources.log) else {
            return Outcome::default();
        };
        let ended_at = now.max(session.last_activity_at);
        let finished = finish(session, ended_at);
        Outcome {
            changed: true,
            finished: Some(finished),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Cause;
    use svc_log::ObjectiveChange;

    // Every id, shard and shop name below is made up, in the shape the game
    // writes them.

    const START: &str = "2026-11-13T16:10:48.019Z";

    fn ms(ts: &str) -> Millis {
        parse_ts(ts).unwrap()
    }

    fn ev(ts: &str, event: LogEventKind) -> LogEvent {
        LogEvent {
            ts: ts.to_owned(),
            event,
        }
    }

    fn header() -> Vec<LogEvent> {
        vec![
            ev(START, LogEventKind::SessionStarted),
            ev(
                "2026-11-13T16:10:48.020Z",
                LogEventKind::Branch {
                    name: "sc-alpha-4.10.0".into(),
                },
            ),
            ev(
                "2026-11-13T16:10:48.021Z",
                LogEventKind::Changelist {
                    number: "13165946".into(),
                },
            ),
            ev(
                "2026-11-13T16:10:49.000Z",
                LogEventKind::SessionPlatform {
                    env_tag: "PUB".into(),
                },
            ),
            ev(
                "2026-11-13T16:11:30.000Z",
                LogEventKind::SessionHandle {
                    handle: "TestPilot".into(),
                },
            ),
            ev(
                "2026-11-13T16:11:31.000Z",
                LogEventKind::SessionAccountId {
                    account_id: "0000001".into(),
                },
            ),
        ]
    }

    fn joined(ts: &str, shard: &str) -> LogEvent {
        ev(
            ts,
            LogEventKind::ShardJoined {
                shard: shard.into(),
            },
        )
    }

    fn quit(ts: &str) -> LogEvent {
        ev(
            ts,
            LogEventKind::SessionEnded {
                reason: "Quit via console command".into(),
            },
        )
    }

    fn started(events: &[LogEvent], replay: bool) -> Reducer {
        let mut r = Reducer::default();
        r.log_session_started(Some("LIVE".into()));
        assert!(r.log_events(events, replay).changed);
        r
    }

    fn kinds(session: &Session) -> Vec<&EntryKind> {
        session.timeline.iter().map(|e| &e.kind).collect()
    }

    fn sample(ts: &str, balance: i64) -> TelemetrySample {
        TelemetrySample {
            at: ms(ts),
            balance: Some(balance),
            ..TelemetrySample::default()
        }
    }

    fn stingray() -> Option<Vehicle> {
        Some(Vehicle {
            class_name: "KRIG_S65_Stingray".into(),
            name: "KRIG_S65_Stingray_7700112233".into(),
        })
    }

    #[test]
    fn a_log_session_runs_from_the_first_line_to_the_quit_line() {
        let mut events = header();
        events.push(joined("2026-11-13T16:13:05.561Z", "pub_euw1b_13165946_100"));
        let mut r = started(&events, false);

        let s = r.current().unwrap();
        assert_eq!(s.id, "2026-11-13T16-10-48.019Z");
        assert_eq!(s.started_at, ms(START));
        assert!(s.live);
        assert_eq!(s.channel.as_deref(), Some("LIVE"));
        assert_eq!(s.env_tag.as_deref(), Some("PUB"));
        assert_eq!(s.branch.as_deref(), Some("sc-alpha-4.10.0"));
        assert_eq!(s.build.as_deref(), Some("13165946"));
        assert_eq!(
            (s.sources.log, s.sources.telemetry),
            (true, false),
            "only the log fed it"
        );
        let json = serde_json::to_string(s).unwrap();
        assert!(
            !json.contains("TestPilot") && !json.contains("0000001"),
            "the handle and the account id are not kept"
        );

        let out = r.log_events(&[quit("2026-11-13T18:40:00.000Z")], false);
        assert!(out.changed && out.finished.is_none());
        // The watcher's signal follows; "now" is later than the quit line.
        let out = r.log_session_ended(ms("2026-11-13T18:40:09.000Z"));
        let done = out.finished.unwrap();
        assert!(!done.live);
        assert_eq!(done.ended_at, Some(ms("2026-11-13T18:40:00.000Z")));
        assert_eq!(done.end_reason.as_deref(), Some("Quit via console command"));
        assert_eq!(
            done.shards[0].left_at, done.ended_at,
            "the last stay ends with the session"
        );
        assert_eq!(r.current(), Some(&done), "it stays as the last session");
        assert_eq!(
            r.log_session_ended(0),
            Outcome::default(),
            "ended only once"
        );
    }

    #[test]
    fn shard_joins_have_durations() {
        let mut events = header();
        events.push(joined("2026-11-13T16:13:00.000Z", "pub_euw1b_13165946_100"));
        events.push(joined("2026-11-13T17:13:00.000Z", "pub_euw1b_13165946_042"));
        let r = started(&events, false);
        let s = r.current().unwrap();
        assert_eq!(s.shards.len(), 2);
        assert_eq!(
            s.shards[0].left_at.unwrap() - s.shards[0].joined_at,
            3_600_000
        );
        assert_eq!(s.shards[1].left_at, None, "still on it");
        assert_eq!(
            kinds(s),
            [
                &EntryKind::ShardJoined {
                    shard: "pub_euw1b_13165946_100".into()
                },
                &EntryKind::ShardJoined {
                    shard: "pub_euw1b_13165946_042".into()
                },
            ]
        );
        assert!(s.timeline.iter().all(|e| e.source == Source::Log));
    }

    #[test]
    fn a_place_is_recorded_when_it_changes() {
        let at = |location: &str, ts: &str| {
            ev(
                ts,
                LogEventKind::LocationInventoryRequested {
                    location: location.into(),
                },
            )
        };
        let mut events = header();
        events.extend([
            at("Stanton4_NewBabbage", "2026-11-13T16:20:00.000Z"),
            // Opening the inventory again in the same place.
            at("Stanton4_NewBabbage", "2026-11-13T16:21:00.000Z"),
            at("RR_JP_StantonMagnus", "2026-11-13T16:50:00.000Z"),
            ev(
                "2026-11-13T16:51:00.000Z",
                LogEventKind::JurisdictionEntered {
                    jurisdiction: "UEE".into(),
                },
            ),
            ev(
                "2026-11-13T16:52:00.000Z",
                LogEventKind::JurisdictionEntered {
                    jurisdiction: "UEE".into(),
                },
            ),
        ]);
        let r = started(&events, false);
        let s = r.current().unwrap();
        assert_eq!(
            kinds(s),
            [
                &EntryKind::Location {
                    location: "Stanton4_NewBabbage".into()
                },
                &EntryKind::Location {
                    location: "RR_JP_StantonMagnus".into()
                },
                &EntryKind::Jurisdiction {
                    jurisdiction: "UEE".into()
                },
            ]
        );
        assert!(!s.timeline[0].locale_dependent);
        assert!(s.timeline[2].locale_dependent, "read from HUD text");
    }

    const MISSION: &str = "0a1b2c3d-1111-4222-8333-444455556666";

    fn contract(ts: &str, change: ContractChange) -> LogEvent {
        ev(
            ts,
            LogEventKind::Contract {
                change,
                mission_id: Some(MISSION.into()),
                title: "Covalex Local Delivery".into(),
            },
        )
    }

    fn engine_end(ts: &str, completion: &str) -> LogEvent {
        ev(
            ts,
            LogEventKind::MissionEnded {
                mission_id: MISSION.into(),
                completion: completion.into(),
                reason: "Mission Ended".into(),
            },
        )
    }

    #[test]
    fn a_mission_goes_from_accepted_through_objectives_to_one_end_entry() {
        let mut events = header();
        events.extend([
            contract("2026-11-13T16:30:00.000Z", ContractChange::Available),
            contract("2026-11-13T16:30:05.000Z", ContractChange::Accepted),
            ev(
                "2026-11-13T16:30:06.000Z",
                LogEventKind::Objective {
                    change: ObjectiveChange::New,
                    mission_id: Some(MISSION.into()),
                    objective_id: Some("obj-1".into()),
                    text: "Collect the package".into(),
                },
            ),
            ev(
                "2026-11-13T16:30:06.100Z",
                LogEventKind::ObjectiveUpserted {
                    mission_id: MISSION.into(),
                    objective_id: "obj-1".into(),
                    state: "MISSION_OBJECTIVE_STATE_INPROGRESS".into(),
                },
            ),
            // The engine line and the HUD line both say it is over.
            engine_end("2026-11-13T16:45:00.000Z", "Complete"),
            contract("2026-11-13T16:45:00.200Z", ContractChange::Completed),
        ]);
        let r = started(&events, false);
        let s = r.current().unwrap();
        assert_eq!(
            kinds(s),
            [
                &EntryKind::MissionAccepted {
                    mission_id: Some(MISSION.into()),
                    title: "Covalex Local Delivery".into(),
                },
                &EntryKind::Objective {
                    change: ObjectiveChange::New,
                    mission_id: Some(MISSION.into()),
                    text: "Collect the package".into(),
                },
                &EntryKind::MissionEnded {
                    mission_id: Some(MISSION.into()),
                    title: Some("Covalex Local Delivery".into()),
                    contract: None,
                    outcome: MissionOutcome::Completed,
                },
            ]
        );
        assert!(s.timeline[0].locale_dependent);
        assert!(
            !s.timeline[2].locale_dependent,
            "the engine line said it too"
        );
        assert_eq!(s.summary().missions_completed, 1);
    }

    #[test]
    fn a_mission_ends_without_english_hud_text() {
        // A translated game: only engine lines fire.
        let mut events = header();
        events.extend([
            ev(
                "2026-11-13T16:30:05.000Z",
                LogEventKind::MissionMarkerCreated {
                    mission_id: MISSION.into(),
                    generator: "Covalex_Delivery".into(),
                    contract: "Covalex_LocalDelivery_Small".into(),
                    contract_definition_id: "9f8e7d6c-0000-4111-8222-333344445555".into(),
                    objective_id: "obj-1".into(),
                },
            ),
            engine_end("2026-11-13T16:45:00.000Z", "Abandon"),
        ]);
        let r = started(&events, false);
        let s = r.current().unwrap();
        assert_eq!(
            kinds(s),
            [&EntryKind::MissionEnded {
                mission_id: Some(MISSION.into()),
                title: None,
                contract: Some("Covalex_LocalDelivery_Small".into()),
                outcome: MissionOutcome::Abandoned,
            }]
        );
        assert_eq!(outcome_of("Fail"), MissionOutcome::Failed);
        assert_eq!(outcome_of("SomethingNew"), MissionOutcome::Ended);
    }

    #[test]
    fn the_hud_end_alone_is_one_entry_too() {
        let mut events = header();
        events.push(contract("2026-11-13T16:45:00.000Z", ContractChange::Failed));
        events.push(engine_end("2026-11-13T16:45:00.300Z", "Fail"));
        let r = started(&events, false);
        let s = r.current().unwrap();
        assert_eq!(s.timeline.len(), 1);
        assert!(!s.timeline[0].locale_dependent);
    }

    fn buy(ts: &str, price: f64) -> LogEvent {
        ev(
            ts,
            LogEventKind::ShopBuyRequested {
                shop: "SCShop_NewBabbage_Tools".into(),
                item_guid: "5e6f7a8b-aaaa-4bbb-8ccc-ddddeeeeffff".into(),
                item_name: "grin_tractor_01".into(),
                price,
                quantity: 1,
            },
        )
    }

    fn answer(ts: &str, success: bool) -> LogEvent {
        ev(
            ts,
            LogEventKind::ShopFlowResponse {
                success,
                result: if success { "Success" } else { "AuthorityError" }.into(),
            },
        )
    }

    fn purchase_outcome(session: &Session) -> RequestOutcome {
        match &session.timeline.last().unwrap().kind {
            EntryKind::ShopPurchase { outcome, .. } => *outcome,
            other => panic!("not a purchase: {other:?}"),
        }
    }

    #[test]
    fn a_shop_answer_settles_the_request_before_it() {
        let mut events = header();
        events.push(buy("2026-11-13T16:20:00.000Z", 19_175.0));
        let mut r = started(&events, false);
        assert_eq!(
            purchase_outcome(r.current().unwrap()),
            RequestOutcome::Requested
        );
        r.log_events(&[answer("2026-11-13T16:20:00.400Z", true)], false);
        assert_eq!(
            purchase_outcome(r.current().unwrap()),
            RequestOutcome::Confirmed
        );
        // An answer with no request before it changes nothing.
        assert!(
            !r.log_events(&[answer("2026-11-13T16:20:01.000Z", false)], false)
                .changed
        );

        r.log_events(
            &[
                buy("2026-11-13T16:25:00.000Z", 300.0),
                answer("2026-11-13T16:25:00.400Z", false),
            ],
            false,
        );
        assert_eq!(
            purchase_outcome(r.current().unwrap()),
            RequestOutcome::Rejected
        );
    }

    #[test]
    fn every_economy_event_is_an_entry() {
        let mut events = header();
        events.extend([
            ev(
                "2026-11-13T16:20:00.000Z",
                LogEventKind::CommodityTradeRequested {
                    side: TradeSide::Buy,
                    shop: "SCShop_Levski_CargoOffice_Commodities".into(),
                    resource_guid: "1a2b3c4d-9999-4888-8777-666655554444".into(),
                    price: 52_800.0,
                    quantity: 4800.0,
                    quantity_unit: Some("cSCU".into()),
                },
            ),
            ev(
                "2026-11-13T16:21:00.000Z",
                LogEventKind::ShopRentalRequested {
                    shop: "SCShop_Rentals_Area18".into(),
                    item_guid: "7c8d9e0f-2222-4333-8444-555566667777".into(),
                    item_name: "RSI_Constellation_Andromeda_GS".into(),
                    price: 0.0,
                },
            ),
            ev(
                "2026-11-13T16:22:00.000Z",
                LogEventKind::Money {
                    change: MoneyChange::Fined,
                    amount: 72_000.0,
                },
            ),
            ev(
                "2026-11-13T16:23:00.000Z",
                LogEventKind::RefineryOrderCompleted {
                    location: "Stanton Gateway".into(),
                },
            ),
            ev(
                "2026-11-13T16:24:00.000Z",
                LogEventKind::BlueprintReceived {
                    name: "Karna Rifle".into(),
                },
            ),
        ]);
        let r = started(&events, false);
        let s = r.current().unwrap();
        assert_eq!(s.timeline.len(), 5);
        assert!(matches!(
            s.timeline[0].kind,
            EntryKind::Trade {
                side: TradeSide::Buy,
                price: 52_800,
                outcome: RequestOutcome::Requested,
                ..
            }
        ));
        assert!(matches!(
            s.timeline[1].kind,
            EntryKind::ShopPurchase {
                rental: true,
                price: 0,
                ..
            }
        ));
        assert_eq!(
            s.timeline[2].kind,
            EntryKind::Money {
                change: MoneyChange::Fined,
                amount: 72_000
            }
        );
        let locale: Vec<bool> = s.timeline.iter().map(|e| e.locale_dependent).collect();
        assert_eq!(locale, [false, false, true, true, true]);
    }

    #[test]
    fn a_replayed_log_rebuilds_the_same_session() {
        let mut events = header();
        events.extend([
            joined("2026-11-13T16:13:05.561Z", "pub_euw1b_13165946_100"),
            contract("2026-11-13T16:30:05.000Z", ContractChange::Accepted),
            buy("2026-11-13T16:40:00.000Z", 19_175.0),
            answer("2026-11-13T16:40:00.400Z", true),
        ]);
        let live = started(&events, false);
        let replayed = started(&events, true);
        assert_eq!(live.current(), replayed.current());

        // In two batches, as a tailer would deliver them.
        let mut split = Reducer::default();
        split.log_session_started(Some("LIVE".into()));
        split.log_events(&events[..7], true);
        split.log_events(&events[7..], true);
        assert_eq!(split.current(), live.current());
    }

    #[test]
    fn a_session_that_was_only_replayed_ends_where_its_log_stops() {
        let mut events = header();
        events.push(joined("2026-11-13T16:13:05.561Z", "pub_euw1b_13165946_100"));
        let mut r = started(&events, true);
        // Hours later, the watcher finds the game gone.
        let done = r
            .log_session_ended(ms("2026-11-13T23:00:00.000Z"))
            .finished
            .unwrap();
        assert_eq!(done.ended_at, Some(ms("2026-11-13T16:13:05.561Z")));
        assert_eq!(done.end_reason, None);

        // Followed live, the end is when it was seen.
        let mut r = started(&events, true);
        r.log_events(
            &[contract(
                "2026-11-13T16:30:05.000Z",
                ContractChange::Accepted,
            )],
            false,
        );
        let done = r
            .log_session_ended(ms("2026-11-13T17:00:00.000Z"))
            .finished
            .unwrap();
        assert_eq!(done.ended_at, Some(ms("2026-11-13T17:00:00.000Z")));
    }

    #[test]
    fn an_announced_log_without_a_datable_line_starts_nothing() {
        let mut r = Reducer::default();
        r.log_session_started(None);
        assert_eq!(r.log_events(&[], false), Outcome::default());
        assert!(r.current().is_none());
        assert_eq!(r.log_session_ended(5), Outcome::default());
        // Events with no session followed are dropped.
        assert_eq!(r.log_events(&header(), false), Outcome::default());
        assert!(r.current().is_none());
    }

    #[test]
    fn telemetry_adds_vehicles_deaths_and_the_wallet_to_a_log_session() {
        let mut r = started(&header(), false);
        let mut first = sample("2026-11-13T16:14:00.000Z", 125_000);
        first.vehicle = stingray();
        assert!(r.telemetry(&first).changed);
        assert!(!r.telemetry(&first).changed, "the same sample again");

        let mut dead = sample("2026-11-13T16:20:00.000Z", 125_000);
        dead.events = vec![TelemetryEvent::Dead, TelemetryEvent::Respawn];
        r.telemetry(&dead);

        let s = r.current().unwrap();
        assert!(s.sources.log && s.sources.telemetry);
        assert_eq!(
            kinds(s),
            [
                &EntryKind::VehicleEntered {
                    class_name: "KRIG_S65_Stingray".into(),
                    name: "KRIG_S65_Stingray_7700112233".into(),
                },
                &EntryKind::VehicleLeft {
                    class_name: "KRIG_S65_Stingray".into(),
                    name: "KRIG_S65_Stingray_7700112233".into(),
                },
                &EntryKind::Died,
                &EntryKind::Respawned,
            ]
        );
        assert!(s.timeline.iter().all(|e| e.source == Source::Telemetry));
        assert_eq!(s.wallet.opening.unwrap().balance, 125_000);
    }

    #[test]
    fn a_balance_change_is_attributed_to_the_log_event_beside_it() {
        let mut r = started(&header(), false);
        r.telemetry(&sample("2026-11-13T16:14:00.000Z", 125_000));

        // The stream is instant; the log line arrives with the next poll.
        r.telemetry(&sample("2026-11-13T16:40:00.300Z", 105_825));
        r.log_events(
            &[
                buy("2026-11-13T16:40:00.000Z", 19_175.0),
                answer("2026-11-13T16:40:00.400Z", true),
            ],
            false,
        );
        // A mission pays an amount the log never states.
        r.log_events(&[engine_end("2026-11-13T16:50:00.000Z", "Complete")], false);
        r.telemetry(&sample("2026-11-13T16:50:01.000Z", 150_825));
        // Nothing in the log explains this one.
        r.telemetry(&sample("2026-11-13T17:30:00.000Z", 150_000));

        let s = r.current().unwrap();
        let causes: Vec<Option<&EntryKind>> = s
            .wallet
            .changes
            .iter()
            .map(|c| match c.cause {
                Cause::Entry { seq } => Some(&s.entry(seq).unwrap().kind),
                Cause::Untracked => None,
            })
            .collect();
        assert!(matches!(causes[0], Some(EntryKind::ShopPurchase { .. })));
        assert!(matches!(causes[1], Some(EntryKind::MissionEnded { .. })));
        assert_eq!(causes[2], None);
        assert_eq!(s.wallet.net, 25_000);
        assert_eq!(s.summary().net, 25_000);
    }

    #[test]
    fn a_rejected_purchase_does_not_explain_a_change() {
        let mut r = started(&header(), false);
        r.telemetry(&sample("2026-11-13T16:14:00.000Z", 125_000));
        r.log_events(&[buy("2026-11-13T16:40:00.000Z", 300.0)], false);
        r.telemetry(&sample("2026-11-13T16:40:00.300Z", 124_700));
        r.log_events(&[answer("2026-11-13T16:40:00.400Z", false)], false);
        assert_eq!(
            r.current().unwrap().wallet.changes[0].cause,
            Cause::Untracked
        );
    }

    #[test]
    fn a_zero_while_loading_is_not_a_loss() {
        let mut r = started(&header(), false);
        r.telemetry(&sample("2026-11-13T16:14:00.000Z", 125_000));
        r.telemetry(&sample("2026-11-13T16:15:00.000Z", 0));
        let wallet = &r.current().unwrap().wallet;
        assert_eq!(wallet.zero_since, Some(ms("2026-11-13T16:15:00.000Z")));
        assert_eq!(wallet.latest.unwrap().balance, 125_000);
        r.telemetry(&sample("2026-11-13T16:15:20.000Z", 125_000));
        let wallet = &r.current().unwrap().wallet;
        assert!(wallet.changes.is_empty());
        assert_eq!(wallet.zero_since, None);
    }

    #[test]
    fn telemetry_alone_is_a_session_until_the_game_closes() {
        let mut r = Reducer::default();
        let mut first = sample("2026-11-13T16:14:00.000Z", 125_000);
        first.vehicle = stingray();
        assert!(r.telemetry(&first).changed);
        r.telemetry(&sample("2026-11-13T16:30:00.000Z", 126_000));

        let s = r.current().unwrap();
        assert_eq!(s.id, "2026-11-13T16-14-00.000Z");
        assert!(s.live && !s.sources.log && s.sources.telemetry);
        assert_eq!(s.channel, None);
        assert!(s.shards.is_empty());

        // The log's end signal is not its end.
        assert_eq!(r.log_session_ended(0), Outcome::default());
        let done = r
            .telemetry_stopped(ms("2026-11-13T17:00:00.000Z"))
            .finished
            .unwrap();
        assert_eq!(done.ended_at, Some(ms("2026-11-13T17:00:00.000Z")));
        assert_eq!(done.wallet.net, 1_000);
        assert!(!done.is_trivial());
        assert_eq!(r.telemetry_stopped(0), Outcome::default());
    }

    #[test]
    fn a_stream_that_only_connected_is_trivial() {
        let mut r = Reducer::default();
        r.telemetry(&sample("2026-11-13T16:14:00.000Z", 125_000));
        let done = r
            .telemetry_stopped(ms("2026-11-13T16:14:05.000Z"))
            .finished
            .unwrap();
        assert!(done.is_trivial());
    }

    #[test]
    fn telemetry_stopping_does_not_end_a_log_session() {
        let mut r = started(&header(), false);
        r.telemetry(&sample("2026-11-13T16:14:00.000Z", 125_000));
        assert_eq!(
            r.telemetry_stopped(ms("2026-11-13T16:20:00.000Z")),
            Outcome::default()
        );
        assert!(r.current().unwrap().live);
    }

    #[test]
    fn telemetry_before_the_log_is_taken_into_the_log_session() {
        let mut r = Reducer::default();
        let mut first = sample("2026-11-13T16:14:00.000Z", 125_000);
        first.vehicle = stingray();
        r.telemetry(&first);
        let mut second = sample("2026-11-13T16:16:00.000Z", 124_000);
        second.vehicle = stingray();
        r.telemetry(&second);

        // The watcher catches up on the running game's log.
        r.log_session_started(Some("LIVE".into()));
        let out = r.log_events(&header(), true);
        assert!(out.changed && out.finished.is_none());

        let s = r.current().unwrap();
        assert_eq!(s.id, "2026-11-13T16-10-48.019Z", "the log dates it");
        assert!(s.sources.log && s.sources.telemetry);
        assert!(matches!(
            s.timeline[0].kind,
            EntryKind::VehicleEntered { .. }
        ));
        assert_eq!(s.wallet.opening.unwrap().balance, 125_000);
        assert_eq!(s.wallet.net, -1_000);
        // Still seated: no second "entered".
        let mut later = sample("2026-11-13T16:18:00.000Z", 124_000);
        later.vehicle = stingray();
        assert!(!r.telemetry(&later).changed);
    }

    #[test]
    fn a_log_read_again_keeps_the_telemetry_it_had_and_explains_it_again() {
        let mut events = header();
        events.push(buy("2026-11-13T16:40:00.000Z", 19_175.0));
        events.push(answer("2026-11-13T16:40:00.400Z", true));
        let mut r = started(&events, false);
        r.telemetry(&sample("2026-11-13T16:14:00.000Z", 125_000));
        r.telemetry(&sample("2026-11-13T16:40:00.300Z", 105_825));
        // The process check called the end too early; the log is picked up
        // again and replayed from its first line.
        r.log_session_ended(ms("2026-11-13T16:45:00.000Z"));
        r.log_session_started(Some("LIVE".into()));
        r.log_events(&events, true);

        let s = r.current().unwrap();
        assert!(s.live);
        assert_eq!(s.timeline.len(), 1, "no entry twice");
        assert_eq!(s.wallet.changes.len(), 1);
        let Cause::Entry { seq } = s.wallet.changes[0].cause else {
            panic!("the replayed purchase explains the change again");
        };
        assert!(matches!(
            s.entry(seq).unwrap().kind,
            EntryKind::ShopPurchase { .. }
        ));
    }

    #[test]
    fn a_restored_session_is_closed_and_taken_back_in_by_its_log() {
        // The app stopped mid-game with this session live.
        let mut before = started(&header(), false);
        before.telemetry(&sample("2026-11-13T16:14:00.000Z", 125_000));
        before.telemetry(&sample("2026-11-13T16:16:00.000Z", 124_000));
        let kept = before.current().unwrap().clone();

        let mut r = Reducer::default();
        let filed = r.restore(kept);
        assert!(!filed.live);
        assert_eq!(filed.ended_at, Some(ms("2026-11-13T16:16:00.000Z")));

        // The game still runs: telemetry reconnects first, then the log is
        // replayed.
        r.telemetry(&sample("2026-11-13T16:25:00.000Z", 130_000));
        r.log_session_started(Some("LIVE".into()));
        r.log_events(&header(), true);

        let s = r.current().unwrap();
        assert_eq!(s.id, filed.id);
        assert!(s.live);
        assert_eq!(s.wallet.opening.unwrap().balance, 125_000);
        assert_eq!(s.wallet.latest.unwrap().balance, 130_000);
        let steps: Vec<_> = s.wallet.changes.iter().map(|c| c.delta).collect();
        assert_eq!(steps, [-1_000, 6_000]);
    }

    #[test]
    fn another_game_run_does_not_take_the_last_session_in() {
        let mut r = started(&header(), false);
        r.telemetry(&sample("2026-11-13T16:14:00.000Z", 125_000));
        r.log_events(&[quit("2026-11-13T18:00:00.000Z")], false);
        r.log_session_ended(ms("2026-11-13T18:00:05.000Z"));

        // The game's last frames, after the quit line.
        let linger = sample("2026-11-13T18:00:02.000Z", 0);
        assert_eq!(r.telemetry(&linger), Outcome::default());
        assert!(!r.current().unwrap().live);

        // The next day.
        r.log_session_started(Some("LIVE".into()));
        r.log_events(
            &[ev("2026-11-14T10:00:00.000Z", LogEventKind::SessionStarted)],
            false,
        );
        let s = r.current().unwrap();
        assert_eq!(s.id, "2026-11-14T10-00-00.000Z");
        assert_eq!(s.wallet.opening, None);
        assert!(!s.sources.telemetry);
    }

    #[test]
    fn a_stream_that_outlives_the_quit_line_opens_no_session() {
        let mut r = started(&header(), false);
        r.telemetry(&sample("2026-11-13T16:14:00.000Z", 125_000));
        r.log_events(&[quit("2026-11-13T18:00:00.000Z")], false);
        r.log_session_ended(ms("2026-11-13T18:00:05.000Z"));

        // Minutes after the quit line the process still streams, seated.
        let mut idle = sample("2026-11-13T18:05:00.000Z", 125_000);
        idle.vehicle = stingray();
        assert_eq!(r.telemetry(&idle), Outcome::default());
        assert!(!r.current().unwrap().live);
        assert!(r.shelved.is_none());

        // The stream stops; the next run, with telemetry alone, is a session.
        assert_eq!(
            r.telemetry_stopped(ms("2026-11-13T18:06:00.000Z")),
            Outcome::default()
        );
        assert!(
            r.telemetry(&sample("2026-11-13T19:00:00.000Z", 125_000))
                .changed
        );
        let s = r.current().unwrap();
        assert_eq!(s.id, "2026-11-13T19-00-00.000Z");
        assert!(s.timeline.is_empty(), "nothing of the idle stream");
    }

    #[test]
    fn a_stream_starting_after_a_logged_session_without_telemetry_is_a_session() {
        let mut r = started(&header(), false);
        r.log_session_ended(ms("2026-11-13T18:00:05.000Z"));
        // No stream was live at the end, so there is none to wait out.
        assert!(
            r.telemetry(&sample("2026-11-13T18:30:00.000Z", 125_000))
                .changed
        );
        assert!(r.current().unwrap().live);
    }

    #[test]
    fn with_telemetry_alone_an_app_restart_keeps_one_session() {
        let mut before = Reducer::default();
        let mut seated = sample("2026-11-13T16:14:00.000Z", 125_000);
        seated.vehicle = stingray();
        before.telemetry(&seated);
        let kept = before.current().unwrap().clone();

        let mut r = Reducer::default();
        let filed = r.restore(kept);
        assert!(!filed.live);
        // The telemetry worker reports "waiting" once at startup.
        assert_eq!(
            r.telemetry_stopped(ms("2026-11-13T16:15:00.000Z")),
            Outcome::default()
        );

        let mut back = sample("2026-11-13T16:20:00.000Z", 126_000);
        back.vehicle = stingray();
        assert!(r.telemetry(&back).changed);
        let s = r.current().unwrap();
        assert_eq!(s.id, filed.id, "the same session, reopened");
        assert!(s.live);
        assert_eq!(s.ended_at, None);
        assert_eq!(s.timeline.len(), 1, "still seated: not entered twice");
        assert_eq!(s.wallet.net, 1_000);
        assert!(r.shelved.is_none());
    }

    #[test]
    fn a_stream_back_long_after_a_restart_is_a_new_session() {
        let mut before = Reducer::default();
        before.telemetry(&sample("2026-11-13T16:14:00.000Z", 125_000));
        before.telemetry(&sample("2026-11-13T16:15:00.000Z", 126_000));
        let kept = before.current().unwrap().clone();

        let mut r = Reducer::default();
        let filed = r.restore(kept);
        let late = filed.last_activity_at + RESUME_WITHIN + 1;
        r.telemetry(&TelemetrySample {
            at: late,
            balance: Some(90_000),
            ..TelemetrySample::default()
        });
        let s = r.current().unwrap();
        assert_ne!(s.id, filed.id);
        assert_eq!(s.wallet.opening.unwrap().balance, 90_000);
        assert!(
            r.shelved.is_none(),
            "a telemetry-only session is not held for a log that cannot come"
        );

        // Only a restart reopens: a session the game's end closed stays closed.
        let mut r = Reducer::default();
        r.telemetry(&sample("2026-11-13T16:14:00.000Z", 125_000));
        r.telemetry(&sample("2026-11-13T16:15:00.000Z", 126_000));
        let done = r
            .telemetry_stopped(ms("2026-11-13T16:16:00.000Z"))
            .finished
            .unwrap();
        r.telemetry(&sample("2026-11-13T16:17:00.000Z", 126_000));
        assert_ne!(r.current().unwrap().id, done.id);
    }

    #[test]
    fn a_log_replay_after_a_restart_does_not_enter_the_vehicle_twice() {
        let mut before = started(&header(), false);
        let mut seated = sample("2026-11-13T16:14:00.000Z", 125_000);
        seated.vehicle = stingray();
        before.telemetry(&seated);
        let kept = before.current().unwrap().clone();

        let mut r = Reducer::default();
        r.restore(kept);
        r.log_session_started(Some("LIVE".into()));
        r.log_events(&header(), true);
        let mut back = sample("2026-11-13T16:25:00.000Z", 125_000);
        back.vehicle = stingray();
        r.telemetry(&back);

        let entered = |s: &Session| {
            s.timeline
                .iter()
                .filter(|e| matches!(e.kind, EntryKind::VehicleEntered { .. }))
                .count()
        };
        assert_eq!(entered(r.current().unwrap()), 1);

        // Left the ship while Starlume was closed: that is recorded.
        let mut r = Reducer::default();
        r.restore(before.current().unwrap().clone());
        r.log_session_started(Some("LIVE".into()));
        r.log_events(&header(), true);
        r.telemetry(&sample("2026-11-13T16:25:00.000Z", 125_000));
        assert!(matches!(
            r.current().unwrap().timeline.last().unwrap().kind,
            EntryKind::VehicleLeft { .. }
        ));
    }

    #[test]
    fn the_logs_start_line_names_the_session() {
        // Whatever comes first in the batch, the start line dates it.
        let mut r = Reducer::default();
        r.log_session_started(Some("LIVE".into()));
        r.log_events(
            &[
                joined("2026-11-13T16:13:05.561Z", "pub_euw1b_13165946_100"),
                ev(START, LogEventKind::SessionStarted),
            ],
            true,
        );
        assert_eq!(r.current().unwrap().id, "2026-11-13T16-10-48.019Z");
    }

    #[test]
    fn clearing_forgets_everything() {
        let mut r = started(&header(), false);
        r.telemetry(&sample("2026-11-13T16:14:00.000Z", 125_000));
        r.clear();
        assert!(r.current().is_none());
        assert!(r.shelved.is_none());
        assert_eq!(r.log_session_ended(0), Outcome::default());
    }

    #[test]
    fn the_timeline_is_ordered_by_time_and_capped() {
        let mut r = started(&header(), false);
        // Telemetry is ahead of the log poll that explains the same moment.
        let mut dead = sample("2026-11-13T16:20:05.000Z", 1);
        dead.events = vec![TelemetryEvent::Dead];
        r.telemetry(&dead);
        r.log_events(
            &[ev(
                "2026-11-13T16:20:00.000Z",
                LogEventKind::BlueprintReceived {
                    name: "Karna Rifle".into(),
                },
            )],
            false,
        );
        let s = r.current().unwrap();
        assert!(matches!(
            s.timeline[0].kind,
            EntryKind::BlueprintReceived { .. }
        ));
        assert_eq!(s.timeline[1].kind, EntryKind::Died);
        assert_ne!(s.timeline[0].seq, s.timeline[1].seq);

        let many: Vec<LogEvent> = (0..MAX_TIMELINE + 5)
            .map(|i| {
                ev(
                    "2026-11-13T17:00:00.000Z",
                    LogEventKind::BlueprintReceived {
                        name: format!("Blueprint {i}"),
                    },
                )
            })
            .collect();
        r.log_events(&many, false);
        let s = r.current().unwrap();
        assert_eq!(s.timeline.len(), MAX_TIMELINE);
        assert_eq!(s.timeline_dropped, 7);
    }

    #[test]
    fn a_session_survives_its_json() {
        let mut events = header();
        events.push(joined("2026-11-13T16:13:05.561Z", "pub_euw1b_13165946_100"));
        events.push(buy("2026-11-13T16:40:00.000Z", 19_175.0));
        let mut r = started(&events, false);
        r.telemetry(&sample("2026-11-13T16:14:00.000Z", 125_000));
        r.telemetry(&sample("2026-11-13T16:40:00.300Z", 105_825));
        let s = r.current().unwrap();
        let json = serde_json::to_string(s).unwrap();
        assert_eq!(&serde_json::from_str::<Session>(&json).unwrap(), s);
        // A file from a version with fewer fields still loads.
        let old: Session = serde_json::from_str(r#"{"id":"x","started_at":5}"#).unwrap();
        assert_eq!(old.started_at, 5);
        assert!(old.timeline.is_empty());
    }
}
