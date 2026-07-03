//! Starlume server — currently auth-only: Discord OAuth (confidential client)
//! issuing per-device tokens to desktop companions. Grows into the v2
//! personal-data / community API on the same bones (axum + sqlx + SQLite).
//!
//! Desktop login flow (one-time-code handoff — no long-lived token ever
//! appears in a browser URL/history):
//!
//! 1. Desktop opens `GET /auth/desktop/start?nonce=N` in the system browser.
//! 2. We mint OAuth `state` S (bound to N, 10-min TTL) and redirect to
//!    Discord's authorize page.
//! 3. Discord calls `GET /auth/discord/callback?code&state=S`; we exchange the
//!    code, fetch `/users/@me`, upsert the user, mint a one-time login code C
//!    (5-min TTL) and redirect to `starlume://auth/callback?nonce=N&code=C`.
//! 4. The desktop exchanges C via `POST /auth/desktop/exchange` for the
//!    device token (stored sha256-hashed) + profile.
//! 5. `GET /api/me` (Bearer device token) returns the profile.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect};
use axum::routing::{get, post};
use axum::{Json, Router};
use sha2::Digest;
use tokio::sync::Mutex;

mod discord;
mod friends;
mod groups;

const STATE_TTL: Duration = Duration::from_secs(10 * 60);
const LOGIN_CODE_TTL: Duration = Duration::from_secs(5 * 60);

struct Config {
    public_url: String,
    discord_client_id: String,
    discord_client_secret: String,
}

impl Config {
    fn redirect_uri(&self) -> String {
        format!(
            "{}/auth/discord/callback",
            self.public_url.trim_end_matches('/')
        )
    }
}

/// A value in one of the short-lived in-memory maps. Single-instance is fine
/// for now; these move to the DB if the server ever runs more than one node.
struct Pending<T> {
    value: T,
    expires: Instant,
}

pub(crate) struct AppCtx {
    config: Config,
    pub(crate) db: sqlx::SqlitePool,
    http: reqwest::Client,
    /// OAuth `state` → the desktop's pending login (nonce + callback).
    states: Mutex<HashMap<String, Pending<PendingLogin>>>,
    /// One-time login code → user id.
    login_codes: Mutex<HashMap<String, Pending<String>>>,
}

/// 256-bit random hex string (uuid v4 uses the OS RNG).
pub(crate) fn random_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

fn sha256_hex(s: &str) -> String {
    hex::encode(sha2::Sha256::digest(s.as_bytes()))
}

fn insert_pending<T>(map: &mut HashMap<String, Pending<T>>, key: String, value: T, ttl: Duration) {
    map.retain(|_, p| p.expires > Instant::now());
    map.insert(
        key,
        Pending {
            value,
            expires: Instant::now() + ttl,
        },
    );
}

fn take_pending<T>(map: &mut HashMap<String, Pending<T>>, key: &str) -> Option<T> {
    map.retain(|_, p| p.expires > Instant::now());
    map.remove(key).map(|p| p.value)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,sqlx=warn".into()),
        )
        .init();

    let config = Config {
        public_url: std::env::var("STARLUME_PUBLIC_URL")
            .unwrap_or_else(|_| "http://localhost:5863".into()),
        discord_client_id: std::env::var("DISCORD_CLIENT_ID")
            .map_err(|_| anyhow::anyhow!("DISCORD_CLIENT_ID not set (see .env.example)"))?,
        discord_client_secret: std::env::var("DISCORD_CLIENT_SECRET")
            .map_err(|_| anyhow::anyhow!("DISCORD_CLIENT_SECRET not set (see .env.example)"))?,
    };
    let bind = std::env::var("STARLUME_BIND").unwrap_or_else(|_| "127.0.0.1:5863".into());
    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://starlume.db".into());

    let db = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(
            db_url
                .parse::<sqlx::sqlite::SqliteConnectOptions>()?
                .create_if_missing(true)
                .foreign_keys(true),
        )
        .await?;
    sqlx::migrate!("./migrations").run(&db).await?;

    let ctx = Arc::new(AppCtx {
        config,
        db,
        http: reqwest::Client::new(),
        states: Mutex::new(HashMap::new()),
        login_codes: Mutex::new(HashMap::new()),
    });

    let app = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/auth/desktop/start", get(desktop_start))
        .route("/auth/discord/callback", get(discord_callback))
        .route("/auth/desktop/exchange", post(desktop_exchange))
        .route("/api/me", get(api_me))
        .route("/api/friends", get(friends::list_friends))
        .route("/api/friends/invites", post(friends::create_invite))
        .route("/api/friends/add", post(friends::add_friend))
        .route("/api/friends/remove", post(friends::remove_friend))
        .route(
            "/api/groups",
            get(groups::list_groups).post(groups::create_group),
        )
        .route("/api/groups/join", post(groups::join_group))
        .route("/api/groups/{id}/invites", post(groups::create_invite))
        .route("/api/groups/{id}/leave", post(groups::leave_group))
        .with_state(ctx);

    tracing::info!("listening on http://{bind}");
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

// ── Handlers ────────────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct StartQuery {
    nonce: String,
    /// Optional callback override for dev multi-instance testing. Restricted
    /// to loopback — anything else would be an open redirect.
    redirect: Option<String>,
}

/// What a completed OAuth flow needs to hand control back to the desktop.
struct PendingLogin {
    nonce: String,
    redirect: Option<String>,
}

fn valid_loopback_redirect(redirect: &str) -> bool {
    url::Url::parse(redirect).is_ok_and(|u| {
        u.scheme() == "http" && matches!(u.host_str(), Some("127.0.0.1") | Some("localhost"))
    })
}

async fn desktop_start(
    State(ctx): State<Arc<AppCtx>>,
    Query(q): Query<StartQuery>,
) -> Result<impl IntoResponse, AppError> {
    if let Some(r) = &q.redirect
        && !valid_loopback_redirect(r)
    {
        return Err(AppError::bad_request(
            "redirect must be a loopback http URL".into(),
        ));
    }
    let state = random_token();
    insert_pending(
        &mut *ctx.states.lock().await,
        state.clone(),
        PendingLogin {
            nonce: q.nonce,
            redirect: q.redirect,
        },
        STATE_TTL,
    );
    Ok(Redirect::temporary(&discord::authorize_url(
        &ctx.config.discord_client_id,
        &ctx.config.redirect_uri(),
        &state,
    )))
}

#[derive(serde::Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

async fn discord_callback(
    State(ctx): State<Arc<AppCtx>>,
    Query(q): Query<CallbackQuery>,
) -> Result<impl IntoResponse, AppError> {
    if let Some(e) = q.error {
        // User hit "Cancel" on the Discord consent screen.
        return Err(AppError::bad_request(format!("Discord returned: {e}")));
    }
    let (code, state) = q
        .code
        .zip(q.state)
        .ok_or_else(|| AppError::bad_request("missing code/state".into()))?;

    let pending = take_pending(&mut *ctx.states.lock().await, &state).ok_or_else(|| {
        AppError::bad_request("unknown or expired state — restart the sign-in from the app".into())
    })?;

    let access_token = discord::exchange_code(
        &ctx.http,
        &ctx.config.discord_client_id,
        &ctx.config.discord_client_secret,
        &code,
        &ctx.config.redirect_uri(),
    )
    .await?;
    let user = discord::fetch_user(&ctx.http, &access_token).await?;

    // Upsert on the immutable discord_id; display fields refresh every login.
    let user_id = uuid::Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO users (id, discord_id, username, avatar) VALUES (?, ?, ?, ?)
         ON CONFLICT(discord_id) DO UPDATE SET
           username = excluded.username,
           avatar = excluded.avatar,
           updated_at = datetime('now')",
    )
    .bind(&user_id)
    .bind(&user.id)
    .bind(user.display_name())
    .bind(&user.avatar)
    .execute(&ctx.db)
    .await?;
    let (user_id,): (String,) = sqlx::query_as("SELECT id FROM users WHERE discord_id = ?")
        .bind(&user.id)
        .fetch_one(&ctx.db)
        .await?;

    let login_code = random_token();
    insert_pending(
        &mut *ctx.login_codes.lock().await,
        login_code.clone(),
        user_id,
        LOGIN_CODE_TTL,
    );

    // Hand control back to the desktop app — deep link normally, loopback
    // for dev profile instances.
    let base = pending
        .redirect
        .as_deref()
        .unwrap_or("starlume://auth/callback");
    let nonce = pending.nonce;
    Ok(Redirect::temporary(&format!(
        "{base}?nonce={nonce}&code={login_code}"
    )))
}

#[derive(serde::Deserialize)]
struct ExchangeBody {
    code: String,
}

#[derive(serde::Serialize)]
struct ExchangeResponse {
    token: String,
    profile: Profile,
}

#[derive(serde::Serialize)]
struct Profile {
    username: String,
    avatar_url: Option<String>,
}

async fn desktop_exchange(
    State(ctx): State<Arc<AppCtx>>,
    Json(body): Json<ExchangeBody>,
) -> Result<Json<ExchangeResponse>, AppError> {
    let user_id = take_pending(&mut *ctx.login_codes.lock().await, &body.code)
        .ok_or_else(|| AppError::unauthorized("unknown or expired login code".into()))?;

    let token = random_token();
    sqlx::query("INSERT INTO device_tokens (id, user_id, token_hash) VALUES (?, ?, ?)")
        .bind(uuid::Uuid::now_v7().to_string())
        .bind(&user_id)
        .bind(sha256_hex(&token))
        .execute(&ctx.db)
        .await?;

    let profile = load_profile(&ctx.db, &user_id).await?;
    Ok(Json(ExchangeResponse { token, profile }))
}

/// Resolve the Bearer device token to a user id (updating `last_seen`).
/// Shared by every `/api/*` handler.
pub(crate) async fn authenticate(ctx: &AppCtx, headers: &HeaderMap) -> Result<String, AppError> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| AppError::unauthorized("missing bearer token".into()))?;

    let row: Option<(String,)> = sqlx::query_as(
        "UPDATE device_tokens SET last_seen = datetime('now')
         WHERE token_hash = ? RETURNING user_id",
    )
    .bind(sha256_hex(token))
    .fetch_optional(&ctx.db)
    .await?;
    row.map(|(user_id,)| user_id)
        .ok_or_else(|| AppError::unauthorized("invalid token".into()))
}

async fn api_me(
    State(ctx): State<Arc<AppCtx>>,
    headers: HeaderMap,
) -> Result<Json<Profile>, AppError> {
    let user_id = authenticate(&ctx, &headers).await?;
    Ok(Json(load_profile(&ctx.db, &user_id).await?))
}

async fn load_profile(db: &sqlx::SqlitePool, user_id: &str) -> Result<Profile, AppError> {
    let (discord_id, username, avatar): (String, String, Option<String>) =
        sqlx::query_as("SELECT discord_id, username, avatar FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_one(db)
            .await?;
    Ok(Profile {
        username,
        avatar_url: avatar
            .map(|hash| format!("https://cdn.discordapp.com/avatars/{discord_id}/{hash}.png")),
    })
}

// ── Error plumbing ──────────────────────────────────────────────────────

pub(crate) struct AppError {
    status: StatusCode,
    message: String,
}

impl AppError {
    pub(crate) fn bad_request(message: String) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message,
        }
    }
    pub(crate) fn unauthorized(message: String) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message,
        }
    }
    pub(crate) fn not_found(message: String) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        (self.status, self.message).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        tracing::error!("db error: {e}");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "internal error".into(),
        }
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        tracing::error!("error: {e:#}");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "internal error".into(),
        }
    }
}
