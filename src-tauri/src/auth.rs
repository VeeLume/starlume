//! Device-token auth against the Starlume server.
//!
//! Flow (browser handoff + deep link + one-time-code exchange — see the
//! design doc in the vault and `crates/server/src/main.rs` for the other
//! half):
//! 1. `login_start` generates a nonce, remembers it, and opens the system
//!    browser at `{server_url}/auth/desktop/start?nonce=…`. The server runs
//!    Discord OAuth there.
//! 2. The server redirects to `starlume://auth/callback?nonce=…&code=…`.
//!    [`handle_deep_link`] verifies the nonce, then exchanges the one-time
//!    code at `POST /auth/desktop/exchange` for the device token (so no
//!    long-lived token ever appears in a browser URL). The token goes into
//!    the Windows Credential Manager (`keyring`); `auth-changed` is emitted.
//! 3. `fetch_profile` reads `GET /api/me` with the token — Discord username +
//!    avatar for the sidebar. `svc-sync` (later) uses the same token.

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_opener::OpenerExt;

use crate::AppState;
use crate::error::AppError;
use crate::notify::{Notification, notify};

const KEYRING_SERVICE: &str = "starlume";
const KEYRING_USER: &str = "device-token";

fn token_entry() -> Result<keyring::Entry, AppError> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        .map_err(|e| AppError::Auth(format!("keyring unavailable: {e}")))
}

/// Read the stored device token, if any. Crate-visible so `svc-sync` wiring
/// can reach it through the shell later.
pub(crate) fn stored_token() -> Option<String> {
    token_entry().ok()?.get_password().ok()
}

fn server_url(state: &AppState) -> Option<String> {
    state
        .settings
        .lock()
        .unwrap()
        .server_url
        .clone()
        .map(|s| s.trim_end_matches('/').to_string())
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct AuthStatus {
    /// A device token is present in the credential store.
    pub logged_in: bool,
    /// A server URL is configured — login is possible at all.
    pub server_configured: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct Profile {
    pub username: String,
    pub avatar_url: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub(crate) fn auth_status(state: tauri::State<'_, AppState>) -> AuthStatus {
    AuthStatus {
        logged_in: stored_token().is_some(),
        server_configured: state.settings.lock().unwrap().server_url.is_some(),
    }
}

/// Kick off the browser login. Fails fast when no server is configured.
#[tauri::command]
#[specta::specta]
pub(crate) fn login_start(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), AppError> {
    state.require_online()?;
    let server =
        server_url(&state).ok_or_else(|| AppError::Config("no server URL configured".into()))?;

    let nonce = uuid::Uuid::new_v4().to_string();
    *state.pending_login.lock().unwrap() = Some(nonce.clone());

    let url = format!("{server}/auth/desktop/start?nonce={nonce}");
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| AppError::Internal(format!("failed to open browser: {e}")))?;
    Ok(())
}

/// The signed-in user's Discord profile, from `GET /api/me`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn fetch_profile(app: AppHandle) -> Result<Profile, AppError> {
    app.state::<AppState>().require_online()?;
    let server = server_url(&app.state::<AppState>())
        .ok_or_else(|| AppError::Config("no server URL configured".into()))?;
    let token = stored_token().ok_or_else(|| AppError::Auth("not signed in".into()))?;

    let response = reqwest::Client::new()
        .get(format!("{server}/api/me"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("profile request failed: {e}")))?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        // Token was revoked server-side — drop the local copy so the UI
        // falls back to the signed-out state instead of erroring forever.
        let _ = token_entry().and_then(|e| {
            e.delete_credential()
                .map_err(|e| AppError::Auth(e.to_string()))
        });
        let _ = app.emit("auth-changed", ());
        notify(
            &app,
            Notification::warning("Session expired")
                .with_body("This device was signed out — sign in again.")
                .with_source("auth"),
        );
        return Err(AppError::Auth("session expired — sign in again".into()));
    }
    if !response.status().is_success() {
        return Err(AppError::Internal(format!(
            "profile request returned {}",
            response.status()
        )));
    }
    response
        .json::<Profile>()
        .await
        .map_err(|e| AppError::Internal(format!("bad profile response: {e}")))
}

/// Drop the device token. Purely local — server-side revocation is a
/// `svc-sync` concern once token management lands there.
#[tauri::command]
#[specta::specta]
pub(crate) fn logout(app: AppHandle) -> Result<(), AppError> {
    match token_entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => {}
        Err(e) => return Err(AppError::Auth(format!("failed to delete token: {e}"))),
    }
    let _ = app.emit("auth-changed", ());
    Ok(())
}

/// Handle an incoming `starlume://` URL. Called from the deep-link plugin's
/// `on_open_url` (which also receives URLs forwarded by second instances via
/// the single-instance plugin).
pub(crate) fn handle_deep_link(app: &AppHandle, url: &url::Url) {
    tracing::info!("deep link: {url}");
    match url.host_str() {
        Some("auth") => handle_auth_callback(app, url),
        other => tracing::warn!("unhandled deep-link host: {other:?}"),
    }
}

fn handle_auth_callback(app: &AppHandle, url: &url::Url) {
    let mut nonce = None;
    let mut code = None;
    for (k, v) in url.query_pairs() {
        match k.as_ref() {
            "nonce" => nonce = Some(v.into_owned()),
            "code" => code = Some(v.into_owned()),
            _ => {}
        }
    }
    let (Some(nonce), Some(code)) = (nonce, code) else {
        tracing::warn!("auth callback missing nonce/code");
        return;
    };

    let state = app.state::<AppState>();
    if state.require_online().is_err() {
        // Online was switched off between login_start and the callback.
        tracing::warn!("auth callback while online features are disabled — ignoring");
        return;
    }
    let pending = state.pending_login.lock().unwrap().take();
    if pending.as_deref() != Some(nonce.as_str()) {
        // Stale or unsolicited callback — a login we didn't start this
        // session. Refuse rather than silently adopting a session.
        tracing::warn!("auth callback nonce mismatch — ignoring");
        return;
    }
    let Some(server) = server_url(&state) else {
        tracing::warn!("auth callback with no server configured — ignoring");
        return;
    };

    // The exchange is a network call; don't block the deep-link handler.
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        match exchange_login_code(&server, &code).await {
            Ok((token, profile)) => {
                match token_entry().and_then(|e| {
                    e.set_password(&token)
                        .map_err(|e| AppError::Auth(format!("failed to store token: {e}")))
                }) {
                    Ok(()) => {
                        tracing::info!("device token stored");
                        let _ = app.emit("auth-changed", ());
                        notify(
                            &app,
                            Notification::success("Signed in")
                                .with_body(format!("as {} via Discord", profile.username))
                                .with_source("auth"),
                        );
                    }
                    Err(e) => tracing::error!("auth callback: {e}"),
                }
            }
            Err(e) => {
                tracing::error!("login-code exchange failed: {e}");
                notify(
                    &app,
                    Notification::error("Sign-in failed")
                        .with_body(e.to_string())
                        .with_source("auth"),
                );
            }
        }
    });
}

async fn exchange_login_code(server: &str, code: &str) -> Result<(String, Profile), AppError> {
    #[derive(serde::Deserialize)]
    struct ExchangeResponse {
        token: String,
        profile: Profile,
    }

    let response = reqwest::Client::new()
        .post(format!("{server}/auth/desktop/exchange"))
        .json(&serde_json::json!({ "code": code }))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("exchange request failed: {e}")))?;
    if !response.status().is_success() {
        return Err(AppError::Auth(format!(
            "exchange returned {}",
            response.status()
        )));
    }
    let body = response
        .json::<ExchangeResponse>()
        .await
        .map_err(|e| AppError::Internal(format!("bad exchange response: {e}")))?;
    Ok((body.token, body.profile))
}
