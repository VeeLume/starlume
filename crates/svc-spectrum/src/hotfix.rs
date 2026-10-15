//! Hotfix Central — the pinned Patch Notes thread CIG edits with each
//! hotfix (one per major version), read through
//! [`sc_dossier::spectrum::Spectrum::hotfix_central`] and cached on disk.
//!
//! Hotfixes get no patch-note thread of their own, so this is where the
//! test-channel card learns whether the HOTFIX channel is open (the
//! "Status: Hotfix Channel Currently OPEN/CLOSED" line) and what a hotfix
//! contains — the part many players never find.
//!
//! **Gated** like the feed: [`SpectrumService::refresh_hotfix`] needs
//! `require_unofficial("devtracker")` first; [`SpectrumService::cached_hotfix`]
//! is a local read.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub use sc_dossier::spectrum::{HotfixCentral, HotfixEntry, HotfixItem, HotfixStatus};

use crate::{Result, SpectrumService, launcher_client};

/// The pacing slot (the call's first request; the second follows at once).
const HOTFIX: &str = "/forum/channel/threads";

/// What is cached on disk.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HotfixCache {
    /// `None` = no Hotfix Central pinned when last read.
    pub central: Option<HotfixCentral>,
    /// When last read, Unix seconds. `0` = never.
    pub fetched_at: i64,
}

impl SpectrumService {
    fn hotfix_path(&self) -> PathBuf {
        self.cache_root.join("hotfix.json")
    }

    /// The cached Hotfix Central, or `None` when never read. No network.
    pub fn cached_hotfix(&self) -> Option<HotfixCache> {
        let cache: HotfixCache = app_kit::load_json(&self.hotfix_path());
        (cache.fetched_at != 0).then_some(cache)
    }

    /// Read Hotfix Central (two paced requests), cache it, and return the
    /// previous and the new cache so the caller can tell what changed.
    ///
    /// **Gated:** the caller must have passed `require_unofficial("devtracker")`.
    pub async fn refresh_hotfix(
        &self,
        user_agent: &str,
    ) -> Result<(Option<HotfixCache>, HotfixCache)> {
        let client = launcher_client(user_agent).await?;
        let central = self.paced(HOTFIX, || client.hotfix_central()).await?;
        let previous = self.cached_hotfix();
        let cache = HotfixCache {
            central,
            fetched_at: chrono::Utc::now().timestamp(),
        };
        if let Err(e) = app_kit::save_json(&self.hotfix_path(), &cache) {
            tracing::warn!("failed to write the Hotfix Central cache: {e}");
        }
        Ok((previous, cache))
    }
}
