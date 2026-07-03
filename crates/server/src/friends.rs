//! Direct friends — the 1:1 base primitive ([`crate::groups`] is the 1:M
//! extension on top). Content-free like groups: a friendship is a mutual
//! edge between two users; features reference friend user-ids as visibility
//! scopes later.
//!
//! Alpha semantics:
//! - a user mints a **friend code** (7-day expiry, multi-use — Steam-code
//!   ergonomics); anyone redeeming it becomes a mutual friend;
//! - redeeming your own code is rejected; re-adding an existing friend is
//!   idempotent;
//! - either side can remove the friendship (both directions deleted).

use std::sync::Arc;

use axum::extract::State;
use axum::{Json, http::HeaderMap};

use crate::{AppCtx, AppError, authenticate, random_token};

const INVITE_TTL_DAYS: i64 = 7;

#[derive(serde::Serialize)]
pub struct FriendUser {
    pub user_id: String,
    pub username: String,
}

async fn load_friends(db: &sqlx::SqlitePool, user_id: &str) -> Result<Vec<FriendUser>, AppError> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT u.id, u.username FROM friendships f
         JOIN users u ON u.id = f.friend_id
         WHERE f.user_id = ? ORDER BY u.username COLLATE NOCASE",
    )
    .bind(user_id)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(user_id, username)| FriendUser { user_id, username })
        .collect())
}

/// GET /api/friends — the caller's friends.
pub async fn list_friends(
    State(ctx): State<Arc<AppCtx>>,
    headers: HeaderMap,
) -> Result<Json<Vec<FriendUser>>, AppError> {
    let user_id = authenticate(&ctx, &headers).await?;
    Ok(Json(load_friends(&ctx.db, &user_id).await?))
}

#[derive(serde::Serialize)]
pub struct InviteResponse {
    pub code: String,
}

/// POST /api/friends/invites — mint a friend code.
pub async fn create_invite(
    State(ctx): State<Arc<AppCtx>>,
    headers: HeaderMap,
) -> Result<Json<InviteResponse>, AppError> {
    let user_id = authenticate(&ctx, &headers).await?;
    let code = random_token()[..10].to_uppercase();
    sqlx::query(
        "INSERT INTO friend_invites (code, user_id, expires_at)
         VALUES (?, ?, datetime('now', ?))",
    )
    .bind(&code)
    .bind(&user_id)
    .bind(format!("+{INVITE_TTL_DAYS} days"))
    .execute(&ctx.db)
    .await?;
    Ok(Json(InviteResponse { code }))
}

#[derive(serde::Deserialize)]
pub struct AddBody {
    pub code: String,
}

/// POST /api/friends/add — redeem a friend code → mutual friendship.
pub async fn add_friend(
    State(ctx): State<Arc<AppCtx>>,
    headers: HeaderMap,
    Json(body): Json<AddBody>,
) -> Result<Json<Vec<FriendUser>>, AppError> {
    let user_id = authenticate(&ctx, &headers).await?;
    let code = body.code.trim().to_uppercase();
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT user_id FROM friend_invites
         WHERE code = ? AND expires_at > datetime('now')",
    )
    .bind(&code)
    .fetch_optional(&ctx.db)
    .await?;
    let (owner_id,) =
        row.ok_or_else(|| AppError::not_found("unknown or expired friend code".into()))?;
    if owner_id == user_id {
        return Err(AppError::bad_request("that's your own friend code".into()));
    }
    // Both directions in one shot; idempotent.
    sqlx::query("INSERT OR IGNORE INTO friendships (user_id, friend_id) VALUES (?, ?), (?, ?)")
        .bind(&user_id)
        .bind(&owner_id)
        .bind(&owner_id)
        .bind(&user_id)
        .execute(&ctx.db)
        .await?;
    Ok(Json(load_friends(&ctx.db, &user_id).await?))
}

#[derive(serde::Deserialize)]
pub struct RemoveBody {
    pub user_id: String,
}

/// POST /api/friends/remove — drop a friendship (both directions).
pub async fn remove_friend(
    State(ctx): State<Arc<AppCtx>>,
    headers: HeaderMap,
    Json(body): Json<RemoveBody>,
) -> Result<Json<Vec<FriendUser>>, AppError> {
    let user_id = authenticate(&ctx, &headers).await?;
    sqlx::query(
        "DELETE FROM friendships
         WHERE (user_id = ? AND friend_id = ?) OR (user_id = ? AND friend_id = ?)",
    )
    .bind(&user_id)
    .bind(&body.user_id)
    .bind(&body.user_id)
    .bind(&user_id)
    .execute(&ctx.db)
    .await?;
    Ok(Json(load_friends(&ctx.db, &user_id).await?))
}
