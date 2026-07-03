//! SC framework surface: install scan + RSI account recognition (ported
//! from Hearth's identity flow, trimmed to the framework core).
//!
//! Accounts are app-level, not module-level — every SC module needs "whose
//! data is this". Model (the Hearth insight): the RSI **handle is mutable**
//! (paid rename), so accounts are keyed by a local id and anchored by the
//! two immutable public-profile fields (`citizen_record`, `enlisted`),
//! captured by an explicit, online-gated verify step. Persisted as
//! `accounts.json` via app-kit.
//!
//! Deliberately not ported yet (they land when accounts get real
//! consumers): rename detection / account merge (Hearth `identity/rename`),
//! multi-account log-archive discovery, platform-scoped personal data.

use tauri::{AppHandle, Manager};

use crate::AppState;
use crate::error::AppError;

const USER_AGENT: &str = concat!(
    "starlume/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/VeeLume/starlume)"
);

/// A recognized RSI account. `citizen_record`/`enlisted` are the immutable
/// anchors from the public profile — `None` until verified.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct RsiAccount {
    pub id: String,
    /// Current handle (mutable — display + scrape key, never identity).
    pub handle: String,
    /// UEE Citizen Record number (immutable anchor 1). f64 for the TS
    /// export; values are ~7 digits.
    pub citizen_record: Option<f64>,
    /// Enlisted date, ISO `YYYY-MM-DD` (immutable anchor 2).
    pub enlisted: Option<String>,
    pub primary_org_sid: Option<String>,
    /// Epoch ms of the last successful profile verification.
    pub last_verified: Option<f64>,
}

pub(crate) fn accounts_path() -> std::path::PathBuf {
    app_kit::app_data_root().join("accounts.json")
}

fn save_accounts(accounts: &[RsiAccount]) -> Result<(), AppError> {
    app_kit::save_json(&accounts_path(), &accounts.to_vec())?;
    Ok(())
}

/// Shell mirror of `svc_discovery::InstallInfo` (specta shape).
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct InstallView {
    pub channel: String,
    pub platform: String,
    pub directory: String,
    pub version: String,
    pub build_id: String,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct ScStatus {
    pub installs: Vec<InstallView>,
    /// Handle the RSI launcher is currently signed in as, if any.
    pub launcher_handle: Option<String>,
    /// The known account matching the launcher handle (auto-created
    /// unverified on first sight).
    pub account: Option<RsiAccount>,
}

/// Scan installs + recognize the launcher account. Local reads only — no
/// network, no online gate. First sight of a handle creates an unverified
/// account row (the Hearth bootstrap pattern).
#[tauri::command]
#[specta::specta]
pub(crate) async fn sc_status(app: AppHandle) -> Result<ScStatus, AppError> {
    let scan = tokio::task::spawn_blocking(svc_discovery::scan)
        .await
        .map_err(|e| AppError::Internal(format!("scan task failed: {e}")))?
        .map_err(|e| AppError::Internal(format!("install scan failed: {e:#}")))?;

    let account = match &scan.launcher_handle {
        Some(handle) => {
            let state = app.state::<AppState>();
            let mut accounts = state.accounts.lock().unwrap();
            let existing = accounts
                .iter()
                .find(|a| a.handle.eq_ignore_ascii_case(handle))
                .cloned();
            match existing {
                Some(account) => Some(account),
                None => {
                    let account = RsiAccount {
                        id: uuid::Uuid::now_v7().to_string(),
                        handle: handle.clone(),
                        citizen_record: None,
                        enlisted: None,
                        primary_org_sid: None,
                        last_verified: None,
                    };
                    accounts.push(account.clone());
                    save_accounts(&accounts)?;
                    tracing::info!("recognized new RSI account: {handle}");
                    Some(account)
                }
            }
        }
        None => None,
    };

    Ok(ScStatus {
        installs: scan
            .installs
            .into_iter()
            .map(|i| InstallView {
                channel: i.channel,
                platform: i.platform,
                directory: i.directory,
                version: i.version,
                build_id: i.build_id,
            })
            .collect(),
        launcher_handle: scan.launcher_handle,
        account,
    })
}

/// Verify an account against the RSI public profile: capture the immutable
/// anchors (citizen record + enlisted). Online-gated — the profile scrape is
/// a network call the user controls.
#[tauri::command]
#[specta::specta]
pub(crate) async fn verify_rsi_account(
    app: AppHandle,
    handle: String,
) -> Result<RsiAccount, AppError> {
    app.state::<AppState>().require_online()?;

    let url = format!(
        "https://robertsspaceindustries.com/en/citizens/{}",
        urlencoded(&handle)
    );
    let response = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| AppError::Internal(e.to_string()))?
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("profile fetch failed: {e}")))?;
    if !response.status().is_success() {
        return Err(AppError::Internal(format!(
            "profile page returned {} for {handle}",
            response.status()
        )));
    }
    let body = response
        .text()
        .await
        .map_err(|e| AppError::Internal(format!("profile fetch failed: {e}")))?;
    let info = svc_discovery::profile::parse(&body)
        .map_err(|e| AppError::Internal(format!("profile parse failed: {e}")))?;

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0);

    let state = app.state::<AppState>();
    let mut accounts = state.accounts.lock().unwrap();
    let account = match accounts
        .iter_mut()
        .find(|a| a.handle.eq_ignore_ascii_case(&handle))
    {
        Some(account) => {
            // The profile is the source of truth for handle casing.
            account.handle = info.handle.clone();
            account.citizen_record = Some(info.citizen_record as f64);
            account.enlisted = Some(info.enlisted.clone());
            account.primary_org_sid = info.primary_org_sid.clone();
            account.last_verified = Some(now_ms);
            account.clone()
        }
        None => {
            let account = RsiAccount {
                id: uuid::Uuid::now_v7().to_string(),
                handle: info.handle.clone(),
                citizen_record: Some(info.citizen_record as f64),
                enlisted: Some(info.enlisted.clone()),
                primary_org_sid: info.primary_org_sid.clone(),
                last_verified: Some(now_ms),
            };
            accounts.push(account.clone());
            account
        }
    };
    save_accounts(&accounts)?;
    Ok(account)
}

/// Minimal URL-path-segment encoder (the Hearth port) — RSI handles are
/// `[A-Za-z0-9_-]`, but encode anything outside that to be safe.
fn urlencoded(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~') {
            out.push(c);
        } else {
            let mut buf = [0u8; 4];
            for b in c.encode_utf8(&mut buf).bytes() {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    out
}
