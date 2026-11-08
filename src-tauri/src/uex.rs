//! UEX prices — the shell side of `svc-uex`: the refresh, its gate, the
//! IPC status, and the change event. App framework, like the patch notes:
//! public reads of a third-party API (UEX Corp, not RSI/CIG), so the gate
//! is `require_online()`, not the unofficial-API opt-in. The cached reads
//! (the ships commands' prices) touch no network.
//!
//! **Refresh cadence.** After a startup delay, then hourly: each list is
//! fetched again only once the `max-age` UEX sent with it has run out
//! (12 h; the marketplace 1 h), and on a settings change (the online
//! switch, the Ships module) or Refresh. Nothing is fetched without Ships.

use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Notify;

use crate::AppState;
use crate::activity::{Activity, ActivityKind};
use crate::error::AppError;

/// Fired after a refresh fetched anything. The ships store reloads prices.
pub const UEX_CHANGED_EVENT: &str = "uex:changed";
/// How often the refresher wakes to see what is due.
const CHECK_EVERY: Duration = Duration::from_secs(60 * 60);
/// Delay before the first check — startup has heavier work first.
const STARTUP_DELAY: Duration = Duration::from_secs(45);
/// An honest user agent, as for RSI's public pages.
const USER_AGENT: &str = concat!("starlume/", env!("CARGO_PKG_VERSION"));

static WAKE: LazyLock<Notify> = LazyLock::new(Notify::new);
/// The last refresh's error, for the page. Session memory.
static LAST_ERROR: Mutex<Option<String>> = Mutex::new(None);

/// Where UEX's prices stand.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UexStatusView {
    /// When the shop prices were last fetched, Unix seconds; null: never.
    pub fetched_at: Option<f64>,
    /// Online mode is on (refreshes can run).
    pub online: bool,
    /// The last refresh's error, if it failed.
    pub error: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub(crate) fn uex_status(state: tauri::State<'_, AppState>) -> UexStatusView {
    UexStatusView {
        fetched_at: state.uex.fetched_at().map(|t| t as f64),
        online: state.require_online().is_ok(),
        error: LAST_ERROR.lock().unwrap().clone(),
    }
}

/// Fetch whatever is due now. **Gated** by `require_online()`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn uex_refresh(app: AppHandle) -> Result<(), AppError> {
    refresh(&app).await
}

/// Wake the refresher (the online switch may have changed).
pub(crate) fn nudge() {
    WAKE.notify_one();
}

/// One refresh of the due lists. **Gated.**
async fn refresh(app: &AppHandle) -> Result<(), AppError> {
    let service = {
        let state = app.state::<AppState>();
        state.require_online()?;
        state.uex.clone()
    };
    let now = chrono::Utc::now().timestamp();
    let first = service.fetched_at().is_none();
    let result = {
        let _activity = first.then(|| {
            Activity::start(app, ActivityKind::Uex, None, "Prices")
                .stage("Reading UEX's price lists…")
        });
        service.refresh(USER_AGENT, now).await
    };
    match result {
        Ok(report) => {
            *LAST_ERROR.lock().unwrap() = None;
            if !report.fetched.is_empty() {
                tracing::info!("UEX: fetched {}", report.fetched.join(", "));
                let _ = app.emit(UEX_CHANGED_EVENT, ());
            }
            Ok(())
        }
        Err(e) => {
            let msg = format!("couldn't read UEX's prices: {e}");
            *LAST_ERROR.lock().unwrap() = Some(msg.clone());
            // Some lists may have landed before the error.
            let _ = app.emit(UEX_CHANGED_EVENT, ());
            Err(AppError::Internal(msg))
        }
    }
}

/// Only Ships shows these prices, so someone without it fetches none
/// (module rule 2: lightweight for someone who wants one module).
fn module_enabled(state: &AppState) -> bool {
    state
        .settings
        .lock()
        .unwrap()
        .enabled_modules
        .iter()
        .any(|m| m == crate::ships::MODULE_ID)
}

/// The background refresher. Silent when offline or without Ships — that is
/// a choice, not an error. Turning either on wakes it (`nudge`).
pub fn spawn_refresher(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(STARTUP_DELAY).await;
        loop {
            let state = app.state::<AppState>();
            let due = module_enabled(&state)
                && state.require_online().is_ok()
                && state.uex.stale(chrono::Utc::now().timestamp());
            if due && let Err(e) = refresh(&app).await {
                tracing::info!("UEX refresh failed: {e}");
            }
            tokio::select! {
                _ = tokio::time::sleep(CHECK_EVERY) => {}
                _ = WAKE.notified() => {}
            }
        }
    });
}
