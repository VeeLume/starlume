//! Telemetry service — the running game's local telemetry stream, reduced to
//! a small current state.
//!
//! Wraps [`sc_dossier::telemetry`]. The game serves
//! `GameTelemetryService` on `127.0.0.1:2942` (plaintext, no auth, about 13–16
//! full frames a second), but only when it was started with
//! [`enable::VAR`]`=true` in the launcher's environment. A frame carries the
//! vehicle, the weapons, the actor's vitals and aUEC balance, and a few
//! events; no coordinates.
//!
//! What this crate adds for Starlume:
//!
//! - [`Reducer`] — folds frames into one [`Snapshot`] (the few fields the app
//!   shows) and says how much each frame changed it.
//! - [`Throttle`] — turns that into "tell the frontend now or not", so a
//!   15 Hz stream becomes state changes.
//! - [`enable`] — the permanent, reversible install of the environment
//!   variable, behind a seam so tests never touch the registry.
//!
//! # Policy contract
//!
//! Reading the stream is **loopback, not an outbound network call**: it does
//! not pass `AppState::require_online()` or `require_unofficial()` (decided
//! 2026-11-13). It has its own opt-in instead (`AppSettings::telemetry_enabled`,
//! off by default), which this crate cannot see — the shell connects only
//! while that setting is on. Anything that *publishes* what is read here
//! (Discord presence, the server) is a network call and keeps the normal
//! gates.
//!
//! # Posture
//!
//! Nothing here polls or reconnects. The shell connects only while the game
//! runs and drops the [`Reducer`] when the opt-in goes off, so the idle cost
//! of an unused feature is zero.

pub mod enable;
mod state;
mod throttle;

use std::time::Duration;

pub use sc_dossier::telemetry::{
    CrewRole, DEFAULT_ADDR, Event, Frame, Locomotion, TelemetryStream, VehicleKind,
};
pub use state::{
    ActorState, Change, EVENTS_KEPT, EventRecord, Gauge, Reducer, Snapshot, VehicleState,
};
pub use throttle::Throttle;

/// How often to look for the game while it is closed: one process scan, no
/// connection attempt.
pub const WAIT_FOR_GAME: Duration = Duration::from_secs(10);
/// How often to try the port while the game runs without the stream. The
/// port opens some time into the game's start, so this also covers loading.
pub const RETRY_STREAM: Duration = Duration::from_secs(5);
/// Connected but no frame for this long (menus, loading): flush what the
/// throttle still holds.
pub const IDLE_AFTER: Duration = Duration::from_secs(5);
/// The least time between two frontend updates for fast-moving values
/// (speed, fuel, vitals).
pub const MINOR_GAP: Duration = Duration::from_secs(1);

/// Open the stream on the game's default loopback address. Fails fast (two
/// seconds at most) when nothing listens: the game is closed, still starting,
/// or was started without [`enable::VAR`].
pub async fn connect() -> anyhow::Result<TelemetryStream> {
    Ok(sc_dossier::telemetry::connect(DEFAULT_ADDR).await?)
}
