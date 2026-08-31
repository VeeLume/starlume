//! Account-data service — read-only reads from CIG's game-services (gRPC)
//! backend via the RSI launcher session, cached on disk.
//!
//! Wraps [`sc_dossier`] (the graduated lab client). The service's job is the
//! Starlume-side concerns the raw client doesn't have: an on-disk cache so a
//! decoration survives a restart without re-hitting the ToS-grey backend, and
//! a shape the shell can mirror across IPC.
//!
//! # Online-policy contract — READ THIS
//!
//! Every method that touches the network ([`DossierService::refresh_blueprints`])
//! is ToS-grey and **must** be called only after the shell has passed the gate
//! `AppState::require_grpc("<feature>")`. This crate cannot see settings, so it
//! cannot gate itself — the gate lives at the call site, exactly like
//! mod-langpatch's pack fetch behind `require_online()`. The cached reads
//! ([`DossierService::cached_blueprints`]) touch no network and need no gate.
//!
//! # Posture
//!
//! Reads are **manual or startup-only, never polled** (the dossier posture).
//! `refresh_*` mints a fresh launcher session, does one query, and drops the
//! connection — no long-lived channel, because the ledger changes rarely and
//! the cheapest ToS-grey footprint is the fewest calls.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The owned-blueprint set — the durable, cacheable projection the tracker
/// decorates catalogs with. Each id is a holotable `blueprint_record_guid`
/// (sc-dossier's `blueprint_id` maps 1:1, validated 102/102 in the lab).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OwnedBlueprints {
    /// Owned blueprint record GUIDs. `BTreeSet` for stable serialization and
    /// O(log n) membership at the decoration site.
    pub blueprint_ids: BTreeSet<String>,
    /// When this set was fetched, Unix seconds. `0` = never (treated as no
    /// cache — see [`DossierService::cached_blueprints`]).
    pub fetched_at: i64,
}

/// Account-data reads + their on-disk cache.
///
/// One cache per install namespace (the launcher has a single logged-in
/// account, so a single file suffices; multi-account keying is deferred until
/// a consumer needs it — the same call the accounts framework makes).
pub struct DossierService {
    cache_root: PathBuf,
}

impl DossierService {
    /// `cache_root` is where the cache files live — the shell passes a
    /// subdirectory of `app_kit::app_data_root()`.
    pub fn new(cache_root: PathBuf) -> Self {
        Self { cache_root }
    }

    fn blueprints_path(&self) -> PathBuf {
        self.cache_root.join("blueprints.json")
    }

    /// The last-fetched owned-blueprint set from disk, or `None` when nothing
    /// has ever been fetched. Cheap, no network — safe to call ungated on
    /// startup so a decoration renders before (or without) a refresh.
    pub fn cached_blueprints(&self) -> Option<OwnedBlueprints> {
        let owned: OwnedBlueprints = app_kit::load_json(&self.blueprints_path());
        (owned.fetched_at != 0).then_some(owned)
    }

    /// Fetch the owned-blueprint set live and cache it.
    ///
    /// **Gated:** the caller must have passed `require_grpc("blueprints")`
    /// first (see the module contract). Mints a launcher session, queries
    /// `BlueprintLibraryService`, drops the connection. `user_agent` is the
    /// gRPC user-agent the shell supplies.
    ///
    /// Errors propagate from [`sc_dossier`] — a stale/absent launcher session
    /// surfaces as [`sc_dossier::Error::Mint`], which the shell renders as a
    /// "sign in to the RSI launcher" hint.
    pub async fn refresh_blueprints(&self, user_agent: &str) -> anyhow::Result<OwnedBlueprints> {
        let dossier = sc_dossier::Dossier::from_launcher(user_agent).await?;
        let blueprints = dossier.owned_blueprints().await?;
        let owned = OwnedBlueprints {
            blueprint_ids: blueprints.into_iter().map(|b| b.blueprint_id).collect(),
            fetched_at: chrono::Utc::now().timestamp(),
        };
        write_cache(&self.blueprints_path(), &owned);
        tracing::info!(
            count = owned.blueprint_ids.len(),
            "refreshed owned blueprints"
        );
        Ok(owned)
    }

    /// Drop the cached blueprint set (sign-out / account switch). No network.
    pub fn clear_blueprints(&self) {
        let path = self.blueprints_path();
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// Persist a cache blob; a write failure is logged, not fatal (the live value
/// is already in hand — a stale cache is a next-launch problem, not this one).
fn write_cache<T: Serialize>(path: &Path, value: &T) {
    if let Err(e) = app_kit::save_json(path, value) {
        tracing::warn!("failed to write dossier cache to {}: {e}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_roundtrips_and_absent_reads_none() {
        let dir = tempfile::tempdir().unwrap();
        let svc = DossierService::new(dir.path().to_path_buf());

        // Nothing fetched yet.
        assert!(svc.cached_blueprints().is_none());

        // Write a set directly (refresh needs a live launcher, not available
        // in tests) and read it back.
        let owned = OwnedBlueprints {
            blueprint_ids: ["a", "b", "c"].iter().map(|s| s.to_string()).collect(),
            fetched_at: 1_700_000_000,
        };
        app_kit::save_json(&svc.blueprints_path(), &owned).unwrap();

        let read = svc.cached_blueprints().expect("cached set present");
        assert_eq!(read.blueprint_ids.len(), 3);
        assert!(read.blueprint_ids.contains("b"));
        assert_eq!(read.fetched_at, 1_700_000_000);

        // Clear removes it.
        svc.clear_blueprints();
        assert!(svc.cached_blueprints().is_none());
    }

    #[test]
    fn fetched_at_zero_is_treated_as_no_cache() {
        let dir = tempfile::tempdir().unwrap();
        let svc = DossierService::new(dir.path().to_path_buf());
        // A default (empty, fetched_at 0) on disk must read as "no cache".
        app_kit::save_json(&svc.blueprints_path(), &OwnedBlueprints::default()).unwrap();
        assert!(svc.cached_blueprints().is_none());
    }
}
