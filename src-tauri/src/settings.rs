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
    /// Run at login (delegated to the autostart plugin on update).
    pub autostart: bool,
    /// Starlume server base URL. `None` → all online features disabled.
    /// A default ships once the server exists.
    pub server_url: Option<String>,
    /// Feature modules the user enabled in onboarding/settings. Module ids
    /// not in this list stay invisible in the UI.
    pub enabled_modules: Vec<String>,
    /// Master online switch (default ON). Off → the app makes **no** network
    /// calls at all: no Discord auth, no RSI fetch, no update check, no gRPC.
    /// Enforced via [`AppState::require_online`](crate::AppState).
    pub online_enabled: bool,
    /// Master switch for CIG game-services (gRPC) calls — the ToS-grey
    /// surface (default OFF, opt-in). Preserved but inert while
    /// `online_enabled` is off. Enforced via `AppState::require_grpc`.
    pub grpc_enabled: bool,
    /// One-time ToS acknowledgement — set automatically the first time
    /// `grpc_enabled` flips on (the UI shows the consent dialog before that).
    pub grpc_consented: bool,
    /// Per-sub-feature allow-list for gRPC calls (ids from
    /// [`GRPC_FEATURES`]), so users can enable e.g. blueprints but not
    /// missions. A feature is live only when `online_enabled` +
    /// `grpc_enabled` + membership here all hold.
    pub grpc_features: Vec<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            onboarding_completed: false,
            start_minimized: false,
            close_to_tray: true,
            autostart: false,
            server_url: None,
            enabled_modules: Vec::new(),
            online_enabled: true,
            grpc_enabled: false,
            grpc_consented: false,
            grpc_features: Vec::new(),
        }
    }
}

/// A gRPC sub-feature users can individually allow. The registry grows as
/// dossier-backed features land (blueprints, missions, inventory,
/// reputation, entitlements/ships…) — each new gRPC call site names its
/// feature id here and gates through `AppState::require_grpc(id)`.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct GrpcFeatureInfo {
    pub id: String,
    pub name: String,
    pub description: String,
}

/// Compiled-in gRPC sub-features. Empty until the first dossier integration.
pub(crate) const GRPC_FEATURES: &[(&str, &str, &str)] = &[
    // ("blueprints", "Owned blueprints", "Read your owned-blueprint set from CIG's backend."),
];

/// List the gRPC sub-features this build knows — drives the Settings toggles.
#[tauri::command]
#[specta::specta]
pub(crate) fn list_grpc_features() -> Vec<GrpcFeatureInfo> {
    GRPC_FEATURES
        .iter()
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
    let (old_autostart, old_grpc) = {
        let old = state.settings.lock().unwrap();
        (old.autostart, old.grpc_enabled)
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
    Ok(settings)
}
