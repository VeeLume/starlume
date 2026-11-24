//! Session captures: the shell side of `svc_telemetry::capture`. With the
//! Session Log's "Capture full sessions" on (`AppSettings::session_capture`,
//! off by default), every telemetry frame of the live session is recorded,
//! and its Game.log is kept beside the frames when the session ends, both
//! compressed, in `sessions/<id>.capture/`. What they are for: studying
//! what the game says during normal play (Rich Presence is reshaped from
//! them), and the dev replay.
//!
//! **No network.** Local files only, so no online gate applies.
//!
//! **Retention.** A capture goes with its session (the Session Log's own
//! cap), and all captures together stay within
//! `AppSettings::session_capture_budget_mb`: the oldest are deleted first,
//! never the one being written.
//!
//! **Memory.** One recorder, holding at most one block of frames
//! (`svc_telemetry::capture::BLOCK_BYTES`).

use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use svc_telemetry::capture::{Recorder, gzip_file};
use tauri::{AppHandle, Manager};

use crate::AppState;

const FRAMES_FILE: &str = "telemetry.jsonl.gz";
const LOG_FILE: &str = "Game.log.gz";

/// The capture of the live session.
struct Capture {
    id: String,
    /// Opened at the first frame: a session read only from the log has
    /// none.
    recorder: Option<Recorder>,
}

static CAPTURE: LazyLock<Mutex<Option<Capture>>> = LazyLock::new(Mutex::default);

fn enabled(app: &AppHandle) -> bool {
    app.state::<AppState>()
        .settings
        .lock()
        .unwrap()
        .session_capture
}

/// One telemetry frame, from the read loop (`telemetry.rs`): kept when
/// captures are on and a session is live. Cheap when off.
pub(crate) fn on_frame(app: &AppHandle, ms: i64, frame: &svc_telemetry::Frame) {
    if !enabled(app) {
        return;
    }
    let Some(id) = crate::session::live_id(app) else {
        return;
    };
    let mut held = CAPTURE.lock().unwrap();
    if held.as_ref().is_none_or(|c| c.id != id) {
        if let Some(done) = held.take() {
            finish(app, done);
        }
        *held = Some(Capture {
            id: id.clone(),
            recorder: None,
        });
    }
    let capture = held.as_mut().expect("set above");
    let recorder = capture.recorder.get_or_insert_with(|| {
        let dir = crate::session::capture_dir(&id).unwrap_or_else(crate::session::sessions_dir);
        Recorder::new(dir.join(FRAMES_FILE))
    });
    if let Err(e) = recorder.push(ms, frame) {
        tracing::warn!("session capture: {e}");
    }
}

/// The live session changed: a new one begins its capture, an ended one
/// finishes (its Game.log is kept).
pub(crate) fn on_session_change(app: &AppHandle) {
    let live = if enabled(app) {
        crate::session::live_id(app)
    } else {
        None
    };
    let mut held = CAPTURE.lock().unwrap();
    if held.as_ref().map(|c| &c.id) == live.as_ref() {
        return;
    }
    if let Some(done) = held.take() {
        finish(app, done);
    }
    *held = live.map(|id| Capture { id, recorder: None });
}

/// Settings changed: switched off, the capture ends; a tighter budget
/// trims now.
pub(crate) fn sync(app: &AppHandle) {
    on_session_change(app);
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || trim(&app));
}

/// Write the frames held in memory (the app is closing).
pub(crate) fn flush() {
    if let Some(r) = CAPTURE
        .lock()
        .unwrap()
        .as_mut()
        .and_then(|c| c.recorder.as_mut())
        && let Err(e) = r.flush()
    {
        tracing::warn!("session capture: {e}");
    }
}

/// A capture is done: write its last frames, then keep its Game.log and
/// trim to the budget, off the caller's thread.
fn finish(app: &AppHandle, capture: Capture) {
    drop(capture.recorder);
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        keep_log(&app, &capture.id);
        trim(&app);
    });
}

/// Keep a compressed copy of the session's Game.log in its capture. The
/// game finished writing it when the session ended; a session that left no
/// trace in the logs keeps only its frames.
fn keep_log(app: &AppHandle, id: &str) {
    let (Some(log), Some(dir)) = (
        crate::session::log_for(app, id),
        crate::session::capture_dir(id),
    ) else {
        return;
    };
    if let Err(e) = gzip_file(&log, &dir.join(LOG_FILE)) {
        tracing::warn!("session capture: keeping {}: {e}", log.display());
    }
}

/// Delete the oldest captures until all fit the budget; the live one stays.
fn trim(app: &AppHandle) {
    let budget = u64::from(
        app.state::<AppState>()
            .settings
            .lock()
            .unwrap()
            .session_capture_budget_mb,
    ) << 20;
    let live = CAPTURE.lock().unwrap().as_ref().map(|c| c.id.clone());
    for dir in over_budget(
        &captures(&crate::session::sessions_dir()),
        budget,
        live.as_deref(),
    ) {
        tracing::info!(
            "session capture: over the budget, deleting {}",
            dir.display()
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// One capture on disk.
#[derive(Debug, Clone, PartialEq)]
struct Kept {
    id: String,
    dir: PathBuf,
    bytes: u64,
}

/// The captures under `sessions`, oldest first (ids sort by start time).
fn captures(sessions: &Path) -> Vec<Kept> {
    let Ok(entries) = std::fs::read_dir(sessions) else {
        return Vec::new();
    };
    let mut out: Vec<Kept> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let id = name.strip_suffix(".capture")?.to_owned();
            let dir = e.path();
            let bytes = std::fs::read_dir(&dir)
                .ok()?
                .flatten()
                .filter_map(|f| f.metadata().ok())
                .map(|m| m.len())
                .sum();
            Some(Kept { id, dir, bytes })
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// The capture folders to delete, oldest first, so the rest fit `budget`.
fn over_budget(kept: &[Kept], budget: u64, live: Option<&str>) -> Vec<PathBuf> {
    let mut total: u64 = kept.iter().map(|k| k.bytes).sum();
    let mut out = Vec::new();
    for k in kept {
        if total <= budget {
            break;
        }
        if Some(k.id.as_str()) == live {
            continue;
        }
        total -= k.bytes;
        out.push(k.dir.clone());
    }
    out
}

/// What the captures take, for Settings.
#[derive(Debug, Clone, Default, serde::Serialize, specta::Type)]
pub struct CaptureUsage {
    pub sessions: u32,
    #[specta(type = f64)]
    pub bytes: u64,
    /// A capture is being written now.
    pub recording: bool,
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn session_capture_usage() -> CaptureUsage {
    let recording = CAPTURE
        .lock()
        .unwrap()
        .as_ref()
        .is_some_and(|c| c.recorder.is_some());
    tokio::task::spawn_blocking(move || {
        let kept = captures(&crate::session::sessions_dir());
        CaptureUsage {
            sessions: kept.len() as u32,
            bytes: kept.iter().map(|k| k.bytes).sum(),
            recording,
        }
    })
    .await
    .unwrap_or_default()
}

/// Open the folder the captures are in.
#[tauri::command]
#[specta::specta]
pub(crate) fn session_capture_reveal(app: AppHandle) -> Result<(), crate::error::AppError> {
    use tauri_plugin_opener::OpenerExt;
    let dir = crate::session::sessions_dir();
    std::fs::create_dir_all(&dir).map_err(|e| crate::error::AppError::Internal(e.to_string()))?;
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|e| crate::error::AppError::Internal(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kept(id: &str, mb: u64) -> Kept {
        Kept {
            id: id.into(),
            dir: PathBuf::from(format!("{id}.capture")),
            bytes: mb << 20,
        }
    }

    #[test]
    fn the_oldest_go_first_until_the_rest_fit() {
        let all = [
            kept("2026-11-20", 300),
            kept("2026-11-21", 200),
            kept("2026-11-22", 100),
        ];
        assert_eq!(
            over_budget(&all, 500 << 20, None),
            vec![PathBuf::from("2026-11-20.capture")]
        );
        assert!(over_budget(&all, 600 << 20, None).is_empty());
        assert_eq!(over_budget(&all, 50 << 20, None).len(), 3);
    }

    #[test]
    fn the_capture_being_written_is_never_trimmed() {
        let all = [kept("2026-11-20", 300), kept("2026-11-21", 200)];
        assert_eq!(
            over_budget(&all, 100 << 20, Some("2026-11-20")),
            vec![PathBuf::from("2026-11-21.capture")]
        );
    }

    #[test]
    fn captures_are_found_beside_the_sessions_oldest_first() {
        let dir = tempfile::tempdir().unwrap();
        for (id, size) in [
            ("2026-11-22T10-00-00.000Z", 30),
            ("2026-11-21T10-00-00.000Z", 10),
        ] {
            let c = dir.path().join(format!("{id}.capture"));
            std::fs::create_dir_all(&c).unwrap();
            std::fs::write(c.join(FRAMES_FILE), vec![0u8; size]).unwrap();
        }
        std::fs::write(dir.path().join("index.json"), "[]").unwrap();
        let found = captures(dir.path());
        assert_eq!(
            found.iter().map(|k| k.bytes).collect::<Vec<_>>(),
            vec![10, 30]
        );
    }
}
