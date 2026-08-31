//! Account-data (gRPC) IPC surface — the shell side of `svc-dossier`.
//!
//! The online-policy gate lives here: [`blueprints_refresh`] passes
//! `require_grpc("blueprints")` before any network read; [`blueprints_owned`]
//! is the ungated cached read (local file only). This is the accounts-model
//! "whose data" concern (see `sc.rs`) applied to gRPC-sourced data —
//! app-level framework for now; the ownership set migrates into the tracker
//! module when it lands.

use tauri::{AppHandle, Manager};

use crate::AppState;
use crate::error::AppError;

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
    // Gate + clone the Arc inside a block so the `State` borrow is released
    // before the await (no borrow of `app` held across the network call).
    let dossier = {
        let state = app.state::<AppState>();
        state.require_grpc("blueprints")?;
        state.dossier.clone()
    };
    let owned = dossier.refresh_blueprints(GRPC_USER_AGENT).await?;
    Ok(owned.into())
}
