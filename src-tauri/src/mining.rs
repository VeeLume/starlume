//! Mining — the shell side of the module: the calculator's IPC surface and
//! the module's persisted state.
//!
//! The catalog filters and the solver are `mod-mining`; the reference data is
//! svc-data's cooked mining index. Nothing here touches the network, so no
//! online gate applies. A solve runs a few hundred player-model attempts per
//! candidate setup, so it runs on the blocking pool.

use std::sync::Mutex;

use mod_mining::{Catalog, MiningState, Named};
use svc_data::mining::{Activation, FloatModifier, MiningModifiers};
use tauri::AppHandle;

use crate::data::cooked_for;
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
    /// `"safe"`, `"okay"`, `"dangerous"`; `null` when it cannot crack.
    pub category: Option<String>,
    pub decline: Option<f32>,
    pub balance: Option<f32>,
    pub safe_climb: Option<f32>,
    pub full_until: Option<f32>,
    pub climb: Vec<ClimbStepView>,
    pub hold_throttle: Option<f32>,
    pub riding: bool,
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
