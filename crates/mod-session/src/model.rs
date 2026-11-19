//! The session model: what one game run looked like.
//!
//! Everything here is plain data, serialised twice: as the JSON the shell
//! keeps under the app data root, and across IPC to the page. Add fields,
//! never rename or repurpose them.
//!
//! **Stored files outlive the version that wrote them**, in both directions
//! (a debug build and a release build can share a data dir):
//! - every stored struct is `#[serde(default)]`, so a file without a field
//!   added later still loads;
//! - unknown fields are ignored (serde's default), so a file with a field
//!   this version does not know still loads;
//! - every enum of this crate has a catch-all variant (`#[serde(other)]`),
//!   so a kind added later reads as "unknown" instead of failing;
//! - a timeline entry or balance change that still cannot be read (an
//!   svc-log enum inside it gained a value) is skipped, not fatal.
//!
//! Times are [`Millis`]; money is whole aUEC (`i64`). Both cross IPC as a
//! JavaScript number: neither comes near 2^53.

use serde::{Deserialize, Deserializer, Serialize};
use svc_log::{BodyPart, MoneyChange, ObjectiveChange, TradeSide};

use crate::time::Millis;

/// Most timeline entries one session keeps; the oldest are dropped first
/// and counted in [`Session::timeline_dropped`].
pub const MAX_TIMELINE: usize = 2000;
/// Most balance changes one session keeps (oldest dropped first, counted
/// in [`Wallet::changes_dropped`]). Opening, latest and net stay right.
pub const MAX_WALLET_CHANGES: usize = 1000;

/// A stored list that skips the items this version cannot read.
fn lenient<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Item<T> {
        Read(T),
        Skipped(serde::de::IgnoredAny),
    }
    let items = Vec::<Item<T>>::deserialize(deserializer)?;
    Ok(items
        .into_iter()
        .filter_map(|item| match item {
            Item::Read(value) => Some(value),
            Item::Skipped(_) => None,
        })
        .collect())
}

/// One game run.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default)]
pub struct Session {
    /// The start time as a file-name-safe string
    /// ([`crate::time::session_id`]). A log read twice gives the same id.
    pub id: String,
    /// From the log's first line; for a telemetry-only session, the first
    /// sample.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub started_at: Millis,
    /// `None` while the session is live.
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub ended_at: Option<Millis>,
    /// The time of the last thing recorded; where a session whose end was
    /// never seen is taken to have ended.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub last_activity_at: Millis,
    /// The game is running and this session is being followed.
    pub live: bool,
    /// The quit reason the log gave; `None` when the end was only observed
    /// (the process went away, or the stream did).
    pub end_reason: Option<String>,
    /// The install channel the log belongs to (`LIVE`, `PTU`).
    pub channel: Option<String>,
    /// The environment tag (`PUB` is the live persistent universe).
    pub env_tag: Option<String>,
    /// The code branch (`sc-alpha-4.10.0`).
    pub branch: Option<String>,
    /// The changelist: the build's identity.
    pub build: Option<String>,
    /// Which sources fed this session.
    pub sources: Sources,
    /// Every shard joined, in order, with how long it was stayed on.
    pub shards: Vec<ShardStay>,
    /// What happened, ordered by time (oldest first).
    #[serde(deserialize_with = "lenient")]
    pub timeline: Vec<Entry>,
    /// Entries dropped from the front to stay within [`MAX_TIMELINE`].
    pub timeline_dropped: u32,
    pub wallet: Wallet,
    /// The next [`Entry::seq`].
    pub next_seq: u32,
}

/// Which sources fed a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default)]
pub struct Sources {
    /// `Game.log` events (svc-log).
    pub log: bool,
    /// The game's telemetry stream (svc-telemetry).
    pub telemetry: bool,
}

/// One stay on a shard.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default)]
pub struct ShardStay {
    /// The shard's name (`pub_euw1b_13165946_100`).
    pub shard: String,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub joined_at: Millis,
    /// The next join, or the session's end; `None` while still on it.
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub left_at: Option<Millis>,
}

/// Where a timeline entry came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// A `Game.log` line.
    Log,
    /// The telemetry stream.
    Telemetry,
    /// A source a later version added.
    #[default]
    #[serde(other)]
    Unknown,
}

/// One mission run, accept to completion ([`Session::runs`]).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ContractRun {
    /// The contract's record name, as the marker line gave it: what the
    /// mission catalog's `contracts` list holds.
    pub contract: String,
    pub seconds: f64,
}

/// One thing that happened.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default)]
pub struct Entry {
    /// Unique within the session and stable while it lives: what a
    /// [`Cause::Entry`] points at. Not the position in the timeline.
    pub seq: u32,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub at: Millis,
    pub source: Source,
    /// Read from English HUD text: with a language pack that translates the
    /// notification this entry would not have been recorded (svc-log's
    /// locale note).
    pub locale_dependent: bool,
    pub kind: EntryKind,
}

/// What an [`Entry`] says. Money is whole aUEC.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EntryKind {
    /// Joined the persistent universe on a shard.
    ShardJoined {
        shard: String,
    },
    /// Arrived at a place with a local inventory. The game's internal name
    /// (`Stanton4_NewBabbage`); recorded when it differs from the last one.
    Location {
        location: String,
    },
    /// "Entered … Jurisdiction"; recorded when it differs from the last.
    Jurisdiction {
        jurisdiction: String,
    },
    MissionAccepted {
        mission_id: Option<String>,
        title: String,
    },
    Objective {
        change: ObjectiveChange,
        mission_id: Option<String>,
        text: String,
    },
    /// A mission is over. Recorded once per mission, from the HUD
    /// notification, the engine's end line, or both.
    MissionEnded {
        mission_id: Option<String>,
        /// The title as the HUD showed it, when a notification named it.
        title: Option<String>,
        /// The contract's internal name, when a marker line tied one to the
        /// mission id: what there is to show without the HUD title.
        contract: Option<String>,
        outcome: MissionOutcome,
    },
    BlueprintReceived {
        name: String,
    },
    RefineryOrderCompleted {
        location: String,
    },
    /// A commodity kiosk trade, as the client asked for it.
    Trade {
        side: TradeSide,
        shop: String,
        resource_guid: String,
        #[cfg_attr(feature = "specta", specta(type = f64))]
        price: i64,
        quantity: f64,
        quantity_unit: Option<String>,
        outcome: RequestOutcome,
    },
    /// A shop purchase or rental, as the client asked for it.
    ShopPurchase {
        shop: String,
        item_guid: String,
        item_name: String,
        #[cfg_attr(feature = "specta", specta(type = f64))]
        price: i64,
        quantity: u32,
        rental: bool,
        outcome: RequestOutcome,
    },
    /// "Awarded N aUEC" / "Fined N UEC".
    Money {
        change: MoneyChange,
        #[cfg_attr(feature = "specta", specta(type = f64))]
        amount: i64,
    },
    /// Seated in a vehicle (from the stream's vehicle, not its seat events,
    /// which are incomplete).
    VehicleEntered {
        class_name: String,
        name: String,
    },
    VehicleLeft {
        class_name: String,
        name: String,
    },
    /// A ship you destroyed: the log names you its killer, and it was not
    /// yours. Once per ship.
    ShipDestroyed {
        class_name: String,
        /// The log's reason (`Combat`, `Collision`).
        reason: String,
    },
    /// A ship of yours was destroyed. Once per ship.
    ShipLost {
        class_name: String,
        /// The log's reason (`Combat`, `Collision`, `SelfDestruct`,
        /// `ShipItemCritical`).
        reason: String,
        /// The player who destroyed it, when the killer was one (an NPC is
        /// named by its entity, which says nothing).
        by: Option<String>,
    },
    /// One of your body parts was injured. `tier` is the med bed it needs:
    /// 3 minor, 2 moderate, 1 severe.
    Injured {
        part: BodyPart,
        tier: u8,
    },
    /// A med bed operated on you.
    Treated {
        parts: Vec<BodyPart>,
        /// The ship the bed is in, as its class; `None` for a station or
        /// hospital bed.
        vehicle: Option<String>,
    },
    /// A visit to the vehicle terminal (ASOP): it fetched your ships, once
    /// or several times close together. Shown only when it explains a
    /// spend ([`Cause::Entry`]): the log does not say what was paid there.
    VehicleTerminal,
    Died,
    Downed,
    Respawned,
    Spawned,
    /// One quantum trip: a destination was set on the star map (the
    /// entry's time) and, if `arrived_at` says so, the drive got there. The
    /// log has no line for the jump's start, so the time between the two
    /// includes aligning and spooling. A destination set and replaced
    /// before any arrival leaves no entry.
    QuantumTravel {
        /// Where the route started, as the game names it for display
        /// (`Nyx Gateway`): shown as it is, never looked up.
        from: Option<String>,
        /// The destination's internal name (`Orison_LOC`).
        to: String,
        /// The ship's class, when the log named one.
        vehicle: Option<String>,
        /// `None`: no arrival was seen (still under way, given up, or the
        /// session ended first).
        #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
        arrived_at: Option<Millis>,
    },
    /// Attachments put on one carried weapon or tool in one go (requests
    /// close together fold into one entry; the entry's time is the first).
    /// What the client asked for: the log has no answer line, and none for
    /// taking an attachment off.
    Attachments {
        /// The weapon's class, when the log said what sits in the slot.
        weapon: Option<String>,
        /// The slot the weapon sits in (`wep_stocked_2`).
        holder_port: String,
        items: Vec<AttachedItem>,
    },
    /// A kind a later version added: kept as a row that says so.
    #[default]
    #[serde(other)]
    Unknown,
}

/// One attachment of an [`EntryKind::Attachments`] entry.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default)]
pub struct AttachedItem {
    /// The attachment's class (`arma_barrel_supp_s2`).
    pub item: String,
    /// The weapon's port it went on (`barrel_attach`).
    pub port: String,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub at: Millis,
}

/// How a mission ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum MissionOutcome {
    Completed,
    Failed,
    Abandoned,
    /// The giver took it back.
    Withdrawn,
    /// Over, in a way this version does not name.
    #[serde(other)]
    Ended,
}

/// What became of a request the client sent a shop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum RequestOutcome {
    /// The server said yes.
    Confirmed,
    /// The server said no: nothing was bought.
    Rejected,
    /// No answer was seen (the log has none for commodity trades), or one
    /// this version does not know.
    #[serde(other)]
    Requested,
}

/// A balance at a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default)]
pub struct BalanceSample {
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub at: Millis,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub balance: i64,
}

/// The session's money, from the telemetry stream's aUEC balance.
///
/// The stream sends the balance with every frame; the track keeps the first
/// trusted one, the last one, and every change between them. See
/// [`crate::wallet`] for which samples are trusted.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default)]
pub struct Wallet {
    /// The first trusted balance of the session.
    pub opening: Option<BalanceSample>,
    /// The last trusted balance.
    pub latest: Option<BalanceSample>,
    /// `latest - opening`; 0 until there are both.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub net: i64,
    /// Every change of the trusted balance, oldest first.
    #[serde(deserialize_with = "lenient")]
    pub changes: Vec<WalletChange>,
    /// Changes dropped from the front to stay within
    /// [`MAX_WALLET_CHANGES`].
    pub changes_dropped: u32,
    /// The stream reads 0 since then, and that 0 is not trusted (yet): the
    /// figures above are from before it.
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub zero_since: Option<Millis>,
    /// The session's first samples read 0 and were not trusted, so the
    /// opening balance is the first one above 0. If the wallet really was
    /// empty, `net` misses the first income.
    pub opened_after_zero: bool,
}

/// One change of the balance.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default)]
pub struct WalletChange {
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub at: Millis,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub from: i64,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub to: i64,
    /// `to - from`.
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub delta: i64,
    pub cause: Cause,
}

/// Why the balance changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Cause {
    /// The timeline entry with this [`Entry::seq`] explains it.
    Entry { seq: u32 },
    /// No log event close enough explains it (or a cause this version
    /// does not know).
    #[default]
    #[serde(other)]
    Untracked,
}

/// A session without its timeline: one row of the history list.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default)]
pub struct SessionSummary {
    pub id: String,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub started_at: Millis,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub ended_at: Option<Millis>,
    pub channel: Option<String>,
    pub branch: Option<String>,
    pub build: Option<String>,
    pub sources: Sources,
    /// The shards joined, in order.
    pub shards: Vec<String>,
    /// Timeline entries kept.
    pub entries: u32,
    pub missions_completed: u32,
    /// Ships you destroyed ([`EntryKind::ShipDestroyed`]).
    pub ships_destroyed: u32,
    /// Ships of yours destroyed ([`EntryKind::ShipLost`]).
    pub ships_lost: u32,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub opening_balance: Option<i64>,
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub latest_balance: Option<i64>,
    #[cfg_attr(feature = "specta", specta(type = f64))]
    pub net: i64,
}

impl Session {
    /// Nothing worth keeping: no log behind it, nothing on the timeline and
    /// no balance change (a stream that connected for a moment).
    pub fn is_trivial(&self) -> bool {
        !self.sources.log && self.timeline.is_empty() && self.wallet.changes.is_empty()
    }

    /// The entry with this `seq`, if it is still on the timeline.
    pub fn entry(&self, seq: u32) -> Option<&Entry> {
        self.timeline.iter().rev().find(|e| e.seq == seq)
    }

    /// The missions you ran start to finish here: accepted and completed
    /// in this session, and tied to their contract by a marker line. What
    /// your own time for a contract is measured from (Plans). A mission
    /// accepted before the session began, or ended any other way, is no
    /// run.
    pub fn runs(&self) -> Vec<ContractRun> {
        let mut accepted: std::collections::HashMap<&str, Millis> =
            std::collections::HashMap::new();
        let mut out = Vec::new();
        for e in &self.timeline {
            match &e.kind {
                EntryKind::MissionAccepted {
                    mission_id: Some(id),
                    ..
                } => {
                    accepted.entry(id.as_str()).or_insert(e.at);
                }
                EntryKind::MissionEnded {
                    mission_id: Some(id),
                    contract: Some(contract),
                    outcome: MissionOutcome::Completed,
                    ..
                } => {
                    if let Some(start) = accepted.remove(id.as_str())
                        && e.at > start
                    {
                        out.push(ContractRun {
                            contract: contract.clone(),
                            seconds: (e.at - start) as f64 / 1000.0,
                        });
                    }
                }
                _ => {}
            }
        }
        out
    }

    pub fn summary(&self) -> SessionSummary {
        let missions_completed = self
            .timeline
            .iter()
            .filter(|e| {
                matches!(
                    e.kind,
                    EntryKind::MissionEnded {
                        outcome: MissionOutcome::Completed,
                        ..
                    }
                )
            })
            .count() as u32;
        let counted =
            |f: fn(&EntryKind) -> bool| self.timeline.iter().filter(|e| f(&e.kind)).count() as u32;
        SessionSummary {
            id: self.id.clone(),
            started_at: self.started_at,
            ended_at: self.ended_at,
            channel: self.channel.clone(),
            branch: self.branch.clone(),
            build: self.build.clone(),
            sources: self.sources,
            shards: self.shards.iter().map(|s| s.shard.clone()).collect(),
            entries: self.timeline.len() as u32,
            missions_completed,
            ships_destroyed: counted(|k| matches!(k, EntryKind::ShipDestroyed { .. })),
            ships_lost: counted(|k| matches!(k, EntryKind::ShipLost { .. })),
            opening_balance: self.wallet.opening.map(|s| s.balance),
            latest_balance: self.wallet.latest.map(|s| s.balance),
            net: self.wallet.net,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_is_a_mission_accepted_and_completed_here_with_its_contract() {
        let accepted = |seq, at, id: &str| Entry {
            seq,
            at,
            kind: EntryKind::MissionAccepted {
                mission_id: Some(id.into()),
                title: "t".into(),
            },
            ..Entry::default()
        };
        let ended = |seq, at, id: &str, contract: Option<&str>, outcome| Entry {
            seq,
            at,
            kind: EntryKind::MissionEnded {
                mission_id: Some(id.into()),
                title: None,
                contract: contract.map(Into::into),
                outcome,
            },
            ..Entry::default()
        };
        let session = Session {
            timeline: vec![
                accepted(1, 1_000, "a"),
                accepted(2, 2_000, "b"),
                accepted(3, 3_000, "c"),
                ended(
                    4,
                    601_000,
                    "a",
                    Some("Covalex_Small"),
                    MissionOutcome::Completed,
                ),
                // Abandoned, and one never tied to its contract: no runs.
                ended(
                    5,
                    9_000,
                    "b",
                    Some("Covalex_Small"),
                    MissionOutcome::Abandoned,
                ),
                ended(6, 9_000, "c", None, MissionOutcome::Completed),
                // Accepted before the session began.
                ended(
                    7,
                    9_000,
                    "d",
                    Some("Covalex_Small"),
                    MissionOutcome::Completed,
                ),
            ],
            ..Session::default()
        };
        assert_eq!(
            session.runs(),
            vec![ContractRun {
                contract: "Covalex_Small".into(),
                seconds: 600.0
            }]
        );
    }

    #[test]
    fn a_file_from_an_older_version_loads() {
        // Fewer fields at every level.
        let old: Session = serde_json::from_str(
            r#"{"id":"2026-11-13T16-10-48.019Z","started_at":5,
                "shards":[{"shard":"pub_euw1b_13165946_100"}],
                "timeline":[{"seq":1,"at":7,"kind":{"type":"died"}}],
                "wallet":{"opening":{"balance":10},"changes":[{"at":8,"to":4}]}}"#,
        )
        .unwrap();
        assert_eq!(old.started_at, 5);
        assert_eq!(old.shards[0].joined_at, 0);
        assert_eq!(old.timeline[0].kind, EntryKind::Died);
        // The kinds added since, with only the fields they cannot do without.
        let sparse: Session = serde_json::from_str(
            r#"{"timeline":[
                 {"seq":1,"kind":{"type":"quantum_travel","to":"NewCorvo_LOC"}},
                 {"seq":2,"kind":{"type":"attachments","holder_port":"wep_sidearm",
                                  "items":[{"item":"tnkr_optics_x1_s1"}]}}]}"#,
        )
        .unwrap();
        assert_eq!(
            sparse.timeline[0].kind,
            EntryKind::QuantumTravel {
                from: None,
                to: "NewCorvo_LOC".into(),
                vehicle: None,
                arrived_at: None,
            }
        );
        assert!(matches!(
            &sparse.timeline[1].kind,
            EntryKind::Attachments { weapon: None, items, .. } if items[0].port.is_empty()
        ));
        assert_eq!(old.timeline[0].source, Source::Unknown);
        assert_eq!(old.wallet.opening.unwrap().balance, 10);
        assert_eq!(old.wallet.changes[0].cause, Cause::Untracked);
    }

    #[test]
    fn a_file_from_a_newer_version_loads() {
        // Unknown fields, kinds, sources, outcomes and causes.
        let newer: Session = serde_json::from_str(
            r#"{"id":"x","started_at":5,"weather":"clear",
                "timeline":[
                  {"seq":1,"at":7,"source":"voice","locale_dependent":false,
                   "kind":{"type":"docked","port":"A1"},"mood":"good"},
                  {"seq":2,"at":8,"source":"log","locale_dependent":false,
                   "kind":{"type":"mission_ended","mission_id":null,"title":null,
                           "contract":null,"outcome":"expired"}},
                  {"seq":3,"at":9,"source":"log","locale_dependent":false,
                   "kind":{"type":"shop_purchase","shop":"s","item_guid":"g",
                           "item_name":"n","price":1,"quantity":1,"rental":false,
                           "outcome":"refunded"}}],
                "wallet":{"changes":[
                  {"at":8,"from":1,"to":4,"delta":3,"cause":{"type":"gift","from":"x"}}]}}"#,
        )
        .unwrap();
        assert_eq!(newer.timeline.len(), 3);
        assert_eq!(newer.timeline[0].kind, EntryKind::Unknown);
        assert_eq!(newer.timeline[0].source, Source::Unknown);
        assert!(matches!(
            newer.timeline[1].kind,
            EntryKind::MissionEnded {
                outcome: MissionOutcome::Ended,
                ..
            }
        ));
        assert!(matches!(
            newer.timeline[2].kind,
            EntryKind::ShopPurchase {
                outcome: RequestOutcome::Requested,
                ..
            }
        ));
        assert_eq!(newer.wallet.changes[0].cause, Cause::Untracked);
        assert_eq!(newer.wallet.changes[0].delta, 3);
    }

    #[test]
    fn an_entry_that_cannot_be_read_is_skipped_not_fatal() {
        // svc-log's enums have no catch-all: a value they gain later makes
        // one entry unreadable, and only that entry is lost.
        let session: Session = serde_json::from_str(
            r#"{"id":"x","timeline":[
                 {"seq":1,"at":7,"source":"log","locale_dependent":true,
                  "kind":{"type":"objective","change":"Paused","mission_id":null,"text":"t"}},
                 {"seq":2,"at":8,"source":"telemetry","locale_dependent":false,
                  "kind":{"type":"died"}},
                 "not an entry at all"]}"#,
        )
        .unwrap();
        assert_eq!(session.timeline.len(), 1);
        assert_eq!(session.timeline[0].seq, 2);
    }

    #[test]
    fn every_kind_survives_its_json() {
        let kinds = [
            EntryKind::Unknown,
            EntryKind::Spawned,
            EntryKind::Money {
                change: MoneyChange::Fined,
                amount: 72_000,
            },
            EntryKind::QuantumTravel {
                from: Some("Tarn Gateway".into()),
                to: "NewCorvo_LOC".into(),
                vehicle: None,
                arrived_at: Some(9),
            },
            EntryKind::Attachments {
                weapon: Some("tnkr_rifle_01".into()),
                holder_port: "wep_stocked_2".into(),
                items: vec![AttachedItem {
                    item: "tnkr_optics_x4_s2".into(),
                    port: "optics_attach".into(),
                    at: 7,
                }],
            },
        ];
        for kind in kinds {
            let json = serde_json::to_string(&kind).unwrap();
            assert_eq!(serde_json::from_str::<EntryKind>(&json).unwrap(), kind);
        }
        assert_eq!(
            serde_json::to_string(&Source::Telemetry).unwrap(),
            r#""telemetry""#
        );
    }
}
