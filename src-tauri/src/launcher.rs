//! The RSI launcher, from Home's Game status card: open it, tell whether the
//! game runs, and whether an install needs an update first.
//!
//! **No Play button.** The launcher (2.18) has no way for another app to
//! press Play — no URL scheme outside dev builds, no arguments, no local
//! endpoint — and replicating Play would mean writing the RSI session to
//! disk ourselves. Decided with Valerie 2026-10-15 (sc-grpc FINDINGS
//! "RSI Launcher → Starting the game from outside"): Starlume starts or
//! focuses the launcher (single-instance, so a second start just brings it
//! forward), and the user presses Play there.
//!
//! **Telemetry.** With the telemetry opt-in on, the launcher is started with
//! `GRPC_CLIENT_SERVER_ENABLED=true` in its environment; the game inherits
//! it and serves its local telemetry stream (`telemetry.rs`). A launcher
//! that already runs keeps the environment it was started with: it has to
//! be quit first, or the variable installed for the user.
//!
//! **Update needed.** `sc_dossier::auth::library()` lists the channels this
//! account may play with their current builds — an unofficial API, so it
//! sits behind `require_unofficial("launcher")`. Cached in memory for
//! [`LIBRARY_MAX_AGE`] (a few strings); a stale launcher session only drops
//! the hint, it never fails the card.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::State;

use crate::AppState;
use crate::error::AppError;

/// The unofficial-API feature id (settings `GRPC_FEATURES`).
pub(crate) const FEATURE_ID: &str = "launcher";
/// Re-read the library when the cached one is older than this.
const LIBRARY_MAX_AGE: Duration = Duration::from_secs(60 * 60);

/// One channel the account may play, as the launcher would install it.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct LibraryChannelView {
    /// `"LIVE"`, `"PTU"`, ….
    pub id: String,
    /// `"4.10.2-live.12913470"` — compare with the install's version label.
    pub version_label: String,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct LauncherStatusView {
    /// The RSI launcher is installed where Starlume looks for it.
    pub launcher_found: bool,
    /// `StarCitizen.exe` (or its EAC bootstrapper) is running.
    pub game_running: bool,
}

static LIBRARY: Mutex<Option<(Instant, Vec<LibraryChannelView>)>> = Mutex::new(None);

/// The launcher executable, if installed in a Program Files folder.
fn launcher_exe() -> Option<PathBuf> {
    ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"]
        .iter()
        .filter_map(std::env::var_os)
        .map(|dir| {
            PathBuf::from(dir)
                .join("Roberts Space Industries")
                .join("RSI Launcher")
                .join("RSI Launcher.exe")
        })
        .find(|p| p.is_file())
}

/// What Starlume adds to the launcher's environment: with the telemetry
/// opt-in on, the variable the game needs to serve its stream; else nothing
/// (the launcher inherits Starlume's environment as it is).
fn launcher_env(telemetry: bool) -> Vec<(&'static str, &'static str)> {
    if telemetry {
        vec![(svc_telemetry::enable::VAR, svc_telemetry::enable::VALUE)]
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::launcher_env;

    #[test]
    fn the_launcher_gets_the_variable_only_with_the_opt_in() {
        assert!(launcher_env(false).is_empty());
        assert_eq!(launcher_env(true), [("GRPC_CLIENT_SERVER_ENABLED", "true")]);
    }
}

/// Whether the launcher is installed and the game runs. Local only: a
/// file check and a process scan, no network.
#[tauri::command]
#[specta::specta]
pub(crate) async fn launcher_status() -> LauncherStatusView {
    LauncherStatusView {
        launcher_found: launcher_exe().is_some(),
        game_running: tokio::task::spawn_blocking(|| {
            svc_discovery::process::game_running_recent(std::time::Duration::from_secs(2))
        })
        .await
        .unwrap_or(false),
    }
}

/// Start the RSI launcher, or bring the running one forward (it is
/// single-instance). No arguments: it ignores them. Local, no network gate —
/// the launcher is the user's own app and does its own sign-in. With the
/// telemetry opt-in on, it starts with the variable that makes the game
/// serve its telemetry stream.
#[tauri::command]
#[specta::specta]
pub(crate) fn launcher_open(state: State<'_, AppState>) -> Result<(), AppError> {
    let exe = launcher_exe()
        .ok_or_else(|| AppError::Internal("the RSI launcher is not installed".into()))?;
    let telemetry = state.settings.lock().unwrap().telemetry_enabled;
    let mut cmd = std::process::Command::new(&exe);
    if let Some(dir) = exe.parent() {
        cmd.current_dir(dir);
    }
    for (name, value) in launcher_env(telemetry) {
        cmd.env(name, value);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Not a child of Starlume's console or job: closing Starlume must not
        // take the launcher with it.
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        cmd.creation_flags(DETACHED_PROCESS);
    }
    cmd.spawn()
        .map(|_| ())
        .map_err(|e| AppError::Internal(format!("could not start the RSI launcher: {e}")))
}

/// The channels this account may play, with their current builds — the
/// launcher's game library. Cached for [`LIBRARY_MAX_AGE`]. **Gated** by
/// `require_unofficial("launcher")`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn launcher_library(
    state: State<'_, AppState>,
) -> Result<Vec<LibraryChannelView>, AppError> {
    if let Some((at, channels)) = LIBRARY.lock().unwrap().as_ref()
        && at.elapsed() < LIBRARY_MAX_AGE
    {
        return Ok(channels.clone());
    }
    state.require_unofficial(FEATURE_ID)?;
    let games = svc_dossier::launcher::library()
        .await
        .map_err(|e| AppError::Internal(format!("the launcher's game library: {e}")))?;
    let channels: Vec<LibraryChannelView> = games
        .into_iter()
        .filter(|g| g.id == "SC")
        .flat_map(|g| g.channels)
        .map(|c| LibraryChannelView {
            id: c.id,
            version_label: c.version_label,
        })
        .collect();
    *LIBRARY.lock().unwrap() = Some((Instant::now(), channels.clone()));
    Ok(channels)
}
