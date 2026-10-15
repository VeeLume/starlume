//! Process lifecycle: plugin registration, tray, window behavior, `run()`.
//!
//! Plugin order is load-bearing: **single-instance must be registered first**
//! (its check runs before anything else initializes). Its `deep-link` feature
//! forwards a second instance's `starlume://` argv into this instance's
//! `on_open_url`, so "user clicks the auth callback while the app runs" and
//! "…while the app is closed" converge on the same handler.

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_deep_link::DeepLinkExt;

use crate::{AppState, auth, ipc, langpatch, watch, window};

fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show Starlume", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    TrayIconBuilder::with_id("main")
        // Its own glyph (icons/source/tray.svg): the app icon's tile would
        // shrink to a dark square at tray size.
        .icon(tauri::image::Image::from_bytes(include_bytes!(
            "../icons/tray.png"
        ))?)
        .tooltip("Starlume")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => window::show(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                window::show(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let state = AppState::load();
    let start_minimized = std::env::args().any(|a| a == "--minimized")
        || state.settings.lock().unwrap().start_minimized;

    let mut builder = tauri::Builder::default();
    // MUST be first — see module docs. Skipped in dev profile mode: a
    // profile instance runs *beside* the default instance by design (the
    // two-account test setup; its auth uses a loopback callback, so it
    // doesn't need the deep-link forwarding either).
    if app_kit::profile().is_none() {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            // A second launch without a deep link is the user looking for the
            // window — bring it up. (Deep-link argv is forwarded separately by
            // the plugin's deep-link feature.)
            window::show(app);
        }));
    }
    builder
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .invoke_handler(ipc::ipc_builder().invoke_handler())
        .setup(move |app| {
            // Keep the TS bindings current in dev — same pattern as Hearth.
            #[cfg(debug_assertions)]
            if let Err(e) = ipc::export_bindings("../src/lib/bindings.ts") {
                tracing::warn!("bindings export failed: {e}");
            }

            // Dev builds aren't installed, so the NSIS-time scheme
            // registration never ran — register starlume:// at runtime.
            // Not in profile mode: registration is machine-global and would
            // steal the scheme from the default instance (profiles use the
            // loopback auth callback instead).
            #[cfg(debug_assertions)]
            if app_kit::profile().is_none() {
                app.deep_link().register_all()?;
            }

            let handle = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    auth::handle_deep_link(&handle, &url);
                }
            });

            build_tray(app)?;

            // Warm the game-data cache in the background (default channel,
            // setting-gated), then reconcile text patches — sequenced so one
            // parse serves both the catalogs and the langpatch derive.
            langpatch::spawn_warm_then_reconcile(app.handle());

            // Fetch owned blueprints in the background (setting + gRPC gated);
            // a change re-applies text patching so the mission text updates.
            crate::dossier::spawn_startup_fetch(app.handle());

            // Poll the dev tracker's staff feed (module + gate checked per
            // poll; idle when either is off).
            crate::devtracker::spawn_poller(app.handle());

            // Watch installs for settled build changes (30s stat-only poll)
            // → InstallChanged on the bus → cache invalidation + re-warm.
            watch::spawn(app.handle());

            // No window is built at startup (tauri.conf `create: false`): a
            // companion start stays a tray icon until the window is first
            // shown (see window.rs).
            // Built after setup returns: a webview built inside setup() in a
            // dev build stays on about:blank (its first navigation is lost).
            if !start_minimized {
                let handle = app.handle().clone();
                app.handle()
                    .run_on_main_thread(move || window::show(&handle))?;
            }

            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { api, .. } => {
                let close_to_tray = window
                    .app_handle()
                    .state::<AppState>()
                    .settings
                    .lock()
                    .unwrap()
                    .close_to_tray;
                if close_to_tray {
                    window::hide_to_tray(window);
                    api.prevent_close();
                } else {
                    // The run loop keeps the app alive with no window (see
                    // below), so closing means quitting explicitly.
                    window.app_handle().exit(0);
                }
            }
            // There is no "minimize requested" event with a prevent API —
            // the standard Tauri pattern is to catch the resize into the
            // minimized state and hide instead. `window::show`
            // unminimizes on the way back, so tray → Show restores normally.
            WindowEvent::Resized(_) => {
                let minimize_to_tray = window
                    .app_handle()
                    .state::<AppState>()
                    .settings
                    .lock()
                    .unwrap()
                    .minimize_to_tray;
                if minimize_to_tray && window.is_minimized().unwrap_or(false) {
                    window::hide_to_tray(window);
                }
            }
            _ => {}
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            // A hidden window closes after a minute (window.rs) and the tray
            // app lives on without one. An explicit exit (tray Quit, closing
            // without close-to-tray) carries a code.
            if let tauri::RunEvent::ExitRequested {
                code: None, api, ..
            } = event
            {
                api.prevent_exit();
            }
        });
}

#[cfg(test)]
mod tests {
    /// The tray glyph is decoded at startup with `?`: a broken PNG would stop
    /// the app before its window ever shows.
    #[test]
    fn tray_icon_decodes() {
        let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png")).unwrap();
        assert_eq!((icon.width(), icon.height()), (32, 32));
    }
}
