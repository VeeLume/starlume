//! App-global preferences. One JSON file (`settings.json` under
//! [`app_kit::app_data_root`]), one snapshot struct, whole-snapshot updates —
//! per-key setters can come back if the surface grows past what one Settings
//! page edits.

use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

use crate::AppState;
use crate::error::AppError;

/// App-global preferences surfaced to the Settings page.
///
/// `#[serde(default)]` keeps old settings files loading after new fields are
/// added — never rename or repurpose a field, add a new one.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(default)]
pub struct AppSettings {
    /// Whether the first-launch onboarding has been completed/skipped.
    pub onboarding_completed: bool,
    /// Launch hidden to tray (companion mode). Also forced by `--minimized`,
    /// which autostart passes.
    pub start_minimized: bool,
    /// Closing the main window hides to tray instead of quitting.
    pub close_to_tray: bool,
    /// Minimizing the main window hides it to tray instead of the taskbar.
    /// Default off — taskbar minimize is the common expectation; tray purists
    /// opt in.
    pub minimize_to_tray: bool,
    /// Run at login (delegated to the autostart plugin on update).
    pub autostart: bool,
    /// Starlume server base URL. `None` → all online features disabled.
    /// A default ships once the server exists.
    pub server_url: Option<String>,
    /// Feature modules the user enabled in onboarding/settings. Module ids
    /// not in this list stay invisible in the UI.
    pub enabled_modules: Vec<String>,
    /// Master online switch (default ON). Off → no Discord auth, no RSI
    /// fetch, no unofficial API — no network calls except **update checks**
    /// (the one documented exception; see CLAUDE.md). Enforced via
    /// [`AppState::require_online`](crate::AppState).
    pub online_enabled: bool,
    /// Master switch for **unofficial API** calls — the ToS-grey surface
    /// (default OFF, opt-in): CIG's game-services gRPC backend and RSI's
    /// Spectrum REST API, any RSI/CIG API Starlume is not an official client
    /// of. Named for gRPC, its first member; the meaning widened when
    /// Spectrum joined (2026-10), the name stays (settings rule: never
    /// rename). Preserved but inert while `online_enabled` is off. Enforced
    /// via `AppState::require_unofficial`.
    pub grpc_enabled: bool,
    /// One-time ToS acknowledgement — set automatically the first time
    /// `grpc_enabled` flips on (the UI shows the consent dialog before that).
    /// Covers every unofficial API, like `grpc_enabled`.
    pub grpc_consented: bool,
    /// Per-feature allow-list for unofficial API calls (ids from
    /// [`GRPC_FEATURES`] — gRPC and Spectrum features alike, despite the
    /// name), so users can enable e.g. blueprints but not the dev tracker. A
    /// feature is live only when `online_enabled` + `grpc_enabled` +
    /// membership here all hold.
    pub grpc_features: Vec<String>,
    /// Show native OS toasts for notifications raised while the window is
    /// hidden/minimized (companion mode). Default ON — a hidden tray app has
    /// no other way to reach the user.
    pub native_notifications: bool,
    /// Warm the game-data cache in the background at app start (scan installs,
    /// cook the newest PU build if its snapshot is missing/stale). Default ON —
    /// catalogs are instantly browsable instead of gated on a manual Load.
    /// Local file reads only, no online implications; the heavy parse runs at
    /// most once per game patch.
    pub auto_load_game_data: bool,
    /// Fetch the owned-blueprint set from CIG's backend at app start (gated
    /// by the `blueprints` gRPC feature — inert unless online + gRPC + that
    /// feature are all on). Default ON: the Hearth-style automation the owned
    /// catalog/text marking rides on.
    #[serde(default = "default_true")]
    pub blueprints_auto_fetch: bool,
    /// When the owned-blueprint set changes, re-apply text patching for
    /// owned installs so the in-game mission text re-renders ownership.
    /// Default ON; honors all the langpatch write-gates.
    #[serde(default = "default_true")]
    pub blueprints_auto_langpatch: bool,
    /// Build archive: how many releases (newest first) keep an archived
    /// game-data build after a patch replaces it. Default 3 — the raw
    /// extract (~35 MB) per kept build, the input for patch diffs.
    pub archive_releases: u32,
    /// Keep every build within a kept release (PTU waves, hotfixes), not
    /// only its newest one. Default off — a power-user option.
    pub archive_all_builds: bool,
    /// Archive PTU / Tech Preview builds too. Default off: Live/Hotfix only.
    pub archive_test_channels: bool,
    /// The Home page layout: the cards the user arranged, in order, each
    /// shown or hidden. Ids are `<owner>:<card>` (`shell:game-status`,
    /// `items:summary`). Empty = the default layout. Cards not listed — a
    /// module enabled later — follow the listed ones at their default
    /// visibility; listed ids no longer available are skipped, not dropped,
    /// so re-enabling a module restores its placement.
    pub home_cards: Vec<HomeCardPref>,
}

/// One card's place on Home (see [`AppSettings::home_cards`]).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct HomeCardPref {
    pub id: String,
    pub visible: bool,
}

fn default_true() -> bool {
    true
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            onboarding_completed: false,
            start_minimized: false,
            close_to_tray: true,
            minimize_to_tray: false,
            autostart: false,
            server_url: None,
            enabled_modules: Vec::new(),
            online_enabled: true,
            grpc_enabled: false,
            grpc_consented: false,
            grpc_features: Vec::new(),
            native_notifications: true,
            auto_load_game_data: true,
            blueprints_auto_fetch: true,
            blueprints_auto_langpatch: true,
            archive_releases: 3,
            archive_all_builds: false,
            archive_test_channels: false,
            home_cards: Vec::new(),
        }
    }
}

/// An unofficial-API feature users can individually allow (named for gRPC,
/// its first member; Spectrum features are listed too). The registry grows
/// as features land (blueprints, missions, inventory, reputation, the dev
/// tracker…) — each new unofficial-API call site names its feature id here
/// and gates through `AppState::require_unofficial(id)`.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct GrpcFeatureInfo {
    pub id: String,
    pub name: String,
    pub description: String,
}

/// Compiled-in unofficial-API features: `(id, name, description)`. The
/// name is historical — Spectrum features belong here as much as gRPC ones.
pub(crate) const GRPC_FEATURES: &[(&str, &str, &str)] = &[
    (
        "blueprints",
        "Owned blueprints",
        "Read your owned-blueprint set from CIG's backend, to mark owned rewards in mission text.",
    ),
    (
        "items",
        "Items & ships",
        "Read where your items and ships are, what is fitted to what, and who owns it, for My Items.",
    ),
    (
        "loadouts",
        "Saved loadouts",
        "Read your ships' stock and saved loadouts, to show them beside the fitted state in My Items.",
    ),
];

/// Unofficial-API features whose consumer hasn't shipped: listed in debug
/// builds only, so release users never see a toggle that does nothing — the
/// Rust half of `$lib/unreleased` for features, like the debug-only half of
/// `modules::registry()`. Move an entry into [`GRPC_FEATURES`] when its
/// consumer ships.
const UNRELEASED_GRPC_FEATURES: &[(&str, &str, &str)] = &[
    (
        "devtracker",
        "Dev tracker",
        "Read the CIG staff posts feed from Spectrum (RSI's forums and chat) with your launcher session, for the dev tracker.",
    ),
    (
        "launcher",
        "Launcher library",
        "Read which game channels your account can play and their current builds from the RSI launcher's API, so Home can tell when an install needs an update.",
    ),
];

/// The unofficial-API features this build offers.
fn grpc_features() -> impl Iterator<Item = &'static (&'static str, &'static str, &'static str)> {
    let unreleased: &[_] = if cfg!(debug_assertions) {
        UNRELEASED_GRPC_FEATURES
    } else {
        &[]
    };
    GRPC_FEATURES.iter().chain(unreleased)
}

/// List the unofficial-API features this build knows — drives the Settings
/// toggles.
#[tauri::command]
#[specta::specta]
pub(crate) fn list_grpc_features() -> Vec<GrpcFeatureInfo> {
    grpc_features()
        .map(|(id, name, description)| GrpcFeatureInfo {
            id: (*id).into(),
            name: (*name).into(),
            description: (*description).into(),
        })
        .collect()
}

impl AppSettings {
    fn path() -> std::path::PathBuf {
        app_kit::app_data_root().join("settings.json")
    }

    pub fn load() -> Self {
        app_kit::load_json(&Self::path())
    }

    pub fn save(&self) -> Result<(), AppError> {
        app_kit::save_json(&Self::path(), self)?;
        Ok(())
    }
}

#[tauri::command]
#[specta::specta]
pub(crate) fn get_settings(state: tauri::State<'_, AppState>) -> AppSettings {
    state.settings.lock().unwrap().clone()
}

/// Replace the settings snapshot: persist to disk, sync side effects
/// (autostart registration), swap the in-memory copy. Returns the new
/// snapshot so the frontend stays consistent without a second call.
#[tauri::command]
#[specta::specta]
pub(crate) fn update_settings(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    mut settings: AppSettings,
) -> Result<AppSettings, AppError> {
    let (old_autostart, old_grpc, old_retention) = {
        let old = state.settings.lock().unwrap();
        (old.autostart, old.grpc_enabled, retention(&old))
    };
    // First enable of the gRPC master records the one-time ToS consent (the
    // UI shows the consent dialog before submitting) — the Hearth pattern.
    if settings.grpc_enabled && !old_grpc {
        settings.grpc_consented = true;
    }
    if settings.autostart != old_autostart {
        let manager = app.autolaunch();
        let result = if settings.autostart {
            manager.enable()
        } else {
            manager.disable()
        };
        if let Err(e) = result {
            return Err(AppError::Internal(format!("autostart change failed: {e}")));
        }
    }
    settings.save()?;
    *state.settings.lock().unwrap() = settings.clone();
    // The change may have opened the dev tracker's gate or enabled it.
    crate::devtracker::nudge();
    crate::patchnotes::nudge();
    crate::commlinks::nudge();
    // A tighter archive retention applies right away; a looser one only
    // keeps future builds (pruned ones are gone).
    if retention(&settings) != old_retention {
        let app = app.clone();
        tauri::async_runtime::spawn(async move { crate::data::prune_archive(&app).await });
    }
    Ok(settings)
}

fn retention(s: &AppSettings) -> (u32, bool, bool) {
    (
        s.archive_releases,
        s.archive_all_builds,
        s.archive_test_channels,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unreleased_features_are_listed_in_debug_builds_only() {
        let ids: Vec<String> = list_grpc_features().into_iter().map(|f| f.id).collect();
        assert!(ids.iter().any(|id| id == "blueprints"));
        assert_eq!(
            ids.iter().any(|id| id == "devtracker"),
            cfg!(debug_assertions)
        );
        // One id, one toggle.
        let mut unique = ids.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), ids.len());
    }
}
