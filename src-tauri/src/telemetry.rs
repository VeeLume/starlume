//! The game's local telemetry stream (`svc-telemetry`): the connection, the
//! current state kept from it, and the install that makes the game serve it.
//!
//! **Not gated by the online policy.** The stream is loopback
//! (`127.0.0.1:2942`), not an outbound call: no `require_online()`, no
//! `require_unofficial()` (decided 2026-11-13, see CLAUDE.md). Its gate is
//! its own opt-in, `AppSettings::telemetry_enabled`, off by default. Anything
//! that sends what is read here somewhere else is a network call and takes
//! the normal gates.
//!
//! **Lifetime.** [`sync`] starts one worker task when the opt-in goes on
//! (startup, `update_settings`) and aborts it when it goes off; with the
//! opt-in off nothing of this module is resident. The worker never tries the
//! port while the game is closed: it looks for the game process every
//! [`svc_telemetry::WAIT_FOR_GAME`] and connects only once it runs. While
//! connected it holds one HTTP/2 connection and a [`Reducer`] (a few hundred
//! bytes).
//!
//! **What the frontend hears.** The state lives here, Rust-side (a suspended
//! webview runs no JS). `telemetry:changed` goes out on a status change, at
//! once for a real change (balance, vehicle, seat, an event) and at most once
//! a second for moving values — never per frame. It carries nothing; the
//! store re-reads [`telemetry_state`].
//!
//! Not an `Activity`: a held connection is a state, not work the user waits
//! on, so the bell stays quiet.

use std::sync::Mutex;
use std::time::Instant;

use svc_telemetry::enable::{InstallState, Installer, RegistryEnv};
use svc_telemetry::{Change, Reducer, Snapshot, TelemetryStream, Throttle};
use tauri::{AppHandle, Emitter, Manager};

use crate::AppState;
use crate::error::AppError;

/// Emitted when the status or the snapshot changed (throttled, see above).
pub(crate) const TELEMETRY_CHANGED_EVENT: &str = "telemetry:changed";

/// Where the stream stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryStatus {
    /// The opt-in is off: nothing runs.
    Off,
    /// On, the game is not running.
    WaitingForGame,
    /// The game runs but nothing answers on the port: it is still starting,
    /// or it was started without the variable.
    NoStream,
    /// Reading.
    Connected,
}

struct Live {
    status: TelemetryStatus,
    /// `Some` while the opt-in is on.
    reducer: Option<Reducer>,
    worker: Option<tauri::async_runtime::JoinHandle<()>>,
    /// Bumped on every start and stop: an aborted worker still runs to its
    /// next await, and must not write into the state of its successor.
    generation: u64,
}

static LIVE: Mutex<Live> = Mutex::new(Live {
    status: TelemetryStatus::Off,
    reducer: None,
    worker: None,
    generation: 0,
});

/// Bring the worker in line with the opt-in: start it when the setting is on,
/// stop it and drop the state when it is off. Called at startup and from
/// `update_settings`.
pub(crate) fn sync(app: &AppHandle) {
    let enabled = app
        .state::<AppState>()
        .settings
        .lock()
        .unwrap()
        .telemetry_enabled;
    {
        let mut live = LIVE.lock().unwrap();
        match (enabled, live.worker.take()) {
            (true, Some(worker)) => {
                live.worker = Some(worker);
                return;
            }
            (false, None) => return,
            (true, None) => {
                live.generation += 1;
                live.reducer = Some(Reducer::default());
                live.status = TelemetryStatus::WaitingForGame;
                live.worker = Some(tauri::async_runtime::spawn(run(
                    app.clone(),
                    live.generation,
                )));
            }
            (false, Some(worker)) => {
                // Dropping the task drops the stream: the connection closes.
                worker.abort();
                live.generation += 1;
                live.reducer = None;
                live.status = TelemetryStatus::Off;
            }
        }
    }
    emit(app);
}

fn emit(app: &AppHandle) {
    let _ = app.emit(TELEMETRY_CHANGED_EVENT, ());
}

/// Run `f` on the live state if `generation` is still the current worker's.
fn with_live<T>(generation: u64, f: impl FnOnce(&mut Live) -> T) -> Option<T> {
    let mut live = LIVE.lock().unwrap();
    (live.generation == generation).then(|| f(&mut live))
}

fn set_status(app: &AppHandle, generation: u64, status: TelemetryStatus) {
    let changed = with_live(generation, |live| {
        std::mem::replace(&mut live.status, status) != status
    });
    if changed == Some(true) {
        emit(app);
    }
}

async fn game_running() -> bool {
    tokio::task::spawn_blocking(crate::launcher::game_running_blocking)
        .await
        .unwrap_or(false)
}

/// The worker: wait for the game, connect, read until the stream ends, again.
async fn run(app: AppHandle, generation: u64) {
    loop {
        let wait = if !game_running().await {
            set_status(&app, generation, TelemetryStatus::WaitingForGame);
            svc_telemetry::WAIT_FOR_GAME
        } else {
            match svc_telemetry::connect().await {
                Ok(stream) => {
                    tracing::info!("telemetry: connected");
                    set_status(&app, generation, TelemetryStatus::Connected);
                    read(&app, generation, stream).await;
                    tracing::info!("telemetry: the stream ended");
                    with_live(generation, |live| {
                        live.reducer.as_mut().map(Reducer::disconnected)
                    });
                    let status = if game_running().await {
                        TelemetryStatus::NoStream
                    } else {
                        TelemetryStatus::WaitingForGame
                    };
                    // The status differs from Connected either way, so this
                    // also announces the dropped vehicle and actor.
                    set_status(&app, generation, status);
                }
                Err(e) => {
                    tracing::debug!("telemetry: no stream: {e:#}");
                    set_status(&app, generation, TelemetryStatus::NoStream);
                }
            }
            svc_telemetry::RETRY_STREAM
        };
        tokio::time::sleep(wait).await;
    }
}

/// Fold frames into the snapshot until the game ends the stream.
async fn read(app: &AppHandle, generation: u64, mut stream: TelemetryStream) {
    let mut throttle = Throttle::new(svc_telemetry::MINOR_GAP);
    loop {
        let announce = match tokio::time::timeout(svc_telemetry::IDLE_AFTER, stream.next()).await {
            // Quiet (menus, loading): let a held change out.
            Err(_) => throttle.flush(Instant::now()),
            Ok(Ok(Some(frame))) => {
                let now = chrono::Utc::now().timestamp();
                let change = with_live(generation, |live| {
                    live.reducer.as_mut().map(|r| r.apply(&frame, now))
                })
                .flatten()
                .unwrap_or(Change::None);
                throttle.offer(change, Instant::now())
            }
            Ok(Ok(None)) => return,
            Ok(Err(e)) => {
                tracing::debug!("telemetry: stream error: {e}");
                return;
            }
        };
        if announce {
            emit(app);
        }
    }
}

// ── IPC views ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct TelemetryView {
    pub status: TelemetryStatus,
    /// `None` while the opt-in is off. With it on, the last known state: the
    /// vehicle and the actor are live only while `status` is `connected`,
    /// the balance and the events stay as last read.
    pub snapshot: Option<TelemetrySnapshotView>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct TelemetrySnapshotView {
    /// aUEC (a number: specta exports no i64; a balance fits a double).
    pub balance: Option<f64>,
    pub vehicle: Option<TelemetryVehicleView>,
    pub actor: Option<TelemetryActorView>,
    /// The last events, newest last. The game's events are incomplete.
    pub events: Vec<TelemetryEventView>,
    /// When the last frame arrived, Unix seconds.
    pub updated_at: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct TelemetryVehicleView {
    /// The class key (`KRIG_S65_Stingray`): look the ship up by this.
    pub class_name: String,
    /// Entity name with an id suffix.
    pub name: String,
    pub manufacturer: String,
    pub kind: TelemetryVehicleKind,
    pub landed: Option<bool>,
    /// m/s. Wrong in quantum travel (the game's value freezes).
    pub speed: Option<f32>,
    /// 0–1.
    pub hydrogen_fuel: Option<f32>,
    /// m above the body's datum; `null` away from a body.
    pub altitude: Option<f32>,
    /// Degrees 0–360; `null` away from a body.
    pub heading: Option<f32>,
    pub powered: Option<bool>,
    pub doors_open: Option<bool>,
    pub doors_locked: Option<bool>,
}

#[derive(Debug, Clone, Copy, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryVehicleKind {
    Unknown,
    Spaceship,
    Ground,
    Gravlev,
    Boat,
}

#[derive(Debug, Clone, Copy, serde::Serialize, specta::Type)]
pub struct TelemetryGauge {
    pub current: f32,
    pub max: f32,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct TelemetryActorView {
    /// 0–100.
    pub health: TelemetryGauge,
    /// 0–1.
    pub oxygen: TelemetryGauge,
    pub hunger: TelemetryGauge,
    pub thirst: TelemetryGauge,
    /// °C.
    pub body_temperature: TelemetryGauge,
    pub locomotion: TelemetryLocomotion,
    pub crew_role: TelemetryCrewRole,
    /// Seat entity name; empty on foot.
    pub seat_name: String,
    pub breathable: bool,
    /// °C; -273.15 in vacuum.
    pub external_temperature: f32,
}

#[derive(Debug, Clone, Copy, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryLocomotion {
    Unknown,
    OnFoot,
    Eva,
    Seated,
}

#[derive(Debug, Clone, Copy, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryCrewRole {
    None,
    Pilot,
    Gunner,
    Crew,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryEventKind {
    Spawn,
    Dead,
    EnterSeat,
    LeaveSeat,
    Respawn,
    Downed,
    /// A type this version doesn't know.
    Other,
}

#[derive(Debug, Clone, Copy, serde::Serialize, specta::Type)]
pub struct TelemetryEventView {
    pub kind: TelemetryEventKind,
    /// Unix seconds.
    pub at: f64,
}

fn gauge(g: svc_telemetry::Gauge) -> TelemetryGauge {
    TelemetryGauge {
        current: g.current,
        max: g.max,
    }
}

fn event_kind(e: svc_telemetry::Event) -> TelemetryEventKind {
    use svc_telemetry::Event as E;
    match e {
        E::Spawn => TelemetryEventKind::Spawn,
        E::Dead => TelemetryEventKind::Dead,
        E::EnterSeat => TelemetryEventKind::EnterSeat,
        E::LeaveSeat => TelemetryEventKind::LeaveSeat,
        E::Respawn => TelemetryEventKind::Respawn,
        E::Downed => TelemetryEventKind::Downed,
        _ => TelemetryEventKind::Other,
    }
}

impl From<&Snapshot> for TelemetrySnapshotView {
    fn from(s: &Snapshot) -> Self {
        use svc_telemetry::{CrewRole as R, Locomotion as L, VehicleKind as K};
        Self {
            balance: s.balance.map(|b| b as f64),
            vehicle: s.vehicle.as_ref().map(|v| TelemetryVehicleView {
                class_name: v.class_name.clone(),
                name: v.name.clone(),
                manufacturer: v.manufacturer.clone(),
                kind: match v.kind {
                    K::Spaceship => TelemetryVehicleKind::Spaceship,
                    K::Ground => TelemetryVehicleKind::Ground,
                    K::Gravlev => TelemetryVehicleKind::Gravlev,
                    K::Boat => TelemetryVehicleKind::Boat,
                    _ => TelemetryVehicleKind::Unknown,
                },
                landed: v.landed,
                speed: v.speed,
                hydrogen_fuel: v.hydrogen_fuel,
                altitude: v.altitude,
                heading: v.heading,
                powered: v.powered,
                doors_open: v.doors_open,
                doors_locked: v.doors_locked,
            }),
            actor: s.actor.as_ref().map(|a| TelemetryActorView {
                health: gauge(a.health),
                oxygen: gauge(a.oxygen),
                hunger: gauge(a.hunger),
                thirst: gauge(a.thirst),
                body_temperature: gauge(a.body_temperature),
                locomotion: match a.locomotion {
                    L::OnFoot => TelemetryLocomotion::OnFoot,
                    L::Eva => TelemetryLocomotion::Eva,
                    L::Seated => TelemetryLocomotion::Seated,
                    _ => TelemetryLocomotion::Unknown,
                },
                crew_role: match a.crew_role {
                    R::Pilot => TelemetryCrewRole::Pilot,
                    R::Gunner => TelemetryCrewRole::Gunner,
                    R::Crew => TelemetryCrewRole::Crew,
                    _ => TelemetryCrewRole::None,
                },
                seat_name: a.seat_name.clone(),
                breathable: a.breathable,
                external_temperature: a.external_temperature,
            }),
            events: s
                .events
                .iter()
                .map(|e| TelemetryEventView {
                    kind: event_kind(e.event),
                    at: e.at as f64,
                })
                .collect(),
            updated_at: s.updated_at.map(|t| t as f64),
        }
    }
}

/// The status and the current snapshot. A memory read: no network, no gate.
#[tauri::command]
#[specta::specta]
pub(crate) fn telemetry_state() -> TelemetryView {
    let live = LIVE.lock().unwrap();
    TelemetryView {
        status: live.status,
        snapshot: live.reducer.as_ref().map(|r| r.snapshot().into()),
    }
}

// ── The environment variable ────────────────────────────────────────────────

/// Whether the variable that makes the game serve the stream is set for the
/// Windows user, and by whom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryEnvState {
    NotInstalled,
    /// Starlume set it; uninstall removes it.
    Installed,
    /// Set, but not by Starlume: left alone.
    SetElsewhere,
}

impl From<InstallState> for TelemetryEnvState {
    fn from(s: InstallState) -> Self {
        match s {
            InstallState::NotInstalled => Self::NotInstalled,
            InstallState::Installed => Self::Installed,
            InstallState::SetElsewhere => Self::SetElsewhere,
        }
    }
}

fn installer() -> Installer<RegistryEnv> {
    Installer::new(
        RegistryEnv,
        app_kit::app_data_root().join("telemetry-env.json"),
    )
}

/// Run one installer step off the async threads (a registry write waits on
/// the settings-change broadcast).
async fn env_step(
    what: &'static str,
    step: fn(&Installer<RegistryEnv>) -> std::io::Result<InstallState>,
) -> Result<TelemetryEnvState, AppError> {
    tokio::task::spawn_blocking(move || step(&installer()))
        .await
        .map_err(|e| AppError::Internal(format!("{what}: {e}")))?
        .map(Into::into)
        .map_err(|e| AppError::Internal(format!("{what}: {e}")))
}

/// Is `GRPC_CLIENT_SERVER_ENABLED=true` set for this Windows user? A registry
/// read (`HKCU\Environment`).
#[tauri::command]
#[specta::specta]
pub(crate) async fn telemetry_env_status() -> Result<TelemetryEnvState, AppError> {
    env_step("could not read the environment variable", Installer::status).await
}

/// Set `GRPC_CLIENT_SERVER_ENABLED=true` for this Windows user
/// (`HKCU\Environment`; never machine-wide, no elevation), so the game
/// serves telemetry however the launcher is started. An explicit user
/// action, never a side effect of the opt-in.
#[tauri::command]
#[specta::specta]
pub(crate) async fn telemetry_env_install() -> Result<TelemetryEnvState, AppError> {
    env_step("could not set the environment variable", Installer::install).await
}

/// Undo [`telemetry_env_install`]: removes only what it set.
#[tauri::command]
#[specta::specta]
pub(crate) async fn telemetry_env_uninstall() -> Result<TelemetryEnvState, AppError> {
    env_step(
        "could not remove the environment variable",
        Installer::uninstall,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_view_mirrors_the_snapshot() {
        let frame: svc_telemetry::Frame = serde_json::from_value(serde_json::json!({
            "vehicle": null, "weapons": null, "actor": null, "turret": null,
            "events": ["EnterSeat", { "Other": 42 }],
        }))
        .unwrap();
        let mut reducer = Reducer::default();
        reducer.apply(&frame, 1_794_600_000);
        let view = TelemetrySnapshotView::from(reducer.snapshot());
        assert_eq!(view.balance, None);
        assert_eq!(view.updated_at, Some(1_794_600_000.0));
        let kinds: Vec<_> = view.events.iter().map(|e| e.kind).collect();
        assert_eq!(
            kinds,
            [TelemetryEventKind::EnterSeat, TelemetryEventKind::Other]
        );
    }

    #[test]
    fn nothing_is_resident_until_the_opt_in_is_on() {
        let view = telemetry_state();
        assert_eq!(view.status, TelemetryStatus::Off);
        assert!(view.snapshot.is_none());
    }
}
