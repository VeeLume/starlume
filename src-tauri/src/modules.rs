//! Feature-module registry.
//!
//! The module rules (see CONTRIBUTING.md): modules never touch the filesystem / game /
//! network directly — they consume `svc-*` services and react to bus events;
//! no module-to-module dependencies; the enabled-set is user-controlled
//! (onboarding picks it) and modules not enabled stay invisible in the UI.
//!
//! The trait is deliberately minimal — it grows exactly the hooks real
//! modules need, no speculative lifecycle API.

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

/// Text patching — the sc-langpatch consolidation (engine in
/// `crates/mod-langpatch`, orchestration in `langpatch.rs`).
struct Langpatch;

impl Module for Langpatch {
    fn id(&self) -> &'static str {
        "langpatch"
    }
    fn name(&self) -> &'static str {
        "Text Patching"
    }
    fn description(&self) -> &'static str {
        "Enrich Star Citizen's in-game text: component grades, illegal-goods markers, weapon stats — kept current automatically after game patches"
    }
}

/// Contract tracker — all contract types, route planning, cargo packing
/// as the hauling view (scope + constraints documented in
/// `crates/mod-contracts`, currently a doc-only stub).
struct Contracts;

impl Module for Contracts {
    fn id(&self) -> &'static str {
        "contracts"
    }
    fn name(&self) -> &'static str {
        "Contract Tracker"
    }
    fn description(&self) -> &'static str {
        "Track active contracts with per-objective progress, plan multi-contract routes — hauling gets cargo packing"
    }
}

/// My Items — the player's items and ships, where they are and how they
/// are fitted (orchestration in `items.rs`, reading svc-dossier).
struct Items;

impl Module for Items {
    fn id(&self) -> &'static str {
        "items"
    }
    fn name(&self) -> &'static str {
        "My Items"
    }
    fn description(&self) -> &'static str {
        "Find any item or ship you own: where it is, what it is fitted to, and your saved loadouts"
    }
}

/// Dev tracker — CIG staff posts from Spectrum, chat included, filtered by
/// content (judgement in `crates/mod-devtracker`, poll in `devtracker.rs`).
struct Devtracker;

impl Module for Devtracker {
    fn id(&self) -> &'static str {
        crate::devtracker::MODULE_ID
    }
    fn name(&self) -> &'static str {
        "Dev Tracker"
    }
    fn description(&self) -> &'static str {
        "CIG staff posts from Spectrum, chat included, filtered down to game news: patches, PTU waves, maintenance, known issues"
    }
}

/// Mining — the calculator: a scanned rock against a loadout (solver in
/// `crates/mod-mining`, IPC in `mining.rs`).
struct Mining;

impl Module for Mining {
    fn id(&self) -> &'static str {
        crate::mining::MODULE_ID
    }
    fn name(&self) -> &'static str {
        "Mining"
    }
    fn description(&self) -> &'static str {
        "Will it crack? Type in a scanned rock and your loadout: breakability, difficulty, the charge bar, and the gadget, modules or support that close the gap"
    }
}

/// All modules offered by this build, enabled or not. Unreleased modules
/// (Contracts, Mining) exist only in debug builds — the Rust half of the frontend's
/// `$lib/unreleased` gate (v0.1.0 scope decision; the Dev Tracker shipped in
/// v0.2.0).
pub(crate) fn registry() -> &'static [&'static dyn Module] {
    if cfg!(debug_assertions) {
        &[&Devtracker, &Langpatch, &Items, &Mining, &Contracts]
    } else {
        &[&Devtracker, &Langpatch, &Items]
    }
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
