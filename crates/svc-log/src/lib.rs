//! Log service — the ONE `Game.log` reader for the whole app.
//!
//! Carved from Hearth's `sensors` (the tailer and its blueprint / session
//! parsers) and widened to the events the session log, the star-map trail,
//! Discord presence and the refinery timer need.
//!
//! - [`event`] — the typed events, each with its line's timestamp.
//! - [`parse`] — pure, per-record recognisers, unit-tested against verbatim
//!   lines from real logs. The format-fragile part, kept small so a log
//!   format break is a local fix.
//! - [`tailer`] — the I/O layer: the incremental [`LogTailer`] (survives the
//!   log being replaced on a game restart) and [`read_log`] for a whole
//!   file.
//! - [`watch`] — the watcher thread: follows whichever install's log the
//!   game is writing, keeps the current session's events, costs a `stat`
//!   while the game is closed.
//!
//! No Tauri, no tokio, no network: reading a local file needs no online
//! gate. The shell owns the thread's lifetime and forwards its signals onto
//! the bus.
//!
//! **Locale.** Six events are read from HUD notifications by their English
//! wording and do not fire with a language pack that translates them
//! (contracts, objectives, money, refinery, blueprints, jurisdictions): see
//! [`event`]. The engine-side events are unaffected.
//!
//! Logs contain other players' handles (party invites, channels). Nothing
//! here turns those into events, and nothing leaves the machine.

pub mod event;
pub mod parse;
pub mod tailer;
pub mod watch;

use std::path::{Path, PathBuf};

pub use event::{ContractChange, LogEvent, LogEventKind, MoneyChange, ObjectiveChange, TradeSide};
pub use parse::parse_record;
pub use tailer::{LogTailer, Poll, RecordJoiner, read_log};
pub use watch::{LogWatch, SessionSnapshot, WatchConfig, WatchSignal};

/// The `Game.log` inside an install's channel directory
/// (`…/StarCitizen/LIVE/Game.log`): the running or last session.
pub fn game_log_path(channel_dir: &Path) -> PathBuf {
    channel_dir.join("Game.log")
}

/// The directory of finished sessions (`…/StarCitizen/LIVE/logbackups/`).
/// The game moves `Game.log` there when it starts.
pub fn log_backups_dir(channel_dir: &Path) -> PathBuf {
    channel_dir.join("logbackups")
}

/// The finished session logs of a channel, oldest first (by modification
/// time). Each is one whole session: read it with [`read_log`]. They never
/// change once written, so a caller that scans many (hundreds, gigabytes)
/// should cache per file by size + mtime, as Hearth's import did.
pub fn backup_logs(channel_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(log_backups_dir(channel_dir)) else {
        return Vec::new();
    };
    let mut logs: Vec<(Option<std::time::SystemTime>, PathBuf)> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|e| e.to_str()) == Some("log"))
        .map(|path| {
            let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
            (modified, path)
        })
        .collect();
    logs.sort();
    logs.into_iter().map(|(_, path)| path).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_logs_lists_only_logs_oldest_first() {
        let dir = tempfile::tempdir().unwrap();
        let backups = log_backups_dir(dir.path());
        std::fs::create_dir_all(&backups).unwrap();
        let old = backups.join("Game Build(12913470) 11 Nov 26 (22 56 37).log");
        let new = backups.join("Game Build(12913470) 12 Nov 26 (01 07 04).log");
        // Written newest first, so the order asserted is not creation order.
        std::fs::write(&new, "b").unwrap();
        std::fs::write(&old, "a").unwrap();
        let earlier = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
        std::fs::File::options()
            .write(true)
            .open(&old)
            .unwrap()
            .set_modified(earlier)
            .unwrap();
        std::fs::write(backups.join("notes.txt"), "x").unwrap();

        assert_eq!(backup_logs(dir.path()), vec![old, new]);
        assert!(backup_logs(&dir.path().join("missing")).is_empty());
    }
}
