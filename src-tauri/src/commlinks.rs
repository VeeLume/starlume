//! Comm-links — the shell side of `svc-commlinks`: the refresh, its gate,
//! the IPC surface, and a quiet note about new comm-links.
//!
//! App framework beside the patch notes (decision 2026-10-15): public RSI
//! pages, gated by `require_online()` only, shown on the News page whatever
//! modules are enabled. The cached read touches no network.
//!
//! **Refresh cadence.** After startup, then whenever the cache is older than
//! [`MAX_AGE`] (checked hourly), on a settings change (the online switch) and
//! on demand. A refresh is two listing pages.

use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Notify;

use crate::AppState;
use crate::activity::{Activity, ActivityKind};
use crate::error::AppError;
use crate::notify::{Notification, notify};

/// Fired after a refresh. The store reloads.
pub const COMM_LINKS_CHANGED_EVENT: &str = "commlinks:changed";
/// Refresh when the cache is older than this.
const MAX_AGE: Duration = Duration::from_secs(2 * 60 * 60);
/// How often the refresher wakes to check the age.
const CHECK_EVERY: Duration = Duration::from_secs(60 * 60);
/// Delay before the first check — after the patch notes' (30 s).
const STARTUP_DELAY: Duration = Duration::from_secs(45);
/// Public pages, honest user agent.
const USER_AGENT: &str = concat!("starlume/", env!("CARGO_PKG_VERSION"));

static WAKE: LazyLock<Notify> = LazyLock::new(Notify::new);
/// The last refresh's error, for the page. Session memory.
static LAST_ERROR: Mutex<Option<String>> = Mutex::new(None);

/// One comm-link as the News page shows it.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct CommLinkView {
    /// Comm-link id (a string: specta exports no u64).
    pub id: String,
    pub url: String,
    pub title: String,
    /// `"post"` | `"video"` | `"slideshow"` | `"poll"` | another slug.
    pub kind: String,
    /// Channel slug from the URL ("transmission", "engineering", …).
    pub channel: Option<String>,
    /// Text only; may be empty.
    pub summary: String,
    pub comments: Option<u32>,
    /// When it appeared, epoch ms: first seen by Starlume, or estimated from
    /// its age on the first fill.
    pub published_ms: Option<f64>,
    /// The downloaded card image for this position on the page (wide for
    /// the newest, card for the next few, thumbnail after), as a local path
    /// for `convertFileSrc` — or a smaller size while the right one is
    /// missing. `null` = none on disk.
    pub image: Option<String>,
    /// The embedded video's own page (YouTube or Vimeo) for a video
    /// comm-link, once looked up — opens via [`comm_links_open`] like `url`.
    pub video_url: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct CommLinksOverview {
    /// The online switch — off means no refreshes (the cache still shows).
    pub online: bool,
    /// When last refreshed, epoch seconds; `null` = never.
    pub fetched_at: Option<f64>,
    pub last_error: Option<String>,
    /// Newest first.
    pub items: Vec<CommLinkView>,
}

fn kind_slug(kind: &svc_commlinks::CommLinkKind) -> String {
    use svc_commlinks::CommLinkKind as K;
    match kind {
        K::Post => "post".into(),
        K::Video => "video".into(),
        K::Slideshow => "slideshow".into(),
        K::Poll => "poll".into(),
        K::Other(s) => s.clone(),
    }
}

/// The cached comm-links. **No network, no gate.**
#[tauri::command]
#[specta::specta]
pub(crate) fn comm_links_overview(state: tauri::State<'_, AppState>) -> CommLinksOverview {
    let cache = state.commlinks.cached();
    CommLinksOverview {
        online: state.require_online().is_ok(),
        fetched_at: cache.as_ref().map(|c| c.fetched_at as f64),
        last_error: LAST_ERROR.lock().unwrap().clone(),
        items: cache
            .as_ref()
            .map(|c| {
                c.items
                    .iter()
                    .enumerate()
                    .map(|(position, i)| CommLinkView {
                        id: i.id.to_string(),
                        url: i.url.clone(),
                        title: i.title.clone(),
                        kind: kind_slug(&i.kind),
                        channel: i.channel.clone(),
                        summary: i.summary.clone(),
                        comments: i.comments,
                        published_ms: c.published(i).map(|t| (t * 1000) as f64),
                        image: state
                            .commlinks
                            .image_path(i, position)
                            .map(|p| p.to_string_lossy().into_owned()),
                        video_url: c.video(i).map(svc_commlinks::Video::watch_url),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

/// Refresh now. **Gated** by `require_online()`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn comm_links_refresh(app: AppHandle) -> Result<(), AppError> {
    refresh(&app).await
}

/// Open a comm-link, or its video, in the browser. Only URLs from the cache:
/// a comm-link on RSI's site or a video it embeds — the frontend cannot open
/// arbitrary links through this.
#[tauri::command]
#[specta::specta]
pub(crate) fn comm_links_open(app: AppHandle, url: String) -> Result<(), AppError> {
    use tauri_plugin_opener::OpenerExt;
    let cache = app.state::<AppState>().commlinks.cached();
    let page = cache
        .as_ref()
        .is_some_and(|c| c.items.iter().any(|i| i.url == url))
        && url.starts_with("https://robertsspaceindustries.com/");
    let video = cache
        .as_ref()
        .is_some_and(|c| c.videos.values().flatten().any(|v| v.watch_url() == url));
    if !page && !video {
        return Err(AppError::Internal("not a known comm-link".into()));
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| AppError::Internal(format!("could not open the browser: {e}")))
}

/// Wake the refresher (the online switch may have changed).
pub(crate) fn nudge() {
    WAKE.notify_one();
}

/// One refresh: read, merge, save; note new comm-links quietly (not the
/// first fill's backlog); emit the change. **Gated.**
async fn refresh(app: &AppHandle) -> Result<(), AppError> {
    let service = {
        let state = app.state::<AppState>();
        state.require_online()?;
        state.commlinks.clone()
    };
    let first = service.cached().is_none();
    let result = {
        let _activity = first.then(|| {
            Activity::start(app, ActivityKind::CommLinks, None, "Comm-links")
                .stage("Reading RSI's comm-links…")
        });
        service.refresh(USER_AGENT).await
    };
    let report = match result {
        Ok(report) => {
            *LAST_ERROR.lock().unwrap() = None;
            report
        }
        Err(e) => {
            let msg = format!("couldn't read RSI's comm-links: {e}");
            *LAST_ERROR.lock().unwrap() = Some(msg.clone());
            let _ = app.emit(COMM_LINKS_CHANGED_EVENT, ());
            return Err(AppError::Internal(msg));
        }
    };
    if !report.first && !report.new_items.is_empty() {
        let n = match report.new_items.as_slice() {
            [one] => Notification::info("New comm-link").with_body(one.title.clone()),
            many => Notification::info(format!("{} new comm-links", many.len()))
                .with_body(many[0].title.clone()),
        };
        // Center only: comm-links are news, not alerts.
        notify(
            app,
            n.with_action("View", "/news")
                .with_source("commlinks")
                .quiet(),
        );
    }
    let _ = app.emit(COMM_LINKS_CHANGED_EVENT, ());
    Ok(())
}

fn stale(app: &AppHandle) -> bool {
    match app.state::<AppState>().commlinks.cached() {
        None => true,
        Some(c) => chrono::Utc::now().timestamp() - c.fetched_at >= MAX_AGE.as_secs() as i64,
    }
}

/// The background refresher. Silent when offline.
///
/// Also lets the webview read the downloaded images: the asset protocol's
/// scope is empty in tauri.conf.json and gets exactly this one directory.
pub fn spawn_refresher(app: &AppHandle) {
    let images = app.state::<AppState>().commlinks.images_dir();
    let _ = std::fs::create_dir_all(&images);
    if let Err(e) = app.asset_protocol_scope().allow_directory(&images, false) {
        tracing::warn!("comm-link images not readable by the page: {e}");
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(STARTUP_DELAY).await;
        loop {
            let online = app.state::<AppState>().require_online().is_ok();
            if online
                && stale(&app)
                && let Err(e) = refresh(&app).await
            {
                tracing::info!("comm-links refresh failed: {e}");
            }
            tokio::select! {
                _ = tokio::time::sleep(CHECK_EVERY) => {}
                _ = WAKE.notified() => {}
            }
        }
    });
}
