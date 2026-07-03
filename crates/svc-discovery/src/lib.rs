//! Install service — SC install discovery, launcher identity, and RSI
//! public-profile parsing. Ported from Hearth's `sc_loader::discover` +
//! `hearth-core::profile` (the proven implementations).
//!
//! Rules (from the workspace layering):
//! - **Standalone on sc-holotable's `installs` feature** — no svarog, no
//!   DCB. This crate must stay cheap to depend on.
//! - **No I/O beyond the launcher-store/filesystem reads sc-holotable does;
//!   no HTTP.** The profile *parser* lives here (pure, testable); the HTTP
//!   fetch lives in the shell where the online-policy gates are.
//! - **No UI types** — the shell mirrors these into specta shapes.
//!
//! Still to come (with their first consumers): the `build_manifest.id`
//! watcher → `InstallChanged` event (lands with mod-langpatch's re-patch),
//! and Hearth's rename detection (lands when accounts get real consumers).

pub mod profile;

use sc_holotable::install::{Channel, Installation};

/// One detected SC installation, flattened to plain data.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InstallInfo {
    /// Channel label, e.g. `"Live"`, `"Ptu"`, `"TechPreview"`.
    pub channel: String,
    /// Auth/services platform: `"prod"` (Live/Hotfix) or `"ptu"` (rest).
    pub platform: String,
    /// Install root on disk, e.g. `C:\Games\StarCitizen\LIVE`.
    pub directory: String,
    /// Launcher-style version label (`"4.7.2-live.11715810"`) when the
    /// launcher store provided it; the manifest version otherwise.
    pub version: String,
    /// `build_manifest.id` build id — the change-detection key for the
    /// future `InstallChanged` watcher.
    pub build_id: String,
}

/// Result of one install scan.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScScan {
    /// All valid installs, highest-priority channel first. Empty when no
    /// SC installation was found (not an error — the UI says so).
    pub installs: Vec<InstallInfo>,
    /// RSI handle from the launcher store, if signed in there.
    pub launcher_handle: Option<String>,
}

/// Scan for SC installations + the launcher identity. **Synchronous** —
/// launcher-store reads take ~50ms; callers on an async runtime wrap this
/// in `spawn_blocking`.
pub fn scan() -> anyhow::Result<ScScan> {
    let mut installs = match sc_holotable::install::discover() {
        Ok(installs) => installs,
        Err(e) => {
            // "Nothing found" is a state, not a failure — a fresh PC or a
            // machine without SC must still onboard cleanly.
            tracing::info!("SC discovery found nothing: {e}");
            Vec::new()
        }
    };
    installs.sort_by_key(|i| i.channel.priority());

    // Best-effort handle read (the Hearth pattern) — failure just means the
    // user never signed into the RSI launcher on this machine.
    let launcher_handle = match sc_holotable::install::read_identity() {
        Ok(identity) => Some(identity.handle),
        Err(e) => {
            tracing::info!("launcher identity unavailable: {e}");
            None
        }
    };

    Ok(ScScan {
        installs: installs.iter().map(flatten).collect(),
        launcher_handle,
    })
}

fn flatten(install: &Installation) -> InstallInfo {
    InstallInfo {
        channel: format!("{:?}", install.channel),
        platform: install
            .platform_id
            .clone()
            .unwrap_or_else(|| platform_for(install.channel).to_string()),
        directory: install.root.display().to_string(),
        version: install
            .launcher_version_label
            .clone()
            .unwrap_or_else(|| install.manifest.version.clone()),
        build_id: install.manifest.build_id.clone(),
    }
}

/// Channel-based fallback when the launcher store didn't provide a
/// `platform_id` (log-fallback discovery). Mirrors CIG's own mapping.
fn platform_for(channel: Channel) -> &'static str {
    match channel {
        Channel::Live | Channel::Hotfix => "prod",
        Channel::Ptu | Channel::Eptu | Channel::TechPreview => "ptu",
    }
}
