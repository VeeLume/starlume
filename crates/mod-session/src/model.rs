//! The session model: what one game run looked like.
//!
//! Everything here is plain data, serialised twice: as the JSON the shell
//! keeps under the app data root, and across IPC to the page. Add fields
//! (`#[serde(default)]` covers old files), never rename or repurpose them.
//!
//! Times are [`Millis`]; money is whole aUEC (`i64`). Both cross IPC as a
//! JavaScript number: neither comes near 2^53.

use serde::{Deserialize, Serialize};
use svc_log::{MoneyChange, ObjectiveChange, TradeSide};

use crate::time::Millis;

/// Most timeline entries one session keeps; the oldest are dropped first
/// and counted in [`Session::timeline_dropped`].
pub const MAX_TIMELINE: usize = 2000;
/// Most balance changes one session keeps (oldest dropped first, counted
/// in [`Wallet::changes_dropped`]). Opening, latest and net stay right.
pub const MAX_WALLET_CHANGES: usize = 1000;

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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// A `Game.log` line.
    Log,
    /// The telemetry stream.
    Telemetry,
}

/// One thing that happened.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    Died,
    Downed,
    Respawned,
    Spawned,
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
    Ended,
}

/// What became of a request the client sent a shop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum RequestOutcome {
    /// No answer was seen (the log has none for commodity trades).
    Requested,
    /// The server said yes.
    Confirmed,
    /// The server said no: nothing was bought.
    Rejected,
}

/// A balance at a time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Cause {
    /// No log event close enough explains it.
    Untracked,
    /// The timeline entry with this [`Entry::seq`] explains it.
    Entry { seq: u32 },
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
            opening_balance: self.wallet.opening.map(|s| s.balance),
            latest_balance: self.wallet.latest.map(|s| s.balance),
            net: self.wallet.net,
        }
    }
}
