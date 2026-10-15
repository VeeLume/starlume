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

fn game_running_blocking() -> bool {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
    let mut sys = System::new();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    sys.processes().values().any(|p| {
        let name = p.name();
        name.eq_ignore_ascii_case("StarCitizen.exe")
            || name.eq_ignore_ascii_case("StarCitizen_Launcher.exe")
    })
}

/// Whether the launcher is installed and the game runs. Local only: a
/// file check and a process scan, no network.
#[tauri::command]
#[specta::specta]
pub(crate) async fn launcher_status() -> LauncherStatusView {
    LauncherStatusView {
        launcher_found: launcher_exe().is_some(),
        game_running: tokio::task::spawn_blocking(game_running_blocking)
            .await
            .unwrap_or(false),
    }
}

/// Start the RSI launcher, or bring the running one forward (it is
/// single-instance). No arguments: it ignores them. Local, no network gate —
/// the launcher is the user's own app and does its own sign-in.
#[tauri::command]
#[specta::specta]
pub(crate) fn launcher_open() -> Result<(), AppError> {
    let exe = launcher_exe()
        .ok_or_else(|| AppError::Internal("the RSI launcher is not installed".into()))?;
    let mut cmd = std::process::Command::new(&exe);
    if let Some(dir) = exe.parent() {
        cmd.current_dir(dir);
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
