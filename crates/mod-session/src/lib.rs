//! Session log module: one game run as a timeline and a wallet track.
//!
//! "What happened this session, and what did it do to my money", built from
//! the two local sources the app already reads:
//!
//! - **`Game.log`** (svc-log): when the session began and ended, the build,
//!   the shards, places, missions, blueprints, refinery orders, trades,
//!   shop purchases, awards and fines.
//! - **The telemetry stream** (svc-telemetry): the aUEC balance, vehicles
//!   entered and left, deaths and respawns.
//!
//! Each source has its own opt-in in the shell; this module works with
//! either one alone and never switches one on. A [`Session`] says in
//! [`Sources`] which of them fed it, and every [`Entry`] says where it came
//! from.
//!
//! - [`model`]: the data, as stored and as sent to the page.
//! - [`reducer`]: [`Reducer`], which folds both sources into the current
//!   session (what a session is, how a replay rebuilds one).
//! - [`wallet`]: which balance samples are trusted (a 0 while loading is
//!   not) and how a change is attributed to a log event.
//! - [`time`]: timestamps and the session id.
//!
//! Pure (module rule 1): no Tauri, no file, no network, no clock. The shell
//! hands events and samples in with their times, files what
//! [`Outcome::finished`] returns, and keeps the live session Rust-side.
//!
//! **Locale.** Entries marked [`Entry::locale_dependent`] were read from
//! English HUD text and do not appear with a language pack that translates
//! it (svc-log's locale note). Mission ends are also read from engine
//! lines, so those survive.
//!
//! Later consumers of the same model (Discord presence, a star-map trail,
//! friends' presence) read it; none of them is built here.

pub mod model;
pub mod names;
pub mod reducer;
pub mod time;
pub mod wallet;

pub use model::{
    BalanceSample, Cause, ContractRun, Entry, EntryKind, MAX_TIMELINE, MAX_WALLET_CHANGES,
    MissionOutcome, RequestOutcome, Session, SessionSummary, ShardStay, Source, Sources, Wallet,
    WalletChange,
};
pub use reducer::{Outcome, Reducer, TelemetryEvent, TelemetrySample, Vehicle};
pub use time::{Millis, is_session_id, parse_ts, session_id};
