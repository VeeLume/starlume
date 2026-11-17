//! Session log: the shell side of the module. `mod-session` is the model
//! and the reducer; this file feeds it, keeps the live session, files the
//! finished ones and serves both to the page.
//!
//! **Sources.** Nothing here scans a process or opens a stream, and no log
//! is tailed. The two readers the app already has call in: the Game.log
//! watcher's signal callback ([`on_log_signal`], `gamelog.rs`) and the
//! telemetry read loop ([`on_telemetry`], `telemetry.rs`). Each has its own
//! opt-in; this module works with either alone, says in
//! [`SessionStateView`] which are off, and never turns one on.
//!
//! **No network.** Everything is local, so no online gate applies.
//!
//! **Only while the module is offered and enabled** (module rule 2): with
//! it off the feeds do nothing, nothing is held and nothing is written.
//!
//! **One queue.** Every feed is a job on one ordered queue, run one at a
//! time on the blocking pool ([`feed`]). So the callers (the watcher's
//! thread, the telemetry task, `update_settings` on the main thread) only
//! enqueue; the reducer calls and all file IO happen off their threads, in
//! the order they were sent. Commands wait for the queue to drain before
//! they read ([`drained`]).
//!
//! **Memory** (CLAUDE.md "Memory"). Resident: one [`Reducer`] with the
//! current session, bounded by `mod_session::MAX_TIMELINE` entries and
//! `MAX_WALLET_CHANGES` balance changes. It lives Rust-side because a
//! suspended webview runs no JS. With both sources off, or the module off,
//! it is dropped. Past sessions are on disk only; the history list reads a
//! small index, one past session reads one file.
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
use svc_log::{LogEvent, WatchSignal};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{mpsc, oneshot};

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
/// How much of a seed is remembered to recognise a batch it already held.
const SEED_TAIL: usize = 512;

/// Payload of [`SESSION_CHANGED_EVENT`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
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
        index.sort_by_key(|s| std::cmp::Reverse(s.started_at));
        for dropped in index.drain(HISTORY_CAP.min(index.len())..) {
            if let Some(old) = self.session_path(&dropped.id) {
                let _ = std::fs::remove_file(old);
            }
        }
        write_compact(&self.index_path(), &index)
    }

    /// File `session` unless there is nothing in it. Returns whether the
    /// history changed.
    fn file_if_worth(&self, session: &Session) -> bool {
        if session.is_trivial() {
            return false;
        }
        match self.file(session) {
            Ok(()) => true,
            Err(e) => {
                tracing::warn!("could not file session {}: {e}", session.id);
                false
            }
        }
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
    /// The end of what [`Host::seed`] replayed, until the next batch
    /// arrives: that batch may be one the seed already held.
    seed_tail: Option<Vec<LogEvent>>,
    checkpointed: Option<Instant>,
}

static HOST: LazyLock<Mutex<Host>> = LazyLock::new(Mutex::default);

fn now_ms() -> Millis {
    chrono::Utc::now().timestamp_millis()
}

/// What the watcher holds of the session it follows, for [`Host::seed`].
struct Watched {
    log: PathBuf,
    /// The log from its first line, oldest first.
    events: Vec<LogEvent>,
}

impl Host {
    /// Anything held that a switched-off module must let go of.
    fn active(&self) -> bool {
        self.restored || self.seeded || self.reducer.current().is_some()
    }

    /// Take one reducer call's outcome to disk. Returns what to announce.
    fn settle(&mut self, store: &Store, outcome: Outcome, now: Instant) -> Option<SessionChange> {
        if let Some(done) = &outcome.finished {
            let history = store.file_if_worth(done);
            store.clear_checkpoint();
            self.checkpointed = None;
            return Some(SessionChange { history });
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
    /// Returns whether the history changed.
    fn restore(&mut self, store: &Store) -> bool {
        if std::mem::replace(&mut self.restored, true) {
            return false;
        }
        store
            .take_checkpoint()
            .is_some_and(|kept| store.file_if_worth(&self.reducer.restore(kept)))
    }

    /// Take in the session the log watcher already follows: the module was
    /// switched on after the watcher started following it, so its signals
    /// went by unheard. A no-op once the watcher's session is known.
    fn seed(&mut self, watched: impl FnOnce() -> Option<Watched>) -> Outcome {
        if std::mem::replace(&mut self.seeded, true) {
            return Outcome::default();
        }
        let Some(watched) = watched() else {
            return Outcome::default();
        };
        self.reducer.log_session_started(channel_of(&watched.log));
        let outcome = self.reducer.log_events(&watched.events, true);
        let tail = watched.events.len().saturating_sub(SEED_TAIL);
        self.seed_tail = Some(watched.events[tail..].to_vec());
        outcome
    }

    /// The part of a watcher batch the seed did not already replay. The
    /// watcher puts a batch into its session before it signals it, so a
    /// seed taken in between holds that batch at its end, and a seed read
    /// from the file may be ahead of the watcher by part of a batch.
    fn unseen<'a>(&mut self, batch: &'a [LogEvent]) -> &'a [LogEvent] {
        let Some(seeded) = self.seed_tail.take() else {
            return batch;
        };
        let held = (1..=batch.len().min(seeded.len()))
            .rev()
            .find(|&n| seeded.ends_with(&batch[..n]))
            .unwrap_or(0);
        &batch[held..]
    }

    /// The module went off: close and file what is live, hold nothing.
    /// Returns what to announce.
    fn shut_down(&mut self, store: &Store, now: Millis) -> Option<SessionChange> {
        if !self.active() {
            return None;
        }
        let mut history = false;
        for outcome in [
            self.reducer.log_session_ended(now),
            self.reducer.telemetry_stopped(now),
        ] {
            if let Some(done) = &outcome.finished {
                history |= store.file_if_worth(done);
            }
        }
        store.clear_checkpoint();
        *self = Host::default();
        Some(SessionChange { history })
    }

    /// One feed, start to finish: restore once, run `call`, take its
    /// outcome to disk, and let go of everything when nothing can feed the
    /// session any more.
    fn run(
        &mut self,
        store: &Store,
        gates: Gates,
        call: impl FnOnce(&mut Host) -> Outcome,
    ) -> Option<SessionChange> {
        if !gates.module {
            return self.shut_down(store, now_ms());
        }
        let restored = self.restore(store);
        let outcome = call(self);
        let mut change = self.settle(store, outcome, Instant::now());
        if restored {
            change = Some(SessionChange { history: true });
        }
        // Both sources off and nothing live: the last session is on disk,
        // and nothing in memory can still grow.
        let idle =
            !gates.log && !gates.telemetry && self.reducer.current().is_some_and(|s| !s.live);
        if idle {
            self.reducer.clear();
            self.seed_tail = None;
            change = change.or(Some(SessionChange { history: false }));
        }
        change
    }
}

/// The install channel a `Game.log` belongs to: its directory's name
/// (`…/StarCitizen/LIVE/Game.log` → `LIVE`).
fn channel_of(log: &Path) -> Option<String> {
    Some(log.parent()?.file_name()?.to_str()?.to_owned())
}

/// The settings a feed depends on, read once per job.
#[derive(Debug, Clone, Copy)]
struct Gates {
    /// The module is offered by this build and enabled.
    module: bool,
    log: bool,
    telemetry: bool,
}

/// Is the module part of this build? An unreleased module is not in a
/// release build's registry, whatever a shared data dir says is enabled.
fn offered() -> bool {
    crate::modules::registry()
        .iter()
        .any(|m| m.id() == MODULE_ID)
}

fn gates(app: &AppHandle) -> Gates {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap();
    Gates {
        module: offered() && settings.enabled_modules.iter().any(|m| m == MODULE_ID),
        log: settings.gamelog_enabled,
        telemetry: settings.telemetry_enabled,
    }
}

// ── The queue ───────────────────────────────────────────────────────────────

type Job = Box<dyn FnOnce() + Send>;

/// The feed queue: one task that runs the jobs in order, each on the
/// blocking pool. Started on first use; idle, it is a parked task.
static QUEUE: LazyLock<mpsc::UnboundedSender<Job>> = LazyLock::new(|| {
    let (tx, mut rx) = mpsc::unbounded_channel::<Job>();
    tauri::async_runtime::spawn(async move {
        while let Some(job) = rx.recv().await {
            if let Err(e) = tauri::async_runtime::spawn_blocking(job).await {
                tracing::warn!("a session log job failed: {e}");
            }
        }
    });
    tx
});

/// Queue one reducer call: it runs after every call queued before it, off
/// the caller's thread, files what it finished and announces what it
/// changed. Never blocks.
fn feed(app: &AppHandle, call: impl FnOnce(&mut Host) -> Outcome + Send + 'static) {
    let app = app.clone();
    let job: Job = Box::new(move || {
        let gates = gates(&app);
        let change = HOST
            .lock()
            .unwrap()
            .run(&Store::at_data_root(), gates, call);
        if let Some(change) = change {
            let _ = app.emit(SESSION_CHANGED_EVENT, change);
        }
    });
    let _ = QUEUE.send(job);
}

/// Wait until everything queued so far has run.
async fn drained() {
    let (tx, rx) = oneshot::channel();
    let job: Job = Box::new(move || {
        let _ = tx.send(());
    });
    if QUEUE.send(job).is_ok() {
        let _ = rx.await;
    }
}

// ── Feeds (called by gamelog.rs, telemetry.rs and settings.rs) ──────────────

/// A signal of the Game.log watcher, on its thread: copied and queued.
pub(crate) fn on_log_signal(app: &AppHandle, signal: &WatchSignal) {
    let signal = signal.clone();
    feed(app, move |host| match signal {
        WatchSignal::SessionStarted { log } => {
            host.seeded = true;
            host.seed_tail = None;
            host.reducer.log_session_started(channel_of(&log));
            Outcome::default()
        }
        WatchSignal::Events { events, replay } => {
            let fresh = host.unseen(&events);
            host.reducer.log_events(fresh, replay)
        }
        WatchSignal::SessionEnded => {
            host.seed_tail = None;
            host.reducer.log_session_ended(now_ms())
        }
    });
}

/// The Game.log opt-in went off: the watcher is gone and will send no end
/// signal.
pub(crate) fn on_log_stopped(app: &AppHandle) {
    feed(app, |host| {
        host.seeded = false;
        host.seed_tail = None;
        host.reducer.log_session_ended(now_ms())
    });
}

/// What the session keeps of a telemetry frame. Built under the telemetry
/// lock, so it only copies; `at` is Unix milliseconds.
///
/// The balance is taken **only from a frame that carries an actor**: the
/// snapshot keeps the last balance across frames without one, and a value
/// carried over is not a reading (it may be another character's).
pub(crate) fn sample_of(
    snapshot: &svc_telemetry::Snapshot,
    frame: &svc_telemetry::Frame,
    at: Millis,
) -> TelemetrySample {
    use svc_telemetry::Event as E;
    TelemetrySample {
        at,
        balance: frame.actor.as_ref().map(|a| a.uec_balance),
        vehicle: snapshot.vehicle.as_ref().map(|v| Vehicle {
            class_name: v.class_name.clone(),
            name: v.name.clone(),
        }),
        events: frame
            .events
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
    feed(app, move |host| host.reducer.telemetry(&sample));
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

/// The settings changed (`update_settings`): with the module switched off,
/// or the last source, what is held is let go of.
pub(crate) fn sync(app: &AppHandle) {
    feed(app, |_| Outcome::default());
}

/// App exit: keep the live session so the next run can close and file it.
/// Synchronous on purpose: the queue does not outlive the runtime.
pub(crate) fn flush(app: &AppHandle) {
    if gates(app).module {
        HOST.lock().unwrap().checkpoint(&Store::at_data_root());
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

/// The session the watcher follows right now, from its first line. The
/// watcher keeps only the newest events of a long session; when it dropped
/// some, the log itself is read, because the session is named by its first
/// line. `None` when no live session is followed.
fn watched(app: &AppHandle) -> Option<Watched> {
    let snapshot = app
        .state::<AppState>()
        .log_watch
        .lock()
        .unwrap()
        .as_ref()
        .map(|w| w.session())?;
    if !snapshot.live {
        // A finished one is history the module never saw: not rebuilt.
        return None;
    }
    let log = PathBuf::from(snapshot.log?);
    let events = if snapshot.dropped == 0 {
        snapshot.events
    } else {
        match svc_log::read_log(&log) {
            Ok(events) => events,
            Err(e) => {
                tracing::warn!("could not read {} whole: {e}", log.display());
                snapshot.events
            }
        }
    };
    Some(Watched { log, events })
}

/// The current session. A memory read, after the queue has drained; the
/// first call of a run also files the session kept from the last one, and
/// the first after the module was switched on takes in the session the log
/// watcher already follows.
#[tauri::command]
#[specta::specta]
pub(crate) async fn session_state(app: AppHandle) -> SessionStateView {
    let gates = gates(&app);
    if gates.module {
        let handle = app.clone();
        feed(&app, move |host| host.seed(|| watched(&handle)));
    }
    drained().await;
    SessionStateView {
        log_enabled: gates.log,
        telemetry_enabled: gates.telemetry,
        current: HOST.lock().unwrap().reducer.current().cloned(),
    }
}

/// The finished sessions, newest first (at most [`HISTORY_CAP`]). One small
/// file read, after the queue has drained (so a session being filed is in
/// it); empty while the module is off.
#[tauri::command]
#[specta::specta]
pub(crate) async fn session_history(app: AppHandle) -> Result<Vec<SessionSummary>, AppError> {
    if !gates(&app).module {
        return Ok(Vec::new());
    }
    // The first feed of a run files the session kept from the last one.
    sync(&app);
    drained().await;
    tokio::task::spawn_blocking(|| Store::at_data_root().index())
        .await
        .map_err(|e| AppError::Internal(format!("session history read failed: {e}")))
}

/// One finished session with its timeline; `None` when it is not kept (any
/// more).
#[tauri::command]
#[specta::specta]
pub(crate) async fn session_get(app: AppHandle, id: String) -> Result<Option<Session>, AppError> {
    if !gates(&app).module {
        return Ok(None);
    }
    tokio::task::spawn_blocking(move || Store::at_data_root().get(&id))
        .await
        .map_err(|e| AppError::Internal(format!("session read failed: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use svc_log::LogEventKind;

    const ALL_ON: Gates = Gates {
        module: true,
        log: true,
        telemetry: true,
    };

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

    fn joined(ts: &str) -> LogEvent {
        ev(
            ts,
            LogEventKind::ShardJoined {
                shard: "pub_euw1b_13165946_100".into(),
            },
        )
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
                joined(&format!("2026-11-{day:02}T16:13:05.561Z")),
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
    fn the_session_live_at_the_last_exit_is_filed_and_announced_by_the_first_feed() {
        let (_dir, store) = store();
        let host = live_host();
        host.checkpoint(&store);

        // The first feed of the next run, whatever it is, announces the
        // filing: the history list that loads beside it must reload.
        let mut next = Host::default();
        let change = next.run(&store, ALL_ON, |_| Outcome::default());
        assert_eq!(change, Some(SessionChange { history: true }));
        assert!(!store.checkpoint_path().exists());
        let index = store.index();
        assert_eq!(index.len(), 1);
        assert!(index[0].ended_at.is_some());
        let last = next.reducer.current().unwrap();
        assert!(!last.live);
        assert_eq!(last.id, index[0].id);

        // Only once per run.
        host.checkpoint(&store);
        assert_eq!(next.run(&store, ALL_ON, |_| Outcome::default()), None);
        assert!(store.checkpoint_path().exists());
    }

    fn snapshot_events() -> Vec<LogEvent> {
        vec![
            ev("2026-11-13T16:10:48.019Z", LogEventKind::SessionStarted),
            joined("2026-11-13T16:13:05.561Z"),
        ]
    }

    fn watched_ptu(events: Vec<LogEvent>) -> Option<Watched> {
        Some(Watched {
            log: PathBuf::from(r"C:\Games\StarCitizen\PTU\Game.log"),
            events,
        })
    }

    #[test]
    fn switching_the_module_on_mid_game_takes_the_watchers_session_in() {
        let mut host = Host::default();
        assert!(host.seed(|| watched_ptu(snapshot_events())).changed);
        let session = host.reducer.current().unwrap();
        assert!(session.live);
        assert_eq!(session.channel.as_deref(), Some("PTU"));
        assert_eq!(session.id, "2026-11-13T16-10-48.019Z");
        assert_eq!(session.shards.len(), 1);
        // Only once: the signals carry on from here, and the watcher is
        // not even asked again.
        assert_eq!(
            host.seed(|| panic!("asked the watcher twice")),
            Outcome::default()
        );

        // Nothing followed: nothing rebuilt.
        let mut host = Host::default();
        assert_eq!(host.seed(|| None), Outcome::default());
        assert!(host.reducer.current().is_none());
    }

    #[test]
    fn a_batch_the_seed_already_held_is_not_replayed_twice() {
        // The watcher put this batch into its session, the seed read the
        // session, and only then did the batch's own signal arrive.
        let mut host = Host::default();
        host.seed(|| watched_ptu(snapshot_events()));
        let late = snapshot_events().split_off(1);
        assert!(host.unseen(&late).is_empty());
        // Checked against one batch only.
        assert_eq!(host.unseen(&late).len(), 1);

        // A seed read from the file can be ahead by part of a batch.
        let mut host = Host::default();
        host.seed(|| watched_ptu(snapshot_events()));
        let mut batch = snapshot_events().split_off(1);
        batch.push(joined("2026-11-13T17:00:00.000Z"));
        let fresh = host.unseen(&batch).to_vec();
        assert_eq!(fresh, [joined("2026-11-13T17:00:00.000Z")]);
        let outcome = host.reducer.log_events(&fresh, false);
        assert!(outcome.changed);
        assert_eq!(host.reducer.current().unwrap().shards.len(), 2);

        // A batch the seed did not hold passes whole.
        let mut host = Host::default();
        host.seed(|| watched_ptu(snapshot_events()));
        let new = [joined("2026-11-13T17:00:00.000Z")];
        assert_eq!(host.unseen(&new).len(), 1);
    }

    #[test]
    fn the_module_going_off_files_the_live_session_and_holds_nothing() {
        let (_dir, store) = store();
        let mut host = live_host();
        host.reducer
            .log_events(&[joined("2026-11-13T16:13:05.561Z")], false);
        host.checkpoint(&store);
        let off = Gates {
            module: false,
            ..ALL_ON
        };

        let change = host.run(&store, off, |_| panic!("fed while off"));
        assert_eq!(change, Some(SessionChange { history: true }));
        assert!(!host.active());
        assert!(!store.checkpoint_path().exists());
        assert_eq!(store.index().len(), 1);

        // Off and empty: nothing happens, nothing is read.
        assert_eq!(host.run(&store, off, |_| panic!("fed while off")), None);
        assert!(!host.restored);
    }

    #[test]
    fn with_both_sources_off_the_last_session_leaves_memory() {
        let (_dir, store) = store();
        let mut host = live_host();
        let both_off = Gates {
            log: false,
            telemetry: false,
            ..ALL_ON
        };

        // The telemetry opt-in's stop arrives first: the log session is
        // still live and must not be dropped unfiled.
        let change = host.run(&store, both_off, |host| {
            host.reducer.telemetry_stopped(now_ms())
        });
        assert_eq!(change, None);
        assert!(host.reducer.current().unwrap().live);

        // Then the log opt-in's stop: filed, then let go of.
        let change = host.run(&store, both_off, |host| {
            host.reducer.log_session_ended(now_ms())
        });
        assert_eq!(change, Some(SessionChange { history: true }));
        assert!(host.reducer.current().is_none());
        assert_eq!(store.index().len(), 1);

        // One source still on: the last session stays to be shown.
        let mut host = live_host();
        let one_on = Gates {
            log: false,
            ..ALL_ON
        };
        host.run(&store, one_on, |host| {
            host.reducer.log_session_ended(now_ms())
        });
        assert!(host.reducer.current().is_some());
    }

    #[test]
    fn the_module_is_offered_by_every_build() {
        // Shipped in v0.6.0: both halves of the registry list it.
        assert!(offered());
    }

    #[test]
    fn the_channel_is_the_logs_directory() {
        assert_eq!(
            channel_of(Path::new(r"C:\Games\StarCitizen\LIVE\Game.log")).as_deref(),
            Some("LIVE")
        );
        assert_eq!(channel_of(Path::new("Game.log")), None);
    }

    fn frame(json: serde_json::Value) -> svc_telemetry::Frame {
        let mut full = serde_json::json!({
            "vehicle": null, "weapons": null, "actor": null, "turret": null, "events": [],
        });
        for (k, v) in json.as_object().unwrap() {
            full[k] = v.clone();
        }
        serde_json::from_value(full).unwrap()
    }

    fn actor(balance: i64) -> serde_json::Value {
        let mut actor = serde_json::to_value(svc_telemetry::Actor::default()).unwrap();
        actor["uec_balance"] = balance.into();
        actor
    }

    #[test]
    fn a_sample_keeps_the_vehicle_and_the_life_events() {
        let frame = frame(serde_json::json!({
            "events": ["EnterSeat", "Dead", { "Other": 42 }, "Respawn"],
        }));
        let mut reducer = svc_telemetry::Reducer::default();
        reducer.apply(&frame, 1_794_586_385);
        let sample = sample_of(reducer.snapshot(), &frame, 1_794_586_385_000);
        assert_eq!(sample.at, 1_794_586_385_000);
        assert_eq!(sample.vehicle, None);
        assert_eq!(
            sample.events,
            [TelemetryEvent::Dead, TelemetryEvent::Respawn]
        );
    }

    #[test]
    fn a_balance_is_a_reading_only_from_a_frame_with_an_actor() {
        let mut reducer = svc_telemetry::Reducer::default();
        let with_actor = frame(serde_json::json!({ "actor": actor(125_000) }));
        reducer.apply(&with_actor, 1);
        assert_eq!(
            sample_of(reducer.snapshot(), &with_actor, 1_000).balance,
            Some(125_000)
        );

        // The snapshot still shows the last balance; the frame said none.
        let without = frame(serde_json::json!({ "events": ["Spawn"] }));
        reducer.apply(&without, 2);
        assert_eq!(reducer.snapshot().balance, Some(125_000));
        assert_eq!(sample_of(reducer.snapshot(), &without, 2_000).balance, None);
    }
}
