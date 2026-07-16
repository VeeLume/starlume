//! Data service — ONE DataCore parse + cooked-snapshot cache per build,
//! shared by every module (this is what kills the 30s-per-app parse of the
//! separate-apps era). The sc-holotable umbrella pin with the `foundations`
//! feature lands here, nowhere else.
//!
//! Rules (from the workspace layering):
//! - **Synchronous** — no tokio, no Tauri, no specta. The shell wraps calls
//!   in `spawn_blocking` and mirrors the plain types into specta views
//!   (the svc-discovery pattern).
//! - **Memory discipline** (docs/memory.md): the raw `Datacore` never
//!   outlives the loader thread — parse → cook → drop, only the cooked
//!   [`CookedData`] crosses back. Cooked indices are evictable
//!   ([`DataService::evict`]) and reload from the processed snapshot in
//!   under a second ([`DataService::get_or_reload_fast`], which never
//!   parses). Parses happen only on explicit [`DataService::load`] calls —
//!   never on a timer.
//!
//! Planned extension (deferred until modules exist to hold leases): the
//! docs/memory.md lease model — modules `acquire()` the data they need and
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

pub mod cooked;
pub mod legality;
pub mod missions;
pub mod weapons;

mod cache;
mod crimestat;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Instant;

use anyhow::{Context, anyhow};
use sc_holotable::asset::{AssetConfig, AssetData, AssetSource, Datacore};
use sc_holotable::build_foundations;

pub use cooked::{
    CookedData, DATA_COOK_VERSION, ItemDetail, ItemPage, ItemQuery, ItemRow, ManufacturerRow,
    ResourceRow, STARLUME_COOK_REV,
};
pub use legality::{JurisdictionRef, LegalityEntry, LegalityKind};
pub use missions::{
    BpPoolEntry, BpPoolReward, CargoLeg, CrimestatRisk, ItemReward, MissionCategory,
    MissionDifficulty, MissionEncounter, MissionEntry, MissionFaction, MissionPayout, MissionPlace,
    MissionPoolFacts, MissionRef, MissionRegion, MissionWave, RepRequirement, RepReward,
    ScripReward, ShipSlot,
};
pub use weapons::{DamageBreakdown, MissileEntry, ShipWeaponEntry, TrackingEntry, WeaponsIndex};

// Selected sc-holotable surface for CookedData consumers (module crates
// depend on svc-data ONLY — the one-pin rule — so what they need to walk
// the snapshot is re-exported here rather than pinned again).
pub use sc_holotable::asset::{Guid, LocaleKey, LocaleMap, RecordCollection};

/// Stack size for the loader thread — see the module docs.
pub const LOADER_STACK_SIZE: usize = 32 * 1024 * 1024;

/// What svc-data needs to know about one SC install. The shell builds this
/// from `svc_discovery::InstallInfo` (svc-data stays decoupled from the
/// discovery crate — it only ever touches the p4k it's pointed at).
#[derive(Debug, Clone)]
pub struct InstallRef {
    /// Lowercase channel label — the per-channel cache-directory key
    /// (e.g. `"live"`).
    pub channel_key: String,
    /// Full path to the install's `Data.p4k`.
    pub p4k_path: PathBuf,
    /// `build_manifest.id` — the snapshot staleness key.
    pub build_id: String,
    /// Version label, recorded as snapshot provenance.
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

/// The shared data service: per-channel cooked bundles + the snapshot
/// cache. Internally synchronized — hold it as a plain field, no outer lock.
pub struct DataService {
    cache_root: PathBuf,
    /// `channel_key → cooked bundle` for everything currently in memory.
    loaded: Mutex<HashMap<String, Arc<CookedData>>>,
}

impl DataService {
    /// `cache_root` is the *parent* of the per-channel directories —
    /// the shell passes `app_data_root().join("cache")`.
    pub fn new(cache_root: PathBuf) -> Self {
        Self {
            cache_root,
            loaded: Mutex::new(HashMap::new()),
        }
    }

    fn channel_dir(&self, channel_key: &str) -> PathBuf {
        cache::channel_dir(&self.cache_root, channel_key)
    }

    /// Cache/load state for one install. Cheap (file existence + map peek).
    pub fn status(&self, install: &InstallRef) -> DataStatus {
        let cooked = self.get(&install.channel_key);
        DataStatus {
            loaded: cooked.is_some(),
            predicted_tier: cache::predict_tier(&self.channel_dir(&install.channel_key)),
            item_count: cooked.as_ref().map(|c| c.item_count()),
            resource_count: cooked.as_ref().map(|c| c.resource_count()),
            mission_count: cooked.as_ref().map(|c| c.mission_count()),
        }
    }

    /// The in-memory bundle for a channel, if present. Never touches disk.
    pub fn get(&self, channel_key: &str) -> Option<Arc<CookedData>> {
        self.loaded.lock().unwrap().get(channel_key).cloned()
    }

    /// [`Self::get`], else a tier-1-only reload (processed snapshot →
    /// memory, sub-second). **Never parses** — returns `None` when only
    /// slower tiers are available, so callers on the query path can't
    /// accidentally trigger a 30s parse (memory.md rule 3).
    pub fn get_or_reload_fast(&self, install: &InstallRef) -> Option<Arc<CookedData>> {
        if let Some(cooked) = self.get(&install.channel_key) {
            return Some(cooked);
        }
        let dir = self.channel_dir(&install.channel_key);
        let cooked = Arc::new(cache::try_load_processed(&dir, install)?);
        self.loaded
            .lock()
            .unwrap()
            .insert(install.channel_key.clone(), Arc::clone(&cooked));
        Some(cooked)
    }

    /// Run the full load waterfall for an install, on a dedicated
    /// 32 MiB-stack thread (see module docs). Blocking — the shell wraps
    /// this in `spawn_blocking`. Idempotent: if the bundle is already in
    /// memory it returns immediately.
    ///
    /// `progress` is called from the loader thread as stages begin.
    pub fn load(
        &self,
        install: &InstallRef,
        progress: impl Fn(Stage) + Send + 'static,
    ) -> anyhow::Result<Arc<CookedData>> {
        if let Some(cooked) = self.get(&install.channel_key) {
            return Ok(cooked);
        }

        let (tx, rx) = mpsc::channel::<anyhow::Result<CookedData>>();
        let dir = self.channel_dir(&install.channel_key);
        let install_for_thread = install.clone();
        std::thread::Builder::new()
            .name("starlume-data-loader".into())
            .stack_size(LOADER_STACK_SIZE)
            .spawn(move || {
                let _ = tx.send(load_blocking(&dir, &install_for_thread, &progress));
            })
            .context("spawning data-loader thread")?;
        let cooked = rx
            .recv()
            .map_err(|_| anyhow!("data-loader thread dropped its sender"))??;

        let cooked = Arc::new(cooked);
        self.loaded
            .lock()
            .unwrap()
            .insert(install.channel_key.clone(), Arc::clone(&cooked));
        Ok(cooked)
    }

    /// Drop one channel's in-memory bundle — the `InstallChanged` path: the
    /// build changed, so the cooked data keyed under this channel is stale
    /// and must not be served from memory (the disk tiers re-validate by
    /// build_id on their own).
    pub fn evict_channel(&self, channel_key: &str) {
        if self.loaded.lock().unwrap().remove(channel_key).is_some() {
            tracing::debug!(
                channel = channel_key,
                "evicted cooked data (install changed)"
            );
        }
    }

    /// Drop every in-memory bundle (window hidden — the tray-idle path).
    /// In-flight queries keep their own `Arc` clones alive; the next query
    /// after show reloads via [`Self::get_or_reload_fast`] in under a second.
    pub fn evict(&self) {
        let mut loaded = self.loaded.lock().unwrap();
        if !loaded.is_empty() {
            tracing::debug!(channels = loaded.len(), "evicting cooked data");
            loaded.clear();
        }
    }

    /// Delete cached snapshot files (one channel, or all) and evict. The
    /// next load is a full live parse.
    pub fn wipe(&self, channel_key: Option<&str>) -> std::io::Result<()> {
        match channel_key {
            Some(key) => {
                self.loaded.lock().unwrap().remove(key);
                remove_dir_if_present(&self.channel_dir(key))
            }
            None => {
                self.evict();
                remove_dir_if_present(&self.cache_root)
            }
        }
    }
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
        let cooked = cook(datacore, asset_data, progress);
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
    drop(assets);

    let cooked = cook(datacore, asset_data, progress);
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
/// must not outlive the cook (docs/memory.md rule 1; the by-value
/// signature enforces it).
fn cook(datacore: Datacore, asset_data: AssetData, progress: &impl Fn(Stage)) -> CookedData {
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
    // Legality + weapons ride the same parse window (README module rule 4:
    // reference catalogs are framework — these also feed mod-langpatch's
    // derive, so the module never needs the raw Datacore).
    let legality = legality::build_legality(&datacore, &foundations.resources);
    let weapons = weapons::build_weapons(&datacore, &foundations.items);
    let holotable = sc_holotable::HolotableSnapshot::from_foundations(&foundations);
    drop(datacore);
    CookedData {
        holotable,
        locale: asset_data.locale,
        missions,
        legality,
        weapons,
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
            scrip: Vec::new(),
            reputation: Vec::new(),
            item_rewards: Vec::new(),
            blueprint_rewards: Vec::new(),
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
    fn predict_tier_follows_file_existence() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(cache::predict_tier(dir.path()), LoadTier::Live);

        std::fs::write(dir.path().join(cache::EXTRACT_SNAPSHOT_NAME), b"x").unwrap();
        assert_eq!(cache::predict_tier(dir.path()), LoadTier::Extract);

        std::fs::write(dir.path().join(cache::PROCESSED_SNAPSHOT_NAME), b"x").unwrap();
        assert_eq!(cache::predict_tier(dir.path()), LoadTier::Processed);
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
        cache::save_processed(&service.channel_dir("live"), &inst, &fake_cooked());
        let cooked = service.get_or_reload_fast(&inst).expect("tier-1 reload");
        assert_eq!(cooked.item_count(), 3);
        let status = service.status(&inst);
        assert!(status.loaded);
        assert_eq!(status.item_count, Some(3));

        // Evict drops memory but not the file; an existing Arc stays valid.
        service.evict();
        assert!(service.get("live").is_none());
        assert_eq!(cooked.item_count(), 3);
        assert!(service.get_or_reload_fast(&inst).is_some());

        // Wipe removes the file too.
        service.wipe(Some("live")).unwrap();
        assert!(service.get_or_reload_fast(&inst).is_none());
        assert_eq!(service.status(&inst).predicted_tier, LoadTier::Live);
    }

    #[test]
    fn load_returns_in_memory_bundle_without_touching_disk() {
        let root = tempfile::tempdir().unwrap();
        let service = DataService::new(root.path().to_path_buf());
        let inst = install("live", "build-1");
        cache::save_processed(&service.channel_dir("live"), &inst, &fake_cooked());

        // First load: tier 1 from disk (p4k path is bogus — proves no parse).
        let loaded = service.load(&inst, |_| {}).expect("tier-1 load");
        assert_eq!(loaded.item_count(), 3);
        // Second load: served from memory.
        let again = service.load(&inst, |_| {}).expect("memory hit");
        assert!(Arc::ptr_eq(&loaded, &again));
    }
}
