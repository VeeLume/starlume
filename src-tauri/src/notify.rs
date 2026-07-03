//! Global notification funnel (backend side) — the Hearth pattern with two
//! Starlume twists:
//!
//! - a **`source`** field naming the module/service that raised it (the
//!   notification center can group/filter by it as modules land);
//! - a **native OS toast fallback**: Starlume is a resident tray app, so when
//!   the main window is hidden the in-app toast stack is invisible —
//!   [`notify`] then also raises a Windows notification (user-toggleable via
//!   `AppSettings::native_notifications`). This is a *display* fallback, not
//!   a network call — it needs no online-policy gate.
//!
//! Backend code that wants to surface a message builds a [`Notification`]
//! and emits it with [`notify`]. The frontend has a matching single funnel
//! (`src/lib/state/notifications.svelte.ts`): every `notify` event lands in
//! one store that drives both the transient toast stack and the persistent
//! notification center.
//!
//! Notifications are **session-memory only** — no DB, no persistence across
//! a restart (the Hearth alpha decision, kept). The session log lives
//! **Rust-side** ([`NotifLog`], a bounded ring buffer) because the webview is
//! suspended while hidden (see [`crate::suspend`]) and runs no JS — events
//! emitted during suspension would otherwise be lost. The frontend hydrates
//! from [`recent_notifications`] on mount and on window focus.

use std::collections::VecDeque;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::AppState;

/// The Tauri event name the frontend store listens on.
pub const NOTIFY_EVENT: &str = "notify";

/// Severity of a notification. Drives colour and the toast auto-dismiss
/// policy on the frontend: `info` / `success` fade, `warning` / `error`
/// persist until dismissed.
#[derive(Debug, Clone, Copy, Serialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum NotifLevel {
    Info,
    Success,
    Warning,
    Error,
}

/// An optional action on a notification — a labelled link the frontend turns
/// into a "View →" affordance that navigates to `href` (an in-app route).
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct NotifAction {
    pub label: String,
    pub href: String,
}

/// A user-facing notification. Built with the [`Notification::success`] /
/// `warning` / … constructors and the `with_*` builders.
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct Notification {
    pub level: NotifLevel,
    pub title: String,
    pub body: Option<String>,
    pub action: Option<NotifAction>,
    /// Module/service id that raised this ("auth", "langpatch", …).
    pub source: Option<String>,
}

impl Notification {
    pub fn new(level: NotifLevel, title: impl Into<String>) -> Self {
        Self {
            level,
            title: title.into(),
            body: None,
            action: None,
            source: None,
        }
    }

    pub fn info(title: impl Into<String>) -> Self {
        Self::new(NotifLevel::Info, title)
    }
    pub fn success(title: impl Into<String>) -> Self {
        Self::new(NotifLevel::Success, title)
    }
    pub fn warning(title: impl Into<String>) -> Self {
        Self::new(NotifLevel::Warning, title)
    }
    pub fn error(title: impl Into<String>) -> Self {
        Self::new(NotifLevel::Error, title)
    }

    /// Attach a secondary detail line.
    pub fn with_body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// Attach a "View →"-style action that navigates to an in-app route.
    pub fn with_action(mut self, label: impl Into<String>, href: impl Into<String>) -> Self {
        self.action = Some(NotifAction {
            label: label.into(),
            href: href.into(),
        });
        self
    }

    /// Name the module/service raising this notification.
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }
}

/// A notification as stored/emitted: the payload plus backend-assigned
/// identity. `ts` is epoch milliseconds as f64 (JS-friendly; u64 would need
/// BigInt handling in the TS export).
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct NotificationRecord {
    pub id: u32,
    pub ts: f64,
    #[serde(flatten)]
    pub payload: Notification,
}

/// Bounded session ring buffer of notifications, held in `AppState`.
pub struct NotifLog {
    items: VecDeque<NotificationRecord>,
    next_id: u32,
}

/// Keep the in-memory log bounded so a long session can't grow without limit
/// (matches the frontend store's cap).
const MAX_ITEMS: usize = 100;

impl Default for NotifLog {
    fn default() -> Self {
        Self {
            items: VecDeque::with_capacity(MAX_ITEMS),
            next_id: 0,
        }
    }
}

impl NotifLog {
    fn push(&mut self, n: Notification) -> NotificationRecord {
        let record = NotificationRecord {
            id: self.next_id,
            ts: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as f64)
                .unwrap_or(0.0),
            payload: n,
        };
        self.next_id += 1;
        if self.items.len() == MAX_ITEMS {
            self.items.pop_front();
        }
        self.items.push_back(record.clone());
        record
    }
}

/// The session backlog, newest first — the frontend store hydrates from this
/// on mount and on window focus (catching up on anything raised while the
/// webview was suspended).
#[tauri::command]
#[specta::specta]
pub(crate) fn recent_notifications(state: tauri::State<'_, AppState>) -> Vec<NotificationRecord> {
    state
        .notif_log
        .lock()
        .unwrap()
        .items
        .iter()
        .rev()
        .cloned()
        .collect()
}

/// Emit a notification. Best-effort on every path: a failed emit/toast is
/// logged, never propagated — a missed notification must not break the caller.
pub fn notify(app: &AppHandle, n: Notification) {
    // Record in the Rust-side session log first — this is the source of
    // truth the frontend re-hydrates from after webview suspension.
    let record = app.state::<AppState>().notif_log.lock().unwrap().push(n);

    // In-app surfaces (toast stack + notification center) — a no-op while
    // the webview is suspended; the focus re-sync covers that window.
    if let Err(e) = app.emit(NOTIFY_EVENT, record.clone()) {
        tracing::warn!("failed to emit notification: {e}");
    }
    let n = record.payload;

    // Native OS toast when nobody can see the in-app one (window hidden or
    // minimized — companion mode). User-toggleable, default on.
    let window_visible = app
        .get_webview_window("main")
        .map(|w| w.is_visible().unwrap_or(false) && !w.is_minimized().unwrap_or(false))
        .unwrap_or(false);
    if window_visible {
        return;
    }
    let native_enabled = app
        .state::<AppState>()
        .settings
        .lock()
        .unwrap()
        .native_notifications;
    if !native_enabled {
        return;
    }
    let mut builder = app.notification().builder().title(&n.title);
    if let Some(body) = &n.body {
        builder = builder.body(body);
    }
    if let Err(e) = builder.show() {
        tracing::warn!("failed to show native notification: {e}");
    }
}
