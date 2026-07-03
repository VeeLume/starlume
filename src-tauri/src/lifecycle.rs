//! Process lifecycle: plugin registration, tray, window behavior, `run()`.
//!
//! Plugin order is load-bearing: **single-instance must be registered first**
//! (its check runs before anything else initializes). Its `deep-link` feature
//! forwards a second instance's `starlume://` argv into this instance's
//! `on_open_url`, so "user clicks the auth callback while the app runs" and
//! "…while the app is closed" converge on the same handler.

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_deep_link::DeepLinkExt;

use crate::{AppState, auth, ipc, suspend};

fn show_main_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        // Restore browser-view visibility first — auto-resumes a suspended
        // webview (docs/memory.md lever 2).
        suspend::resume_webview(&w);
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Hide the main window to the tray and suspend its webview so the hidden
/// WebView2 releases its working set instead of idling at ~100+ MB.
fn hide_to_tray(window: &tauri::Window) {
    let _ = window.hide();
    if let Some(w) = window.app_handle().get_webview_window("main") {
        suspend::suspend_webview(&w);
    }
}

fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show Starlume", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().expect("bundled icon").clone())
        .tooltip("Starlume")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main_window(app),
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
                show_main_window(tray.app_handle());
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
            show_main_window(app);
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

            // Window starts hidden (tauri.conf `visible: false`) so companion
            // mode never flashes a frame; show it unless we're told not to.
            if !start_minimized {
                show_main_window(app.handle());
            } else {
                // Companion start: suspend the never-shown webview once its
                // first navigation has had time to complete (TrySuspend
                // fails before that — the delay is the cheap fix).
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    if let Some(w) = handle.get_webview_window("main")
                        && !w.is_visible().unwrap_or(true)
                    {
                        suspend::suspend_webview(&w);
                    }
                });
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
                    hide_to_tray(window);
                    api.prevent_close();
                }
            }
            // There is no "minimize requested" event with a prevent API —
            // the standard Tauri pattern is to catch the resize into the
            // minimized state and hide instead. `show_main_window`
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
                    hide_to_tray(window);
                }
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
