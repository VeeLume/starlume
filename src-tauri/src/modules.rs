//! Feature-module registry.
//!
//! The module rules (see README): modules never touch the filesystem / game /
//! network directly — they consume `svc-*` services and react to bus events;
//! no module-to-module dependencies; the enabled-set is user-controlled
//! (onboarding picks it) and modules not enabled stay invisible in the UI.
//!
//! The registry is empty until the first carve-out (`mod-cargo`, per the
//! migration order). The trait is deliberately minimal — it grows exactly the
//! hooks the first real module needs, no speculative lifecycle API.

use crate::AppState;

/// A Starlume feature module (langpatch, tracker, cargo, fleet, …).
pub trait Module: Send + Sync {
    /// Stable id — used in `AppSettings::enabled_modules` and frontend routes.
    fn id(&self) -> &'static str;
    /// Human-facing name for onboarding / settings.
    fn name(&self) -> &'static str;
    /// One-liner shown in the onboarding module picker.
    fn description(&self) -> &'static str;
}

/// All modules compiled into this build, enabled or not.
pub(crate) fn registry() -> &'static [&'static dyn Module] {
    &[]
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct ModuleInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
}

/// List compiled-in modules with their enabled state — drives the onboarding
/// picker and the settings module list.
#[tauri::command]
#[specta::specta]
pub(crate) fn list_modules(state: tauri::State<'_, AppState>) -> Vec<ModuleInfo> {
    let enabled = state.settings.lock().unwrap().enabled_modules.clone();
    registry()
        .iter()
        .map(|m| ModuleInfo {
            id: m.id().to_string(),
            name: m.name().to_string(),
            description: m.description().to_string(),
            enabled: enabled.iter().any(|e| e == m.id()),
        })
        .collect()
}
