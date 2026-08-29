//! Game-data commands: the shell surface over `svc-data` (the shared
//! DataCore parse + cooked-snapshot cache) for the Game Data section, plus
//! the startup warm ([`spawn_startup_warm`]) that cooks the default channel
//! (newest PU build) in the background so catalogs are instantly browsable.
//!
//! Everything here is **local file reads only** — no network, so no online
//! gate (the p4k, the snapshots, and the cooked indices all live on this
//! machine).
//!
//! During a load, coarse progress is emitted as a plain `data:progress`
//! event (the notify.rs pattern): `{ channel, stage }` with stage one of
//! `loading-snapshot | opening-p4k | extracting | parsing | cooking |
//! saving`. The frontend store hand-types that payload.

use tauri::{AppHandle, Emitter, Manager};

use svc_data::{InstallRef, ItemQuery, LoadTier, Stage};

use crate::AppState;
use crate::error::AppError;

pub const DATA_PROGRESS_EVENT: &str = "data:progress";

/// Fired after anything that changes cache/load state (explicit load, wipe,
/// startup warm) — the frontend refreshes statuses and invalidates its
/// catalog caches on this. Payload-free by design.
pub const DATA_CHANGED_EVENT: &str = "data:changed";

pub(crate) fn emit_changed(app: &AppHandle) {
    let _ = app.emit(DATA_CHANGED_EVENT, ());
}

#[derive(Debug, Clone, serde::Serialize)]
struct DataProgress {
    channel: String,
    stage: &'static str,
}

fn stage_str(stage: Stage) -> &'static str {
    match stage {
        Stage::LoadingSnapshot => "loading-snapshot",
        Stage::OpeningP4k => "opening-p4k",
        Stage::Extracting => "extracting",
        Stage::Parsing => "parsing",
        Stage::Cooking => "cooking",
        Stage::Saving => "saving",
    }
}

/// Which cache tier the next load of a channel will likely hit.
#[derive(Debug, Clone, Copy, serde::Serialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum DataTierView {
    /// Cooked snapshot on disk — loads in about a second.
    Processed,
    /// Raw extract snapshot only — re-parse, ~15–20s.
    Extract,
    /// Nothing cached — full `Data.p4k` parse, ~30s+.
    Live,
}

impl From<LoadTier> for DataTierView {
    fn from(tier: LoadTier) -> Self {
        match tier {
            LoadTier::Processed => Self::Processed,
            LoadTier::Extract => Self::Extract,
            LoadTier::Live => Self::Live,
        }
    }
}

/// Cache/load state of one install, for the status cards.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct DataStatusView {
    /// Channel label as discovery reports it, e.g. `"Live"`.
    pub channel: String,
    pub version: String,
    pub build_id: String,
    pub loaded: bool,
    pub predicted_tier: DataTierView,
    pub item_count: Option<u32>,
    pub resource_count: Option<u32>,
    pub mission_count: Option<u32>,
    /// The channel the app treats as "the one the user wants" — newest PU
    /// build (Live/Hotfix), warmed at startup, preselected by the catalogs.
    pub is_default: bool,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct ItemRowView {
    pub guid: String,
    pub name: String,
    pub item_type: String,
    pub item_sub_type: String,
    pub size: i32,
    pub grade: i32,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct ItemPageView {
    pub total: u32,
    pub rows: Vec<ItemRowView>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct ItemDetailView {
    pub guid: String,
    pub name: String,
    pub short_name: Option<String>,
    pub description: Option<String>,
    pub item_type: String,
    pub item_sub_type: String,
    pub size: i32,
    pub grade: i32,
    pub record_path: Option<String>,
    /// Combat stats when the item is a ship weapon (weapons-index join).
    pub ship_weapon: Option<ShipWeaponStatsView>,
    /// Combat stats when the item is a missile / torpedo.
    pub missile: Option<MissileStatsView>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct DamageBreakdownView {
    pub physical: f32,
    pub energy: f32,
    pub distortion: f32,
    pub thermal: f32,
    pub biochemical: f32,
    pub stun: f32,
    /// Scalar total across all damage types (the "alpha" figure).
    pub total: f32,
}

impl From<svc_data::DamageBreakdown> for DamageBreakdownView {
    fn from(d: svc_data::DamageBreakdown) -> Self {
        Self {
            total: d.total(),
            physical: d.physical,
            energy: d.energy,
            distortion: d.distortion,
            thermal: d.thermal,
            biochemical: d.biochemical,
            stun: d.stun,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct ShipWeaponStatsView {
    pub size: i32,
    pub item_sub_type: String,
    pub damage: Option<DamageBreakdownView>,
    pub penetration_m: Option<f32>,
    pub ammo_speed: Option<f32>,
    pub ammo_lifetime: Option<f32>,
    pub total_ammo: Option<i32>,
    pub capacitor: Option<f32>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct MissileStatsView {
    pub size: i32,
    pub is_torpedo: bool,
    pub damage: Option<DamageBreakdownView>,
    pub speed: Option<f32>,
    pub arm_time: f32,
    pub tracking: Option<TrackingView>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct TrackingView {
    pub signal: String,
    pub lock_time: f32,
    pub lock_angle_deg: f32,
    pub lock_range_min_m: f32,
    pub lock_range_max_m: f32,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct ResourceRowView {
    pub guid: String,
    pub name: String,
    pub description: Option<String>,
    pub refined_into: Option<String>,
    pub density_kg_per_m3: Option<f32>,
    /// Drug/contraband verdict when any jurisdiction outlaws this resource.
    pub legality: Option<ResourceLegalityView>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct ResourceLegalityView {
    /// `"drug"` or `"contraband"`.
    pub kind: String,
    pub jurisdictions: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct ManufacturerRowView {
    pub guid: String,
    pub code: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct ItemTypeFacetView {
    pub item_type: String,
    pub count: u32,
}

// ── Mission catalog views ──────────────────────────────────────────────────
// Specta mirrors of `svc_data::missions::*` (the svc-discovery pattern:
// svc-* crates stay specta-free, the shell mirrors what crosses the IPC).
// The whole pooled list ships at once — a few hundred templates after
// pooling; filtering is client-side (the Hearth model).

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct MissionEntryView {
    pub mission_id: String,
    pub title: Option<String>,
    pub debug_name: String,
    pub description: Option<String>,
    pub category: Option<MissionCategoryView>,
    pub faction: Option<MissionFactionView>,
    pub difficulty: Option<MissionDifficultyView>,
    pub payout: MissionPayoutView,
    pub once_only: bool,
    pub shareable: bool,
    pub illegal: bool,
    pub cooldown_seconds: Option<f32>,
    pub scrip: Vec<ScripRewardView>,
    pub reputation: Vec<RepRewardView>,
    pub item_rewards: Vec<ItemRewardView>,
    pub blueprint_rewards: Vec<BpPoolRewardView>,
    pub rep_required: Vec<RepRequirementView>,
    pub chain_required: Vec<MissionRefView>,
    pub locations: Vec<MissionRegionView>,
    pub encounters: Vec<MissionEncounterView>,
    pub cargo: Vec<CargoLegView>,
    pub placeholders: Vec<String>,
    pub instance_count: u32,
    pub facts: MissionPoolFactsView,
}

/// Within-pool divergence flags + crimestat (`svc_data::MissionPoolFacts`).
/// When a `*_mixed` flag is set, the representative's value on that axis is
/// one of several — the UI surfaces the ambiguity instead of stating it as
/// fact.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct MissionPoolFactsView {
    pub shareable_mixed: bool,
    pub once_only_mixed: bool,
    pub illegal_mixed: bool,
    pub cooldowns_mixed: bool,
    pub scrip_mixed: bool,
    pub rep_mixed: bool,
    pub encounters_mixed: bool,
    /// `"none"` / `"moderate"` / `"high"` — killing friendly NPCs risks a
    /// crimestat (high = no HUD markers to tell friend from foe).
    pub crimestat: String,
    pub crimestat_mixed: bool,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct MissionCategoryView {
    pub name: Option<String>,
    pub icon: String,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct MissionFactionView {
    pub guid: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct MissionDifficultyView {
    pub mechanical_skill: u8,
    pub mental_load: u8,
    pub risk_of_loss: u8,
    pub game_knowledge: u8,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct MissionPayoutView {
    pub calculated: bool,
    pub fixed: Option<i32>,
    pub estimate: Option<i32>,
    pub buy_in: i32,
    pub time_to_complete: f32,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct ScripRewardView {
    pub name: Option<String>,
    pub amount: i32,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct RepRewardView {
    pub faction_guid: Option<String>,
    pub amount: Option<i32>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct ItemRewardView {
    pub entity_guid: String,
    pub name: Option<String>,
    pub amount: i32,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct BpPoolRewardView {
    pub pool_name: String,
    pub chance: f32,
    pub blueprints: Vec<BpPoolEntryView>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct BpPoolEntryView {
    pub blueprint_record_guid: String,
    pub name: Option<String>,
    pub weight: f32,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct RepRequirementView {
    pub faction: Option<String>,
    pub min_rank: Option<String>,
    pub max_rank: Option<String>,
    pub min_rank_index: Option<i32>,
    pub max_rank_index: Option<i32>,
    pub exclude: bool,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct MissionRefView {
    pub mission_id: String,
    pub title: Option<String>,
    pub once_only: bool,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct MissionRegionView {
    pub system: String,
    pub name: String,
    pub places: Vec<MissionPlaceView>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct MissionPlaceView {
    pub name: Option<String>,
    pub record_name: String,
    pub kind: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct MissionEncounterView {
    pub label: String,
    pub difficulty: Option<String>,
    pub waves: Vec<MissionWaveView>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct MissionWaveView {
    pub name: String,
    pub ships: Vec<ShipSlotView>,
    pub cargo: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct ShipSlotView {
    pub count_min: i32,
    pub count_max: i32,
    pub ships: Vec<String>,
    pub factions: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct CargoLegView {
    pub commodity: Option<String>,
    pub commodity_guid: Option<String>,
    pub min_scu: f32,
    pub max_scu: f32,
    pub max_box: f32,
}

impl From<svc_data::MissionEntry> for MissionEntryView {
    fn from(m: svc_data::MissionEntry) -> Self {
        Self {
            mission_id: m.mission_id,
            title: m.title,
            debug_name: m.debug_name,
            description: m.description,
            category: m.category.map(|c| MissionCategoryView {
                name: c.name,
                icon: c.icon,
            }),
            faction: m.faction.map(|f| MissionFactionView {
                guid: f.guid,
                name: f.name,
            }),
            difficulty: m.difficulty.map(|d| MissionDifficultyView {
                mechanical_skill: d.mechanical_skill,
                mental_load: d.mental_load,
                risk_of_loss: d.risk_of_loss,
                game_knowledge: d.game_knowledge,
            }),
            payout: MissionPayoutView {
                calculated: m.payout.calculated,
                fixed: m.payout.fixed,
                estimate: m.payout.estimate,
                buy_in: m.payout.buy_in,
                time_to_complete: m.payout.time_to_complete,
            },
            once_only: m.once_only,
            shareable: m.shareable,
            illegal: m.illegal,
            cooldown_seconds: m.cooldown_seconds,
            scrip: m
                .scrip
                .into_iter()
                .map(|s| ScripRewardView {
                    name: s.name,
                    amount: s.amount,
                })
                .collect(),
            reputation: m
                .reputation
                .into_iter()
                .map(|r| RepRewardView {
                    faction_guid: r.faction_guid,
                    amount: r.amount,
                })
                .collect(),
            item_rewards: m
                .item_rewards
                .into_iter()
                .map(|i| ItemRewardView {
                    entity_guid: i.entity_guid,
                    name: i.name,
                    amount: i.amount,
                })
                .collect(),
            blueprint_rewards: m
                .blueprint_rewards
                .into_iter()
                .map(|p| BpPoolRewardView {
                    pool_name: p.pool_name,
                    chance: p.chance,
                    blueprints: p
                        .blueprints
                        .into_iter()
                        .map(|e| BpPoolEntryView {
                            blueprint_record_guid: e.blueprint_record_guid,
                            name: e.name,
                            weight: e.weight,
                        })
                        .collect(),
                })
                .collect(),
            rep_required: m
                .rep_required
                .into_iter()
                .map(|r| RepRequirementView {
                    faction: r.faction,
                    min_rank: r.min_rank,
                    max_rank: r.max_rank,
                    min_rank_index: r.min_rank_index,
                    max_rank_index: r.max_rank_index,
                    exclude: r.exclude,
                })
                .collect(),
            chain_required: m
                .chain_required
                .into_iter()
                .map(|c| MissionRefView {
                    mission_id: c.mission_id,
                    title: c.title,
                    once_only: c.once_only,
                })
                .collect(),
            locations: m
                .locations
                .into_iter()
                .map(|l| MissionRegionView {
                    system: l.system,
                    name: l.name,
                    places: l
                        .places
                        .into_iter()
                        .map(|p| MissionPlaceView {
                            name: p.name,
                            record_name: p.record_name,
                            kind: p.kind,
                        })
                        .collect(),
                })
                .collect(),
            encounters: m
                .encounters
                .into_iter()
                .map(|e| MissionEncounterView {
                    label: e.label,
                    difficulty: e.difficulty,
                    waves: e
                        .waves
                        .into_iter()
                        .map(|w| MissionWaveView {
                            name: w.name,
                            ships: w
                                .ships
                                .into_iter()
                                .map(|s| ShipSlotView {
                                    count_min: s.count_min,
                                    count_max: s.count_max,
                                    ships: s.ships,
                                    factions: s.factions,
                                })
                                .collect(),
                            cargo: w.cargo,
                        })
                        .collect(),
                })
                .collect(),
            cargo: m
                .cargo
                .into_iter()
                .map(|c| CargoLegView {
                    commodity: c.commodity,
                    commodity_guid: c.commodity_guid,
                    min_scu: c.min_scu,
                    max_scu: c.max_scu,
                    max_box: c.max_box,
                })
                .collect(),
            placeholders: m.placeholders,
            instance_count: m.instance_count,
            facts: MissionPoolFactsView {
                shareable_mixed: m.facts.shareable_mixed,
                once_only_mixed: m.facts.once_only_mixed,
                illegal_mixed: m.facts.illegal_mixed,
                cooldowns_mixed: m.facts.cooldowns_mixed,
                scrip_mixed: m.facts.scrip_mixed,
                rep_mixed: m.facts.rep_mixed,
                encounters_mixed: m.facts.encounters_mixed,
                crimestat: match m.facts.crimestat {
                    svc_data::CrimestatRisk::None => "none".into(),
                    svc_data::CrimestatRisk::Moderate => "moderate".into(),
                    svc_data::CrimestatRisk::High => "high".into(),
                },
                crimestat_mixed: m.facts.crimestat_mixed,
            },
        }
    }
}

/// Rescan installs (cheap, ~50ms of launcher-store reads) and refresh the
/// shared `InstallRef` cache the query commands resolve channels against.
/// The snapshot staleness key comes from `InstallInfo::staleness_key` —
/// never the raw build_id, which is literally `"None"` on current Live
/// builds.
pub(crate) async fn refresh_installs(app: &AppHandle) -> Result<Vec<InstallRef>, AppError> {
    let scan = tokio::task::spawn_blocking(svc_discovery::scan)
        .await
        .map_err(|e| AppError::Internal(format!("scan task failed: {e}")))?
        .map_err(|e| AppError::Internal(format!("install scan failed: {e:#}")))?;
    let installs: Vec<InstallRef> = scan
        .installs
        .into_iter()
        .map(|i| InstallRef {
            channel_key: i.channel.to_ascii_lowercase(),
            p4k_path: std::path::PathBuf::from(&i.directory).join("Data.p4k"),
            build_id: i.staleness_key(),
            version: i.version,
        })
        .collect();
    *app.state::<AppState>().installs.lock().unwrap() = installs.clone();
    Ok(installs)
}

/// The changelist embedded in a version label (`"4.8.3-live.12122953"` →
/// `12122953`) — monotonic across channels, so it orders builds by recency.
/// `0` when the label doesn't end in digits (sorts last, which is right).
fn changelist(version: &str) -> u64 {
    version
        .rsplit('.')
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

/// The install the app treats as the user's default: the newest **PU** build
/// (Live/Hotfix — what people actually play; Hotfix is Live-staged and can be
/// newer), falling back to the newest install of any channel (PTU-only
/// machines exist).
fn default_install(installs: &[InstallRef]) -> Option<&InstallRef> {
    installs
        .iter()
        .filter(|i| matches!(i.channel_key.as_str(), "live" | "hotfix"))
        .max_by_key(|i| changelist(&i.version))
        .or_else(|| installs.iter().max_by_key(|i| changelist(&i.version)))
}

/// Resolve a channel to its `InstallRef` — from the cached scan, or a fresh
/// one when the cache is cold (first query after startup).
async fn install_for(app: &AppHandle, channel: &str) -> Result<InstallRef, AppError> {
    let key = channel.to_ascii_lowercase();
    let cached = {
        let state = app.state::<AppState>();
        let installs = state.installs.lock().unwrap();
        installs.iter().find(|i| i.channel_key == key).cloned()
    };
    if let Some(install) = cached {
        return Ok(install);
    }
    refresh_installs(app)
        .await?
        .into_iter()
        .find(|i| i.channel_key == key)
        .ok_or_else(|| AppError::Config(format!("no SC install found for channel '{channel}'")))
}

fn status_view(
    state: &AppState,
    install: &InstallRef,
    channel: String,
    is_default: bool,
) -> DataStatusView {
    let status = state.data.status(install);
    DataStatusView {
        channel,
        version: install.version.clone(),
        build_id: install.build_id.clone(),
        loaded: status.loaded,
        predicted_tier: status.predicted_tier.into(),
        item_count: status.item_count.map(|n| n as u32),
        resource_count: status.resource_count.map(|n| n as u32),
        mission_count: status.mission_count.map(|n| n as u32),
        is_default,
    }
}

/// Restore the display casing discovery uses (`InstallRef` only keeps the
/// lowercase cache key).
pub(crate) fn display_channel(key: &str) -> String {
    let mut c = key.chars();
    match c.next() {
        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Scan installs and report each one's cache/load state. Local reads only.
#[tauri::command]
#[specta::specta]
pub(crate) async fn data_status(app: AppHandle) -> Result<Vec<DataStatusView>, AppError> {
    let installs = refresh_installs(&app).await?;
    let default_key = default_install(&installs).map(|i| i.channel_key.clone());
    let state = app.state::<AppState>();
    Ok(installs
        .iter()
        .map(|i| {
            let is_default = default_key.as_deref() == Some(i.channel_key.as_str());
            status_view(&state, i, display_channel(&i.channel_key), is_default)
        })
        .collect())
}

/// Run the load waterfall for one install on a blocking thread, streaming
/// stage progress as `data:progress` events. Shared by the explicit
/// `data_load` command and the startup warm.
async fn load_with_progress(
    app: &AppHandle,
    install: &InstallRef,
    channel_label: String,
) -> Result<(), AppError> {
    let service = app.state::<AppState>().data.clone();
    let progress_app = app.clone();
    let install_for_load = install.clone();
    tokio::task::spawn_blocking(move || {
        service.load(&install_for_load, move |stage| {
            let _ = progress_app.emit(
                DATA_PROGRESS_EVENT,
                DataProgress {
                    channel: channel_label.clone(),
                    stage: stage_str(stage),
                },
            );
        })
    })
    .await
    .map_err(|e| AppError::Internal(format!("data load task failed: {e}")))?
    .map_err(|e| AppError::Internal(format!("data load failed: {e:#}")))?;
    Ok(())
}

/// Run the load waterfall for one channel (processed snapshot → extract
/// snapshot → live parse). The slow tiers take ~15–45s; progress streams
/// via the `data:progress` event.
#[tauri::command]
#[specta::specta]
pub(crate) async fn data_load(app: AppHandle, channel: String) -> Result<DataStatusView, AppError> {
    // Rescan first: a load must see the install's *current* build_id, or a
    // patch since the last scan would cook under a stale key.
    let installs = refresh_installs(&app).await?;
    let install = install_for(&app, &channel).await?;
    load_with_progress(&app, &install, channel.clone()).await?;
    emit_changed(&app);

    let is_default =
        default_install(&installs).map(|i| i.channel_key.as_str()) == Some(&install.channel_key);
    let state = app.state::<AppState>();
    Ok(status_view(&state, &install, channel, is_default))
}

/// Run the startup warm: scan installs and make sure the default channel's
/// cooked snapshot exists, so the catalogs are browsable without a manual
/// Load. Gated on the `auto_load_game_data` setting. Local file reads only.
/// Failures notify and return — callers sequence follow-up work (the
/// langpatch reconcile) after this regardless.
///
/// Memory discipline (docs/memory.md): the durable product of the warm is the
/// **snapshot on disk** — if the window is hidden when the cook finishes
/// (companion start), the in-memory bundle is evicted immediately; the first
/// catalog query after show reloads it in under a second.
pub(crate) async fn run_startup_warm(app: &AppHandle) {
    if !app
        .state::<AppState>()
        .settings
        .lock()
        .unwrap()
        .auto_load_game_data
    {
        return;
    }
    if let Err(e) = startup_warm(app).await {
        tracing::warn!("startup game-data warm failed: {e}");
        crate::notify::notify(
            app,
            crate::notify::Notification::warning("Game data load failed")
                .with_body(e.to_string())
                .with_source("data"),
        );
    }
}

async fn startup_warm(app: &AppHandle) -> Result<(), AppError> {
    let installs = refresh_installs(app).await?;
    let Some(install) = default_install(&installs).cloned() else {
        tracing::debug!("no SC install found; skipping game-data warm");
        return Ok(());
    };
    let status = app.state::<AppState>().data.status(&install);
    if status.loaded {
        return Ok(());
    }
    // Only a slow-tier warm (new patch / first run) is worth announcing;
    // the sub-second snapshot path stays silent.
    let was_cold = status.predicted_tier != LoadTier::Processed;
    let label = display_channel(&install.channel_key);
    tracing::info!(
        channel = %label,
        version = %install.version,
        tier = ?status.predicted_tier,
        "startup game-data warm"
    );
    load_with_progress(app, &install, label.clone()).await?;

    let hidden = app
        .get_webview_window("main")
        .map(|w| !w.is_visible().unwrap_or(false))
        .unwrap_or(true);
    if hidden {
        app.state::<AppState>().data.evict();
    }
    emit_changed(app);
    if was_cold {
        crate::notify::notify(
            app,
            crate::notify::Notification::success(format!("Game data ready — {label}"))
                .with_body(format!(
                    "{} cooked and cached; catalogs are up to date.",
                    install.version
                ))
                .with_source("data"),
        );
    }
    Ok(())
}

/// Delete cached snapshots for one channel (or all). The next load is a
/// full live parse.
#[tauri::command]
#[specta::specta]
pub(crate) async fn data_wipe(app: AppHandle, channel: Option<String>) -> Result<(), AppError> {
    let state = app.state::<AppState>();
    state
        .data
        .wipe(channel.map(|c| c.to_ascii_lowercase()).as_deref())?;
    emit_changed(&app);
    Ok(())
}

/// The cooked bundle for query commands. Tier-1 reload at most (sub-second);
/// never parses — the user starts parses explicitly via `data_load`.
async fn cooked_for(
    app: &AppHandle,
    channel: &str,
) -> Result<std::sync::Arc<svc_data::CookedData>, AppError> {
    let install = install_for(app, channel).await?;
    app.state::<AppState>()
        .data
        .get_or_reload_fast(&install)
        .ok_or_else(|| {
            AppError::Config("game data not loaded — open Game Data and press Load".into())
        })
}

/// Search inventory items by name/GUID substring, optionally filtered to
/// one item type. Paginated; `total` counts all matches.
#[tauri::command]
#[specta::specta]
pub(crate) async fn data_search_items(
    app: AppHandle,
    channel: String,
    query: String,
    item_type: Option<String>,
    offset: u32,
    limit: u32,
) -> Result<ItemPageView, AppError> {
    let cooked = cooked_for(&app, &channel).await?;
    let page = cooked.search_items(&ItemQuery {
        text: query,
        item_type,
        offset: offset as usize,
        limit: (limit as usize).clamp(1, 500),
    });
    Ok(ItemPageView {
        total: page.total as u32,
        rows: page
            .rows
            .into_iter()
            .map(|r| ItemRowView {
                guid: r.guid,
                name: r.name,
                item_type: r.item_type,
                item_sub_type: r.item_sub_type,
                size: r.size,
                grade: r.grade,
            })
            .collect(),
    })
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn data_item_detail(
    app: AppHandle,
    channel: String,
    guid: String,
) -> Result<ItemDetailView, AppError> {
    let cooked = cooked_for(&app, &channel).await?;
    let d = cooked
        .item_detail(&guid)
        .ok_or_else(|| AppError::Config(format!("unknown item '{guid}'")))?;
    Ok(ItemDetailView {
        guid: d.guid,
        name: d.name,
        short_name: d.short_name,
        description: d.description,
        item_type: d.item_type,
        item_sub_type: d.item_sub_type,
        size: d.size,
        grade: d.grade,
        record_path: d.record_path,
        ship_weapon: d.ship_weapon.map(|w| ShipWeaponStatsView {
            size: w.size,
            item_sub_type: w.item_sub_type,
            damage: w.damage.map(Into::into),
            penetration_m: w.penetration_m,
            ammo_speed: w.ammo_speed,
            ammo_lifetime: w.ammo_lifetime,
            total_ammo: w.total_ammo,
            capacitor: w.capacitor,
        }),
        missile: d.missile.map(|m| MissileStatsView {
            size: m.size,
            is_torpedo: m.is_torpedo,
            damage: m.damage.map(Into::into),
            speed: m.speed,
            arm_time: m.arm_time,
            tracking: m.tracking.map(|t| TrackingView {
                signal: t.signal,
                lock_time: t.lock_time,
                lock_angle_deg: t.lock_angle_deg,
                lock_range_min_m: t.lock_range_min_m,
                lock_range_max_m: t.lock_range_max_m,
            }),
        }),
    })
}

/// The whole item catalog in one shot — the client-side pipeline's corpus
/// (the same rows `data_search_items` pages through, unpaged).
#[tauri::command]
#[specta::specta]
pub(crate) async fn data_items_all(
    app: AppHandle,
    channel: String,
) -> Result<Vec<ItemRowView>, AppError> {
    let cooked = cooked_for(&app, &channel).await?;
    let page = cooked.search_items(&ItemQuery {
        text: String::new(),
        item_type: None,
        offset: 0,
        limit: usize::MAX,
    });
    Ok(page
        .rows
        .into_iter()
        .map(|r| ItemRowView {
            guid: r.guid,
            name: r.name,
            item_type: r.item_type,
            item_sub_type: r.item_sub_type,
            size: r.size,
            grade: r.grade,
        })
        .collect())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn data_resources(
    app: AppHandle,
    channel: String,
) -> Result<Vec<ResourceRowView>, AppError> {
    let cooked = cooked_for(&app, &channel).await?;
    Ok(cooked
        .resources()
        .into_iter()
        .map(|r| ResourceRowView {
            guid: r.guid,
            name: r.name,
            description: r.description,
            refined_into: r.refined_into,
            density_kg_per_m3: r.density_kg_per_m3,
            legality: r.legality.map(|l| ResourceLegalityView {
                kind: match l.kind {
                    svc_data::LegalityKind::Drug => "drug".into(),
                    svc_data::LegalityKind::Contraband => "contraband".into(),
                },
                jurisdictions: l.jurisdictions,
            }),
        })
        .collect())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn data_manufacturers(
    app: AppHandle,
    channel: String,
) -> Result<Vec<ManufacturerRowView>, AppError> {
    let cooked = cooked_for(&app, &channel).await?;
    Ok(cooked
        .manufacturers()
        .into_iter()
        .map(|m| ManufacturerRowView {
            guid: m.guid,
            code: m.code,
            name: m.name,
        })
        .collect())
}

/// The full pooled mission catalog for a channel. A few hundred templates
/// after pooling — ships whole; the frontend filters/sorts client-side.
#[tauri::command]
#[specta::specta]
pub(crate) async fn data_missions(
    app: AppHandle,
    channel: String,
) -> Result<Vec<MissionEntryView>, AppError> {
    let cooked = cooked_for(&app, &channel).await?;
    Ok(cooked
        .missions
        .iter()
        .cloned()
        .map(MissionEntryView::from)
        .collect())
}

/// Distinct item types with counts — feeds the search type filter.
#[tauri::command]
#[specta::specta]
pub(crate) async fn data_item_types(
    app: AppHandle,
    channel: String,
) -> Result<Vec<ItemTypeFacetView>, AppError> {
    let cooked = cooked_for(&app, &channel).await?;
    Ok(cooked
        .item_type_facets()
        .into_iter()
        .map(|(item_type, count)| ItemTypeFacetView {
            item_type,
            count: count as u32,
        })
        .collect())
}
