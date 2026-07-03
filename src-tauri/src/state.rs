//! Shared app state, managed by Tauri and reachable from every command via
//! `tauri::State<'_, AppState>`.

use std::sync::Mutex;

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
}

impl AppState {
    /// Load durable state from disk. Called once, before the Tauri builder.
    pub fn load() -> Self {
        Self {
            settings: Mutex::new(AppSettings::load()),
            pending_login: Mutex::new(None),
            notif_log: Mutex::new(NotifLog::default()),
        }
    }

    // ── Online policy gates ─────────────────────────────────────────────
    //
    // INVARIANT (see CLAUDE.md): every outbound network call in this app —
    // shell, services, modules — passes one of these gates first. New code
    // that talks to the network without a gate call is a bug.

    /// Master online switch. `Err` when the user has turned all online
    /// features off — no Discord, no RSI fetch, no gRPC, nothing. Sole
    /// exception: **update checks** (see CLAUDE.md — legit app function,
    /// no ToS implications).
    pub fn require_online(&self) -> Result<(), AppError> {
        if self.settings.lock().unwrap().online_enabled {
            Ok(())
        } else {
            Err(AppError::Config(
                "online features are disabled (Settings → Online)".into(),
            ))
        }
    }

    /// Gate for CIG game-services (gRPC) calls — the ToS-grey surface.
    /// Requires, in order: the online master switch, the gRPC master switch
    /// (opt-in, consent-gated), and the specific sub-feature being enabled —
    /// so users can allow e.g. blueprints but not missions.
    ///
    /// No call sites yet — the first dossier-backed feature brings them
    /// (tests below pin the semantics until then). Drop the allow with the
    /// first real caller.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn require_grpc(&self, feature: &str) -> Result<(), AppError> {
        let settings = self.settings.lock().unwrap();
        if !settings.online_enabled {
            return Err(AppError::Config(
                "online features are disabled (Settings → Online)".into(),
            ));
        }
        if !settings.grpc_enabled {
            return Err(AppError::Config(
                "game-services (gRPC) calls are disabled (Settings → Online)".into(),
            ));
        }
        if !settings.grpc_features.iter().any(|f| f == feature) {
            return Err(AppError::Config(format!(
                "gRPC feature '{feature}' is not enabled (Settings → Online)"
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
        }
    }

    #[test]
    fn defaults_are_online_but_grpc_off() {
        let state = state_with(AppSettings::default());
        assert!(state.require_online().is_ok());
        assert!(state.require_grpc("blueprints").is_err());
    }

    #[test]
    fn grpc_needs_master_and_feature() {
        let mut settings = AppSettings {
            grpc_enabled: true,
            ..AppSettings::default()
        };
        settings.grpc_features.push("blueprints".into());
        let state = state_with(settings);
        assert!(state.require_grpc("blueprints").is_ok());
        // Master on, but this feature not in the allow-list.
        assert!(state.require_grpc("missions").is_err());
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
        assert!(state.require_grpc("blueprints").is_err());
    }
}
