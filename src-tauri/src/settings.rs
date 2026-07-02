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
        }
    }
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
    settings: AppSettings,
) -> Result<AppSettings, AppError> {
    let old_autostart = state.settings.lock().unwrap().autostart;
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
