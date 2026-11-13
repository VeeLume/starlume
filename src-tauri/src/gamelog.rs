//! Game.log wiring — svc-log's watcher onto the [`bus`](crate::bus) and the
//! webview, plus the one command that reads the current session.
//!
//! **Opt-in.** `AppSettings::gamelog_enabled`, off by default (as Hearth's
//! log sensor was). [`sync`] starts the watcher thread when it goes on
//! (startup, `update_settings`) and stops it when it goes off; with it off
//! nothing of this module is resident and no log is statted or read.
//!
//! **Not gated by the online policy.** Reading a local file is not a
//! network call. Anything that sends what is read here somewhere else is,
//! and takes the normal gates.
//!
//! **Cost while on.** With the game closed the watcher stats each install's
//! `Game.log` every five seconds and does nothing else; svc-log's `watch`
//! docs have the full model. Its rare process check goes through
//! `svc_discovery::process::game_running_recent`, the app's one check.
//!
//! Two ways out, for two kinds of listener:
//! - **Rust consumers** (session log, presence, the refinery timer, when
//!   they land) subscribe to the bus: `GameSession` and `GameLog`.
//! - **The webview** gets [`GAMELOG_CHANGED_EVENT`] with the same batch,
//!   and re-reads [`gamelog_session`] on focus: a suspended webview misses
//!   events, so the session itself is held Rust-side ("disposable
//!   frontend", CLAUDE.md).
//!
//! Both carry `replay`: a batch read while catching up on a session that
//! was already running when the watcher found it. History, not news — see
//! [`BusEvent::GameLog`].

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use svc_log::{LogEvent, SessionSnapshot, WatchSignal};
use tauri::{AppHandle, Emitter, Manager};

use crate::AppState;
use crate::bus::BusEvent;
use crate::error::AppError;

/// Emitted for every watcher signal, payload [`GameLogChange`].
pub const GAMELOG_CHANGED_EVENT: &str = "gamelog:changed";

/// How old a process scan may be for the watcher's quiet check.
const RUNNING_MAX_AGE: Duration = Duration::from_secs(5);

/// Payload of [`GAMELOG_CHANGED_EVENT`].
#[derive(Debug, Clone, serde::Serialize)]
struct GameLogChange {
    /// The game is running and its log is being followed.
    live: bool,
    /// A new session began: drop what was shown before `events`.
    started: bool,
    /// `events` were already in the log when the watcher found it (they
    /// can be hours old): show them, don't announce them.
    replay: bool,
    /// The events this change adds, oldest first.
    events: Arc<[LogEvent]>,
}

/// The `Game.log` of every known install, from the shell's install mirror.
fn log_paths(app: &AppHandle) -> Vec<PathBuf> {
    app.state::<AppState>()
        .installs
        .lock()
        .unwrap()
        .iter()
        .filter_map(|install| install.p4k_path.parent())
        .map(svc_log::game_log_path)
        .collect()
}

/// Bring the watcher in line with the opt-in: start it when the setting is
/// on, stop it when it is off. Called at startup and from `update_settings`.
pub(crate) fn sync(app: &AppHandle) {
    let state = app.state::<AppState>();
    let enabled = state.settings.lock().unwrap().gamelog_enabled;
    let mut watch = state.log_watch.lock().unwrap();
    match (enabled, watch.is_some()) {
        (true, false) => {
            *watch = Some(start(app));
            tracing::info!("Game.log watcher started");
        }
        (false, true) => {
            let stopped = watch.take();
            // Join outside the lock: the thread's last signal may be on its
            // way through a command that wants it.
            drop(watch);
            drop(stopped);
            forget_last_session();
            tracing::info!("Game.log watcher stopped");
            let _ = app.emit(
                GAMELOG_CHANGED_EVENT,
                GameLogChange {
                    live: false,
                    started: true,
                    replay: false,
                    events: Arc::from([]),
                },
            );
        }
        _ => {}
    }
}

fn start(app: &AppHandle) -> svc_log::LogWatch {
    let sources = {
        let app = app.clone();
        move || log_paths(&app)
    };
    let on_signal = {
        let app = app.clone();
        let bus = app.state::<AppState>().bus.clone();
        move |signal: WatchSignal| {
            let change = match signal {
                WatchSignal::SessionStarted { .. } => {
                    // The on-demand copy of the last session is stale now.
                    forget_last_session();
                    let _ = bus.send(BusEvent::GameSession { live: true });
                    GameLogChange {
                        live: true,
                        started: true,
                        replay: false,
                        events: Arc::from([]),
                    }
                }
                WatchSignal::Events { events, replay } => {
                    let events: Arc<[LogEvent]> = events.into();
                    // No receivers is fine (nothing subscribed yet).
                    let _ = bus.send(BusEvent::GameLog {
                        events: events.clone(),
                        replay,
                    });
                    GameLogChange {
                        live: true,
                        started: false,
                        replay,
                        events,
                    }
                }
                WatchSignal::SessionEnded => {
                    let _ = bus.send(BusEvent::GameSession { live: false });
                    GameLogChange {
                        live: false,
                        started: false,
                        replay: false,
                        events: Arc::from([]),
                    }
                }
            };
            let _ = app.emit(GAMELOG_CHANGED_EVENT, change);
        }
    };
    let watch = svc_log::watch::spawn(
        svc_log::WatchConfig::default(),
        sources,
        || svc_discovery::process::game_running_recent(RUNNING_MAX_AGE),
        on_signal,
    );

    // The watcher reads the install mirror; make sure a scan has filled it
    // even when nothing else at startup does (game-data auto-load off).
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if log_paths(&app).is_empty() {
            let _ = crate::data::refresh_installs(&app).await;
        }
    });
    watch
}

/// Stop the watcher thread (app exit).
pub fn stop(app: &AppHandle) {
    let watch = app.state::<AppState>().log_watch.lock().unwrap().take();
    drop(watch);
}

/// The current game session's events: the live one, or the last one seen.
/// Empty while the opt-in is off (nothing is read then).
///
/// When no session was seen since the watcher started (the game was closed
/// all along), the watcher holds nothing; the last session is then read
/// from the most recently written `Game.log` and kept until that file
/// changes or a session starts.
#[tauri::command]
#[specta::specta]
pub(crate) async fn gamelog_session(app: AppHandle) -> Result<SessionSnapshot, AppError> {
    let held = {
        let state = app.state::<AppState>();
        let watch = state.log_watch.lock().unwrap();
        match watch.as_ref() {
            None => return Ok(SessionSnapshot::default()),
            Some(watch) => watch.session(),
        }
    };
    if held.log.is_some() {
        return Ok(held);
    }

    let mut paths = log_paths(&app);
    if paths.is_empty() {
        let _ = crate::data::refresh_installs(&app).await;
        paths = log_paths(&app);
    }
    tokio::task::spawn_blocking(move || last_session(paths))
        .await
        .map_err(|e| AppError::Internal(format!("log read task failed: {e}")))?
}

/// The last on-demand read, keyed by the file it came from as it was then.
struct LastSession {
    path: PathBuf,
    len: u64,
    modified: SystemTime,
    snapshot: SessionSnapshot,
}

static LAST_SESSION: Mutex<Option<LastSession>> = Mutex::new(None);

fn forget_last_session() {
    *LAST_SESSION.lock().unwrap() = None;
}

/// The most recently written of `paths`, as a finished session. Parsed
/// once per state of the file: a repeat call costs a stat per path.
fn last_session(paths: Vec<PathBuf>) -> Result<SessionSnapshot, AppError> {
    let newest = paths
        .into_iter()
        .filter_map(|path| {
            let meta = std::fs::metadata(&path).ok()?;
            Some((meta.modified().ok()?, meta.len(), path))
        })
        .max();
    let Some((modified, len, path)) = newest else {
        return Ok(SessionSnapshot::default());
    };
    let mut last = LAST_SESSION.lock().unwrap();
    if let Some(cached) = last.as_ref()
        && cached.path == path
        && cached.len == len
        && cached.modified == modified
    {
        return Ok(cached.snapshot.clone());
    }

    let mut events = svc_log::read_log(&path)?;
    let cap = svc_log::WatchConfig::default().max_events;
    let dropped = events.len().saturating_sub(cap);
    events.drain(..dropped);
    let snapshot = SessionSnapshot {
        live: false,
        log: Some(path.display().to_string()),
        events,
        dropped: dropped as u32,
    };
    *last = Some(LastSession {
        path,
        len,
        modified,
        snapshot: snapshot.clone(),
    });
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD: &str = "<2026-11-12T00:07:13.188Z> Log started on Thu Nov 12 00:07:13 2026\n";
    const NEW: &str = concat!(
        "<2026-11-13T16:10:48.019Z> Log started on Fri Nov 13 16:10:48 2026\n",
        "<2026-11-13T16:13:05.561Z> [Notice] <Join PU> address[34.156.32.60] port[64385] shard[pub_euw1b_13165946_100] locationId[-281470681677823] [Team_GameServices][GIM][Matchmaking]\n",
    );

    fn older(path: &std::path::Path, by: u64) {
        let earlier = SystemTime::now() - Duration::from_secs(by);
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(earlier)
            .unwrap();
    }

    // One test: the cache is a process-wide static.
    #[test]
    fn the_last_session_is_the_log_written_last_and_is_parsed_once_per_file_state() {
        assert_eq!(
            last_session(Vec::new()).unwrap(),
            SessionSnapshot::default(),
            "no install is an empty session"
        );

        let dir = tempfile::tempdir().unwrap();
        let live = dir.path().join("live.log");
        let ptu = dir.path().join("ptu.log");
        std::fs::write(&ptu, OLD).unwrap();
        older(&ptu, 3600);
        std::fs::write(&live, NEW).unwrap();
        older(&live, 60);
        let paths = vec![ptu, live.clone(), dir.path().join("missing.log")];

        let session = last_session(paths.clone()).unwrap();
        assert!(!session.live);
        assert_eq!(session.log, Some(live.display().to_string()));
        assert_eq!(session.events.len(), 2);
        assert_eq!(session.dropped, 0);

        // Same file state: answered from the cache. Proof: the cached
        // snapshot is what comes back, even after it is tampered with.
        LAST_SESSION
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .snapshot
            .dropped = 7;
        assert_eq!(last_session(paths.clone()).unwrap().dropped, 7);

        // The file changed: read again.
        std::fs::write(&live, format!("{NEW}{OLD}")).unwrap();
        let again = last_session(paths.clone()).unwrap();
        assert_eq!(again.events.len(), 3);
        assert_eq!(again.dropped, 0);

        forget_last_session();
        assert!(LAST_SESSION.lock().unwrap().is_none());
    }
}
