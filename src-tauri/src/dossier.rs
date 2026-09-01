//! Account-data (gRPC) IPC surface — the shell side of `svc-dossier`.
//!
//! The online-policy gate lives here: [`blueprints_refresh`] passes
//! `require_grpc("blueprints")` before any network read; [`blueprints_owned`]
//! is the ungated cached read (local file only). This is the accounts-model
//! "whose data" concern (see `sc.rs`) applied to gRPC-sourced data —
//! app-level framework for now; the ownership set migrates into the tracker
//! module when it lands.

use tauri::{AppHandle, Emitter, Manager};

use crate::AppState;
use crate::error::AppError;

/// Fired after a blueprint refresh (startup or manual). The frontend store
/// reloads the owned set on this, so catalog/text decoration re-renders.
pub const BLUEPRINTS_CHANGED_EVENT: &str = "blueprints:changed";

/// gRPC user-agent for backend calls (distinct from the RSI-profile scrape UA
/// in `sc.rs`; both identify Starlume, honestly).
const GRPC_USER_AGENT: &str = concat!("starlume/", env!("CARGO_PKG_VERSION"));

/// The owned-blueprint set as it crosses IPC (specta mirror of
/// `svc_dossier::OwnedBlueprints`).
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct OwnedBlueprintsView {
    /// Owned blueprint record GUIDs (= holotable `blueprint_record_guid`).
    pub blueprint_ids: Vec<String>,
    /// When fetched, epoch **seconds** (`f64` for the TS export). `null` when
    /// nothing has been fetched yet.
    pub fetched_at: Option<f64>,
}

impl From<svc_dossier::OwnedBlueprints> for OwnedBlueprintsView {
    fn from(o: svc_dossier::OwnedBlueprints) -> Self {
        Self {
            blueprint_ids: o.blueprint_ids.into_iter().collect(),
            fetched_at: Some(o.fetched_at as f64),
        }
    }
}

/// The cached owned-blueprint set, or an empty/null one when nothing has been
/// fetched. **No network, no gate** — safe on startup so a decoration renders
/// from the last fetch.
#[tauri::command]
#[specta::specta]
pub(crate) fn blueprints_owned(state: tauri::State<'_, AppState>) -> OwnedBlueprintsView {
    match state.dossier.cached_blueprints() {
        Some(owned) => owned.into(),
        None => OwnedBlueprintsView {
            blueprint_ids: Vec::new(),
            fetched_at: None,
        },
    }
}

/// Fetch the owned-blueprint set live from CIG's backend and cache it.
///
/// **Gated** by `require_grpc("blueprints")` — the master online switch, the
/// gRPC master, and the per-feature allow-list must all be on. A stale/absent
/// launcher session surfaces as an error the UI renders as a sign-in hint.
#[tauri::command]
#[specta::specta]
pub(crate) async fn blueprints_refresh(app: AppHandle) -> Result<OwnedBlueprintsView, AppError> {
    refresh_and_react(&app).await
}

/// Fetch the owned set, and if it changed, react: emit the changed event and
/// (when `blueprints_auto_langpatch` is on) re-apply text patching so the
/// in-game mission text re-renders ownership. Shared by the manual command
/// and the startup automation.
///
/// **Gated:** `require_grpc("blueprints")` — the network read never happens
/// without the consent the user gave in Settings.
async fn refresh_and_react(app: &AppHandle) -> Result<OwnedBlueprintsView, AppError> {
    let (dossier, auto_langpatch) = {
        let state = app.state::<AppState>();
        state.require_grpc("blueprints")?;
        let auto = state.settings.lock().unwrap().blueprints_auto_langpatch;
        (state.dossier.clone(), auto)
    };

    let old = dossier
        .cached_blueprints()
        .map(|o| o.blueprint_ids)
        .unwrap_or_default();
    let owned = dossier.refresh_blueprints(GRPC_USER_AGENT).await?;
    let changed = owned.blueprint_ids != old;

    let _ = app.emit(BLUEPRINTS_CHANGED_EVENT, ());

    // A changed owned set moves mission_enhancer's fingerprint salt, so this
    // re-applies for owned installs (reconcile_all self-gates on the module +
    // auto_patch + all the write-gates).
    if changed && auto_langpatch {
        tracing::info!("owned blueprints changed — re-applying text patching");
        crate::langpatch::reconcile_all(app).await;
    }

    Ok(owned.into())
}

/// Startup automation: when `blueprints_auto_fetch` is on and the gRPC gate
/// permits, fetch the owned set in the background (and react to a change).
/// Silent on the gate/off cases — this is best-effort background work.
pub fn spawn_startup_fetch(app: &AppHandle) {
    let (auto_fetch, gate_ok) = {
        let state = app.state::<AppState>();
        let auto = state.settings.lock().unwrap().blueprints_auto_fetch;
        (auto, state.require_grpc("blueprints").is_ok())
    };
    if !auto_fetch || !gate_ok {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = refresh_and_react(&app).await {
            tracing::info!("startup blueprint fetch skipped: {e}");
        }
    });
}
