//! Friend-group commands — thin authed wrappers over the server's
//! `/api/groups` endpoints. Content-free by design (see the server's
//! `groups.rs` and the design doc): groups are visibility containers,
//! features attach to them later by id.

use tauri::{AppHandle, Manager};

use crate::error::AppError;
use crate::{AppState, auth};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct GroupMember {
    pub username: String,
    pub is_owner: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FriendGroup {
    pub id: String,
    pub name: String,
    pub is_owner: bool,
    pub members: Vec<GroupMember>,
}

/// Authenticated JSON request against the configured server. The one place
/// desktop group calls touch the network — gated by `require_online`.
async fn api<T: serde::de::DeserializeOwned>(
    app: &AppHandle,
    method: reqwest::Method,
    path: &str,
    body: Option<serde_json::Value>,
) -> Result<T, AppError> {
    let state = app.state::<AppState>();
    state.require_online()?;
    let server = state
        .settings
        .lock()
        .unwrap()
        .server_url
        .clone()
        .map(|s| s.trim_end_matches('/').to_string())
        .ok_or_else(|| AppError::Config("no server URL configured".into()))?;
    let token = auth::stored_token().ok_or_else(|| AppError::Auth("not signed in".into()))?;

    let mut request = reqwest::Client::new()
        .request(method, format!("{server}{path}"))
        .bearer_auth(token);
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("request failed: {e}")))?;

    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(AppError::Auth("session expired — sign in again".into()));
    }
    if !status.is_success() {
        let detail = response.text().await.unwrap_or_default();
        return Err(AppError::Internal(format!(
            "server returned {status}: {detail}"
        )));
    }
    response
        .json::<T>()
        .await
        .map_err(|e| AppError::Internal(format!("bad response: {e}")))
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn list_groups(app: AppHandle) -> Result<Vec<FriendGroup>, AppError> {
    api(&app, reqwest::Method::GET, "/api/groups", None).await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn create_group(app: AppHandle, name: String) -> Result<FriendGroup, AppError> {
    api(
        &app,
        reqwest::Method::POST,
        "/api/groups",
        Some(serde_json::json!({ "name": name })),
    )
    .await
}

/// Mint an invite code for a group (member-only; 7-day expiry server-side).
#[tauri::command]
#[specta::specta]
pub(crate) async fn create_invite(app: AppHandle, group_id: String) -> Result<String, AppError> {
    #[derive(serde::Deserialize)]
    struct InviteResponse {
        code: String,
    }
    let response: InviteResponse = api(
        &app,
        reqwest::Method::POST,
        &format!("/api/groups/{group_id}/invites"),
        None,
    )
    .await?;
    Ok(response.code)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn join_group(app: AppHandle, code: String) -> Result<FriendGroup, AppError> {
    api(
        &app,
        reqwest::Method::POST,
        "/api/groups/join",
        Some(serde_json::json!({ "code": code })),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn leave_group(app: AppHandle, group_id: String) -> Result<(), AppError> {
    api::<()>(
        &app,
        reqwest::Method::POST,
        &format!("/api/groups/{group_id}/leave"),
        None,
    )
    .await
}
