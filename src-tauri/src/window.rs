//! The main window's lifecycle: built on first show, suspended when hidden,
//! closed outright after a minute hidden.
//!
//! Why close it: a suspended WebView2 gives back its working set but keeps
//! its commit (release build, 4.10.2: ~166 MB hidden; `MemoryUsageTargetLevel
//! = Low` changed nothing). With the window closed the tray app is the
//! Starlume process alone, ~11 MB commit. Rebuilding costs ~0.5 s to a
//! mounted front end, so the first minute hidden only suspends — reopening
//! right away stays instant. The frontend is disposable (CLAUDE.md, rule 6),
//! so nothing is lost but its place: the window's geometry and its route are
//! kept here and restored on rebuild (the route via [`take_restore_route`]).
//!
//! A companion start (`--minimized`) builds no window at all
//! (`create: false` in tauri.conf).

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, WebviewWindow, WebviewWindowBuilder,
};

use crate::{AppState, suspend};

const MAIN: &str = "main";

/// How long a hidden window keeps its suspended webview before it closes.
const CLOSE_HIDDEN_AFTER: Duration = Duration::from_secs(60);

/// Bumped on every show: a pending close fires only if none came since.
static SHOWS: AtomicU64 = AtomicU64::new(0);

/// Where the window was, for the next build.
#[derive(Clone, Copy)]
struct Geometry {
    position: LogicalPosition<f64>,
    size: LogicalSize<f64>,
    maximized: bool,
}

static GEOMETRY: Mutex<Option<Geometry>> = Mutex::new(None);
/// The page the closed window showed (path, query and hash).
static ROUTE: Mutex<Option<String>> = Mutex::new(None);

/// Show the main window, building it first if it is closed (or was never
/// built).
pub fn show(app: &AppHandle) {
    SHOWS.fetch_add(1, Ordering::SeqCst);
    let window = match app.get_webview_window(MAIN) {
        Some(w) => {
            // Restore browser-view visibility first: auto-resumes a
            // suspended webview (see suspend.rs).
            suspend::resume_webview(&w);
            w
        }
        None => match build(app) {
            Ok(w) => w,
            Err(e) => {
                tracing::warn!("building the main window failed: {e}");
                return;
            }
        },
    };
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
}

fn build(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == MAIN)
        .cloned()
        .unwrap_or_default();
    let mut builder = WebviewWindowBuilder::from_config(app, &config)?;
    if let Some(g) = *GEOMETRY.lock().unwrap() {
        builder = builder
            .position(g.position.x, g.position.y)
            .inner_size(g.size.width, g.size.height)
            .maximized(g.maximized);
    }
    builder.build()
}

/// Hide the main window to the tray: suspend its webview now, evict the
/// cooked SC data ("cooked data is evictable", CLAUDE.md), and close the
/// window if it is still hidden after [`CLOSE_HIDDEN_AFTER`].
pub fn hide_to_tray(window: &tauri::Window) {
    remember_geometry(window);
    let _ = window.hide();
    let app = window.app_handle().clone();
    if let Some(w) = app.get_webview_window(MAIN) {
        suspend::suspend_webview(&w);
    }
    app.state::<AppState>().data.evict();

    let shows = SHOWS.load(Ordering::SeqCst);
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(CLOSE_HIDDEN_AFTER).await;
        if SHOWS.load(Ordering::SeqCst) != shows {
            return;
        }
        let Some(w) = app.get_webview_window(MAIN) else {
            return;
        };
        if w.is_visible().unwrap_or(true) {
            return;
        }
        if let Ok(url) = w.url() {
            let route = format!(
                "{}{}{}",
                url.path(),
                url.query().map(|q| format!("?{q}")).unwrap_or_default(),
                url.fragment().map(|f| format!("#{f}")).unwrap_or_default()
            );
            *ROUTE.lock().unwrap() = Some(route);
        }
        tracing::debug!("closing the hidden main window");
        let _ = w.destroy();
    });
}

/// Keep the window's place for the next build. A minimized window reports
/// an off-screen position, so it keeps the last real one.
fn remember_geometry(window: &tauri::Window) {
    if window.is_minimized().unwrap_or(false) {
        return;
    }
    let (Ok(scale), Ok(pos), Ok(size)) = (
        window.scale_factor(),
        window.outer_position(),
        window.inner_size(),
    ) else {
        return;
    };
    *GEOMETRY.lock().unwrap() = Some(Geometry {
        position: pos.to_logical(scale),
        size: size.to_logical(scale),
        maximized: window.is_maximized().unwrap_or(false),
    });
}

/// The page a closed window showed, once: the rebuilt frontend navigates
/// back to it on mount.
#[tauri::command]
#[specta::specta]
pub fn take_restore_route() -> Option<String> {
    ROUTE.lock().unwrap().take()
}

/// When the startup update check last ran in this process.
static UPDATE_CHECKED: Mutex<Option<Instant>> = Mutex::new(None);
const UPDATE_CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

/// Whether the frontend's startup update check is due, and if so, mark it
/// done. Every window build mounts the layout, so without this a rebuilt
/// window would re-check (and re-prompt) on every reopen; the first mount
/// of a process always checks.
#[tauri::command]
#[specta::specta]
pub fn update_check_due() -> bool {
    let mut last = UPDATE_CHECKED.lock().unwrap();
    let due = last.is_none_or(|t| t.elapsed() >= UPDATE_CHECK_EVERY);
    if due {
        *last = Some(Instant::now());
    }
    due
}
