//! Background-activity registry — what the app is doing right now, for the
//! notification center's "Now" section, the inline surfaces (catalog card,
//! langpatch install row, the data banner) and the tray tooltip.
//!
//! Activities are the mutable sibling of notifications: a notification is an
//! immutable event in the session log, an activity is live state that changes
//! stage and then disappears. Two lists, one surface (the 2026-10-11 decision).
//! How an activity *ends* is told through notifications as before — failures
//! as warning/error, meaningful successes as quiet entries, routine ones not
//! at all — so the registry itself only knows "running" and "waiting".
//!
//! Rust-side for the same reason as [`NotifLog`](crate::notify::NotifLog): a
//! hidden webview is suspended and runs no JS (frontend rule 6). Every change
//! emits the whole (small) snapshot as `activity:changed`; the frontend also
//! re-syncs from [`current_activities`] on mount and on window focus.
//!
//! Producers hold an [`Activity`] guard. Dropping it ends the activity, so
//! an early return, an `?` or a panic can never leave one stuck at "running".

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::AppState;

/// The Tauri event the frontend store listens on. Payload: the full snapshot.
pub const ACTIVITY_EVENT: &str = "activity:changed";

/// What kind of work an activity is — inline surfaces filter on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum ActivityKind {
    /// svc-data load waterfall (snapshot load, parse, cook).
    DataLoad,
    /// mod-langpatch: pack download, re-patch, waiting for SC to exit.
    Langpatch,
    /// svc-dossier owned-blueprints fetch.
    Blueprints,
    /// svc-dossier inventory fetch (stashes, entitlements, loadouts).
    Inventory,
    /// mod-devtracker: the first fill of the staff feed (svc-spectrum).
    Devtracker,
    /// svc-patchnotes: reading RSI's patch-note pages.
    PatchNotes,
}

/// Where an activity is. `stage` is a user-facing label ("Parsing
/// DataCore…"); work without stages leaves it empty.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ActivityPhase {
    Running {
        stage: Option<String>,
    },
    /// Blocked on a condition outside the app, not on progress.
    Waiting {
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ActivityRecord {
    pub id: u32,
    pub kind: ActivityKind,
    /// Lowercase channel key the work is about ("live"), if any.
    pub channel: Option<String>,
    pub title: String,
    pub phase: ActivityPhase,
    /// Epoch milliseconds as f64 (the `NotificationRecord` convention).
    pub started: f64,
}

/// The live activities, held in `AppState`. Short by nature — a handful at
/// most — so it's a plain Vec in start order.
#[derive(Default)]
pub struct ActivityLog {
    items: Vec<ActivityRecord>,
    next_id: u32,
}

/// The current activities, oldest first — the frontend store hydrates from
/// this on mount and on window focus.
#[tauri::command]
#[specta::specta]
pub(crate) fn current_activities(state: tauri::State<'_, AppState>) -> Vec<ActivityRecord> {
    state.activities.lock().unwrap().items.clone()
}

/// A running activity. Ends when dropped.
pub struct Activity {
    app: AppHandle,
    id: u32,
}

impl Activity {
    pub fn start(
        app: &AppHandle,
        kind: ActivityKind,
        channel: Option<&str>,
        title: impl Into<String>,
    ) -> Self {
        let id = {
            let state = app.state::<AppState>();
            let mut log = state.activities.lock().unwrap();
            let id = log.next_id;
            log.next_id += 1;
            log.items.push(ActivityRecord {
                id,
                kind,
                channel: channel.map(str::to_owned),
                title: title.into(),
                phase: ActivityPhase::Running { stage: None },
                started: now_ms(),
            });
            id
        };
        publish(app);
        Self {
            app: app.clone(),
            id,
        }
    }

    /// Move to a new stage. Returns `self` so a one-stage activity reads
    /// `Activity::start(..).stage(..)`.
    pub fn stage(self, stage: impl Into<String>) -> Self {
        self.set_stage(stage);
        self
    }

    /// [`Self::stage`] through a shared reference (stage callbacks).
    pub fn set_stage(&self, stage: impl Into<String>) {
        self.set_phase(ActivityPhase::Running {
            stage: Some(stage.into()),
        });
    }

    /// Block on an outside condition ("Star Citizen is running").
    pub fn wait(self, reason: impl Into<String>) -> Self {
        self.set_phase(ActivityPhase::Waiting {
            reason: reason.into(),
        });
        self
    }

    fn set_phase(&self, phase: ActivityPhase) {
        {
            let state = self.app.state::<AppState>();
            let mut log = state.activities.lock().unwrap();
            let Some(item) = log.items.iter_mut().find(|a| a.id == self.id) else {
                return;
            };
            item.phase = phase;
        }
        publish(&self.app);
    }
}

impl Drop for Activity {
    fn drop(&mut self) {
        self.app
            .state::<AppState>()
            .activities
            .lock()
            .unwrap()
            .items
            .retain(|a| a.id != self.id);
        publish(&self.app);
    }
}

fn now_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0)
}

/// Push the snapshot to the webview (a no-op while suspended — the focus
/// re-sync covers that) and mirror it into the tray tooltip, the only signal
/// a hidden window has.
fn publish(app: &AppHandle) {
    let snapshot = app
        .state::<AppState>()
        .activities
        .lock()
        .unwrap()
        .items
        .clone();
    if let Err(e) = app.emit(ACTIVITY_EVENT, &snapshot) {
        tracing::warn!("failed to emit activity snapshot: {e}");
    }
    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_tooltip(Some(tooltip(&snapshot)));
    }
}

/// "Starlume", or "Starlume — <first activity>" with a count of the rest.
/// Windows caps tray tooltips at 127 UTF-16 units; stay well under.
fn tooltip(items: &[ActivityRecord]) -> String {
    let Some(first) = items.first() else {
        return "Starlume".into();
    };
    let detail = match &first.phase {
        ActivityPhase::Running { stage: Some(stage) } => format!("{}: {stage}", first.title),
        ActivityPhase::Running { stage: None } => first.title.clone(),
        ActivityPhase::Waiting { reason } => format!("{}: {reason}", first.title),
    };
    let mut text = format!("Starlume — {detail}");
    if items.len() > 1 {
        text.push_str(&format!(" (+{} more)", items.len() - 1));
    }
    if text.chars().count() > 120 {
        text = text.chars().take(119).collect::<String>() + "…";
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(title: &str, phase: ActivityPhase) -> ActivityRecord {
        ActivityRecord {
            id: 0,
            kind: ActivityKind::DataLoad,
            channel: None,
            title: title.into(),
            phase,
            started: 0.0,
        }
    }

    #[test]
    fn tooltip_names_the_first_activity_and_counts_the_rest() {
        assert_eq!(tooltip(&[]), "Starlume");
        let parsing = record(
            "Game data — Live",
            ActivityPhase::Running {
                stage: Some("Parsing DataCore…".into()),
            },
        );
        assert_eq!(
            tooltip(std::slice::from_ref(&parsing)),
            "Starlume — Game data — Live: Parsing DataCore…"
        );
        let waiting = record(
            "Text patching",
            ActivityPhase::Waiting {
                reason: "Star Citizen is running".into(),
            },
        );
        assert_eq!(
            tooltip(&[parsing, waiting]),
            "Starlume — Game data — Live: Parsing DataCore… (+1 more)"
        );
    }

    #[test]
    fn tooltip_stays_under_the_windows_cap() {
        let long = record(&"x".repeat(300), ActivityPhase::Running { stage: None });
        assert!(tooltip(&[long]).chars().count() <= 120);
    }
}
