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
//! Every method that touches the network ([`DossierService::refresh_blueprints`],
//! [`DossierService::refresh_inventory`]) is ToS-grey and **must** be called only after the shell has passed the gate
//! `AppState::require_grpc("<feature>")`. This crate cannot see settings, so it
//! cannot gate itself — the gate lives at the call site, exactly like
//! mod-langpatch's pack fetch behind `require_online()`. The cached reads
//! ([`DossierService::cached_blueprints`], [`DossierService::cached_inventory`])
//! touch no network and need no gate.
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

pub mod inventory;

pub use inventory::Inventory;
/// CRC32C of a lowercased item-port name — how stash trees name ports.
pub use sc_dossier::port_crc;

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
    /// "sign in to the RSI launcher" hint; a launcher too old for the mint API
    /// as [`sc_dossier::Error::LauncherOutdated`].
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

    fn inventory_path(&self) -> PathBuf {
        self.cache_root.join("inventory.json")
    }

    fn previous_inventory_path(&self) -> PathBuf {
        self.cache_root.join("inventory.prev.json")
    }

    fn inventory_archive_dir(&self) -> PathBuf {
        self.cache_root.join("inventory-archive")
    }

    /// The last-fetched inventory tier from disk, or `None` when nothing has
    /// ever been fetched. No network, no gate.
    pub fn cached_inventory(&self) -> Option<Inventory> {
        read_inventory(&self.inventory_path())
    }

    /// The fetch before the cached one — what "since the last refresh"
    /// compares against. No network, no gate.
    pub fn previous_inventory(&self) -> Option<Inventory> {
        read_inventory(&self.previous_inventory_path())
    }

    /// The archived builds' keys, the most recently fetched first. Each holds
    /// the last fetch made under that build — what "since the patch"
    /// compares against. No network, no gate.
    pub fn inventory_archive(&self) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(self.inventory_archive_dir()) else {
            return Vec::new();
        };
        let mut builds: Vec<(std::time::SystemTime, String)> = entries
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().into_string().ok()?;
                let key = name.strip_suffix(".json")?.to_string();
                let modified = e.metadata().ok()?.modified().ok()?;
                Some((modified, key))
            })
            .collect();
        builds.sort_by_key(|b| std::cmp::Reverse(b.0));
        builds.into_iter().map(|(_, key)| key).collect()
    }

    /// The last fetch made under build `key`, or `None` when the archive has
    /// none. No network, no gate.
    pub fn archived_inventory(&self, key: &str) -> Option<Inventory> {
        if !is_build_key(key) {
            return None;
        }
        read_inventory(&self.inventory_archive_dir().join(format!("{key}.json")))
    }

    /// Make `inventory` the cached fetch: the one before it becomes the
    /// previous fetch, and it replaces its build's archived fetch. The
    /// archive keeps the newest [`ARCHIVED_BUILDS`].
    fn store_inventory(&self, inventory: &Inventory) {
        let current = self.inventory_path();
        if current.exists() {
            let previous = self.previous_inventory_path();
            // Windows can't rename onto an existing file.
            let _ = std::fs::remove_file(&previous);
            if let Err(e) = std::fs::rename(&current, &previous) {
                tracing::warn!("failed to keep the previous inventory: {e}");
            }
        }
        write_cache(&current, inventory);

        let Some(build) = &inventory.build else {
            return;
        };
        if !is_build_key(&build.key) {
            tracing::warn!(key = build.key, "not archiving under an unsafe build key");
            return;
        }
        let dir = self.inventory_archive_dir();
        write_cache(&dir.join(format!("{}.json", build.key)), inventory);
        for key in self.inventory_archive().into_iter().skip(ARCHIVED_BUILDS) {
            if let Err(e) = std::fs::remove_file(dir.join(format!("{key}.json"))) {
                tracing::warn!(key, "failed to prune an archived inventory: {e}");
            }
        }
    }

    /// Fetch the inventory tier live and cache it: every stash tree and the
    /// entitlement ledger, plus the stock and saved loadouts when
    /// `with_loadouts`. One launcher session for all of it. `build` is the
    /// game build it is fetched under — the archive keeps the last fetch per
    /// build, so a patch's losses stay comparable.
    ///
    /// **Gated:** the caller must have passed `require_grpc("items")`, and
    /// `require_grpc("loadouts")` for `with_loadouts` (see the module
    /// contract). A stash that fails (the server's AQL limit on a big ship)
    /// lands in [`Inventory::failed`] instead of failing the fetch.
    pub async fn refresh_inventory(
        &self,
        user_agent: &str,
        with_loadouts: bool,
        build: Option<inventory::BuildTag>,
    ) -> anyhow::Result<Inventory> {
        let dossier = sc_dossier::Dossier::from_launcher(user_agent).await?;
        let stashes = dossier.stashes().await?;
        let entitlements = dossier.entitlements().await?;
        let loadouts = if with_loadouts {
            Some(dossier.loadouts().await?)
        } else {
            None
        };
        let inventory = Inventory {
            fetched_at: chrono::Utc::now().timestamp(),
            stashes: stashes
                .stashes
                .iter()
                .map(|s| inventory::StashRecord {
                    location: (&s.location).into(),
                    items: s.items.iter().map(Into::into).collect(),
                })
                .collect(),
            failed: stashes
                .failed
                .iter()
                .map(|(location, e)| inventory::FailedStash {
                    location: location.into(),
                    error: e.to_string(),
                })
                .collect(),
            entitlements: entitlements.iter().map(Into::into).collect(),
            loadouts: loadouts.map(|l| l.iter().map(Into::into).collect()),
            build,
        };
        self.store_inventory(&inventory);
        tracing::info!(
            stashes = inventory.stashes.len(),
            failed = inventory.failed.len(),
            entitlements = inventory.entitlements.len(),
            loadouts = inventory.loadouts.as_ref().map(Vec::len),
            build = inventory.build.as_ref().map(|b| b.version.as_str()),
            "refreshed inventory"
        );
        Ok(inventory)
    }

    /// Drop the cached inventory, the previous fetch and the archive
    /// (sign-out / account switch). No network.
    pub fn clear_inventory(&self) {
        for path in [self.inventory_path(), self.previous_inventory_path()] {
            if path.exists() {
                let _ = std::fs::remove_file(&path);
            }
        }
        let archive = self.inventory_archive_dir();
        if archive.exists() {
            let _ = std::fs::remove_dir_all(&archive);
        }
    }

    /// Drop the cached blueprint set (sign-out / account switch). No network.
    pub fn clear_blueprints(&self) {
        let path = self.blueprints_path();
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// Builds whose last inventory fetch the archive keeps, newest fetch first.
/// One fetch is ~3 MB of JSON on a large account; disk only, never resident.
const ARCHIVED_BUILDS: usize = 10;

fn read_inventory(path: &Path) -> Option<Inventory> {
    let inventory: Inventory = app_kit::load_json(path);
    (inventory.fetched_at != 0).then_some(inventory)
}

/// A build key is a file name in the archive: nothing that walks out of it.
fn is_build_key(key: &str) -> bool {
    !key.is_empty()
        && !key.starts_with('.')
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
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

    fn fetch(at: i64, build: Option<&str>) -> Inventory {
        Inventory {
            fetched_at: at,
            build: build.map(|key| inventory::BuildTag {
                version: format!("4.10.2-live.{key}"),
                key: key.into(),
            }),
            ..Inventory::default()
        }
    }

    #[test]
    fn a_refresh_keeps_the_previous_fetch_and_the_last_one_per_build() {
        let dir = tempfile::tempdir().unwrap();
        let svc = DossierService::new(dir.path().to_path_buf());

        svc.store_inventory(&fetch(1, Some("100")));
        assert!(svc.previous_inventory().is_none());
        svc.store_inventory(&fetch(2, Some("100")));
        svc.store_inventory(&fetch(3, Some("200")));

        assert_eq!(svc.cached_inventory().unwrap().fetched_at, 3);
        assert_eq!(svc.previous_inventory().unwrap().fetched_at, 2);
        // One file per build, holding the last fetch under it.
        assert_eq!(svc.inventory_archive(), ["200", "100"]);
        assert_eq!(svc.archived_inventory("100").unwrap().fetched_at, 2);

        // A fetch without a build rotates but doesn't archive.
        svc.store_inventory(&fetch(4, None));
        assert_eq!(svc.previous_inventory().unwrap().fetched_at, 3);
        assert_eq!(svc.inventory_archive().len(), 2);

        svc.clear_inventory();
        assert!(svc.cached_inventory().is_none());
        assert!(svc.previous_inventory().is_none());
        assert!(svc.inventory_archive().is_empty());
    }

    #[test]
    fn the_archive_keeps_the_newest_builds() {
        let dir = tempfile::tempdir().unwrap();
        let svc = DossierService::new(dir.path().to_path_buf());
        for n in 0..ARCHIVED_BUILDS as i64 + 2 {
            svc.store_inventory(&fetch(n + 1, Some(&format!("{}", 100 + n))));
            // The archive orders by modification time.
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let kept = svc.inventory_archive();
        assert_eq!(kept.len(), ARCHIVED_BUILDS);
        assert_eq!(kept[0], format!("{}", 100 + ARCHIVED_BUILDS + 1));
        assert!(!kept.contains(&"100".to_string()));
        assert!(!kept.contains(&"101".to_string()));
    }

    #[test]
    fn an_archive_key_cannot_walk_out_of_the_archive() {
        assert!(is_build_key("12881860"));
        assert!(is_build_key("4.10.2-live"));
        assert!(!is_build_key("../inventory"));
        assert!(!is_build_key(".."));
        assert!(!is_build_key("a/b"));
        assert!(!is_build_key(""));
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
