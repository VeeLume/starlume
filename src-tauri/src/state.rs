//! Shared app state, managed by Tauri and reachable from every command via
//! `tauri::State<'_, AppState>`.

use std::sync::{Arc, Mutex};

use crate::activity::ActivityLog;
use crate::error::AppError;
use crate::notify::NotifLog;
use crate::settings::AppSettings;

pub struct AppState {
    /// In-memory settings snapshot; the on-disk JSON is the durable copy.
    pub settings: Mutex<AppSettings>,
    /// Nonce of an in-flight browser login, if any. Set by `login_start`,
    /// consumed by the deep-link callback.
    pub pending_login: Mutex<Option<String>>,
    /// Session notification log (Rust-side so webview suspension can't lose
    /// entries — see `notify.rs`).
    pub notif_log: Mutex<NotifLog>,
    /// Live background activities (Rust-side for the same suspension
    /// reason — see `activity.rs`).
    pub activities: Mutex<ActivityLog>,
    /// Recognized RSI accounts (durable copy: `accounts.json`).
    pub accounts: Mutex<Vec<crate::sc::RsiAccount>>,
    /// The shared SC data service (internally synchronized). `Arc` so the
    /// long-running `load` can be moved into `spawn_blocking`.
    pub data: Arc<svc_data::DataService>,
    /// Installs from the most recent scan, as svc-data refs — so data query
    /// commands resolve a channel without rescanning. Refreshed by
    /// `data_status` / `data_load` and the install watcher.
    pub installs: Mutex<Vec<svc_data::InstallRef>>,
    /// The in-process event bus (see `bus.rs`). Subscribe via
    /// `state.bus.subscribe()`.
    pub bus: crate::bus::Bus,
    /// Keeps the svc-discovery watcher thread alive for the app's lifetime
    /// (set once in setup; dropping it stops the thread).
    pub install_watch: Mutex<Option<svc_discovery::watch::WatchHandle>>,
    /// Account-data (gRPC) service + disk cache. Reads are ToS-grey — every
    /// network method is gated by `require_unofficial` at the call site.
    pub dossier: Arc<svc_dossier::DossierService>,
    /// Spectrum (the staff feed) service + disk cache + the app-wide request
    /// pace. Reads are ToS-grey — every network method is gated by
    /// `require_unofficial("devtracker")` at the call site (`devtracker.rs`).
    pub spectrum: Arc<svc_spectrum::SpectrumService>,
    /// LIVE patch notes (RSI's public patch-note pages) + disk cache. Public
    /// reads — every network method is gated by `require_online()` at the
    /// call site (`patchnotes.rs`).
    pub patchnotes: Arc<svc_patchnotes::PatchNotesService>,
}

impl AppState {
    /// Load durable state from disk. Called once, before the Tauri builder.
    pub fn load() -> Self {
        Self {
            settings: Mutex::new(AppSettings::load()),
            pending_login: Mutex::new(None),
            notif_log: Mutex::new(NotifLog::default()),
            activities: Mutex::new(ActivityLog::default()),
            accounts: Mutex::new(app_kit::load_json(&crate::sc::accounts_path())),
            data: Arc::new(svc_data::DataService::new(
                app_kit::app_data_root().join("cache"),
            )),
            installs: Mutex::new(Vec::new()),
            bus: crate::bus::new_bus(),
            install_watch: Mutex::new(None),
            dossier: Arc::new(svc_dossier::DossierService::new(
                app_kit::app_data_root().join("dossier"),
            )),
            spectrum: Arc::new(svc_spectrum::SpectrumService::new(
                app_kit::app_data_root().join("spectrum"),
            )),
            patchnotes: Arc::new(svc_patchnotes::PatchNotesService::new(
                app_kit::app_data_root().join("patchnotes"),
            )),
        }
    }

    // ── Online policy gates ─────────────────────────────────────────────
    //
    // INVARIANT (see CLAUDE.md): every outbound network call in this app —
    // shell, services, modules — passes one of these gates first. New code
    // that talks to the network without a gate call is a bug.

    /// Master online switch. `Err` when the user has turned all online
    /// features off — no Discord, no RSI fetch, no unofficial API, nothing.
    /// Sole exception: **update checks** (see CLAUDE.md — legit app
    /// function, no ToS implications).
    pub fn require_online(&self) -> Result<(), AppError> {
        if self.settings.lock().unwrap().online_enabled {
            Ok(())
        } else {
            Err(AppError::Config(
                "online features are disabled (Settings → Online)".into(),
            ))
        }
    }

    /// Gate for **unofficial API** calls — the ToS-grey surface: any RSI/CIG
    /// API Starlume is not an official client of. Today that is CIG's
    /// game-services gRPC backend (svc-dossier) and RSI's Spectrum REST API
    /// (svc-spectrum). Requires, in order: the online master switch, the
    /// unofficial-API master switch (opt-in, consent-gated; persisted as
    /// `grpc_enabled`), and the specific feature being enabled (persisted in
    /// `grpc_features`) — so users can allow e.g. blueprints but not the dev
    /// tracker. Feature ids are registered in `settings::GRPC_FEATURES`.
    pub fn require_unofficial(&self, feature: &str) -> Result<(), AppError> {
        let settings = self.settings.lock().unwrap();
        if !settings.online_enabled {
            return Err(AppError::Config(
                "online features are disabled (Settings → Online)".into(),
            ));
        }
        if !settings.grpc_enabled {
            return Err(AppError::Config(
                "unofficial API calls are disabled (Settings → Online)".into(),
            ));
        }
        if !settings.grpc_features.iter().any(|f| f == feature) {
            return Err(AppError::Config(format!(
                "unofficial API feature '{feature}' is not enabled (Settings → Online)"
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_with(settings: AppSettings) -> AppState {
        AppState {
            settings: Mutex::new(settings),
            pending_login: Mutex::new(None),
            notif_log: Mutex::new(NotifLog::default()),
            activities: Mutex::new(ActivityLog::default()),
            accounts: Mutex::new(Vec::new()),
            data: Arc::new(svc_data::DataService::new(std::env::temp_dir())),
            installs: Mutex::new(Vec::new()),
            bus: crate::bus::new_bus(),
            install_watch: Mutex::new(None),
            dossier: Arc::new(svc_dossier::DossierService::new(std::env::temp_dir())),
            spectrum: Arc::new(svc_spectrum::SpectrumService::new(std::env::temp_dir())),
            patchnotes: Arc::new(svc_patchnotes::PatchNotesService::new(std::env::temp_dir())),
        }
    }

    #[test]
    fn defaults_are_online_but_unofficial_off() {
        let state = state_with(AppSettings::default());
        assert!(state.require_online().is_ok());
        assert!(state.require_unofficial("blueprints").is_err());
    }

    #[test]
    fn unofficial_needs_master_and_feature() {
        let mut settings = AppSettings {
            grpc_enabled: true,
            ..AppSettings::default()
        };
        settings.grpc_features.push("blueprints".into());
        let state = state_with(settings);
        assert!(state.require_unofficial("blueprints").is_ok());
        // Master on, but this feature not in the allow-list.
        assert!(state.require_unofficial("missions").is_err());
        assert!(state.require_unofficial("devtracker").is_err());
    }

    #[test]
    fn spectrum_passes_the_same_gate() {
        // The dev tracker (Spectrum, not gRPC) rides the same master switch.
        let mut settings = AppSettings::default();
        settings.grpc_features.push("devtracker".into());
        let state = state_with(settings.clone());
        assert!(state.require_unofficial("devtracker").is_err());
        settings.grpc_enabled = true;
        let state = state_with(settings.clone());
        assert!(state.require_unofficial("devtracker").is_ok());
        settings.online_enabled = false;
        let state = state_with(settings);
        assert!(state.require_unofficial("devtracker").is_err());
    }

    #[test]
    fn offline_trumps_everything() {
        let mut settings = AppSettings {
            online_enabled: false,
            grpc_enabled: true,
            ..AppSettings::default()
        };
        settings.grpc_features.push("blueprints".into());
        let state = state_with(settings);
        assert!(state.require_online().is_err());
        assert!(state.require_unofficial("blueprints").is_err());
    }
}
