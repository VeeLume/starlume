//! Session log: the shell side of the module. `mod-session` is the model
//! and the reducer; this file feeds it, keeps the live session, files the
//! finished ones and serves both to the page.
//!
//! **Sources.** Nothing here reads a log, scans a process or opens a
//! stream. The two readers the app already has call in: the Game.log
//! watcher's signal callback ([`on_log_signal`], `gamelog.rs`) and the
//! telemetry read loop ([`on_telemetry`], `telemetry.rs`). Each has its own
//! opt-in; this module works with either alone, says in
//! [`SessionStateView`] which are off, and never turns one on.
//!
//! **No network.** Everything is local, so no online gate applies.
//!
//! **Only while the module is enabled** (module rule 2): with it off the
//! feed calls return at once, nothing is held and nothing is written.
//!
//! **Memory** (CLAUDE.md "Memory"). Resident: one [`Reducer`] with the
//! current session, bounded by `mod_session::MAX_TIMELINE` entries and
//! `MAX_WALLET_CHANGES` balance changes. It lives Rust-side because a
//! suspended webview runs no JS. Past sessions are on disk only; the
//! history list reads a small index, one past session reads one file.
//!
//! **On disk** (`<app data>/sessions/`, compact JSON through app-kit's
//! atomic write):
//! - `<id>.json`: one finished session per file. Per file rather than one
//!   history file, so the list never loads every timeline and finishing a
//!   session rewrites one small file, not all of them.
//! - `index.json`: a [`SessionSummary`] per kept session, newest first.
//! - `current.json`: the live session, written at most every
//!   [`CHECKPOINT_EVERY`] and on exit. The log can rebuild its own half
//!   after a restart; the telemetry half (the wallet) exists nowhere else.
//!
//! **Retention:** the newest [`HISTORY_CAP`] sessions; older files are
//! deleted when a session is filed.

use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use mod_session::{
    Millis, Outcome, Reducer, Session, SessionSummary, TelemetryEvent, TelemetrySample, Vehicle,
};
use svc_log::WatchSignal;
use tauri::{AppHandle, Emitter, Manager};

use crate::AppState;
use crate::error::AppError;
use crate::telemetry::TelemetryStatus;

/// Module id (Rust registry, settings, frontend descriptor).
pub(crate) const MODULE_ID: &str = "session";

/// Emitted when the current session or the history changed, payload
/// [`SessionChange`]. The store re-reads [`session_state`].
pub(crate) const SESSION_CHANGED_EVENT: &str = "session:changed";

/// Most finished sessions kept on disk.
const HISTORY_CAP: usize = 100;
/// The least time between two writes of the live session.
const CHECKPOINT_EVERY: Duration = Duration::from_secs(60);

/// Payload of [`SESSION_CHANGED_EVENT`].
#[derive(Debug, Clone, Copy, serde::Serialize)]
struct SessionChange {
    /// A session was filed: the history list changed too.
    history: bool,
}

// ── Store ───────────────────────────────────────────────────────────────────

/// The sessions directory.
struct Store {
    dir: PathBuf,
}

fn write_compact<T: serde::Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    let bytes = serde_json::to_vec(value)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    app_kit::atomic_write(path, &bytes)
}

impl Store {
    fn at_data_root() -> Self {
        Self {
            dir: app_kit::app_data_root().join("sessions"),
        }
    }

    fn index_path(&self) -> PathBuf {
        self.dir.join("index.json")
    }

    fn checkpoint_path(&self) -> PathBuf {
        self.dir.join("current.json")
    }

    /// `None` for an id that is not one of ours: an id from the webview
    /// never becomes a path unchecked.
    fn session_path(&self, id: &str) -> Option<PathBuf> {
        mod_session::is_session_id(id).then(|| self.dir.join(format!("{id}.json")))
    }

    /// The kept sessions, newest first.
    fn index(&self) -> Vec<SessionSummary> {
        app_kit::load_json(&self.index_path())
    }

    fn get(&self, id: &str) -> Option<Session> {
        let bytes = std::fs::read(self.session_path(id)?).ok()?;
        serde_json::from_slice(&bytes)
            .inspect_err(|e| tracing::warn!("session {id} is unreadable: {e}"))
            .ok()
    }

    /// File a finished session (again: the same id replaces its file) and
    /// drop what falls out of the retention cap.
    fn file(&self, session: &Session) -> std::io::Result<()> {
        let Some(path) = self.session_path(&session.id) else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "not a session id",
            ));
        };
        write_compact(&path, session)?;
        let mut index = self.index();
        index.retain(|s| s.id != session.id);
        index.push(session.summary());
        index.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        for dropped in index.drain(HISTORY_CAP.min(index.len())..) {
            if let Some(old) = self.session_path(&dropped.id) {
                let _ = std::fs::remove_file(old);
            }
        }
        write_compact(&self.index_path(), &index)
    }

    fn checkpoint(&self, session: &Session) -> std::io::Result<()> {
        write_compact(&self.checkpoint_path(), session)
    }

    /// The live session of the last run of the app, removed from disk.
    fn take_checkpoint(&self) -> Option<Session> {
        let path = self.checkpoint_path();
        let bytes = std::fs::read(&path).ok()?;
        let _ = std::fs::remove_file(&path);
        serde_json::from_slice(&bytes)
            .inspect_err(|e| tracing::warn!("the kept live session is unreadable: {e}"))
            .ok()
    }

    fn clear_checkpoint(&self) {
        let _ = std::fs::remove_file(self.checkpoint_path());
    }
}

// ── Host ────────────────────────────────────────────────────────────────────

/// What the shell holds for the module while it is enabled.
#[derive(Default)]
struct Host {
    reducer: Reducer,
    /// The checkpoint of the last run was looked for.
    restored: bool,
    /// The log watcher's current session was taken in, by its signals or
    /// by [`Host::seed`].
    seeded: bool,
    checkpointed: Option<Instant>,
}

static HOST: LazyLock<Mutex<Host>> = LazyLock::new(Mutex::default);

fn now_ms() -> Millis {
    chrono::Utc::now().timestamp_millis()
}

impl Host {
    /// Take one reducer call's outcome to disk. Returns what to announce.
    fn settle(&mut self, store: &Store, outcome: Outcome, now: Instant) -> Option<SessionChange> {
        if let Some(done) = &outcome.finished {
            if !done.is_trivial()
                && let Err(e) = store.file(done)
            {
                tracing::warn!("could not file session {}: {e}", done.id);
            }
            store.clear_checkpoint();
            self.checkpointed = None;
            return Some(SessionChange {
                history: !done.is_trivial(),
            });
        }
        if !outcome.changed {
            return None;
        }
        let due = self
            .checkpointed
            .is_none_or(|at| now.duration_since(at) >= CHECKPOINT_EVERY);
        if due {
            self.checkpoint(store);
            self.checkpointed = Some(now);
        }
        Some(SessionChange { history: false })
    }

    fn checkpoint(&self, store: &Store) {
        if let Some(live) = self.reducer.current().filter(|s| s.live)
            && let Err(e) = store.checkpoint(live)
        {
            tracing::warn!("could not keep the live session: {e}");
        }
    }

    /// Once per run: the session that was live when the app last stopped
    /// is closed and filed. If the game still runs, its log's replay
    /// rebuilds it under the same id and takes the telemetry half back in.
    fn restore(&mut self, store: &Store) {
        if std::mem::replace(&mut self.restored, true) {
            return;
        }
        if let Some(kept) = store.take_checkpoint() {
            let filed = self.reducer.restore(kept);
            if !filed.is_trivial()
                && let Err(e) = store.file(&filed)
            {
                tracing::warn!("could not file session {}: {e}", filed.id);
            }
        }
    }

    /// Take in the session the log watcher already holds: the module was
    /// switched on after the watcher started following it, so its signals
    /// went by unheard.
    fn seed(&mut self, snapshot: &svc_log::SessionSnapshot) -> Outcome {
        if std::mem::replace(&mut self.seeded, true) {
            return Outcome::default();
        }
        let Some(log) = &snapshot.log else {
            return Outcome::default();
        };
        if !snapshot.live {
            // A finished one is history the module never saw: not rebuilt.
            return Outcome::default();
        }
        self.reducer.log_session_started(channel_of(Path::new(log)));
        self.reducer.log_events(&snapshot.events, true)
    }
}

/// The install channel a `Game.log` belongs to: its directory's name
/// (`…/StarCitizen/LIVE/Game.log` → `LIVE`).
fn channel_of(log: &Path) -> Option<String> {
    Some(log.parent()?.file_name()?.to_str()?.to_owned())
}

fn enabled(app: &AppHandle) -> bool {
    app.state::<AppState>()
        .settings
        .lock()
        .unwrap()
        .enabled_modules
        .iter()
        .any(|m| m == MODULE_ID)
}

/// Run one reducer call, file what it finished, announce what it changed.
/// With the module off, drop whatever was held instead.
fn feed(app: &AppHandle, call: impl FnOnce(&mut Host) -> Outcome) {
    let change = {
        let mut host = HOST.lock().unwrap();
        if !enabled(app) {
            if host.restored || host.seeded {
                // Switched off: nothing stays resident. The checkpoint
                // stays on disk and is filed when the module comes back.
                host.checkpoint(&Store::at_data_root());
                *host = Host::default();
            }
            return;
        }
        let store = Store::at_data_root();
        host.restore(&store);
        let outcome = call(&mut host);
        host.settle(&store, outcome, Instant::now())
    };
    if let Some(change) = change {
        let _ = app.emit(SESSION_CHANGED_EVENT, change);
    }
}

// ── Feeds (called by gamelog.rs and telemetry.rs) ───────────────────────────

/// A signal of the Game.log watcher, on its thread. Cheap: a reducer call;
/// a file write only when a session ends or the checkpoint is due.
pub(crate) fn on_log_signal(app: &AppHandle, signal: &WatchSignal) {
    feed(app, |host| match signal {
        WatchSignal::SessionStarted { log } => {
            host.seeded = true;
            host.reducer.log_session_started(channel_of(log));
            Outcome::default()
        }
        WatchSignal::Events { events, replay } => host.reducer.log_events(events, *replay),
        WatchSignal::SessionEnded => host.reducer.log_session_ended(now_ms()),
    });
}

/// The Game.log opt-in went off: the watcher is gone and will send no end
/// signal.
pub(crate) fn on_log_stopped(app: &AppHandle) {
    feed(app, |host| {
        host.seeded = false;
        host.reducer.log_session_ended(now_ms())
    });
}

/// What the session keeps of a telemetry frame. Built under the telemetry
/// lock, so it only copies; `at` is Unix milliseconds.
pub(crate) fn sample_of(
    snapshot: &svc_telemetry::Snapshot,
    events: &[svc_telemetry::Event],
    at: Millis,
) -> TelemetrySample {
    use svc_telemetry::Event as E;
    TelemetrySample {
        at,
        balance: snapshot.balance,
        vehicle: snapshot.vehicle.as_ref().map(|v| Vehicle {
            class_name: v.class_name.clone(),
            name: v.name.clone(),
        }),
        events: events
            .iter()
            .filter_map(|e| match e {
                E::Spawn => Some(TelemetryEvent::Spawn),
                E::Dead => Some(TelemetryEvent::Dead),
                E::Downed => Some(TelemetryEvent::Downed),
                E::Respawn => Some(TelemetryEvent::Respawn),
                // Seat events are incomplete; vehicles come from the
                // snapshot instead.
                _ => None,
            })
            .collect(),
    }
}

/// A telemetry frame that changed something a person would notice (the
/// read loop passes only those: a few a minute, not fifteen a second).
pub(crate) fn on_telemetry(app: &AppHandle, sample: TelemetrySample) {
    feed(app, |host| host.reducer.telemetry(&sample));
}

/// The telemetry status changed. Game closed or opt-in off ends a session
/// that only telemetry fed; a stream that merely dropped does not.
pub(crate) fn on_telemetry_status(app: &AppHandle, status: TelemetryStatus) {
    if matches!(
        status,
        TelemetryStatus::WaitingForGame | TelemetryStatus::Off
    ) {
        feed(app, |host| host.reducer.telemetry_stopped(now_ms()));
    }
}

/// App exit: keep the live session so the next run can close and file it.
pub(crate) fn flush(app: &AppHandle) {
    let host = HOST.lock().unwrap();
    if enabled(app) {
        host.checkpoint(&Store::at_data_root());
    }
}

// ── IPC ─────────────────────────────────────────────────────────────────────

/// The current session and which sources feed it.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct SessionStateView {
    /// The Game.log opt-in (`gamelog_enabled`). Off: no session bounds from
    /// the log, no shards, places, missions, trades or purchases.
    pub log_enabled: bool,
    /// The telemetry opt-in (`telemetry_enabled`). Off: no wallet, no
    /// vehicles, no deaths.
    pub telemetry_enabled: bool,
    /// The live session, or the last one seen in this run of the app.
    pub current: Option<Session>,
}

/// The current session. A memory read; the first call after the module was
/// switched on also takes in the session the log watcher already follows.
#[tauri::command]
#[specta::specta]
pub(crate) fn session_state(app: AppHandle) -> SessionStateView {
    let state = app.state::<AppState>();
    let (log_enabled, telemetry_enabled) = {
        let settings = state.settings.lock().unwrap();
        (settings.gamelog_enabled, settings.telemetry_enabled)
    };
    if !enabled(&app) {
        return SessionStateView {
            log_enabled,
            telemetry_enabled,
            current: None,
        };
    }
    // The watcher's session is read with the host lock released: the two
    // locks are never held together, here or on the watcher's thread.
    let watched = {
        let needs_seed = !HOST.lock().unwrap().seeded;
        needs_seed
            .then(|| {
                state
                    .log_watch
                    .lock()
                    .unwrap()
                    .as_ref()
                    .map(|w| w.session())
            })
            .flatten()
    };
    feed(&app, |host| match &watched {
        Some(snapshot) => host.seed(snapshot),
        None => Outcome::default(),
    });
    SessionStateView {
        log_enabled,
        telemetry_enabled,
        current: HOST.lock().unwrap().reducer.current().cloned(),
    }
}

/// The finished sessions, newest first (at most [`HISTORY_CAP`]). One small
/// file read; empty while the module is off.
#[tauri::command]
#[specta::specta]
pub(crate) async fn session_history(app: AppHandle) -> Result<Vec<SessionSummary>, AppError> {
    if !enabled(&app) {
        return Ok(Vec::new());
    }
    tokio::task::spawn_blocking(|| Store::at_data_root().index())
        .await
        .map_err(|e| AppError::Internal(format!("session history read failed: {e}")))
}

/// One finished session with its timeline; `None` when it is not kept (any
/// more).
#[tauri::command]
#[specta::specta]
pub(crate) async fn session_get(app: AppHandle, id: String) -> Result<Option<Session>, AppError> {
    if !enabled(&app) {
        return Ok(None);
    }
    tokio::task::spawn_blocking(move || Store::at_data_root().get(&id))
        .await
        .map_err(|e| AppError::Internal(format!("session read failed: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use svc_log::{LogEvent, LogEventKind};

    fn store() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store {
            dir: dir.path().join("sessions"),
        };
        (dir, store)
    }

    fn ev(ts: &str, event: LogEventKind) -> LogEvent {
        LogEvent {
            ts: ts.to_owned(),
            event,
        }
    }

    /// A finished log session that started at `day` (two digits) of
    /// November 2026, with one entry.
    fn finished(day: u32) -> Session {
        let mut reducer = Reducer::default();
        reducer.log_session_started(Some("LIVE".into()));
        reducer.log_events(
            &[
                ev(
                    &format!("2026-11-{day:02}T16:10:48.019Z"),
                    LogEventKind::SessionStarted,
                ),
                ev(
                    &format!("2026-11-{day:02}T16:13:05.561Z"),
                    LogEventKind::ShardJoined {
                        shard: "pub_euw1b_13165946_100".into(),
                    },
                ),
                ev(
                    &format!("2026-11-{day:02}T18:00:00.000Z"),
                    LogEventKind::SessionEnded {
                        reason: "Quit via console command".into(),
                    },
                ),
            ],
            false,
        );
        reducer.log_session_ended(0).finished.unwrap()
    }

    #[test]
    fn a_filed_session_is_listed_and_read_back() {
        let (_dir, store) = store();
        assert!(store.index().is_empty());
        assert_eq!(store.get("2026-11-13T16-10-48.019Z"), None);

        let session = finished(13);
        store.file(&session).unwrap();
        store.file(&finished(11)).unwrap();
        store.file(&finished(12)).unwrap();

        let ids: Vec<String> = store.index().into_iter().map(|s| s.id).collect();
        assert_eq!(
            ids,
            [
                "2026-11-13T16-10-48.019Z",
                "2026-11-12T16-10-48.019Z",
                "2026-11-11T16-10-48.019Z",
            ],
            "newest first, whatever the filing order"
        );
        assert_eq!(store.get(&session.id), Some(session.clone()));

        // Filed again (its log was read again): replaced, not doubled.
        store.file(&session).unwrap();
        assert_eq!(store.index().len(), 3);

        // Compact: one line.
        let raw = std::fs::read_to_string(store.session_path(&session.id).unwrap()).unwrap();
        assert!(!raw.contains('\n'));
    }

    #[test]
    fn an_id_that_is_not_one_never_becomes_a_path() {
        let (_dir, store) = store();
        assert_eq!(store.session_path("..\\..\\settings"), None);
        assert_eq!(store.session_path("index"), None);
        assert_eq!(store.get("../settings"), None);
        let mut bad = finished(13);
        bad.id = "../escape".into();
        assert!(store.file(&bad).is_err());
    }

    #[test]
    fn the_history_keeps_the_newest_sessions_and_deletes_the_rest() {
        let (_dir, store) = store();
        let template = finished(13);
        for i in 0..(HISTORY_CAP as i64 + 3) {
            let mut session = template.clone();
            session.started_at += i * 86_400_000;
            session.id = mod_session::session_id(session.started_at);
            store.file(&session).unwrap();
        }
        let index = store.index();
        assert_eq!(index.len(), HISTORY_CAP);
        assert!(
            index.windows(2).all(|w| w[0].started_at > w[1].started_at),
            "newest first"
        );
        assert_eq!(store.get(&template.id), None, "the oldest file is gone");
        let files = std::fs::read_dir(&store.dir).unwrap().count();
        assert_eq!(files, HISTORY_CAP + 1, "the sessions and the index");
    }

    fn live_host() -> Host {
        let mut host = Host::default();
        host.reducer.log_session_started(Some("LIVE".into()));
        host.reducer.log_events(
            &[ev("2026-11-13T16:10:48.019Z", LogEventKind::SessionStarted)],
            false,
        );
        host
    }

    #[test]
    fn the_live_session_is_kept_at_most_once_a_minute_and_filed_when_it_ends() {
        let (_dir, store) = store();
        let mut host = live_host();
        let start = Instant::now();
        let changed = || Outcome {
            changed: true,
            finished: None,
        };

        assert!(host.settle(&store, Outcome::default(), start).is_none());
        assert!(!store.checkpoint_path().exists(), "nothing changed");

        let change = host.settle(&store, changed(), start).unwrap();
        assert!(!change.history);
        assert!(store.checkpoint_path().exists());

        // A second change inside the minute is announced but not written.
        std::fs::remove_file(store.checkpoint_path()).unwrap();
        assert!(
            host.settle(&store, changed(), start + Duration::from_secs(5))
                .is_some()
        );
        assert!(!store.checkpoint_path().exists());
        host.settle(&store, changed(), start + CHECKPOINT_EVERY);
        assert!(store.checkpoint_path().exists());

        let ended = host.reducer.log_session_ended(now_ms());
        let change = host.settle(&store, ended, start).unwrap();
        assert!(change.history);
        assert!(!store.checkpoint_path().exists(), "no live session left");
        assert_eq!(store.index().len(), 1);
    }

    #[test]
    fn a_session_with_nothing_in_it_is_not_filed() {
        let (_dir, store) = store();
        let mut host = Host::default();
        host.reducer.telemetry(&TelemetrySample {
            at: 1_794_586_385_000,
            balance: Some(125_000),
            ..TelemetrySample::default()
        });
        let ended = host.reducer.telemetry_stopped(1_794_586_390_000);
        let change = host.settle(&store, ended, Instant::now()).unwrap();
        assert!(!change.history);
        assert!(store.index().is_empty());
    }

    #[test]
    fn the_session_live_at_the_last_exit_is_closed_and_filed_at_the_next_start() {
        let (_dir, store) = store();
        let host = live_host();
        host.checkpoint(&store);

        let mut next = Host::default();
        next.restore(&store);
        assert!(!store.checkpoint_path().exists());
        let index = store.index();
        assert_eq!(index.len(), 1);
        assert!(index[0].ended_at.is_some());
        let last = next.reducer.current().unwrap();
        assert!(!last.live);
        assert_eq!(last.id, index[0].id);

        // Only once per run.
        host.checkpoint(&store);
        next.restore(&store);
        assert!(store.checkpoint_path().exists());
    }

    #[test]
    fn switching_the_module_on_mid_game_takes_the_watchers_session_in() {
        let mut host = Host::default();
        let snapshot = svc_log::SessionSnapshot {
            live: true,
            log: Some(r"C:\Games\StarCitizen\PTU\Game.log".into()),
            events: vec![
                ev("2026-11-13T16:10:48.019Z", LogEventKind::SessionStarted),
                ev(
                    "2026-11-13T16:13:05.561Z",
                    LogEventKind::ShardJoined {
                        shard: "ptu_use1a_13165946_007".into(),
                    },
                ),
            ],
            dropped: 0,
        };
        assert!(host.seed(&snapshot).changed);
        let session = host.reducer.current().unwrap();
        assert!(session.live);
        assert_eq!(session.channel.as_deref(), Some("PTU"));
        assert_eq!(session.shards.len(), 1);
        // Only once: the signals carry on from here.
        assert_eq!(host.seed(&snapshot), Outcome::default());

        // A session that is already over is not rebuilt.
        let mut host = Host::default();
        let over = svc_log::SessionSnapshot {
            live: false,
            ..snapshot
        };
        assert_eq!(host.seed(&over), Outcome::default());
        assert!(host.reducer.current().is_none());
    }

    #[test]
    fn the_channel_is_the_logs_directory() {
        assert_eq!(
            channel_of(Path::new(r"C:\Games\StarCitizen\LIVE\Game.log")).as_deref(),
            Some("LIVE")
        );
        assert_eq!(channel_of(Path::new("Game.log")), None);
    }

    #[test]
    fn a_sample_keeps_the_balance_the_vehicle_and_the_life_events() {
        let frame: svc_telemetry::Frame = serde_json::from_value(serde_json::json!({
            "vehicle": null, "weapons": null, "actor": null, "turret": null,
            "events": ["EnterSeat", "Dead", { "Other": 42 }, "Respawn"],
        }))
        .unwrap();
        let mut reducer = svc_telemetry::Reducer::default();
        reducer.apply(&frame, 1_794_586_385);
        let sample = sample_of(reducer.snapshot(), &frame.events, 1_794_586_385_000);
        assert_eq!(sample.at, 1_794_586_385_000);
        assert_eq!(sample.balance, None);
        assert_eq!(sample.vehicle, None);
        assert_eq!(
            sample.events,
            [TelemetryEvent::Dead, TelemetryEvent::Respawn]
        );
    }
}
