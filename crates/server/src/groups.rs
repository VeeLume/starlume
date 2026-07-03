//! Friend groups — the first community primitive (FriendGroup from the
//! Hearth v2 design). **Content-free by design**: a group is a named member
//! set with invite codes. Features attach later by referencing group ids as
//! visibility scopes; nothing feature-shaped may live in this module.
//!
//! Alpha semantics, kept simple on purpose:
//! - anyone can create groups; the creator is owner + first member;
//! - any member can mint invite codes (7-day expiry);
//! - joining via a valid code is idempotent;
//! - anyone can leave, owners included; the last member leaving deletes the
//!   group. Ownership currently only matters for display.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::{Json, http::HeaderMap};

use crate::{AppCtx, AppError, authenticate, random_token};

const INVITE_TTL_DAYS: i64 = 7;

#[derive(serde::Serialize)]
pub struct GroupMember {
    pub username: String,
    pub is_owner: bool,
}

#[derive(serde::Serialize)]
pub struct FriendGroup {
    pub id: String,
    pub name: String,
    pub is_owner: bool,
    pub members: Vec<GroupMember>,
}

async fn load_group(
    db: &sqlx::SqlitePool,
    group_id: &str,
    viewer_id: &str,
) -> Result<FriendGroup, AppError> {
    let (name, owner_id): (String, String) =
        sqlx::query_as("SELECT name, owner_id FROM groups WHERE id = ?")
            .bind(group_id)
            .fetch_one(db)
            .await?;
    let members: Vec<(String, String)> = sqlx::query_as(
        "SELECT u.id, u.username FROM group_members m
         JOIN users u ON u.id = m.user_id
         WHERE m.group_id = ? ORDER BY m.joined_at",
    )
    .bind(group_id)
    .fetch_all(db)
    .await?;
    Ok(FriendGroup {
        id: group_id.to_string(),
        name,
        is_owner: owner_id == viewer_id,
        members: members
            .into_iter()
            .map(|(id, username)| GroupMember {
                username,
                is_owner: id == owner_id,
            })
            .collect(),
    })
}

async fn require_membership(
    db: &sqlx::SqlitePool,
    group_id: &str,
    user_id: &str,
) -> Result<(), AppError> {
    let member: Option<(i64,)> =
        sqlx::query_as("SELECT 1 FROM group_members WHERE group_id = ? AND user_id = ?")
            .bind(group_id)
            .bind(user_id)
            .fetch_optional(db)
            .await?;
    member
        .map(|_| ())
        .ok_or_else(|| AppError::not_found("not a member of this group".into()))
}

/// GET /api/groups — every group the caller is a member of.
pub async fn list_groups(
    State(ctx): State<Arc<AppCtx>>,
    headers: HeaderMap,
) -> Result<Json<Vec<FriendGroup>>, AppError> {
    let user_id = authenticate(&ctx, &headers).await?;
    let group_ids: Vec<(String,)> = sqlx::query_as(
        "SELECT g.id FROM groups g
         JOIN group_members m ON m.group_id = g.id
         WHERE m.user_id = ? ORDER BY g.created_at",
    )
    .bind(&user_id)
    .fetch_all(&ctx.db)
    .await?;
    let mut groups = Vec::with_capacity(group_ids.len());
    for (id,) in group_ids {
        groups.push(load_group(&ctx.db, &id, &user_id).await?);
    }
    Ok(Json(groups))
}

#[derive(serde::Deserialize)]
pub struct CreateGroupBody {
    pub name: String,
}

/// POST /api/groups — create; the caller becomes owner + first member.
pub async fn create_group(
    State(ctx): State<Arc<AppCtx>>,
    headers: HeaderMap,
    Json(body): Json<CreateGroupBody>,
) -> Result<Json<FriendGroup>, AppError> {
    let user_id = authenticate(&ctx, &headers).await?;
    let name = body.name.trim();
    if name.is_empty() || name.len() > 64 {
        return Err(AppError::bad_request(
            "group name must be 1–64 characters".into(),
        ));
    }
    let group_id = uuid::Uuid::now_v7().to_string();
    sqlx::query("INSERT INTO groups (id, name, owner_id) VALUES (?, ?, ?)")
        .bind(&group_id)
        .bind(name)
        .bind(&user_id)
        .execute(&ctx.db)
        .await?;
    sqlx::query("INSERT INTO group_members (group_id, user_id) VALUES (?, ?)")
        .bind(&group_id)
        .bind(&user_id)
        .execute(&ctx.db)
        .await?;
    Ok(Json(load_group(&ctx.db, &group_id, &user_id).await?))
}

#[derive(serde::Serialize)]
pub struct InviteResponse {
    pub code: String,
}

/// POST /api/groups/{id}/invites — mint an invite code (member-only).
pub async fn create_invite(
    State(ctx): State<Arc<AppCtx>>,
    headers: HeaderMap,
    Path(group_id): Path<String>,
) -> Result<Json<InviteResponse>, AppError> {
    let user_id = authenticate(&ctx, &headers).await?;
    require_membership(&ctx.db, &group_id, &user_id).await?;
    // Short + uppercase: meant to be read aloud / pasted in a Discord chat.
    let code = random_token()[..10].to_uppercase();
    sqlx::query(
        "INSERT INTO group_invites (code, group_id, created_by, expires_at)
         VALUES (?, ?, ?, datetime('now', ?))",
    )
    .bind(&code)
    .bind(&group_id)
    .bind(&user_id)
    .bind(format!("+{INVITE_TTL_DAYS} days"))
    .execute(&ctx.db)
    .await?;
    Ok(Json(InviteResponse { code }))
}

#[derive(serde::Deserialize)]
pub struct JoinBody {
    pub code: String,
}

/// POST /api/groups/join — join via invite code (idempotent for members).
pub async fn join_group(
    State(ctx): State<Arc<AppCtx>>,
    headers: HeaderMap,
    Json(body): Json<JoinBody>,
) -> Result<Json<FriendGroup>, AppError> {
    let user_id = authenticate(&ctx, &headers).await?;
    let code = body.code.trim().to_uppercase();
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT group_id FROM group_invites
         WHERE code = ? AND expires_at > datetime('now')",
    )
    .bind(&code)
    .fetch_optional(&ctx.db)
    .await?;
    let (group_id,) =
        row.ok_or_else(|| AppError::not_found("unknown or expired invite code".into()))?;
    sqlx::query("INSERT OR IGNORE INTO group_members (group_id, user_id) VALUES (?, ?)")
        .bind(&group_id)
        .bind(&user_id)
        .execute(&ctx.db)
        .await?;
    Ok(Json(load_group(&ctx.db, &group_id, &user_id).await?))
}

/// POST /api/groups/{id}/leave — leave; the last member out deletes the group.
pub async fn leave_group(
    State(ctx): State<Arc<AppCtx>>,
    headers: HeaderMap,
    Path(group_id): Path<String>,
) -> Result<Json<()>, AppError> {
    let user_id = authenticate(&ctx, &headers).await?;
    require_membership(&ctx.db, &group_id, &user_id).await?;
    sqlx::query("DELETE FROM group_members WHERE group_id = ? AND user_id = ?")
        .bind(&group_id)
        .bind(&user_id)
        .execute(&ctx.db)
        .await?;
    sqlx::query(
        "DELETE FROM groups WHERE id = ?
         AND NOT EXISTS (SELECT 1 FROM group_members WHERE group_id = ?)",
    )
    .bind(&group_id)
    .bind(&group_id)
    .execute(&ctx.db)
    .await?;
    Ok(Json(()))
}
