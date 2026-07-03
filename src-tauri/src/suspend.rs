//! WebView2 suspension — lever 2 of `docs/memory.md`.
//!
//! A hidden WebView2 keeps its full ~100–200 MB until suspended; suspension
//! pauses the renderer (timers, JS) and lets Windows reclaim most of its
//! working set. Requirements and behavior (per the WebView2 docs):
//!
//! - The browser view must be **invisible** first — hiding the Win32 window
//!   alone is not enough, so we set `ICoreWebView2Controller::IsVisible =
//!   false` explicitly before `TrySuspend`.
//! - `TrySuspend` is best-effort (fails e.g. before the first navigation
//!   completes or with DevTools open) — failures are logged, never fatal.
//! - Setting `IsVisible = true` **auto-resumes** a suspended WebView, so the
//!   resume path is just visibility restoration before `window.show()`.
//!
//! While suspended, the frontend runs no JS — emitted events don't reach the
//! store. That's why the notification log lives Rust-side (`notify::NotifLog`)
//! and the frontend re-hydrates on window focus.

#[cfg(windows)]
pub fn suspend_webview(window: &tauri::WebviewWindow) {
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_3;
    use webview2_com::TrySuspendCompletedHandler;
    use windows_core::Interface;

    let result = window.with_webview(|webview| unsafe {
        let controller = webview.controller();
        if let Err(e) = controller.SetIsVisible(false) {
            tracing::warn!("suspend: SetIsVisible(false) failed: {e}");
            return;
        }
        let core = match controller.CoreWebView2() {
            Ok(core) => core,
            Err(e) => {
                tracing::warn!("suspend: CoreWebView2 unavailable: {e}");
                return;
            }
        };
        let core3: ICoreWebView2_3 = match core.cast() {
            Ok(core3) => core3,
            Err(_) => {
                tracing::info!("suspend: WebView2 runtime lacks ICoreWebView2_3 — skipping");
                return;
            }
        };
        let handler = TrySuspendCompletedHandler::create(Box::new(|error_code, is_successful| {
            match (error_code, is_successful) {
                (Ok(()), true) => tracing::debug!("webview suspended"),
                (Ok(()), false) => tracing::debug!("webview suspension declined"),
                (Err(e), _) => tracing::warn!("webview suspension failed: {e}"),
            }
            Ok(())
        }));
        if let Err(e) = core3.TrySuspend(&handler) {
            tracing::warn!("suspend: TrySuspend failed: {e}");
        }
    });
    if let Err(e) = result {
        tracing::warn!("suspend: with_webview failed: {e}");
    }
}

/// Restore visibility (which auto-resumes a suspended WebView). Call before
/// `window.show()`.
#[cfg(windows)]
pub fn resume_webview(window: &tauri::WebviewWindow) {
    let result = window.with_webview(|webview| unsafe {
        let controller = webview.controller();
        if let Err(e) = controller.SetIsVisible(true) {
            tracing::warn!("resume: SetIsVisible(true) failed: {e}");
        }
    });
    if let Err(e) = result {
        tracing::warn!("resume: with_webview failed: {e}");
    }
}

#[cfg(not(windows))]
pub fn suspend_webview(_window: &tauri::WebviewWindow) {}

#[cfg(not(windows))]
pub fn resume_webview(_window: &tauri::WebviewWindow) {}
