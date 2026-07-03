//! Direct-friend commands — thin authed wrappers over `/api/friends`. The
//! 1:1 base primitive; groups are the 1:M extension. Content-free (see the
//! server's `friends.rs`).

use tauri::AppHandle;

use crate::error::AppError;
use crate::groups::api;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct FriendUser {
    pub user_id: String,
    pub username: String,
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn list_friends(app: AppHandle) -> Result<Vec<FriendUser>, AppError> {
    api(&app, reqwest::Method::GET, "/api/friends", None).await
}

/// Mint a friend code (7-day expiry, multi-use).
#[tauri::command]
#[specta::specta]
pub(crate) async fn create_friend_invite(app: AppHandle) -> Result<String, AppError> {
    #[derive(serde::Deserialize)]
    struct InviteResponse {
        code: String,
    }
    let response: InviteResponse =
        api(&app, reqwest::Method::POST, "/api/friends/invites", None).await?;
    Ok(response.code)
}

/// Redeem a friend code → mutual friendship. Returns the updated list.
#[tauri::command]
#[specta::specta]
pub(crate) async fn add_friend(app: AppHandle, code: String) -> Result<Vec<FriendUser>, AppError> {
    api(
        &app,
        reqwest::Method::POST,
        "/api/friends/add",
        Some(serde_json::json!({ "code": code })),
    )
    .await
}

/// Drop a friendship (both directions). Returns the updated list.
#[tauri::command]
#[specta::specta]
pub(crate) async fn remove_friend(
    app: AppHandle,
    user_id: String,
) -> Result<Vec<FriendUser>, AppError> {
    api(
        &app,
        reqwest::Method::POST,
        "/api/friends/remove",
        Some(serde_json::json!({ "user_id": user_id })),
    )
    .await
}
