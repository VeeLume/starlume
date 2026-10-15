//! Dev tracker — the shell side of the module: the poll, its gate, the
//! module's persisted state, and the IPC surface.
//!
//! The judgement (content filter, author modes, read state) is
//! `mod-devtracker`; the network read is `svc-spectrum`. The gate lives here:
//! every fetch passes `require_unofficial("devtracker")` first, and the module
//! must be enabled. The cached reads touch no network.
//!
//! **Polling.** A background task polls one page every [`POLL_INTERVAL`],
//! hidden or not (decided with Valerie 2026-10-15: ops notifications must be
//! timely) — one paced request, the feed loaded from disk and dropped, nothing
//! resident. A settings change or a manual refresh wakes it early. The first
//! poll on an empty cache backfills [`FIRST_BACKFILL_DAYS`].

use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use mod_devtracker::{AuthorMode, DevtrackerConfig, ReadState};
use svc_spectrum::{FeedPost, PostKind, QuoteKind};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Notify;

use crate::AppState;
use crate::activity::{Activity, ActivityKind};
use crate::error::AppError;
use crate::notify::{Notification, notify};

/// Module id (Rust registry, settings, frontend descriptor).
pub(crate) const MODULE_ID: &str = "devtracker";
/// Fired after the feed or the module's state changed. The store reloads.
pub const DEVTRACKER_CHANGED_EVENT: &str = "devtracker:changed";
/// How often the background poll runs.
const POLL_INTERVAL: Duration = Duration::from_secs(5 * 60);
/// Delay before the first poll — startup has heavier work to do first.
const STARTUP_DELAY: Duration = Duration::from_secs(20);
/// How far back the first fetch on an empty cache reaches.
const FIRST_BACKFILL_DAYS: i64 = 14;
/// Posts on the first fill older than this count as read — a fresh tracker
/// starts with the last day unread, not two weeks of backlog.
const FIRST_UNREAD_WINDOW_MS: i64 = 24 * 60 * 60 * 1000;
/// Posts whose context (what they answer) one poll resolves at most.
const CONTEXT_PER_POLL: usize = 12;
/// How far back the page's feed reaches.
const VIEW_DAYS: i64 = 30;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;

/// Wakes the poller early (manual refresh, settings change).
static WAKE: LazyLock<Notify> = LazyLock::new(Notify::new);
/// The last poll's error, for the page ("launcher not signed in"). Session
/// memory: a restart retries anyway.
static LAST_ERROR: Mutex<Option<String>> = Mutex::new(None);
/// Serializes config/read-state read-modify-writes across commands.
static STATE_LOCK: Mutex<()> = Mutex::new(());

fn dir() -> std::path::PathBuf {
    app_kit::app_data_root().join("devtracker")
}

fn load_config() -> DevtrackerConfig {
    app_kit::load_json(&dir().join("config.json"))
}

fn save_config(config: &DevtrackerConfig) -> Result<(), AppError> {
    Ok(app_kit::save_json(&dir().join("config.json"), config)?)
}

fn load_read() -> ReadState {
    app_kit::load_json(&dir().join("read.json"))
}

fn save_read(read: &ReadState) -> Result<(), AppError> {
    Ok(app_kit::save_json(&dir().join("read.json"), read)?)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn module_enabled(state: &AppState) -> bool {
    state
        .settings
        .lock()
        .unwrap()
        .enabled_modules
        .iter()
        .any(|m| m == MODULE_ID)
}

fn spectrum_error(e: svc_spectrum::Error) -> AppError {
    AppError::Internal(e.to_string())
}

// ── Views ───────────────────────────────────────────────────────────────────

/// One post as the page shows it (specta mirror; module crates stay
/// specta-free).
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct DevPostView {
    /// Read key (`kind:id`).
    pub key: String,
    /// `"chat"` | `"thread"` | `"reply"`.
    pub kind: String,
    pub author_id: String,
    pub author: String,
    /// The chat lobby or forum channel.
    pub place: Option<String>,
    /// The forum thread's title, for threads and replies.
    pub subject: Option<String>,
    /// Plain text, entities decoded, line breaks kept.
    pub body: String,
    /// Epoch milliseconds.
    pub time_ms: f64,
    pub url: String,
    /// On the feed; `false` = filtered out (shown only with the toggle).
    pub shown: bool,
    /// Why — the filter's reason, or the author override.
    pub reason: String,
    /// A strong game-ops signal (maintenance, PTU, hotfix, known issue).
    pub ops: bool,
    pub unread: bool,
    /// What the post answers, quoted (forum parent / thread opener / the
    /// mentioned member's latest chat message). `null` until resolved or when
    /// there is nothing to quote.
    pub quote: Option<QuoteView>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct QuoteView {
    /// `"reply"` | `"opener"` | `"mention"`.
    pub kind: String,
    pub author: String,
    /// Plain text, line breaks kept; empty when erased.
    pub text: String,
    pub erased: bool,
}

/// Whether the tracker can fetch right now, and if not, why.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(tag = "state", content = "reason", rename_all = "lowercase")]
pub enum DevtrackerGate {
    Open,
    /// The online master or the unofficial-API opt-in is off.
    Closed(String),
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct DevtrackerOverview {
    pub gate: DevtrackerGate,
    /// When the feed was last fetched, epoch seconds; `null` = never.
    pub fetched_at: Option<f64>,
    /// Spectrum is rate-limiting: paused until (epoch seconds).
    pub throttled_until: Option<f64>,
    /// The last poll's error, if it failed.
    pub last_error: Option<String>,
    /// The last [`VIEW_DAYS`] days, newest first, filtered-out posts included.
    pub posts: Vec<DevPostView>,
    /// Unread posts on the feed.
    pub unread: u32,
    pub notify_ops: bool,
    /// Patch-note threads in the whole cache, newest first.
    pub patch_notes: Vec<PatchNoteView>,
}

/// A patch-note thread and what its title says.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct PatchNoteView {
    /// Read key of the thread opener (opens it on Spectrum).
    pub key: String,
    pub subject: String,
    pub version: Option<String>,
    /// `"LIVE"`, `"PTU"`, `"EPTU"`, `"Tech Preview"`, `"Hotfix"`, `"Evocati"`.
    pub channel: Option<String>,
    pub build: Option<String>,
    /// `"RC2"`, `"Wave 1"`.
    pub stage: Option<String>,
    pub author: String,
    /// Epoch milliseconds.
    pub time_ms: f64,
}

/// One staff account in the follow list.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct DevAuthorView {
    pub id: String,
    pub nickname: String,
    /// `"always"` | `"filter"` | `"mute"`.
    pub mode: String,
    /// Posts in the cached feed over the last [`VIEW_DAYS`] days.
    pub posts: u32,
    /// Of those, how many the content filter keeps.
    pub kept: u32,
    /// On the follow list; `false` = staff seen in the feed, not listed.
    pub listed: bool,
}

fn mode_str(mode: AuthorMode) -> &'static str {
    match mode {
        AuthorMode::Always => "always",
        AuthorMode::Filter => "filter",
        AuthorMode::Mute => "mute",
    }
}

fn kind_str(kind: PostKind) -> &'static str {
    match kind {
        PostKind::Chat => "chat",
        PostKind::Thread => "thread",
        PostKind::Reply => "reply",
    }
}

fn post_view(post: &FeedPost, config: &DevtrackerConfig, read: &ReadState) -> DevPostView {
    let d = mod_devtracker::decide(post, config);
    let reason = match d.mode {
        AuthorMode::Always => "always shown (follow list)".to_owned(),
        AuthorMode::Mute => "muted (follow list)".to_owned(),
        AuthorMode::Filter => d.verdict.reason.label(),
    };
    DevPostView {
        key: mod_devtracker::read_key(post),
        kind: kind_str(post.kind).into(),
        author_id: post.author.id.clone(),
        author: post.author.nickname.clone(),
        place: post
            .lobby
            .as_ref()
            .or(post.channel.as_ref())
            .map(|p| p.name.clone()),
        subject: post.thread.as_ref().map(|t| t.subject.clone()),
        body: mod_devtracker::filter::decode_entities(&post.body),
        time_ms: post.time_ms as f64,
        url: post.url.clone(),
        shown: d.shown,
        reason,
        ops: d.ops,
        unread: d.shown && !read.is_read(post),
        quote: post.context.as_ref().map(|q| QuoteView {
            kind: match q.kind {
                QuoteKind::Reply => "reply",
                QuoteKind::Opener => "opener",
                QuoteKind::Mention => "mention",
            }
            .into(),
            author: q.author.clone(),
            text: q.text.clone(),
            erased: q.erased,
        }),
    }
}

fn gate(state: &AppState) -> DevtrackerGate {
    match state.require_unofficial(MODULE_ID) {
        Ok(()) => DevtrackerGate::Open,
        Err(e) => DevtrackerGate::Closed(e.to_string()),
    }
}

/// The feed and the module's state. **No network, no gate.**
#[tauri::command]
#[specta::specta]
pub(crate) fn devtracker_overview(state: tauri::State<'_, AppState>) -> DevtrackerOverview {
    let feed = state.spectrum.cached_feed();
    let config = load_config();
    let read = load_read();
    let since = now_ms() - VIEW_DAYS * DAY_MS;
    let posts: Vec<DevPostView> = feed
        .as_ref()
        .map(|f| {
            f.posts
                .iter()
                .take_while(|p| p.time_ms >= since)
                .map(|p| post_view(p, &config, &read))
                .collect()
        })
        .unwrap_or_default();
    DevtrackerOverview {
        gate: gate(&state),
        fetched_at: feed.as_ref().map(|f| f.fetched_at as f64),
        throttled_until: state.spectrum.throttled_until().map(|t| t as f64),
        last_error: LAST_ERROR.lock().unwrap().clone(),
        unread: posts.iter().filter(|p| p.unread).count() as u32,
        posts,
        notify_ops: config.notify_ops,
        patch_notes: feed
            .as_ref()
            .map(|f| {
                f.posts
                    .iter()
                    .filter_map(|p| {
                        let note = mod_devtracker::patch::patch_note(p)?;
                        Some(PatchNoteView {
                            key: mod_devtracker::read_key(p),
                            subject: p.thread.as_ref().map(|t| t.subject.clone())?,
                            version: note.version,
                            channel: note.channel,
                            build: note.build,
                            stage: note.stage,
                            author: p.author.nickname.clone(),
                            time_ms: p.time_ms as f64,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

/// The follow list, then the staff the feed saw that are not on it. Each
/// group most active first. **No network, no gate.**
#[tauri::command]
#[specta::specta]
pub(crate) fn devtracker_authors(state: tauri::State<'_, AppState>) -> Vec<DevAuthorView> {
    let config = load_config();
    let since = now_ms() - VIEW_DAYS * DAY_MS;
    let mut rows: Vec<DevAuthorView> = Vec::new();
    let mut index = std::collections::HashMap::new();
    let mut row =
        |rows: &mut Vec<DevAuthorView>, id: &str, nickname: &str, listed: bool| -> usize {
            *index.entry(id.to_owned()).or_insert_with(|| {
                rows.push(DevAuthorView {
                    id: id.to_owned(),
                    nickname: nickname.to_owned(),
                    mode: mode_str(config.mode(id)).into(),
                    posts: 0,
                    kept: 0,
                    listed,
                });
                rows.len() - 1
            })
        };
    for f in config.follow_list() {
        row(&mut rows, &f.id, &f.nickname, true);
    }
    if let Some(feed) = state.spectrum.cached_feed() {
        for p in feed.posts.iter().take_while(|p| p.time_ms >= since) {
            let i = row(&mut rows, &p.author.id, &p.author.nickname, false);
            // The feed's nickname is current; the list's may be stale.
            rows[i].nickname = p.author.nickname.clone();
            rows[i].posts += 1;
            let is_chat = p.kind == PostKind::Chat;
            let lobby = p.lobby.as_ref().map(|l| l.name.as_str());
            if mod_devtracker::filter::classify(is_chat, &p.body, lobby).keep {
                rows[i].kept += 1;
            }
        }
    }
    rows.sort_by(|a, b| {
        b.listed
            .cmp(&a.listed)
            .then_with(|| b.posts.cmp(&a.posts))
            .then_with(|| a.nickname.to_lowercase().cmp(&b.nickname.to_lowercase()))
    });
    rows
}

/// Put an account on the follow list (one the feed has seen: id + handle
/// known locally).
#[tauri::command]
#[specta::specta]
pub(crate) fn devtracker_follow(
    app: AppHandle,
    author_id: String,
    nickname: String,
) -> Result<(), AppError> {
    edit_config(&app, |c| c.follow(&author_id, &nickname))
}

/// Take an account off the follow list (its override goes too).
#[tauri::command]
#[specta::specta]
pub(crate) fn devtracker_unfollow(app: AppHandle, author_id: String) -> Result<(), AppError> {
    edit_config(&app, |c| c.unfollow(&author_id))
}

/// Put an account on the follow list by its Spectrum handle. **Gated**: the
/// handle → member id lookup is a Spectrum request.
#[tauri::command]
#[specta::specta]
pub(crate) async fn devtracker_follow_handle(
    app: AppHandle,
    handle: String,
) -> Result<(), AppError> {
    let handle = handle.trim().to_owned();
    if handle.is_empty() {
        return Err(AppError::Config("enter a Spectrum handle".into()));
    }
    let spectrum = {
        let state = app.state::<AppState>();
        state.require_unofficial(MODULE_ID)?;
        state.spectrum.clone()
    };
    let id = spectrum
        .member_id(crate::dossier::GRPC_USER_AGENT, &handle)
        .await
        .map_err(|e| AppError::Config(format!("couldn't find '{handle}' on Spectrum: {e}")))?;
    edit_config(&app, |c| c.follow(&id, &handle))
}

/// Read-modify-write the module config, then announce the change.
fn edit_config(app: &AppHandle, edit: impl FnOnce(&mut DevtrackerConfig)) -> Result<(), AppError> {
    {
        let _guard = STATE_LOCK.lock().unwrap();
        let mut config = load_config();
        edit(&mut config);
        save_config(&config)?;
    }
    let _ = app.emit(DEVTRACKER_CHANGED_EVENT, ());
    Ok(())
}

/// Set one author's mode (`"always"` | `"filter"` | `"mute"`).
#[tauri::command]
#[specta::specta]
pub(crate) fn devtracker_set_author_mode(
    app: AppHandle,
    author_id: String,
    mode: String,
) -> Result<(), AppError> {
    let mode = match mode.as_str() {
        "always" => AuthorMode::Always,
        "filter" => AuthorMode::Filter,
        "mute" => AuthorMode::Mute,
        other => return Err(AppError::Internal(format!("unknown author mode '{other}'"))),
    };
    {
        let _guard = STATE_LOCK.lock().unwrap();
        let mut config = load_config();
        config.set_mode(&author_id, mode);
        save_config(&config)?;
    }
    let _ = app.emit(DEVTRACKER_CHANGED_EVENT, ());
    Ok(())
}

/// Back to the defaults: the seeded follow list, every author filtered by
/// content.
#[tauri::command]
#[specta::specta]
pub(crate) fn devtracker_reset_authors(app: AppHandle) -> Result<(), AppError> {
    edit_config(&app, DevtrackerConfig::reset_follows)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn devtracker_set_notify_ops(app: AppHandle, on: bool) -> Result<(), AppError> {
    {
        let _guard = STATE_LOCK.lock().unwrap();
        let mut config = load_config();
        config.notify_ops = on;
        save_config(&config)?;
    }
    let _ = app.emit(DEVTRACKER_CHANGED_EVENT, ());
    Ok(())
}

/// Mark posts read by key.
#[tauri::command]
#[specta::specta]
pub(crate) fn devtracker_mark_read(app: AppHandle, keys: Vec<String>) -> Result<(), AppError> {
    {
        let _guard = STATE_LOCK.lock().unwrap();
        let mut read = load_read();
        for k in keys {
            read.mark(k);
        }
        save_read(&read)?;
    }
    let _ = app.emit(DEVTRACKER_CHANGED_EVENT, ());
    Ok(())
}

/// Mark everything up to now read.
#[tauri::command]
#[specta::specta]
pub(crate) fn devtracker_mark_all_read(app: AppHandle) -> Result<(), AppError> {
    {
        let _guard = STATE_LOCK.lock().unwrap();
        let posts = app
            .state::<AppState>()
            .spectrum
            .cached_feed()
            .map(|f| f.posts)
            .unwrap_or_default();
        let mut read = load_read();
        read.mark_all(now_ms(), &posts);
        save_read(&read)?;
    }
    let _ = app.emit(DEVTRACKER_CHANGED_EVENT, ());
    Ok(())
}

/// Open a post on Spectrum in the user's browser and mark it read. The URL
/// comes from the cached feed, never from the caller — the frontend cannot
/// open arbitrary links through this.
#[tauri::command]
#[specta::specta]
pub(crate) fn devtracker_open_post(app: AppHandle, key: String) -> Result<(), AppError> {
    use tauri_plugin_opener::OpenerExt;
    let url = app
        .state::<AppState>()
        .spectrum
        .cached_feed()
        .and_then(|f| {
            f.posts
                .into_iter()
                .find(|p| mod_devtracker::read_key(p) == key)
        })
        .map(|p| p.url)
        .filter(|u| u.starts_with("https://robertsspaceindustries.com/"))
        .ok_or_else(|| AppError::Internal("that post is no longer in the feed".into()))?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| AppError::Internal(format!("could not open the browser: {e}")))?;
    devtracker_mark_read(app, vec![key])
}

/// Fetch now. **Gated** by `require_unofficial("devtracker")`; errors
/// (launcher session, throttle) come back to the caller.
#[tauri::command]
#[specta::specta]
pub(crate) async fn devtracker_refresh(app: AppHandle) -> Result<(), AppError> {
    poll(&app).await
}

// ── The poll ────────────────────────────────────────────────────────────────

/// Wake the poller now (a settings change may have opened the gate or
/// enabled the module).
pub(crate) fn nudge() {
    WAKE.notify_one();
}

/// One poll: backfill on an empty cache, else the newest page; notify about
/// new ops posts; emit the change. **Gated.**
async fn poll(app: &AppHandle) -> Result<(), AppError> {
    let spectrum = {
        let state = app.state::<AppState>();
        if !module_enabled(&state) {
            return Err(AppError::Config(
                "the dev tracker module is not enabled".into(),
            ));
        }
        state.require_unofficial(MODULE_ID)?;
        state.spectrum.clone()
    };
    let agent = crate::dossier::GRPC_USER_AGENT;
    let first = spectrum.cached_feed().is_none();
    let result = if first {
        let _activity = Activity::start(app, ActivityKind::Devtracker, None, "Dev tracker").stage(
            format!("Reading the last {FIRST_BACKFILL_DAYS} days of staff posts…"),
        );
        spectrum
            .backfill(agent, now_ms() - FIRST_BACKFILL_DAYS * DAY_MS)
            .await
    } else {
        spectrum.fetch_newest(agent).await
    };
    let report = match result {
        Ok(report) => {
            *LAST_ERROR.lock().unwrap() = None;
            report
        }
        Err(e) => {
            let err = spectrum_error(e);
            *LAST_ERROR.lock().unwrap() = Some(err.to_string());
            // A partial fill is cached; the page shows it and the error.
            let _ = app.emit(DEVTRACKER_CHANGED_EVENT, ());
            return Err(err);
        }
    };
    if first {
        // Start with the last day unread, not two weeks of backlog.
        let _guard = STATE_LOCK.lock().unwrap();
        let mut read = load_read();
        if read.read_through_ms == 0 {
            read.read_through_ms = now_ms() - FIRST_UNREAD_WINDOW_MS;
            save_read(&read)?;
        }
    } else if !report.added.is_empty() {
        notify_ops(app, &report.added);
    }
    let _ = app.emit(DEVTRACKER_CHANGED_EVENT, ());
    fill_context(app, &spectrum).await;
    Ok(())
}

/// Resolve what the newest shown posts answer, a few per poll (each costs
/// one or two paced requests), so quotes fill in over a few polls after a
/// backfill. Best effort: a failure waits for the next poll.
async fn fill_context(app: &AppHandle, spectrum: &svc_spectrum::SpectrumService) {
    let Some(feed) = spectrum.cached_feed() else {
        return;
    };
    let config = load_config();
    let keys: Vec<(PostKind, String)> = feed
        .posts
        .iter()
        .filter(|p| !p.context_checked && mod_devtracker::decide(p, &config).shown)
        .take(CONTEXT_PER_POLL)
        .map(|p| (p.kind, p.id.clone()))
        .collect();
    drop(feed);
    match spectrum
        .fill_context(crate::dossier::GRPC_USER_AGENT, &keys)
        .await
    {
        Ok(0) => {}
        Ok(n) => {
            tracing::info!(posts = n, "dev tracker context resolved");
            let _ = app.emit(DEVTRACKER_CHANGED_EVENT, ());
        }
        Err(e) => tracing::info!("dev tracker context: {e}"),
    }
}

/// Raise one notification for the new posts with a strong ops signal.
fn notify_ops(app: &AppHandle, added: &[FeedPost]) {
    let config = load_config();
    if !config.notify_ops {
        return;
    }
    let ops: Vec<&FeedPost> = added
        .iter()
        .filter(|p| mod_devtracker::decide(p, &config).ops)
        .collect();
    let Some(first) = ops.first() else { return };
    let headline = |p: &FeedPost| match &p.thread {
        Some(t) => t.subject.clone(),
        None => {
            let text = mod_devtracker::filter::plain_text(&p.body);
            let cut: String = text.chars().take(120).collect();
            if cut.len() < text.len() {
                format!("{cut}…")
            } else {
                cut
            }
        }
    };
    let title = if ops.len() == 1 {
        format!("{} posted", first.author.nickname)
    } else {
        format!("{} new dev posts", ops.len())
    };
    notify(
        app,
        Notification::info(title)
            .with_body(headline(first))
            .with_action("View", "/devtracker")
            .with_source(MODULE_ID),
    );
}

/// The background poller: every [`POLL_INTERVAL`] (or when nudged), poll if
/// the module is enabled and the gate is open. Silent when off — that is the
/// common case and not an error.
pub fn spawn_poller(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(STARTUP_DELAY).await;
        loop {
            let ready = {
                let state = app.state::<AppState>();
                module_enabled(&state) && state.require_unofficial(MODULE_ID).is_ok()
            };
            if ready && let Err(e) = poll(&app).await {
                tracing::info!("dev tracker poll failed: {e}");
            }
            tokio::select! {
                _ = tokio::time::sleep(POLL_INTERVAL) => {}
                _ = WAKE.notified() => {}
            }
        }
    });
}
