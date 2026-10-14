//! Install-watcher wiring — svc-discovery's settled [`InstallEvent`]s onto
//! the [`bus`](crate::bus), plus the shell's own reactions.
//!
//! Shell reactions to `InstallChanged` (a game patch finished downloading):
//! 1. Evict the in-memory cooked bundles. Bundles are keyed by build, so
//!    the new build would never be served the old data anyway — this just
//!    frees the old build's memory (the rest reload in under a second).
//! 2. Refresh the shared install mirror (build ids changed).
//! 3. Tell the user (tray-resident app — a toast is the whole UI).
//! 4. Emit `data:changed` so open catalog pages re-fetch statuses.
//! 5. Re-run the startup-warm path — it re-checks the `auto_load_game_data`
//!    setting and re-cooks the default channel iff its snapshot is stale,
//!    which is exactly the post-patch situation. Honors "parse only on
//!    build change" (CLAUDE.md): the parse spike is triggered by a build change, never a timer.
//!
//! Future consumers (mod-langpatch's re-patch) subscribe to the bus
//! themselves; they don't hook in here.

use tauri::{AppHandle, Manager};

use crate::bus::BusEvent;
use crate::{AppState, data, langpatch, notify};

/// Start the watcher thread (producer) and the shell's reaction task
/// (consumer). Called once from setup.
pub fn spawn(app: &AppHandle) {
    let state = app.state::<AppState>();

    let bus = state.bus.clone();
    let handle =
        svc_discovery::watch::spawn(svc_discovery::watch::WatchConfig::default(), move |event| {
            let event = match event {
                svc_discovery::watch::InstallEvent::Changed(info) => BusEvent::InstallChanged(info),
                svc_discovery::watch::InstallEvent::Removed { channel } => {
                    BusEvent::InstallRemoved { channel }
                }
            };
            // No receivers is fine (nothing subscribed yet).
            let _ = bus.send(event);
        });
    *state.install_watch.lock().unwrap() = Some(handle);

    let mut rx = state.bus.subscribe();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(BusEvent::InstallChanged(info)) => on_install_changed(&app, info).await,
                Ok(BusEvent::InstallRemoved { channel }) => {
                    tracing::info!(channel, "install removed");
                    let _ = data::refresh_installs(&app).await;
                    data::emit_changed(&app);
                }
                // Missed events — resync from current state.
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    let _ = data::refresh_installs(&app).await;
                    data::emit_changed(&app);
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

async fn on_install_changed(app: &AppHandle, info: svc_discovery::InstallInfo) {
    tracing::info!(
        channel = %info.channel,
        version = %info.version,
        "install changed (settled)"
    );

    app.state::<AppState>().data.evict();
    let _ = data::refresh_installs(app).await;

    notify::notify(
        app,
        notify::Notification::info(format!("Star Citizen updated — {}", info.channel))
            .with_body(format!("{} detected.", info.version))
            .with_source("discovery"),
    );
    data::emit_changed(app);

    // Re-cook the default channel in the background if the user wants
    // auto-loaded game data (the warm no-ops when the snapshot is current),
    // then reconcile text patches — the post-patch re-patch, sequenced so
    // one parse serves both.
    langpatch::spawn_warm_then_reconcile(app);
}
