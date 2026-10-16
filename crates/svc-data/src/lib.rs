//! Data service — ONE DataCore parse + cooked-snapshot cache per build,
//! shared by every module (this is what kills the 30s-per-app parse of the
//! separate-apps era). The sc-holotable umbrella pin with the `foundations`
//! feature lands here, nowhere else.
//!
//! Rules (from the workspace layering):
//! - **Synchronous** — no tokio, no Tauri, no specta. The shell wraps calls
//!   in `spawn_blocking` and mirrors the plain types into specta views
//!   (the svc-discovery pattern).
//! - **Memory discipline** (CLAUDE.md "Memory"): the raw `Datacore` never
//!   outlives the loader thread — parse → cook → drop, only the cooked
//!   [`CookedData`] crosses back. Cooked indices are evictable
//!   ([`DataService::evict`]) and reload from the processed snapshot in
//!   under a second ([`DataService::get_or_reload_fast`], which never
//!   parses). Parses happen only on explicit [`DataService::load`] calls —
//!   never on a timer.
//!
//! Planned extension (deferred until modules exist to hold leases): the
//! lease model (CLAUDE.md "Memory") — modules `acquire()` the data they need and
//! eviction waits for the lease count to reach zero. Today's consumers are
//! per-request IPC queries, so drop-on-hide + fast reload covers them; a
//! resident module (e.g. mod-cargo's overlay) brings the lease API with it.
//!
//! # Load waterfall (inside [`DataService::load`])
//!
//! 1. **Processed snapshot** (`foundations.cook`) — deserialize the cooked
//!    data directly. Sub-second, no parsing.
//! 2. **Raw extract snapshot** (`extract.snap`) — hydrate captured DCB +
//!    `global.ini` bytes into a live `Datacore`, cook, persist a fresh
//!    `foundations.cook`. Skips p4k reads; still pays the DCB parse.
//! 3. **Live parse** — open `Data.p4k`, extract, parse, cook. Persists both
//!    snapshots. The cold path (~30s).
//!
//! Snapshot failures are non-fatal and fall through (see [`cache`]); only a
//! live-parse failure propagates.
//!
//! # Stack-size workaround (Windows)
//!
//! The generated record decoder's match arms are deep enough to overflow
//! the default thread stack, so the waterfall runs on a dedicated thread
//! with an explicit 32 MiB stack ([`LOADER_STACK_SIZE`]) — the pattern
//! proven in Hearth / sc-langpatch / bulkhead.

pub mod builds;
pub mod cooked;
pub mod crafting;
pub mod legality;
pub mod mining;
pub mod missions;
pub mod places;
pub mod weapons;

mod cache;
mod crimestat;

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Instant;

use anyhow::{Context, anyhow};
use sc_holotable::asset::{AssetConfig, AssetData, AssetSource, Datacore};
use sc_holotable::build_foundations;

pub use builds::{BuildMeta, RetentionPolicy, changelist, is_prod_channel, release};
pub use cooked::{
    CookedData, DATA_COOK_VERSION, ItemDetail, ItemPage, ItemQuery, ItemRow, ManufacturerRow,
    ResourceRow, STARLUME_COOK_REV,
};
pub use crafting::{CraftingIndex, CraftingProperty};
pub use legality::{JurisdictionRef, LegalityEntry, LegalityKind};
pub use mining::{ElementEntry, GadgetEntry, LaserEntry, MiningIndex, ModuleEntry, RulesetEntry};
pub use missions::{
    BpPoolEntry, BpPoolReward, CargoLeg, CrimestatRisk, EncounterKind, EventLadder, EventReward,
    ItemReward, MissionCategory, MissionDifficulty, MissionEncounter, MissionEntry, MissionFaction,
    MissionPayout, MissionPlace, MissionPoolFacts, MissionRef, MissionRegion, MissionWave,
    RepRequirement, RepReward, ScripReward, ShipSlot, SpawnSlot,
};
pub use weapons::{DamageBreakdown, MissileEntry, ShipWeaponEntry, TrackingEntry, WeaponsIndex};

// Selected sc-holotable surface for CookedData consumers (module crates
// depend on svc-data ONLY — the one-pin rule — so what they need to walk
// the snapshot is re-exported here rather than pinned again).
pub use sc_holotable::asset::{Guid, LocaleKey, LocaleMap, RecordCollection, class_crc};
pub use sc_holotable::locations::{Location, LocationKind, Locations};
// The rock generator walks providers, their places and geometry tags.
/// sc-gathering's full surface (provider groups and entries) for the generator.
pub use sc_holotable::gathering;
pub use sc_holotable::gathering::{GatheringMode, ProviderLocations, Providers};
pub use sc_holotable::resources::Resources;
pub use sc_holotable::tags::Tags;

/// Stack size for the loader thread — see the module docs.
pub const LOADER_STACK_SIZE: usize = 32 * 1024 * 1024;

/// What svc-data needs to know about one SC install. The shell builds this
/// from `svc_discovery::InstallInfo` (svc-data stays decoupled from the
/// discovery crate — it only ever touches the p4k it's pointed at).
#[derive(Debug, Clone)]
pub struct InstallRef {
    /// Lowercase channel label (e.g. `"live"`). The cache is keyed by
    /// build ([`InstallRef::build_key`]), not by this.
    pub channel_key: String,
    /// Full path to the install's `Data.p4k`.
    pub p4k_path: PathBuf,
    /// The install's staleness key (`build_manifest.id`, or the version
    /// label when that is the literal `"None"`).
    pub build_id: String,
    /// Version label (`4.10.2-live.12881860`) — release + changelist.
    pub version: String,
}

/// Which cache tier a load will (likely) use. Prediction is by file
/// existence — a stale snapshot still falls through to a slower tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadTier {
    /// Processed snapshot present — sub-second.
    Processed,
    /// Only the raw extract snapshot — re-parse, ~15-20s.
    Extract,
    /// Nothing cached — live `Data.p4k` parse, ~30s+.
    Live,
}

/// Coarse progress stages for the load waterfall, forwarded to the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Checking / deserializing cached snapshots.
    LoadingSnapshot,
    /// Opening the live `Data.p4k`.
    OpeningP4k,
    /// Extracting asset data (DCB bytes + locale).
    Extracting,
    /// Parsing the DataCore (the long pole).
    Parsing,
    /// Building the cooked indices.
    Cooking,
    /// Persisting snapshots.
    Saving,
}

/// Cache/load state for one install, as shown on the Game Data page.
#[derive(Debug, Clone)]
pub struct DataStatus {
    /// An in-memory cooked bundle is present for this channel + build.
    pub loaded: bool,
    pub predicted_tier: LoadTier,
    /// `Some` when loaded.
    pub item_count: Option<usize>,
    pub resource_count: Option<usize>,
    pub mission_count: Option<usize>,
}

/// One build directory as the Settings page lists it.
#[derive(Debug, Clone)]
pub struct BuildInfo {
    pub meta: BuildMeta,
    /// Installed on some channel right now (never pruned).
    pub current: bool,
    /// Bytes on disk (snapshots + metadata).
    pub bytes: u64,
    /// The raw extract snapshot is present — what an archive keeps.
    pub has_extract: bool,
    /// The cooked snapshot is present — current builds only, after a prune.
    pub has_processed: bool,
}

/// What [`DataService::prune`] did.
#[derive(Debug, Clone, Default)]
pub struct PruneReport {
    /// Build keys whose directories were deleted.
    pub deleted: Vec<String>,
    /// Archived builds whose cooked snapshot was dropped (extract kept).
    pub slimmed: usize,
}

/// The shared data service: cooked bundles per build + the build-keyed
/// snapshot cache and its archive. Internally synchronized — hold it as a
/// plain field, no outer lock.
pub struct DataService {
    cache_root: PathBuf,
    /// `build key → cooked bundle` for everything currently in memory.
    loaded: Mutex<HashMap<String, Arc<CookedData>>>,
    /// `build key → parse lock` — the single-flight guard behind
    /// [`Self::load`]. Concurrent loads of one build (two channels on the
    /// same changelist included) serialize on this and the losers pick up
    /// the winner's bundle instead of re-parsing (the "ONE DCB parse per
    /// build" invariant under concurrency; without it, every trigger
    /// landing inside the multi-minute cold-parse window paid its own full
    /// parse).
    parse_locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
    /// Build keys with a load in flight — [`Self::prune`] never touches
    /// them, whatever install list it was handed.
    loading: Mutex<HashSet<String>>,
}

impl DataService {
    /// `cache_root` is the parent of `builds/` — the shell passes
    /// `app_data_root().join("cache")`.
    pub fn new(cache_root: PathBuf) -> Self {
        Self {
            cache_root,
            loaded: Mutex::new(HashMap::new()),
            parse_locks: Mutex::new(HashMap::new()),
            loading: Mutex::new(HashSet::new()),
        }
    }

    /// The install's build directory. Migrates the per-channel layout on
    /// first sight: a legacy `cache/<channel>/` moves to the build key of
    /// the channel's current install. If it held an older build, the
    /// snapshot stamps catch that at load and it is rebuilt in place.
    fn build_dir(&self, install: &InstallRef) -> PathBuf {
        let root = builds::builds_root(&self.cache_root);
        let dir = root.join(install.build_key());
        let legacy = self.cache_root.join(&install.channel_key);
        if !dir.exists() && legacy.is_dir() {
            match std::fs::create_dir_all(&root).and_then(|()| std::fs::rename(&legacy, &dir)) {
                Ok(()) => {
                    builds::touch_meta(&dir, install);
                    tracing::info!(
                        channel = %install.channel_key,
                        build = %install.build_key(),
                        "moved per-channel cache to the build-keyed layout"
                    );
                }
                Err(e) => tracing::warn!("migrating {} failed: {e}", legacy.display()),
            }
        }
        dir
    }

    /// Cache/load state for one install. Cheap (file existence + map peek).
    pub fn status(&self, install: &InstallRef) -> DataStatus {
        let cooked = self.get(install);
        DataStatus {
            loaded: cooked.is_some(),
            predicted_tier: cache::predict_tier(&self.build_dir(install), install),
            item_count: cooked.as_ref().map(|c| c.item_count()),
            resource_count: cooked.as_ref().map(|c| c.resource_count()),
            mission_count: cooked.as_ref().map(|c| c.mission_count()),
        }
    }

    /// The in-memory bundle for an install's build, if present. Never
    /// touches disk.
    pub fn get(&self, install: &InstallRef) -> Option<Arc<CookedData>> {
        self.loaded
            .lock()
            .unwrap()
            .get(&install.build_key())
            .cloned()
    }

    /// [`Self::get`], else a tier-1-only reload (processed snapshot →
    /// memory, sub-second). **Never parses** — returns `None` when only
    /// slower tiers are available, so callers on the query path can't
    /// accidentally trigger a 30s parse ("parse only on build change", CLAUDE.md).
    pub fn get_or_reload_fast(&self, install: &InstallRef) -> Option<Arc<CookedData>> {
        if let Some(cooked) = self.get(install) {
            return Some(cooked);
        }
        let dir = self.build_dir(install);
        let cooked = Arc::new(cache::try_load_processed(&dir, install)?);
        builds::touch_meta(&dir, install);
        self.loaded
            .lock()
            .unwrap()
            .insert(install.build_key(), Arc::clone(&cooked));
        Some(cooked)
    }

    /// Run the full load waterfall for an install, on a dedicated
    /// 32 MiB-stack thread (see module docs). Blocking — the shell wraps
    /// this in `spawn_blocking`. Idempotent: if the bundle is already in
    /// memory it returns immediately.
    ///
    /// Single-flight per build: concurrent callers block until the first
    /// one's parse finishes, then share its bundle. Only the winning
    /// caller's `progress` sees stage events — waiters block silently.
    pub fn load(
        &self,
        install: &InstallRef,
        progress: impl Fn(Stage) + Send + 'static,
    ) -> anyhow::Result<Arc<CookedData>> {
        if let Some(cooked) = self.get(install) {
            return Ok(cooked);
        }
        let key = install.build_key();

        // Serialize on the build's parse lock; recover a poisoned lock
        // (a panicked winner must not brick the build for the process
        // lifetime). Entries are never removed — a handful of builds per
        // session, each entry a few bytes.
        let lock = Arc::clone(
            self.parse_locks
                .lock()
                .unwrap()
                .entry(key.clone())
                .or_default(),
        );
        let _guard = lock.lock().unwrap_or_else(|e| e.into_inner());

        // Lost the race → the winner already stored the bundle.
        if let Some(cooked) = self.get(install) {
            return Ok(cooked);
        }

        self.loading.lock().unwrap().insert(key.clone());
        let dir = self.build_dir(install);
        builds::touch_meta(&dir, install);
        let result = load_on_thread(dir, install, progress);
        self.loading.lock().unwrap().remove(&key);

        let cooked = Arc::new(result?);
        self.loaded.lock().unwrap().insert(key, Arc::clone(&cooked));
        Ok(cooked)
    }

    /// The mining places (provider ↔ place) for this install's build, read
    /// from disk per call (~48 KB; nothing stays resident). `None` until
    /// [`Self::build_mining_places`] ran for this build.
    pub fn mining_places(&self, install: &InstallRef) -> Option<ProviderLocations> {
        places::load(&self.build_dir(install), install)
    }

    /// Build and persist the mining places from the live p4k (see
    /// [`places`]). Blocking and heavy — minutes, a multi-GB peak: run it on
    /// its own thread, once per build, never alongside a parse. The build
    /// counts as loading meanwhile, so [`Self::prune`] leaves it alone.
    pub fn build_mining_places(&self, install: &InstallRef) -> anyhow::Result<()> {
        let key = install.build_key();
        self.loading.lock().unwrap().insert(key.clone());
        let dir = self.build_dir(install);
        let result = places::build(&install.p4k_path).and_then(|p| places::save(&dir, install, p));
        self.loading.lock().unwrap().remove(&key);
        result
    }

    /// Drop every in-memory bundle (window hidden — the tray-idle path; an
    /// install change). In-flight queries keep their own `Arc` clones
    /// alive; the next query reloads via [`Self::get_or_reload_fast`] in
    /// under a second.
    pub fn evict(&self) {
        let mut loaded = self.loaded.lock().unwrap();
        if !loaded.is_empty() {
            tracing::debug!(builds = loaded.len(), "evicting cooked data");
            loaded.clear();
        }
    }

    /// Delete cached snapshots — one install's build, or the whole cache
    /// (archive included) — and evict. The next load is a full live parse.
    pub fn wipe(&self, install: Option<&InstallRef>) -> std::io::Result<()> {
        match install {
            Some(install) => {
                self.loaded.lock().unwrap().remove(&install.build_key());
                remove_dir_if_present(&self.build_dir(install))
            }
            None => {
                self.evict();
                remove_dir_if_present(&self.cache_root)
            }
        }
    }

    /// Delete one archived build by key. Refuses a build an install is on
    /// (or one mid-load) — that is [`Self::wipe`]'s job, from the install's
    /// side — and any key that isn't a listed build (the key comes over
    /// IPC; it must never name an arbitrary directory).
    pub fn delete_build(&self, key: &str, current: &[InstallRef]) -> anyhow::Result<()> {
        let build = self
            .builds(current)
            .into_iter()
            .find(|b| b.meta.key == key)
            .ok_or_else(|| anyhow!("no cached build {key}"))?;
        if build.current || self.loading.lock().unwrap().contains(key) {
            return Err(anyhow!(
                "build {key} is in use by an install; wipe it from the install instead"
            ));
        }
        self.loaded.lock().unwrap().remove(key);
        remove_dir_if_present(&builds::builds_root(&self.cache_root).join(key))
            .with_context(|| format!("deleting build {key}"))?;
        tracing::info!(build = key, "deleted archived build");
        Ok(())
    }

    /// Every build on disk, newest changelist first, marked current when
    /// one of `current` is installed on it.
    pub fn builds(&self, current: &[InstallRef]) -> Vec<BuildInfo> {
        // Resolving the dirs first migrates any per-channel leftovers.
        for install in current {
            self.build_dir(install);
        }
        let current: HashSet<String> = current.iter().map(InstallRef::build_key).collect();
        let mut out: Vec<BuildInfo> = builds::list(&self.cache_root)
            .into_iter()
            .map(|(dir, meta, bytes)| BuildInfo {
                current: current.contains(&meta.key),
                bytes,
                has_extract: dir.join(cache::EXTRACT_SNAPSHOT_NAME).exists(),
                has_processed: dir.join(cache::PROCESSED_SNAPSHOT_NAME).exists(),
                meta,
            })
            .collect();
        out.sort_by_key(|b| std::cmp::Reverse(b.meta.changelist));
        out
    }

    /// Apply the archive retention ([`builds::plan_prune`]): delete the
    /// builds the policy drops, and slim the archived survivors to their
    /// extract snapshot (the cooked one is a disposable cache — and an
    /// archive without an extract has nothing worth keeping, so it goes).
    /// Builds of `current` installs and builds mid-load are never touched;
    /// archived bundles are evicted from memory.
    pub fn prune(&self, current: &[InstallRef], policy: RetentionPolicy) -> PruneReport {
        // Current installs' legacy dirs migrate first (cheap renames), so
        // only true orphans pay the snapshot read in `adopt_orphans`.
        for install in current {
            self.build_dir(install);
        }
        self.adopt_orphans();
        let builds = self.builds(current);
        let loading = self.loading.lock().unwrap().clone();
        let mut protected: BTreeSet<String> = builds
            .iter()
            .filter(|b| b.current || loading.contains(&b.meta.key))
            .map(|b| b.meta.key.clone())
            .collect();
        // Belt and braces: the newest build each installed channel has
        // used stays, even if this scan's labels didn't match it (a scan
        // that fell back to the bare manifest version has no changelist,
        // so its build key differs) — a slimmed current build means a
        // multi-minute re-parse.
        for install in current {
            if let Some(newest) = builds
                .iter()
                .filter(|b| b.meta.channels.contains(&install.channel_key))
                .max_by_key(|b| b.meta.changelist)
            {
                protected.insert(newest.meta.key.clone());
            }
        }
        let metas: Vec<BuildMeta> = builds.iter().map(|b| b.meta.clone()).collect();
        let mut drop: BTreeSet<String> = builds::plan_prune(&metas, &protected, policy)
            .into_iter()
            .collect();
        drop.extend(
            builds
                .iter()
                .filter(|b| !protected.contains(&b.meta.key) && !b.has_extract)
                .map(|b| b.meta.key.clone()),
        );

        let root = builds::builds_root(&self.cache_root);
        let mut report = PruneReport::default();
        for b in builds.iter().filter(|b| !protected.contains(&b.meta.key)) {
            let dir = root.join(&b.meta.key);
            self.loaded.lock().unwrap().remove(&b.meta.key);
            if drop.contains(&b.meta.key) {
                match remove_dir_if_present(&dir) {
                    Ok(()) => report.deleted.push(b.meta.key.clone()),
                    Err(e) => tracing::warn!("pruning build {} failed: {e}", b.meta.key),
                }
            } else if b.has_processed && cache::drop_processed(&dir).is_ok() {
                report.slimmed += 1;
            }
        }
        if !report.deleted.is_empty() || report.slimmed > 0 {
            tracing::info!(
                deleted = ?report.deleted,
                slimmed = report.slimmed,
                "pruned the build archive"
            );
        }
        report
    }
}

impl DataService {
    /// Per-channel-era directories whose channel is no longer installed
    /// never meet [`Self::build_dir`]'s migration (it runs per current
    /// install, and [`Self::builds`] has already run it). Adopt each into
    /// the archive under the build its extract snapshot was stamped with,
    /// so retention decides its fate like any other archived build; one
    /// without a readable extract holds nothing worth keeping and goes.
    /// One-time per directory — a full read of its extract snapshot.
    fn adopt_orphans(&self) {
        let Ok(entries) = std::fs::read_dir(&self.cache_root) else {
            return;
        };
        let root = builds::builds_root(&self.cache_root);
        for dir in entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
            let Some(channel) = dir.file_name().and_then(|n| n.to_str()).map(str::to_owned) else {
                continue;
            };
            if channel == builds::BUILDS_DIR {
                continue;
            }
            let Some(meta) = cache::extract_meta(&dir) else {
                tracing::info!(
                    channel,
                    "dropping legacy cache dir without a usable extract"
                );
                let _ = remove_dir_if_present(&dir);
                continue;
            };
            let install = InstallRef {
                channel_key: channel.clone(),
                p4k_path: PathBuf::new(),
                build_id: meta.build_id,
                version: meta.game_version,
            };
            let target = root.join(install.build_key());
            let adopted = if target.exists() {
                // The archive already has this build.
                remove_dir_if_present(&dir)
            } else {
                std::fs::create_dir_all(&root).and_then(|()| std::fs::rename(&dir, &target))
            };
            match adopted {
                Ok(()) => {
                    builds::touch_meta(&target, &install);
                    tracing::info!(
                        channel,
                        version = %install.version,
                        "adopted legacy cache dir into the build archive"
                    );
                }
                Err(e) => tracing::warn!("adopting {} failed: {e}", dir.display()),
            }
        }
    }
}

/// Run [`load_blocking`] on the dedicated big-stack loader thread.
fn load_on_thread(
    dir: PathBuf,
    install: &InstallRef,
    progress: impl Fn(Stage) + Send + 'static,
) -> anyhow::Result<CookedData> {
    let (tx, rx) = mpsc::channel::<anyhow::Result<CookedData>>();
    let install_for_thread = install.clone();
    std::thread::Builder::new()
        .name("starlume-data-loader".into())
        .stack_size(LOADER_STACK_SIZE)
        .spawn(move || {
            let _ = tx.send(load_blocking(&dir, &install_for_thread, &progress));
        })
        .context("spawning data-loader thread")?;
    rx.recv()
        .map_err(|_| anyhow!("data-loader thread dropped its sender"))?
}

/// Read the raw base `global.ini` bytes (UTF-16 LE as CIG ships it) from an
/// install's `Data.p4k`. A single-file archive read — seconds, no DCB parse.
/// The apply stage of text patching needs the *raw* file (line order, `,P`
/// suffixes, duplicates), which the parsed [`LocaleMap`] deliberately
/// doesn't preserve.
pub fn read_base_global_ini(p4k_path: &std::path::Path) -> anyhow::Result<Vec<u8>> {
    let assets =
        AssetSource::open(p4k_path).with_context(|| format!("opening {}", p4k_path.display()))?;
    assets
        .find_and_read(|name| {
            // Entry names come with either separator depending on the
            // archive writer — normalize before matching.
            name.to_ascii_lowercase()
                .replace('\\', "/")
                .ends_with("localization/english/global.ini")
        })
        .context("reading global.ini from p4k")?
        .map(|(_, bytes)| bytes)
        .ok_or_else(|| anyhow!("global.ini not found in {}", p4k_path.display()))
}

fn remove_dir_if_present(dir: &std::path::Path) -> std::io::Result<()> {
    match std::fs::remove_dir_all(dir) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// The waterfall body — runs on the loader thread.
fn load_blocking(
    dir: &std::path::Path,
    install: &InstallRef,
    progress: &impl Fn(Stage),
) -> anyhow::Result<CookedData> {
    let start = Instant::now();
    progress(Stage::LoadingSnapshot);

    // ── Tier 1: processed snapshot (sub-second) ───────────────────────
    if let Some(cooked) = cache::try_load_processed(dir, install) {
        tracing::info!(
            channel = %install.channel_key,
            elapsed_ms = start.elapsed().as_millis(),
            "data loaded from processed snapshot"
        );
        return Ok(cooked);
    }

    // ── Tier 2: raw extract snapshot (skip p4k reads) ─────────────────
    if let Some((asset_data, datacore)) = cache::try_load_extract(dir, install) {
        // The extract snapshot has no meshes: open the p4k for the rock
        // meshes alone (non-fatal; the generator stays empty without it).
        progress(Stage::OpeningP4k);
        let assets = AssetSource::open(&install.p4k_path)
            .inspect_err(|e| tracing::warn!("opening the p4k for the rock meshes: {e:#}"))
            .ok();
        let cooked = cook(datacore, asset_data, assets.as_ref(), progress);
        drop(assets);
        progress(Stage::Saving);
        cache::save_processed(dir, install, &cooked);
        tracing::info!(
            channel = %install.channel_key,
            elapsed_ms = start.elapsed().as_millis(),
            "data built from raw extract snapshot"
        );
        return Ok(cooked);
    }

    // ── Tier 3: live parse (cold path) ────────────────────────────────
    tracing::info!(channel = %install.channel_key, "no usable snapshot; parsing live Data.p4k");
    progress(Stage::OpeningP4k);
    let assets = AssetSource::open(&install.p4k_path)
        .with_context(|| format!("opening {}", install.p4k_path.display()))?;

    progress(Stage::Extracting);
    let asset_data =
        AssetData::extract(&assets, &AssetConfig::standard()).context("AssetData::extract")?;

    progress(Stage::Parsing);
    let datacore = Datacore::parse(&assets, &asset_data).context("Datacore::parse")?;

    // Capture the raw bytes while `assets` is still open; non-fatal.
    progress(Stage::Saving);
    cache::save_extract(dir, install, &assets);

    // The p4k stays open through the cook for the rock meshes.
    let cooked = cook(datacore, asset_data, Some(&assets), progress);
    drop(assets);
    progress(Stage::Saving);
    cache::save_processed(dir, install, &cooked);

    tracing::info!(
        channel = %install.channel_key,
        elapsed_ms = start.elapsed().as_millis(),
        "data built from live parse"
    );
    Ok(cooked)
}

/// Build the cooked bundle and **consume the Datacore** — the raw parse
/// must not outlive the cook ("no raw DataCore after the cook",
/// CLAUDE.md; the by-value
/// signature enforces it).
///
/// `assets` is the live p4k when the caller has it open: the rock meshes
/// (mining) are read from it.
fn cook(
    datacore: Datacore,
    asset_data: AssetData,
    assets: Option<&AssetSource>,
    progress: &impl Fn(Stage),
) -> CookedData {
    progress(Stage::Cooking);
    let foundations = build_foundations(&datacore);
    // Missions cook off the same parse, reusing the foundations' item /
    // resource indices (cheap relative to the parse; ~2-3s of the cook).
    let missions = missions::build_missions(
        &datacore,
        &foundations.items,
        &foundations.resources,
        &asset_data.locale,
    );
    // Legality + weapons ride the same parse window (CONTRIBUTING module rule 4:
    // reference catalogs are framework — these also feed mod-langpatch's
    // derive, so the module never needs the raw Datacore).
    let legality = legality::build_legality(&datacore, &foundations.resources);
    let weapons = weapons::build_weapons(&datacore, &foundations.items);
    let crafting = crafting::build_crafting(&datacore);
    let mining = mining::build_mining(
        &datacore,
        &foundations.items,
        &foundations.resources,
        assets,
    );
    let holotable = sc_holotable::HolotableSnapshot::from_foundations(&foundations);
    drop(datacore);
    CookedData {
        holotable,
        locale: asset_data.locale,
        missions,
        legality,
        weapons,
        crafting,
        mining,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_holotable::asset::{Guid, LocaleKey, LocaleMap, RecordPath, RecordPaths};
    use sc_holotable::items::{Item, Items};

    fn guid(b: u8) -> Guid {
        Guid::from_bytes([b; 16])
    }

    fn item(name_key: &str, item_type: &str) -> Item {
        use sc_holotable::asset::generated::{EItemSubType, EItemType};
        Item {
            name_key: Some(LocaleKey::new(name_key)),
            short_name_key: None,
            desc_key: None,
            item_type: EItemType::from_dcb_str(item_type),
            item_sub_type: EItemSubType::from_dcb_str("UNDEFINED"),
            size: 2,
            grade: 1,
        }
    }

    /// A small fake bundle: two weapons, one armor, one NOITEM marker.
    fn fake_cooked() -> CookedData {
        let mut items = Items::new();
        items.insert(guid(1), item("@item_NameLaserAlpha", "WeaponPersonal"));
        items.insert(guid(2), item("@item_NameLaserBeta", "WeaponPersonal"));
        items.insert(guid(3), item("@item_NameHelmet", "Char_Armor_Helmet"));
        items.insert(guid(4), item("@noitem_marker", "NOITEM_Player"));

        let mut locale = LocaleMap::new();
        locale.set("item_NameLaserAlpha", "Laser Alpha");
        locale.set("item_NameLaserBeta", "Laser Beta");
        locale.set("item_NameHelmet", "Sturdy Helmet");

        let mut paths = RecordPaths::new();
        paths.insert(RecordPath {
            guid: guid(1),
            name: "LaserAlpha".into(),
            struct_index: 0,
            is_main: true,
            path: "libs/foundry/records/weapons/laser_alpha.xml".into(),
        });

        CookedData {
            holotable: sc_holotable::HolotableSnapshot {
                items: Some(items),
                paths: Some(paths),
                ..Default::default()
            },
            locale,
            mining: Default::default(),
            missions: vec![fake_mission()],
            legality: vec![legality::LegalityEntry {
                resource_guid: guid(5).to_string(),
                record_name: "WiDoW".into(),
                name_key: "@items_commodities_widow".into(),
                kind: legality::LegalityKind::Drug,
                jurisdictions: vec![legality::JurisdictionRef {
                    name_key: Some("@jurisdiction_stanton".into()),
                    record_name: "Stanton".into(),
                }],
            }],
            weapons: weapons::WeaponsIndex {
                ship_weapons: vec![weapons::ShipWeaponEntry {
                    guid: guid(6).to_string(),
                    record_name: "GATS_BallisticGatling_S1".into(),
                    name_key: Some("@item_NameGATS_BallisticGatling_S1".into()),
                    desc_key: Some("@item_DescGATS_BallisticGatling_S1".into()),
                    size: 1,
                    item_sub_type: "Gun".into(),
                    damage: Some(weapons::DamageBreakdown {
                        physical: 25.0,
                        energy: 0.0,
                        distortion: 0.0,
                        thermal: 0.0,
                        biochemical: 0.0,
                        stun: 0.0,
                    }),
                    penetration_m: Some(0.5),
                    ammo_speed: Some(1200.0),
                    ammo_lifetime: Some(2.0),
                    total_ammo: Some(1500),
                    capacitor: None,
                }],
                missiles: Vec::new(),
            },
            crafting: crafting::CraftingIndex {
                properties: vec![crafting::CraftingProperty {
                    guid: guid(8).to_string(),
                    crc: sc_holotable::asset::class_crc(&guid(8)),
                    record_name: "CraftingGameplayPropertyDef.GPP_Weapon_Damage".into(),
                    name_key: "@StatName_Damage".into(),
                    unit_key: "@LOC_EMPTY".into(),
                    factor_up_is_better: Some(true),
                }],
            },
        }
    }

    /// A minimal mission entry — enough to prove the snapshot round-trip
    /// carries the mission catalog.
    fn fake_mission() -> missions::MissionEntry {
        missions::MissionEntry {
            mission_id: guid(7).to_string(),
            title: Some("Test Delivery".into()),
            title_key: Some("@mission_title_testdelivery".into()),
            debug_name: "TestDelivery_01".into(),
            description: None,
            description_key: None,
            category: None,
            faction: None,
            difficulty: None,
            payout: missions::MissionPayout {
                calculated: false,
                fixed: Some(5000),
                estimate: None,
                buy_in: 0,
                time_to_complete: 0.0,
            },
            once_only: false,
            shareable: true,
            illegal: false,
            cooldown_seconds: None,
            abandon_cooldown_seconds: None,
            scrip: Vec::new(),
            reputation: Vec::new(),
            item_rewards: Vec::new(),
            blueprint_rewards: Vec::new(),
            event_rewards: Vec::new(),
            rep_required: Vec::new(),
            chain_required: Vec::new(),
            locations: Vec::new(),
            encounters: Vec::new(),
            cargo: Vec::new(),
            placeholders: Vec::new(),
            instance_count: 3,
            facts: missions::MissionPoolFacts {
                crimestat: missions::CrimestatRisk::Moderate,
                cooldowns_mixed: true,
                ..Default::default()
            },
        }
    }

    fn install(dir_key: &str, build_id: &str) -> InstallRef {
        InstallRef {
            channel_key: dir_key.into(),
            p4k_path: PathBuf::from("Z:/nonexistent/Data.p4k"),
            build_id: build_id.into(),
            version: "4.8.0-test".into(),
        }
    }

    #[test]
    fn cooked_round_trips_through_processed_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let inst = install("live", "build-1");
        let cooked = fake_cooked();
        cache::save_processed(dir.path(), &inst, &cooked);

        let loaded = cache::try_load_processed(dir.path(), &inst).expect("round trip");
        assert_eq!(loaded.item_count(), 3); // NOITEM excluded
        assert_eq!(
            loaded.item_detail(&guid(1).to_string()).unwrap().name,
            "Laser Alpha"
        );
        // The mission catalog rides the same snapshot.
        assert_eq!(loaded.mission_count(), 1);
        let damage = loaded
            .crafting
            .by_crc(sc_holotable::asset::class_crc(&guid(8)))
            .expect("crafting property round-trips");
        assert_eq!(damage.factor_up_is_better, Some(true));
        assert_eq!(loaded.missions[0].title.as_deref(), Some("Test Delivery"));
        assert_eq!(loaded.missions[0].payout.fixed, Some(5000));
        // Rev-3 additions ride it too: loc keys + facts + legality + weapons.
        assert_eq!(
            loaded.missions[0].title_key.as_deref(),
            Some("@mission_title_testdelivery")
        );
        assert_eq!(
            loaded.missions[0].facts.crimestat,
            missions::CrimestatRisk::Moderate
        );
        assert!(loaded.missions[0].facts.cooldowns_mixed);
        assert_eq!(loaded.legality.len(), 1);
        assert_eq!(loaded.legality[0].record_name, "WiDoW");
        assert_eq!(loaded.legality[0].kind, legality::LegalityKind::Drug);
        assert_eq!(loaded.legality[0].jurisdictions[0].record_name, "Stanton");
        assert_eq!(loaded.weapons.ship_weapons.len(), 1);
        assert_eq!(loaded.weapons.ship_weapons[0].damage.unwrap().total(), 25.0);
    }

    #[test]
    fn processed_snapshot_stale_on_build_change() {
        let dir = tempfile::tempdir().unwrap();
        let inst = install("live", "build-1");
        cache::save_processed(dir.path(), &inst, &fake_cooked());

        let patched = install("live", "build-2");
        assert!(cache::try_load_processed(dir.path(), &patched).is_none());
        // The original build still loads.
        assert!(cache::try_load_processed(dir.path(), &inst).is_some());
    }

    #[test]
    fn search_filters_sorts_and_paginates() {
        let cooked = fake_cooked();

        // Empty text matches all inventory items, sorted by name.
        let all = cooked.search_items(&ItemQuery {
            limit: 10,
            ..Default::default()
        });
        assert_eq!(all.total, 3);
        assert_eq!(all.rows[0].name, "Laser Alpha");
        assert_eq!(all.rows[2].name, "Sturdy Helmet");

        // Substring match is case-insensitive.
        let lasers = cooked.search_items(&ItemQuery {
            text: "laser".into(),
            limit: 10,
            ..Default::default()
        });
        assert_eq!(lasers.total, 2);

        // Type filter.
        let armor = cooked.search_items(&ItemQuery {
            item_type: Some("Char_Armor_Helmet".into()),
            limit: 10,
            ..Default::default()
        });
        assert_eq!(armor.total, 1);
        assert_eq!(armor.rows[0].name, "Sturdy Helmet");

        // Pagination: total stays full, rows window moves.
        let page2 = cooked.search_items(&ItemQuery {
            offset: 2,
            limit: 10,
            ..Default::default()
        });
        assert_eq!(page2.total, 3);
        assert_eq!(page2.rows.len(), 1);
    }

    #[test]
    fn unresolvable_name_falls_back_to_stripped_key() {
        let mut cooked = fake_cooked();
        cooked.locale = LocaleMap::new(); // wipe the locale
        let all = cooked.search_items(&ItemQuery {
            text: "item_namehelmet".into(),
            limit: 10,
            ..Default::default()
        });
        assert_eq!(all.total, 1);
        assert_eq!(all.rows[0].name, "item_NameHelmet"); // @ stripped
    }

    #[test]
    fn item_detail_includes_record_path() {
        let cooked = fake_cooked();
        let detail = cooked.item_detail(&guid(1).to_string()).unwrap();
        assert_eq!(
            detail.record_path.as_deref(),
            Some("libs/foundry/records/weapons/laser_alpha.xml")
        );
        assert_eq!(detail.item_type, "WeaponPersonal");
        // Unknown / garbage GUIDs are None, not errors.
        assert!(cooked.item_detail(&guid(9).to_string()).is_none());
        assert!(cooked.item_detail("not-a-guid").is_none());
    }

    #[test]
    fn item_type_facets_count_inventory_items_only() {
        let cooked = fake_cooked();
        let facets = cooked.item_type_facets();
        assert_eq!(
            facets,
            vec![
                ("WeaponPersonal".to_string(), 2),
                ("Char_Armor_Helmet".to_string(), 1)
            ]
        );
    }

    #[test]
    fn predict_tier_follows_current_snapshots() {
        let dir = tempfile::tempdir().unwrap();
        let inst = install("live", "build-1");
        assert_eq!(cache::predict_tier(dir.path(), &inst), LoadTier::Live);

        // A snapshot file without a matching key is not trusted.
        std::fs::write(dir.path().join(cache::EXTRACT_SNAPSHOT_NAME), b"x").unwrap();
        assert_eq!(cache::predict_tier(dir.path(), &inst), LoadTier::Live);

        cache::save_processed(dir.path(), &inst, &fake_cooked());
        assert_eq!(cache::predict_tier(dir.path(), &inst), LoadTier::Processed);

        // An SC patch makes the processed snapshot stale → predict the parse.
        let patched = install("live", "build-2");
        assert_eq!(cache::predict_tier(dir.path(), &patched), LoadTier::Live);
    }

    #[test]
    fn concurrent_loads_single_flight() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let root = tempfile::tempdir().unwrap();
        let service = std::sync::Arc::new(DataService::new(root.path().to_path_buf()));
        let inst = install("live", "build-1");
        cache::save_processed(&service.build_dir(&inst), &inst, &fake_cooked());

        // Progress fires only inside the load waterfall — callers that
        // lose the single-flight race return the winner's bundle without
        // ever entering it, so exactly one thread reports stages.
        let reporters = std::sync::Arc::new(AtomicUsize::new(0));
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let service = std::sync::Arc::clone(&service);
                let reporters = std::sync::Arc::clone(&reporters);
                let inst = inst.clone();
                std::thread::spawn(move || {
                    let fired = std::sync::Arc::new(AtomicUsize::new(0));
                    let fired_cb = std::sync::Arc::clone(&fired);
                    let cooked = service
                        .load(&inst, move |_| {
                            fired_cb.store(1, Ordering::SeqCst);
                        })
                        .expect("load");
                    if fired.load(Ordering::SeqCst) == 1 {
                        reporters.fetch_add(1, Ordering::SeqCst);
                    }
                    assert_eq!(cooked.item_count(), 3);
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(
            reporters.load(Ordering::SeqCst),
            1,
            "exactly one caller should run the load waterfall"
        );
    }

    #[test]
    fn service_status_get_evict_wipe_lifecycle() {
        let root = tempfile::tempdir().unwrap();
        let service = DataService::new(root.path().to_path_buf());
        let inst = install("live", "build-1");

        // Nothing cached: not loaded, Live tier, fast reload declines.
        let status = service.status(&inst);
        assert!(!status.loaded);
        assert_eq!(status.predicted_tier, LoadTier::Live);
        assert!(service.get_or_reload_fast(&inst).is_none());

        // Seed a processed snapshot on disk → fast reload picks it up.
        cache::save_processed(&service.build_dir(&inst), &inst, &fake_cooked());
        let cooked = service.get_or_reload_fast(&inst).expect("tier-1 reload");
        assert_eq!(cooked.item_count(), 3);
        let status = service.status(&inst);
        assert!(status.loaded);
        assert_eq!(status.item_count, Some(3));

        // Evict drops memory but not the file; an existing Arc stays valid.
        service.evict();
        assert!(service.get(&inst).is_none());
        assert_eq!(cooked.item_count(), 3);
        assert!(service.get_or_reload_fast(&inst).is_some());

        // Wipe removes the file too.
        service.wipe(Some(&inst)).unwrap();
        assert!(service.get_or_reload_fast(&inst).is_none());
        assert_eq!(service.status(&inst).predicted_tier, LoadTier::Live);
    }

    #[test]
    fn load_returns_in_memory_bundle_without_touching_disk() {
        let root = tempfile::tempdir().unwrap();
        let service = DataService::new(root.path().to_path_buf());
        let inst = install("live", "build-1");
        cache::save_processed(&service.build_dir(&inst), &inst, &fake_cooked());

        // First load: tier 1 from disk (p4k path is bogus — proves no parse).
        let loaded = service.load(&inst, |_| {}).expect("tier-1 load");
        assert_eq!(loaded.item_count(), 3);
        // Second load: served from memory.
        let again = service.load(&inst, |_| {}).expect("memory hit");
        assert!(Arc::ptr_eq(&loaded, &again));
    }

    fn labelled(channel: &str, label: &str) -> InstallRef {
        InstallRef {
            channel_key: channel.into(),
            p4k_path: PathBuf::from("Z:/nonexistent/Data.p4k"),
            build_id: label.into(),
            version: label.into(),
        }
    }

    #[test]
    fn channels_on_one_changelist_share_a_build() {
        let root = tempfile::tempdir().unwrap();
        let service = DataService::new(root.path().to_path_buf());
        let ptu = labelled("ptu", "4.10.2-ptu.12881860");
        let live = labelled("live", "4.10.2-live.12881860");
        // Cooked while it was the PTU build…
        cache::save_processed(&service.build_dir(&ptu), &ptu, &fake_cooked());
        // …and served to Live after promotion, without a parse.
        let cooked = service.load(&live, |_| {}).expect("shared build");
        assert_eq!(cooked.item_count(), 3);
        assert!(Arc::ptr_eq(&cooked, &service.get(&ptu).unwrap()));
    }

    #[test]
    fn legacy_channel_dir_moves_into_the_build_layout() {
        let root = tempfile::tempdir().unwrap();
        let service = DataService::new(root.path().to_path_buf());
        let live = labelled("live", "4.10.2-live.12881860");
        // A per-channel-era snapshot: stamped with the full label.
        let legacy = root.path().join("live");
        let old_style = InstallRef {
            version: "4.10.2-live.12881860".into(),
            build_id: "4.10.2-live.12881860".into(),
            ..live.clone()
        };
        std::fs::create_dir_all(&legacy).unwrap();
        fake_cooked()
            .save(
                sc_holotable::asset::SnapshotMeta {
                    build_id: old_style.build_id.clone(),
                    ..cache::meta_for(&old_style)
                },
                &legacy.join(cache::PROCESSED_SNAPSHOT_NAME),
            )
            .unwrap();

        assert!(service.load(&live, |_| {}).is_ok(), "loads without a parse");
        assert!(!legacy.exists());
        let builds = service.builds(std::slice::from_ref(&live));
        assert_eq!(builds.len(), 1);
        assert_eq!(builds[0].meta.key, "12881860");
        assert!(builds[0].current);
        assert_eq!(builds[0].meta.release, "4.10.2");
    }

    #[test]
    fn legacy_snapshot_stamped_with_a_manifest_id_still_loads() {
        let root = tempfile::tempdir().unwrap();
        let service = DataService::new(root.path().to_path_buf());
        // CIG ships a real build_manifest.id again: the staleness key (and
        // the per-channel-era stamp) is that GUID, not the version label.
        let live = InstallRef {
            build_id: "711733b7-5053-4184-973d-6181e8ee9f32".into(),
            ..labelled("live", "4.10.2-live.12881860")
        };
        let legacy = root.path().join("live");
        std::fs::create_dir_all(&legacy).unwrap();
        fake_cooked()
            .save(
                sc_holotable::asset::SnapshotMeta {
                    build_id: live.build_id.clone(),
                    ..cache::meta_for(&live)
                },
                &legacy.join(cache::PROCESSED_SNAPSHOT_NAME),
            )
            .unwrap();
        // The p4k path is bogus: a load that fell through would fail.
        assert!(
            service.load(&live, |_| {}).is_ok(),
            "no re-parse after migration"
        );
    }

    #[test]
    fn prune_keeps_a_channels_newest_build_when_the_scan_label_is_off() {
        let root = tempfile::tempdir().unwrap();
        let service = DataService::new(root.path().to_path_buf());
        let live = labelled("live", "4.10.2-live.500");
        let dir = service.build_dir(&live);
        builds::touch_meta(&dir, &live);
        cache::save_processed(&dir, &live, &fake_cooked());
        std::fs::write(dir.join(cache::EXTRACT_SNAPSHOT_NAME), b"extract").unwrap();
        // A scan that fell back to the bare manifest version: same channel,
        // but its build key no longer names the build on disk.
        let flapped = InstallRef {
            version: "4.10.2".into(),
            build_id: "711733b7-guid".into(),
            ..live.clone()
        };
        let report = service.prune(std::slice::from_ref(&flapped), RetentionPolicy::default());
        assert!(report.deleted.is_empty());
        assert_eq!(
            report.slimmed, 0,
            "the live build keeps its cooked snapshot"
        );
    }

    #[test]
    fn delete_build_removes_archives_only() {
        let root = tempfile::tempdir().unwrap();
        let service = DataService::new(root.path().to_path_buf());
        let old = labelled("live", "4.10.1-live.400");
        let current = labelled("live", "4.10.2-live.500");
        for inst in [&old, &current] {
            let dir = service.build_dir(inst);
            builds::touch_meta(&dir, inst);
            std::fs::write(dir.join(cache::EXTRACT_SNAPSHOT_NAME), b"extract").unwrap();
        }
        let installed = std::slice::from_ref(&current);

        assert!(service.delete_build("500", installed).is_err(), "in use");
        assert!(
            service.delete_build("..", installed).is_err(),
            "not a build"
        );
        service.delete_build("400", installed).unwrap();
        let keys: Vec<String> = service
            .builds(installed)
            .into_iter()
            .map(|b| b.meta.key)
            .collect();
        assert_eq!(keys, ["500"]);
    }

    #[test]
    fn prune_adopts_orphaned_legacy_dirs_into_the_archive() {
        let root = tempfile::tempdir().unwrap();
        let service = DataService::new(root.path().to_path_buf());
        // A per-channel-era dir for a channel that is no longer installed,
        // holding an extract stamped with its build…
        let hotfix = labelled("hotfix", "4.10.1-hotfix.400");
        let orphan = root.path().join("hotfix");
        std::fs::create_dir_all(&orphan).unwrap();
        sc_holotable::asset::ExtractSnapshot {
            meta: sc_holotable::asset::SnapshotMeta {
                build_id: hotfix.build_id.clone(),
                ..cache::meta_for(&hotfix)
            },
            files: Default::default(),
        }
        .save(&orphan.join(cache::EXTRACT_SNAPSHOT_NAME))
        .unwrap();
        // …and one with nothing usable in it.
        let junk = root.path().join("ptu");
        std::fs::create_dir_all(&junk).unwrap();
        std::fs::write(junk.join("foundations.cook"), b"x").unwrap();

        let current = labelled("live", "4.10.2-live.500");
        service.prune(std::slice::from_ref(&current), RetentionPolicy::default());

        assert!(!orphan.exists() && !junk.exists());
        let builds = service.builds(std::slice::from_ref(&current));
        assert_eq!(builds.len(), 1);
        assert_eq!(builds[0].meta.key, "400");
        assert_eq!(builds[0].meta.release, "4.10.1");
        assert!(builds[0].meta.is_prod() && !builds[0].current);
    }

    #[test]
    fn prune_slims_archives_and_drops_what_retention_excludes() {
        let root = tempfile::tempdir().unwrap();
        let service = DataService::new(root.path().to_path_buf());
        let seed = |inst: &InstallRef| {
            let dir = service.build_dir(inst);
            builds::touch_meta(&dir, inst);
            cache::save_processed(&dir, inst, &fake_cooked());
            std::fs::write(dir.join(cache::EXTRACT_SNAPSHOT_NAME), b"extract").unwrap();
        };
        let old_release = labelled("live", "4.10.1-live.400");
        let hotfixed = labelled("live", "4.10.2-live.490");
        let current = labelled("live", "4.10.2-live.500");
        let ptu = labelled("ptu", "4.11.0-ptu.510");
        for inst in [&old_release, &hotfixed, &current, &ptu] {
            seed(inst);
        }
        service.get_or_reload_fast(&old_release).expect("in memory");

        let report = service.prune(std::slice::from_ref(&current), RetentionPolicy::default());
        let mut deleted = report.deleted.clone();
        deleted.sort();
        // The hotfix replaced 490; the PTU build isn't archived by default.
        assert_eq!(deleted, ["490", "510"]);
        assert_eq!(report.slimmed, 1);

        let builds = service.builds(std::slice::from_ref(&current));
        let keys: Vec<&str> = builds.iter().map(|b| b.meta.key.as_str()).collect();
        assert_eq!(keys, ["500", "400"]);
        // The current build keeps its cooked snapshot; the archive only its extract.
        assert!(builds[0].has_processed && builds[0].has_extract);
        assert!(!builds[1].has_processed && builds[1].has_extract);
        // An archived bundle doesn't stay in memory.
        assert!(service.get(&old_release).is_none());
    }
}
