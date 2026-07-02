//! Shared app state, managed by Tauri and reachable from every command via
//! `tauri::State<'_, AppState>`.

use std::sync::Mutex;

use crate::settings::AppSettings;

pub struct AppState {
    /// In-memory settings snapshot; the on-disk JSON is the durable copy.
    pub settings: Mutex<AppSettings>,
    /// Nonce of an in-flight browser login, if any. Set by `login_start`,
    /// consumed by the deep-link callback.
    pub pending_login: Mutex<Option<String>>,
}

impl AppState {
    /// Load durable state from disk. Called once, before the Tauri builder.
    pub fn load() -> Self {
        Self {
            settings: Mutex::new(AppSettings::load()),
            pending_login: Mutex::new(None),
        }
    }
}
