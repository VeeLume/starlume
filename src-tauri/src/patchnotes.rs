//! Patch notes — the shell side of `svc-patchnotes`: the refresh, its gate,
//! the IPC surface, and the "new LIVE release" notification.
//!
//! App framework (decision 2026-10-15): the LIVE notes are public RSI pages,
//! read for Home's up-to-date check and the Patch Notes page whatever modules
//! are enabled. The gate is `require_online()` — public reads, not the
//! unofficial-API opt-in. The cached read touches no network.
//!
//! **Refresh cadence.** At startup (after a delay), then whenever the cache
//! is older than [`MAX_AGE`] (checked hourly), on a settled install change,
//! on a settings change (the online switch), and on Refresh. A refresh is
//! two or three page reads; the cache is loaded per call and dropped.
//!
//! **Image.** The newest LIVE release's article card image (wide size) is
//! downloaded with the refresh into `patchnotes/images/` through
//! `svc_commlinks::media` — the comm-links' downloader — and shown on the
//! News page's LIVE card from the local file.

use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use svc_commlinks::media::{self, ImageSize, WantedImage};

use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Notify;

use crate::AppState;
use crate::activity::{Activity, ActivityKind};
use crate::bus::BusEvent;
use crate::error::AppError;
use crate::notify::{Notification, notify};

/// Fired after a refresh changed the cache. The store reloads.
pub const PATCH_NOTES_CHANGED_EVENT: &str = "patchnotes:changed";
/// Refresh when the cache is older than this.
const MAX_AGE: Duration = Duration::from_secs(6 * 60 * 60);
/// How often the refresher wakes to check the age.
const CHECK_EVERY: Duration = Duration::from_secs(60 * 60);
/// Delay before the first check — startup has heavier work first.
const STARTUP_DELAY: Duration = Duration::from_secs(30);
/// Public pages, honest user agent (the RSI profile lookup's footing).
const USER_AGENT: &str = concat!("starlume/", env!("CARGO_PKG_VERSION"));

static WAKE: LazyLock<Notify> = LazyLock::new(Notify::new);
/// The last refresh's error, for the page. Session memory.
static LAST_ERROR: Mutex<Option<String>> = Mutex::new(None);

// ── Views ───────────────────────────────────────────────────────────────────

/// One LIVE release as the page and Home show it (specta mirror of
/// `sc_dossier::patchnotes::Release`, overview fields only).
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct ReleaseView {
    /// Stable key (title + date as written).
    pub key: String,
    /// The RSI article it is in (opens via [`patch_notes_open`]).
    pub article_url: String,
    /// "Star Citizen Alpha 4.10.2 LIVE".
    pub title: String,
    /// "4.10.2" (or "4.10" as the title writes it).
    pub version: Option<String>,
    /// "LIVE".
    pub channel: Option<String>,
    /// The patch name ("RSI Discovery Month"; "Update" on point releases).
    pub name: Option<String>,
    /// ISO date (`2026-10-09`) when the banner date parsed.
    pub date: Option<String>,
    /// The banner date as written.
    pub date_text: String,
    /// "4.10.2-LIVE.12881860".
    pub build: Option<String>,
    /// "12881860".
    pub changelist: Option<String>,
    pub build_info: Vec<BuildInfoView>,
    /// One-line headline items. May be empty (then link out only).
    pub highlights: Vec<String>,
    pub bug_fixes: Option<u32>,
    pub known_issues: Option<u32>,
    /// The article's card image on disk (local path for `convertFileSrc`);
    /// only the newest LIVE release's article has one. `null` = none.
    pub image: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct BuildInfoView {
    pub label: String,
    pub value: String,
}

/// An article only listed (older than the parsed window).
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct ArticleRefView {
    pub url: String,
    pub title: String,
    pub version: Option<String>,
    /// Relative, as RSI's index shows it ("1 year ago").
    pub age_text: String,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct PatchNotesOverview {
    /// The online switch — off means no refreshes (the cache still shows).
    pub online: bool,
    /// When last refreshed, epoch seconds; `null` = never.
    pub fetched_at: Option<f64>,
    pub last_error: Option<String>,
    /// Parsed releases, newest first. `releases[0]` is what LIVE is on.
    pub releases: Vec<ReleaseView>,
    /// Older articles, listed only.
    pub older: Vec<ArticleRefView>,
}

/// Where the article images live — readable by the webview (asset scope).
fn images_dir() -> PathBuf {
    app_kit::app_data_root().join("patchnotes").join("images")
}

/// The image an article should have: the newest LIVE release's, wide.
fn wanted_image(cache: &svc_patchnotes::PatchNotesCache) -> Option<WantedImage> {
    let (article, _) = cache.latest()?;
    let thumb = cache
        .index
        .iter()
        .find(|r| r.id == article.id)?
        .thumbnail
        .as_deref()?;
    WantedImage::new(article.id, thumb, ImageSize::Wide)
}

/// An article's image on disk, if any.
fn article_image(
    cache: &svc_patchnotes::PatchNotesCache,
    article: &svc_patchnotes::PatchNoteArticle,
) -> Option<String> {
    let thumb = cache
        .index
        .iter()
        .find(|r| r.id == article.id)?
        .thumbnail
        .as_deref()?;
    media::best_on_disk(&images_dir(), article.id, thumb, ImageSize::Wide)
        .map(|p| p.to_string_lossy().into_owned())
}

fn release_view(
    article: &svc_patchnotes::PatchNoteArticle,
    r: &svc_patchnotes::Release,
    image: Option<String>,
) -> ReleaseView {
    ReleaseView {
        key: format!("{}|{}", r.title, r.date_text),
        article_url: article.url.clone(),
        title: r.title.clone(),
        version: r.version.clone(),
        channel: r.channel.clone(),
        name: r.name.clone(),
        date: r.date.map(|d| d.to_string()),
        date_text: r.date_text.clone(),
        build: r.build.as_ref().map(|b| b.raw.clone()),
        changelist: r.build.as_ref().and_then(|b| b.changelist.clone()),
        build_info: r
            .build_info
            .iter()
            .map(|(label, value)| BuildInfoView {
                label: label.clone(),
                value: value.clone(),
            })
            .collect(),
        highlights: r.highlights.clone(),
        bug_fixes: r.counts.bug_fixes,
        known_issues: r.counts.known_issues,
        image,
    }
}

/// The cached patch notes. **No network, no gate.**
#[tauri::command]
#[specta::specta]
pub(crate) fn patch_notes_overview(state: tauri::State<'_, AppState>) -> PatchNotesOverview {
    let cache = state.patchnotes.cached();
    let parsed: Vec<u64> = cache
        .as_ref()
        .map(|c| c.articles.iter().map(|a| a.id).collect())
        .unwrap_or_default();
    PatchNotesOverview {
        online: state.require_online().is_ok(),
        fetched_at: cache.as_ref().map(|c| c.fetched_at as f64),
        last_error: LAST_ERROR.lock().unwrap().clone(),
        releases: cache
            .as_ref()
            .map(|c| {
                c.releases()
                    .map(|(a, r)| release_view(a, r, article_image(c, a)))
                    .collect()
            })
            .unwrap_or_default(),
        older: cache
            .as_ref()
            .map(|c| {
                c.index
                    .iter()
                    .filter(|r| !parsed.contains(&r.id))
                    .map(|r| ArticleRefView {
                        url: r.url.clone(),
                        title: r.title.clone(),
                        version: r.version.clone(),
                        age_text: r.age_text.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

/// Refresh now. **Gated** by `require_online()`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn patch_notes_refresh(app: AppHandle) -> Result<(), AppError> {
    refresh(&app).await
}

/// Open a patch-note article in the browser. Only URLs from the cache, only
/// on RSI's site — the frontend cannot open arbitrary links through this.
#[tauri::command]
#[specta::specta]
pub(crate) fn patch_notes_open(app: AppHandle, url: String) -> Result<(), AppError> {
    use tauri_plugin_opener::OpenerExt;
    let known = app
        .state::<AppState>()
        .patchnotes
        .cached()
        .is_some_and(|c| {
            c.index.iter().any(|r| r.url == url) || c.articles.iter().any(|a| a.url == url)
        });
    if !known || !url.starts_with("https://robertsspaceindustries.com/") {
        return Err(AppError::Internal("not a known patch-note link".into()));
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| AppError::Internal(format!("could not open the browser: {e}")))
}

// ── The refresh ─────────────────────────────────────────────────────────────

/// Wake the refresher (the online switch may have changed).
pub(crate) fn nudge() {
    WAKE.notify_one();
}

/// One refresh: read, merge, save; announce new LIVE releases (not the
/// first fill's backlog); emit the change. **Gated.**
async fn refresh(app: &AppHandle) -> Result<(), AppError> {
    let service = {
        let state = app.state::<AppState>();
        state.require_online()?;
        state.patchnotes.clone()
    };
    let first = service.cached().is_none();
    let result = {
        let _activity = first.then(|| {
            Activity::start(app, ActivityKind::PatchNotes, None, "Patch notes")
                .stage("Reading RSI's patch notes…")
        });
        service.refresh(USER_AGENT).await
    };
    let report = match result {
        Ok(report) => {
            *LAST_ERROR.lock().unwrap() = None;
            // Still inside the gate: the LIVE card's image.
            if let Some(cache) = service.cached() {
                let wanted: Vec<WantedImage> = wanted_image(&cache).into_iter().collect();
                media::sync(&images_dir(), &wanted, USER_AGENT).await;
            }
            report
        }
        Err(e) => {
            let msg = format!("couldn't read RSI's patch notes: {e}");
            *LAST_ERROR.lock().unwrap() = Some(msg.clone());
            let _ = app.emit(PATCH_NOTES_CHANGED_EVENT, ());
            return Err(AppError::Internal(msg));
        }
    };
    if !report.first
        && let Some(newest) = report.new_releases.first()
    {
        let mut n = Notification::info(match (&newest.version, &newest.channel) {
            (Some(v), Some(c)) => format!("Star Citizen {v} {c} patch notes"),
            _ => newest.title.clone(),
        });
        let summary: Vec<&str> = newest
            .name
            .iter()
            .chain(newest.highlights.iter())
            .map(String::as_str)
            .take(3)
            .collect();
        if !summary.is_empty() {
            n = n.with_body(summary.join(" · "));
        }
        notify(
            app,
            n.with_action("View", "/news").with_source("patchnotes"),
        );
    }
    let _ = app.emit(PATCH_NOTES_CHANGED_EVENT, ());
    Ok(())
}

fn stale(app: &AppHandle) -> bool {
    match app.state::<AppState>().patchnotes.cached() {
        None => true,
        Some(c) => chrono::Utc::now().timestamp() - c.fetched_at >= MAX_AGE.as_secs() as i64,
    }
}

/// The background refresher. Silent when offline — that is a choice, not an
/// error.
pub fn spawn_refresher(app: &AppHandle) {
    // The webview may read the article images (the asset scope starts empty).
    let images = images_dir();
    let _ = std::fs::create_dir_all(&images);
    if let Err(e) = app.asset_protocol_scope().allow_directory(&images, false) {
        tracing::warn!("patch-note images not readable by the page: {e}");
    }
    let mut bus = app.state::<AppState>().bus.subscribe();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(STARTUP_DELAY).await;
        let mut force = false;
        loop {
            let online = app.state::<AppState>().require_online().is_ok();
            if online
                && (force || stale(&app))
                && let Err(e) = refresh(&app).await
            {
                tracing::info!("patch-notes refresh failed: {e}");
            }
            force = tokio::select! {
                _ = tokio::time::sleep(CHECK_EVERY) => false,
                _ = WAKE.notified() => false,
                event = bus.recv() => matches!(event, Ok(BusEvent::InstallChanged(_))),
            };
        }
    });
}
