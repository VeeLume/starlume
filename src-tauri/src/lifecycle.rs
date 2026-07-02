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

use crate::{AppState, auth, ipc};

fn show_main_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
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

    tauri::Builder::default()
        // MUST be first — see module docs.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            // A second launch without a deep link is the user looking for the
            // window — bring it up. (Deep-link argv is forwarded separately by
            // the plugin's deep-link feature.)
            show_main_window(app);
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
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
            #[cfg(debug_assertions)]
            app.deep_link().register_all()?;

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
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let close_to_tray = window
                    .app_handle()
                    .state::<AppState>()
                    .settings
                    .lock()
                    .unwrap()
                    .close_to_tray;
                if close_to_tray {
                    let _ = window.hide();
                    api.prevent_close();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
