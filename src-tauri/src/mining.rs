//! Mining — the shell side of the module: the calculator's IPC surface and
//! the module's persisted state.
//!
//! The catalog filters and the solver are `mod-mining`; the reference data is
//! svc-data's cooked mining index. Nothing here touches the network, so no
//! online gate applies. A solve runs a few hundred player-model attempts per
//! candidate setup, so it runs on the blocking pool.
//!
//! **Mining places.** The rock generator names areas after their places,
//! which takes svc-data's heavy once-per-build pass over the game's
//! placements (minutes, a multi-GB peak). It runs as a background activity
//! only while the module is enabled, never while Star Citizen runs (the
//! app must stay light while the game is played) and never beside a
//! game-data load; the result is a small file per build. Until then areas
//! carry their record names.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use mod_mining::{Catalog, MiningState, Named};
use svc_data::InstallRef;
use svc_data::mining::{Activation, FloatModifier, MiningModifiers};
use tauri::AppHandle;
use tauri::{Emitter, Manager};

use crate::AppState;
use crate::activity::{Activity, ActivityKind};
use crate::data::{cooked_for, install_for};
use crate::error::AppError;

/// Module id (Rust registry, settings, frontend descriptor).
pub(crate) const MODULE_ID: &str = "mining";

/// Serializes state read-modify-writes across commands.
static STATE_LOCK: Mutex<()> = Mutex::new(());

fn state_path() -> std::path::PathBuf {
    app_kit::app_data_root().join("mining").join("state.json")
}

fn load_state() -> MiningState {
    app_kit::load_json(&state_path())
}

fn save_state(state: &MiningState) -> Result<(), AppError> {
    Ok(app_kit::save_json(&state_path(), state)?)
}

async fn catalog_for(app: &AppHandle, channel: &str) -> Result<Catalog, AppError> {
    let cooked = cooked_for(app, channel).await?;
    Catalog::build(&cooked.mining, &cooked.locale).ok_or_else(|| {
        AppError::Config(
            "this build's game data has no mining rules — reload it in Settings → Game data".into(),
        )
    })
}

// ── Catalog views ───────────────────────────────────────────────────────────

/// A laser's, module's or gadget's mining modifiers, in percent. `null`
/// when the item does not change that value.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ModifiersView {
    pub resistance: Option<f32>,
    pub instability: Option<f32>,
    pub window_size: Option<f32>,
    pub window_rate: Option<f32>,
    pub shatter_damage: Option<f32>,
    pub cluster_factor: Option<f32>,
    pub catastrophic_rate: Option<f32>,
}

fn pct(m: &Option<FloatModifier>) -> Option<f32> {
    m.map(|m| m.value).filter(|v| *v != 0.0)
}

impl From<&MiningModifiers> for ModifiersView {
    fn from(m: &MiningModifiers) -> Self {
        Self {
            resistance: pct(&m.resistance),
            instability: pct(&m.laser_instability),
            window_size: pct(&m.optimal_charge_window_size),
            window_rate: pct(&m.optimal_charge_window_rate),
            shatter_damage: pct(&m.shatter_damage),
            cluster_factor: pct(&m.cluster_factor),
            catastrophic_rate: pct(&m.catastrophic_charge_window_rate),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MiningLaserView {
    pub id: String,
    pub name: String,
    pub size: i32,
    pub module_slots: u32,
    /// Fracturing energy per second ("Laser Power").
    pub power: f32,
    pub extraction_power: f32,
    /// Full power up to here, m.
    pub optimal_range_m: Option<f32>,
    /// No power beyond here, m.
    pub max_range_m: Option<f32>,
    pub throttle_minimum: f32,
    pub modifiers: ModifiersView,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MiningModuleView {
    pub id: String,
    pub name: String,
    /// Power multiplier on the fracturing beam (×1.5 for a Surge).
    pub power_multiplier: f32,
    pub active: bool,
    pub duration_s: Option<f32>,
    pub uses: Option<i32>,
    pub modifiers: ModifiersView,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MiningGadgetView {
    pub id: String,
    pub name: String,
    pub modifiers: ModifiersView,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MiningElementView {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MiningCatalogView {
    pub lasers: Vec<MiningLaserView>,
    pub modules: Vec<MiningModuleView>,
    pub gadgets: Vec<MiningGadgetView>,
    pub elements: Vec<MiningElementView>,
}

fn catalog_view(c: &Catalog) -> MiningCatalogView {
    // Modules multiply beams by fire action; the fracturing beam's is the
    // one the calculator shows.
    let fracture_action = c
        .lasers
        .iter()
        .find_map(|l| l.value.stats.fracture_beam().map(|b| b.fire_action))
        .unwrap_or(0);
    MiningCatalogView {
        lasers: c
            .lasers
            .iter()
            .map(|l| {
                let s = &l.value.stats;
                let beam = s.fracture_beam();
                MiningLaserView {
                    id: l.id.clone(),
                    name: l.name.clone(),
                    size: l.value.size,
                    module_slots: s.module_slots,
                    power: beam.map_or(0.0, |b| b.energy_per_second),
                    extraction_power: s.extraction_beam().map_or(0.0, |b| b.energy_per_second),
                    optimal_range_m: beam.map(|b| b.full_damage_range),
                    max_range_m: beam.map(|b| b.zero_damage_range),
                    throttle_minimum: s.throttle_minimum,
                    modifiers: (&s.modifiers).into(),
                }
            })
            .collect(),
        modules: c
            .modules
            .iter()
            .map(|m| {
                let (active, duration_s, uses) = match m.value.activation {
                    Activation::Passive => (false, None, None),
                    Activation::Active { duration_s, uses } => (true, duration_s, uses),
                };
                MiningModuleView {
                    id: m.id.clone(),
                    name: m.name.clone(),
                    power_multiplier: m.value.damage_multiplier(fracture_action),
                    active,
                    duration_s,
                    uses,
                    modifiers: (&m.value.modifiers).into(),
                }
            })
            .collect(),
        gadgets: c
            .gadgets
            .iter()
            .map(|g| MiningGadgetView {
                id: g.id.clone(),
                name: g.name.clone(),
                modifiers: (&g.value.modifiers).into(),
            })
            .collect(),
        elements: c
            .element_names
            .iter()
            .map(|e: &Named<_>| MiningElementView {
                id: e.id.clone(),
                name: e.name.clone(),
            })
            .collect(),
    }
}

/// The calculator's pickers: ship lasers, modules, gadgets, ship-rock elements.
#[tauri::command]
#[specta::specta]
pub(crate) async fn mining_catalog(
    app: AppHandle,
    channel: String,
) -> Result<MiningCatalogView, AppError> {
    Ok(catalog_view(&catalog_for(&app, &channel).await?))
}

// ── Inputs ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LaserInputView {
    pub laser: String,
    pub modules: Vec<String>,
    /// Which ship the laser is on; lasers on one ship split the wave.
    pub owner: u32,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LoadoutView {
    pub lasers: Vec<LaserInputView>,
    pub gadget: Option<String>,
}

impl From<LoadoutView> for mod_mining::LoadoutInput {
    fn from(v: LoadoutView) -> Self {
        Self {
            lasers: v
                .lasers
                .into_iter()
                .map(|l| mod_mining::LaserInput {
                    laser: l.laser,
                    modules: l.modules,
                    owner: l.owner,
                })
                .collect(),
            gadget: v.gadget,
        }
    }
}

impl From<mod_mining::LoadoutInput> for LoadoutView {
    fn from(v: mod_mining::LoadoutInput) -> Self {
        Self {
            lasers: v
                .lasers
                .into_iter()
                .map(|l| LaserInputView {
                    laser: l.laser,
                    modules: l.modules,
                    owner: l.owner,
                })
                .collect(),
            gadget: v.gadget,
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RockEntryView {
    pub element: String,
    /// 0..100.
    pub percent: f32,
    pub quality: Option<u16>,
}

#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RockView {
    /// Tonnes, as the scanner shows it.
    pub mass: f32,
    pub entries: Vec<RockEntryView>,
}

// ── Report views ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DifficultyView {
    pub runs: u32,
    /// Share of attempts that cracked cleanly (controlled pool first).
    pub controlled: f32,
    /// Share that overcharged.
    pub overcharged: f32,
    pub timed_out: f32,
    pub median_time_s: Option<f32>,
    /// Mean danger pool at fracture: 0 is clean.
    pub mean_danger: f32,
    /// Mean share of the attempt spent inside the window.
    pub in_window: f32,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EvalView {
    /// Heaviest breakable mass over the rock's mass: ≥ 1 cracks.
    pub margin: f32,
    pub power: f32,
    pub need: f32,
    pub resistance: f32,
    pub instability: f32,
    pub window_low: f32,
    pub window_high: f32,
    pub balance_throttle: Option<f32>,
    pub difficulty: Option<DifficultyView>,
}

impl From<mod_mining::Eval> for EvalView {
    fn from(e: mod_mining::Eval) -> Self {
        Self {
            margin: e.margin,
            power: e.power,
            need: e.need,
            resistance: e.resistance,
            instability: e.instability,
            window_low: e.window_low,
            window_high: e.window_high,
            balance_throttle: e.balance_throttle,
            difficulty: e.difficulty.map(|d| DifficultyView {
                runs: d.runs,
                controlled: d.controlled,
                overcharged: d.overcharged,
                timed_out: d.timed_out,
                median_time_s: d.median_time_s,
                mean_danger: d.mean_danger,
                in_window: d.in_window,
            }),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RockFactsView {
    pub mass: f32,
    pub scu: f32,
    pub resistance: f32,
    pub instability: f32,
    pub window_low: f32,
    pub window_high: f32,
    pub gadget_resistance: f32,
    pub gadget_instability: f32,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct YourSetupView {
    pub eval: EvalView,
    pub scanner_resistance_pct: f32,
    pub scanner_instability: f32,
    pub guide: GuideView,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ClimbStepView {
    pub until_charge: f32,
    pub throttle: f32,
}

/// What to do at each charge level (`mod_mining::ThrottleGuide`). Charges
/// and throttles are 0..1; jumps are shares of the bar.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GuideView {
    pub window_low: f32,
    pub window_high: f32,
    pub window_high_off: f32,
    pub throttle_minimum: f32,
    /// `"safe"`, `"okay"`, `"dangerous"`, `"tooStrong"`; `null` when it
    /// cannot crack.
    pub category: Option<String>,
    pub decline: Option<f32>,
    pub balance: Option<f32>,
    pub safe_climb: Option<f32>,
    pub full_until: Option<f32>,
    pub climb: Vec<ClimbStepView>,
    pub hold_throttle: Option<f32>,
    pub riding: bool,
    pub too_strong: bool,
    pub climb_at_minimum: Option<f32>,
    pub pulse_on_share: Option<f32>,
    pub window_cross_s: Option<f32>,
    pub keep_on_below: Option<f32>,
    pub lost_from: Option<f32>,
    pub wave_max: f32,
    pub wave_period_s: f32,
    pub wave_period_max_s: f32,
    pub damping: f32,
    pub peak_jump_at_balance: Option<f32>,
    pub peak_jump_avg_at_balance: Option<f32>,
    pub peak_jump_full: f32,
    pub drain_at_minimum: Option<f32>,
    pub time_to_window_s: Option<f32>,
    pub time_in_window_s: Option<f32>,
    pub crack_above_s: Option<f32>,
    pub payback_edge: Option<f32>,
    pub payback_top: Option<f32>,
    pub danger_fill_edge_s: Option<f32>,
    pub danger_fill_top_s: Option<f32>,
    pub danger_drain_s: Option<f32>,
    pub back_from_top_off_s: Option<f32>,
    pub back_from_top_min_s: Option<f32>,
    pub modifier_persistence_s: f32,
}

impl From<mod_mining::ThrottleGuide> for GuideView {
    fn from(g: mod_mining::ThrottleGuide) -> Self {
        Self {
            window_low: g.window_low,
            window_high: g.window_high,
            window_high_off: g.window_high_off,
            throttle_minimum: g.throttle_minimum,
            category: g.category.map(|c| {
                match c {
                    mod_mining::Category::Safe => "safe",
                    mod_mining::Category::Okay => "okay",
                    mod_mining::Category::Dangerous => "dangerous",
                    mod_mining::Category::TooStrong => "tooStrong",
                }
                .into()
            }),
            decline: g.decline,
            balance: g.balance,
            safe_climb: g.safe_climb,
            full_until: g.full_until,
            climb: g
                .climb
                .into_iter()
                .map(|s| ClimbStepView {
                    until_charge: s.until_charge,
                    throttle: s.throttle,
                })
                .collect(),
            hold_throttle: g.hold_throttle,
            riding: g.riding,
            too_strong: g.too_strong,
            climb_at_minimum: g.climb_at_minimum,
            pulse_on_share: g.pulse_on_share,
            window_cross_s: g.window_cross_s,
            keep_on_below: g.keep_on_below,
            lost_from: g.lost_from,
            wave_max: g.wave_max,
            wave_period_s: g.wave_period_s,
            wave_period_max_s: g.wave_period_max_s,
            damping: g.damping,
            peak_jump_at_balance: g.peak_jump_at_balance,
            peak_jump_avg_at_balance: g.peak_jump_avg_at_balance,
            peak_jump_full: g.peak_jump_full,
            drain_at_minimum: g.drain_at_minimum,
            time_to_window_s: g.time_to_window_s,
            time_in_window_s: g.time_in_window_s,
            crack_above_s: g.crack_above_s,
            payback_edge: g.payback_edge,
            payback_top: g.payback_top,
            danger_fill_edge_s: g.danger_fill_edge_s,
            danger_fill_top_s: g.danger_fill_top_s,
            danger_drain_s: g.danger_drain_s,
            back_from_top_off_s: g.back_from_top_off_s,
            back_from_top_min_s: g.back_from_top_min_s,
            modifier_persistence_s: g.modifier_persistence_s,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GadgetRowView {
    /// `null` for "no gadget".
    pub gadget: Option<String>,
    pub eval: EvalView,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct KitRowView {
    pub modules: Vec<String>,
    /// Set when an active module runs out before the median fracture.
    pub runs_out_s: Option<f32>,
    pub eval: EvalView,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SupportRowView {
    pub laser: String,
    pub modules: Vec<String>,
    pub eval: EvalView,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MiningReportView {
    pub rock: RockFactsView,
    pub yours: Option<YourSetupView>,
    pub gadgets: Vec<GadgetRowView>,
    pub passive_kits: Vec<KitRowView>,
    pub active_kits: Vec<KitRowView>,
    pub support: Vec<SupportRowView>,
}

impl From<mod_mining::Report> for MiningReportView {
    fn from(r: mod_mining::Report) -> Self {
        let kits = |k: Vec<mod_mining::KitRow>| {
            k.into_iter()
                .map(|k| KitRowView {
                    modules: k.modules,
                    runs_out_s: k.runs_out_s,
                    eval: k.eval.into(),
                })
                .collect()
        };
        Self {
            rock: RockFactsView {
                mass: r.rock.mass,
                scu: r.rock.scu,
                resistance: r.rock.resistance,
                instability: r.rock.instability,
                window_low: r.rock.window_low,
                window_high: r.rock.window_high,
                gadget_resistance: r.rock.gadget_resistance,
                gadget_instability: r.rock.gadget_instability,
            },
            yours: r.yours.map(|y| YourSetupView {
                eval: y.eval.into(),
                scanner_resistance_pct: y.scanner_resistance_pct,
                scanner_instability: y.scanner_instability,
                guide: y.guide.into(),
            }),
            gadgets: r
                .gadgets
                .into_iter()
                .map(|g| GadgetRowView {
                    gadget: g.gadget,
                    eval: g.eval.into(),
                })
                .collect(),
            passive_kits: kits(r.passive_kits),
            active_kits: kits(r.active_kits),
            support: r
                .support
                .into_iter()
                .map(|s| SupportRowView {
                    laser: s.laser,
                    modules: s.modules,
                    eval: s.eval.into(),
                })
                .collect(),
        }
    }
}

/// Solve a scanned rock against a loadout: the verdict, our difficulty, the
/// charge bar, and the best gadget, module swaps and support.
#[tauri::command]
#[specta::specta]
pub(crate) async fn mining_solve(
    app: AppHandle,
    channel: String,
    loadout: LoadoutView,
    rock: RockView,
) -> Result<MiningReportView, AppError> {
    let catalog = catalog_for(&app, &channel).await?;
    let loadout: mod_mining::LoadoutInput = loadout.into();
    let rock = mod_mining::RockInput {
        mass: rock.mass,
        entries: rock
            .entries
            .into_iter()
            .map(|e| mod_mining::EntryInput {
                element: e.element,
                percent: e.percent,
                quality: e.quality,
            })
            .collect(),
    };
    tauri::async_runtime::spawn_blocking(move || mod_mining::solve(&catalog, &loadout, &rock))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .map(Into::into)
        .map_err(|e| AppError::Config(e.to_string()))
}

// ── Saved loadouts ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SavedLoadoutView {
    pub id: String,
    pub name: String,
    pub loadout: LoadoutView,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MiningStateView {
    pub current: LoadoutView,
    pub saved: Vec<SavedLoadoutView>,
}

fn state_view(s: MiningState) -> MiningStateView {
    MiningStateView {
        current: s.current.into(),
        saved: s
            .saved
            .into_iter()
            .map(|s| SavedLoadoutView {
                id: s.id,
                name: s.name,
                loadout: s.loadout.into(),
            })
            .collect(),
    }
}

/// The calculator's loadout and the saved ones.
#[tauri::command]
#[specta::specta]
pub(crate) fn mining_state() -> MiningStateView {
    state_view(load_state())
}

/// Remember the calculator's loadout (it opens with it next time).
#[tauri::command]
#[specta::specta]
pub(crate) fn mining_set_current(loadout: LoadoutView) -> Result<MiningStateView, AppError> {
    let _guard = STATE_LOCK.lock().unwrap();
    let mut state = load_state();
    state.current = loadout.into();
    save_state(&state)?;
    Ok(state_view(state))
}

/// Save `loadout` under `name`; a saved loadout of the same name is replaced.
#[tauri::command]
#[specta::specta]
pub(crate) fn mining_save_loadout(
    name: String,
    loadout: LoadoutView,
) -> Result<MiningStateView, AppError> {
    if name.trim().is_empty() {
        return Err(AppError::Config("give the loadout a name".into()));
    }
    let _guard = STATE_LOCK.lock().unwrap();
    let mut state = load_state();
    state.save(&name, loadout.into(), || uuid::Uuid::new_v4().to_string());
    save_state(&state)?;
    Ok(state_view(state))
}

#[tauri::command]
#[specta::specta]
pub(crate) fn mining_delete_loadout(id: String) -> Result<MiningStateView, AppError> {
    let _guard = STATE_LOCK.lock().unwrap();
    let mut state = load_state();
    state.delete(&id);
    save_state(&state)?;
    Ok(state_view(state))
}

// ── Rock generator ──────────────────────────────────────────────────────────

/// Fired when the mining places for a build were written. The store reloads
/// the areas.
pub const PLACES_EVENT: &str = "mining:places";

/// One places pass at a time, process-wide.
static PLACES_RUNNING: AtomicBool = AtomicBool::new(false);

fn module_enabled(app: &AppHandle) -> bool {
    app.state::<AppState>()
        .settings
        .lock()
        .unwrap()
        .enabled_modules
        .iter()
        .any(|m| m == MODULE_ID)
}

/// Start the mining-places pass for `install` in the background unless it
/// ran for this build, runs already, or the module is off.
fn ensure_places(app: &AppHandle, install: InstallRef) {
    if !module_enabled(app)
        || app
            .state::<AppState>()
            .data
            .mining_places(&install)
            .is_some()
        || PLACES_RUNNING.swap(true, Ordering::SeqCst)
    {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut activity = Activity::start(
            &app,
            ActivityKind::MiningPlaces,
            Some(&install.channel_key),
            "Mining places",
        );
        // Wait out the game and any game-data load; 30 s polls, capped at 12 h.
        for _ in 0..1440 {
            let reason = if crate::langpatch::sc_running().await {
                Some("Waiting for Star Citizen to exit")
            } else if app
                .state::<AppState>()
                .activities
                .lock()
                .unwrap()
                .has(ActivityKind::DataLoad)
            {
                Some("Waiting for the game data")
            } else {
                None
            };
            match reason {
                Some(r) => activity = activity.wait(r),
                None => break,
            }
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        }
        let activity =
            activity.stage("Reading where rocks spawn (once per game build, a few minutes)");
        let worker_app = app.clone();
        let worker_install = install.clone();
        // Its own thread with the loader's stack: the placement decode is deep.
        let result = tauri::async_runtime::spawn_blocking(move || {
            std::thread::Builder::new()
                .name("mining-places".into())
                .stack_size(svc_data::LOADER_STACK_SIZE)
                .spawn(move || {
                    worker_app
                        .state::<AppState>()
                        .data
                        .build_mining_places(&worker_install)
                })
                .map_err(anyhow::Error::from)
                .and_then(|h| {
                    h.join()
                        .map_err(|_| anyhow::anyhow!("mining places thread panicked"))?
                })
        })
        .await;
        drop(activity);
        PLACES_RUNNING.store(false, Ordering::SeqCst);
        match result {
            Ok(Ok(())) => {
                tracing::info!(channel = %install.channel_key, "mining places written");
                let _ = app.emit(PLACES_EVENT, ());
            }
            Ok(Err(e)) => tracing::warn!("mining places failed: {e:#}"),
            Err(e) => tracing::warn!("mining places task failed: {e}"),
        }
    });
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SpawnPlaceView {
    pub id: String,
    pub name: String,
}

impl From<mod_mining::Place> for SpawnPlaceView {
    fn from(p: mod_mining::Place) -> Self {
        Self {
            id: p.id,
            name: p.name,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OreOddsView {
    pub element: String,
    /// Share of rocks holding it.
    pub chance: f32,
    /// Its share of the rock when present.
    pub mean_share: f32,
    pub max_share: f32,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AreaRockView {
    pub rock_type: String,
    pub name: String,
    /// Share of the area's ship-mineable rocks.
    pub share: f32,
    pub rollable: bool,
    pub ores: Vec<OreOddsView>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AreaView {
    pub id: String,
    pub name: String,
    /// Named after the provider's record, not a place.
    pub from_record: bool,
    pub places: Vec<SpawnPlaceView>,
    pub rocks: Vec<AreaRockView>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MiningAreasView {
    pub areas: Vec<AreaView>,
    /// `"ready"` (places known), `"building"` (the pass runs or waits),
    /// `"pending"` (not run yet).
    pub places: String,
}

/// The rock generator's areas: where each ship rock type spawns, how often,
/// and its ore odds. Starts the mining-places pass in the background when
/// this build has none yet.
#[tauri::command]
#[specta::specta]
pub(crate) async fn mining_areas(
    app: AppHandle,
    channel: String,
) -> Result<MiningAreasView, AppError> {
    let install = install_for(&app, &channel).await?;
    let cooked = cooked_for(&app, &channel).await?;
    ensure_places(&app, install.clone());
    let places = app.state::<AppState>().data.mining_places(&install);
    let state = if places.is_some() {
        "ready"
    } else if PLACES_RUNNING.load(Ordering::SeqCst) {
        "building"
    } else {
        "pending"
    };
    let areas = tauri::async_runtime::spawn_blocking(move || -> Result<Vec<AreaView>, AppError> {
        let catalog = catalog_of(&cooked)?;
        let src = sources(&cooked, places.as_ref())?;
        Ok(mod_mining::areas(&src, &catalog)
            .into_iter()
            .map(|a| AreaView {
                id: a.id,
                name: a.name,
                from_record: a.from_record,
                places: a.places.into_iter().map(Into::into).collect(),
                rocks: a
                    .rocks
                    .into_iter()
                    .map(|r| AreaRockView {
                        rock_type: r.rock_type,
                        name: r.name,
                        share: r.share,
                        rollable: r.rollable,
                        ores: r
                            .ores
                            .into_iter()
                            .map(|o| OreOddsView {
                                element: o.element,
                                chance: o.chance,
                                mean_share: o.mean_share,
                                max_share: o.max_share,
                            })
                            .collect(),
                    })
                    .collect(),
            })
            .collect())
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;
    Ok(MiningAreasView {
        areas,
        places: state.into(),
    })
}

fn catalog_of(cooked: &svc_data::CookedData) -> Result<Catalog, AppError> {
    Catalog::build(&cooked.mining, &cooked.locale).ok_or_else(|| {
        AppError::Config(
            "this build's game data has no mining rules — reload it in Settings → Game data".into(),
        )
    })
}

fn sources<'a>(
    cooked: &'a svc_data::CookedData,
    places: Option<&'a svc_data::ProviderLocations>,
) -> Result<mod_mining::Sources<'a>, AppError> {
    let h = &cooked.holotable;
    let missing = || {
        AppError::Config(
            "the game data lacks the spawn tables — reload it in Settings → Game data".into(),
        )
    };
    Ok(mod_mining::Sources {
        index: &cooked.mining,
        providers: h.gathering.as_ref().ok_or_else(missing)?,
        tags: h.tags.as_ref().ok_or_else(missing)?,
        resources: h.resources.as_ref().ok_or_else(missing)?,
        locations: h.locations.as_ref().ok_or_else(missing)?,
        locale: &cooked.locale,
        places,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum RockClassView {
    Safe,
    Okay,
    Dangerous,
    TooStrong,
    Impossible,
}

impl From<RockClassView> for mod_mining::RockClass {
    fn from(c: RockClassView) -> Self {
        match c {
            RockClassView::Safe => Self::Safe,
            RockClassView::Okay => Self::Okay,
            RockClassView::Dangerous => Self::Dangerous,
            RockClassView::TooStrong => Self::TooStrong,
            RockClassView::Impossible => Self::Impossible,
        }
    }
}

/// Which rolled rock to hand out.
#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PickView {
    Average,
    MostCommon,
    Smallest,
    Largest,
    MostOre { element: String },
    BestQuality { element: String },
    Class { class: RockClassView },
}

#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GenerateRequestView {
    pub area: String,
    pub rock_type: String,
    pub pick: PickView,
    pub place: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedEntryView {
    pub element: String,
    /// 0..100.
    pub percent: f32,
    pub quality: Option<u16>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ClassSharesView {
    pub safe: f32,
    pub okay: f32,
    pub dangerous: f32,
    pub too_strong: f32,
    pub impossible: f32,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedView {
    /// Tonnes, as the scanner shows it.
    pub mass: f32,
    pub entries: Vec<GeneratedEntryView>,
    pub scu: f32,
    /// The draw it came from.
    pub seed: u32,
    pub place: Option<SpawnPlaceView>,
    pub rolled: u32,
    pub classes: Option<ClassSharesView>,
    /// For a most-common pick: the share of rolls holding its ore mix.
    pub pick_share: Option<f32>,
}

/// Roll a rock of a type in an area: average, smallest, largest, most of an
/// ore, best quality of an ore, or a typical rock of a class for `loadout`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn mining_generate(
    app: AppHandle,
    channel: String,
    request: GenerateRequestView,
    loadout: Option<LoadoutView>,
) -> Result<GeneratedView, AppError> {
    let install = install_for(&app, &channel).await?;
    let cooked = cooked_for(&app, &channel).await?;
    let places = app.state::<AppState>().data.mining_places(&install);
    let request = mod_mining::GenerateRequest {
        area: request.area,
        rock_type: request.rock_type,
        pick: match request.pick {
            PickView::Average => mod_mining::Pick::Average,
            PickView::MostCommon => mod_mining::Pick::MostCommon,
            PickView::Smallest => mod_mining::Pick::Smallest,
            PickView::Largest => mod_mining::Pick::Largest,
            PickView::MostOre { element } => mod_mining::Pick::MostOre { element },
            PickView::BestQuality { element } => mod_mining::Pick::BestQuality { element },
            PickView::Class { class } => mod_mining::Pick::Class {
                class: class.into(),
            },
        },
        place: request.place,
    };
    let loadout: Option<mod_mining::LoadoutInput> = loadout.map(Into::into);
    tauri::async_runtime::spawn_blocking(move || -> Result<GeneratedView, AppError> {
        let catalog = catalog_of(&cooked)?;
        let src = sources(&cooked, places.as_ref())?;
        let g = mod_mining::generate(&src, &catalog, &request, loadout.as_ref())
            .map_err(|e| AppError::Config(e.to_string()))?;
        Ok(GeneratedView {
            mass: g.rock.mass,
            entries: g
                .rock
                .entries
                .into_iter()
                .map(|e| GeneratedEntryView {
                    element: e.element,
                    percent: e.percent,
                    quality: e.quality,
                })
                .collect(),
            scu: g.scu,
            seed: g.seed as u32,
            place: g.place.map(Into::into),
            rolled: g.rolled,
            classes: g.classes.map(|c| ClassSharesView {
                safe: c.safe,
                okay: c.okay,
                dangerous: c.dangerous,
                too_strong: c.too_strong,
                impossible: c.impossible,
            }),
            pick_share: g.pick_share,
        })
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}
