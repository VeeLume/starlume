//! Is the game running? The one process check for the whole app: the text
//! patch's write gate, the launcher card and the log watcher all ask here.
//!
//! A process scan (~10 ms, its buffers freed after each), not a watcher.
//! Two ways to ask:
//! - [`game_running`] scans now. For a decision that must not be stale: the
//!   write gate before `global.ini` is touched.
//! - [`game_running_recent`] reuses the last scan when it is young enough.
//!   For everything that asks on a timer or in passing (the telemetry
//!   reader's wait for the game, the log watcher's quiet check, the launcher
//!   card): however many of them are on, they share one scan per period
//!   instead of each running its own.

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The last scan's answer, and when it was taken.
static LAST: Mutex<Option<(Instant, bool)>> = Mutex::new(None);

/// Is the game running: `StarCitizen.exe` or its EAC bootstrapper? Blocking
/// (a process scan, ~10 ms): from async code call it via `spawn_blocking`.
///
/// The launcher's shader pre-compile (since 4.10.3) starts the same
/// `StarCitizen.exe` with `-PSOCacheGen` while only the launcher is open.
/// That is not the game: it holds back nothing (a text patch, the launcher
/// page's "running"), so it doesn't count.
pub fn game_running() -> bool {
    let running = scan();
    *LAST.lock().unwrap() = Some((Instant::now(), running));
    running
}

/// [`game_running`], but a scan younger than `max_age` answers instead of a
/// new one. Blocking when it does scan.
pub fn game_running_recent(max_age: Duration) -> bool {
    if let Some(running) = fresh(*LAST.lock().unwrap(), Instant::now(), max_age) {
        return running;
    }
    game_running()
}

/// The remembered answer, if it is younger than `max_age`.
fn fresh(last: Option<(Instant, bool)>, now: Instant, max_age: Duration) -> Option<bool> {
    let (at, running) = last?;
    (now.saturating_duration_since(at) < max_age).then_some(running)
}

fn scan() -> bool {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_cmd(UpdateKind::OnlyIfNotSet),
    );
    sys.processes().values().any(|p| {
        let name = p.name();
        (name.eq_ignore_ascii_case("StarCitizen.exe")
            || name.eq_ignore_ascii_case("StarCitizen_Launcher.exe"))
            && !is_shader_cache_build(p.cmd())
    })
}

/// The launcher's shader pre-compile: the game binary with `-PSOCacheGen`.
fn is_shader_cache_build(cmd: &[std::ffi::OsString]) -> bool {
    cmd.iter()
        .skip(1)
        .any(|a| a.to_string_lossy().eq_ignore_ascii_case("-PSOCacheGen"))
}

#[cfg(test)]
mod tests {
    use super::{fresh, is_shader_cache_build};
    use std::ffi::OsString;
    use std::time::{Duration, Instant};

    #[test]
    fn a_young_scan_answers_and_an_old_one_does_not() {
        let t0 = Instant::now();
        let age = Duration::from_secs(5);
        assert_eq!(fresh(None, t0, age), None);
        assert_eq!(
            fresh(Some((t0, true)), t0 + Duration::from_secs(4), age),
            Some(true)
        );
        assert_eq!(
            fresh(Some((t0, false)), t0 + Duration::from_secs(4), age),
            Some(false)
        );
        assert_eq!(
            fresh(Some((t0, true)), t0 + Duration::from_secs(5), age),
            None
        );
    }

    fn cmd(args: &[&str]) -> Vec<OsString> {
        args.iter().map(OsString::from).collect()
    }

    #[test]
    fn the_shader_pre_compile_is_not_the_game() {
        let exe = r"C:\Games\StarCitizen\LIVE\Bin64\StarCitizen.exe";
        assert!(is_shader_cache_build(&cmd(&[exe, "-PSOCacheGen"])));
        assert!(is_shader_cache_build(&cmd(&[exe, "-psocachegen"])));
        assert!(!is_shader_cache_build(&cmd(&[exe])));
        assert!(!is_shader_cache_build(&cmd(&[exe, "-no_login_dialog"])));
        // An unreadable command line (another user's process) is the game.
        assert!(!is_shader_cache_build(&[]));
    }
}
