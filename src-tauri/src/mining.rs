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
    /// Who works it: a crew mate (their ship brought along) or a MOLE
    /// seat's name; `null` for you.
    pub who: Option<String>,
    /// The crew loadout (its id) the laser came from.
    pub crew: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LoadoutView {
    pub lasers: Vec<LaserInputView>,
    pub gadget: Option<String>,
    /// The laser (index) that leads a crew; `null`: the first one you work.
    pub lead: Option<u32>,
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
                    who: l.who,
                    crew: l.crew,
                })
                .collect(),
            gadget: v.gadget,
            lead: v.lead,
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
                    who: l.who,
                    crew: l.crew,
                })
                .collect(),
            gadget: v.gadget,
            lead: v.lead,
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
    /// What the lasers (no gadget) put on the rock.
    pub effect: EffectView,
    /// One per laser, in loadout order.
    pub operators: Vec<OperatorView>,
    /// The wave for everyone at the plan's moments.
    pub total: Vec<TotalRowView>,
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
    /// `"safe"`, `"okay"`, `"dangerous"`, `"tooMuchPower"`; `null` when it
    /// cannot crack.
    pub category: Option<String>,
    pub decline: Option<f32>,
    pub balance: Option<f32>,
    pub safe_climb: Option<f32>,
    pub full_until: Option<f32>,
    pub climb: Vec<ClimbStepView>,
    pub hold_throttle: Option<f32>,
    pub riding: bool,
    pub too_much_power: bool,
    pub climb_at_minimum: Option<f32>,
    pub pulse_on_share: Option<f32>,
    pub window_cross_s: Option<f32>,
    pub keep_on_below: Option<f32>,
    pub lost_from: Option<f32>,
    /// The wave-lift table, lowest throttle first.
    pub jumps: Vec<JumpRowView>,
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
                    mod_mining::Category::TooMuchPower => "tooMuchPower",
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
            too_much_power: g.too_much_power,
            climb_at_minimum: g.climb_at_minimum,
            pulse_on_share: g.pulse_on_share,
            window_cross_s: g.window_cross_s,
            keep_on_below: g.keep_on_below,
            lost_from: g.lost_from,
            jumps: g
                .jumps
                .into_iter()
                .map(|j| JumpRowView {
                    throttle: j.throttle,
                    label: j.label,
                    big: j.big,
                    usual: j.usual,
                })
                .collect(),
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

/// At this throttle, how far one big and one usual wave lift the charge
/// (shares of the bar).
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct JumpRowView {
    pub throttle: f32,
    /// `"hold"`, `"climb limit"`, `"balance"`, `"minimum"`; `null` where a
    /// usual wave starts to jump out of the window.
    pub label: Option<String>,
    pub big: f32,
    pub usual: f32,
}

/// What lasers put on the rock: power, and the change to resistance,
/// instability and window as fractions (`-0.3`: 30 % less).
#[derive(Debug, Clone, Copy, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EffectView {
    pub power: f32,
    pub resistance: f32,
    pub instability: f32,
    pub window: f32,
}

impl From<mod_mining::Effect> for EffectView {
    fn from(e: mod_mining::Effect) -> Self {
        Self {
            power: e.power,
            resistance: e.resistance,
            instability: e.instability,
            window: e.window,
        }
    }
}

fn jump_view(j: mod_mining::JumpRow) -> JumpRowView {
    JumpRowView {
        throttle: j.throttle,
        label: j.label,
        big: j.big,
        usual: j.usual,
    }
}

/// One laser and whoever works it.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OperatorView {
    pub index: u32,
    pub laser: String,
    pub owner: u32,
    pub who: Option<String>,
    pub crew: Option<String>,
    /// The lead follows the steps; the others hold.
    pub lead: bool,
    /// A support's throttle through the climb and the window.
    pub hold: Option<f32>,
    pub effect: EffectView,
    pub jumps: Vec<JumpRowView>,
}

/// One moment of the plan for everyone.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TotalRowView {
    pub label: Option<String>,
    /// Each operator's throttle.
    pub throttles: Vec<f32>,
    pub big: f32,
    pub usual: f32,
    /// Each operator's share of the lift (their share of the gain).
    pub shares: Vec<f32>,
}

/// A crew ship beside yours on the rock.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CrewRowView {
    /// The crew loadout's id.
    pub id: String,
    pub owner: String,
    pub name: String,
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
    /// Your crew's ships as support, best first.
    pub crew: Vec<CrewRowView>,
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
                effect: y.effect.into(),
                operators: y
                    .operators
                    .into_iter()
                    .map(|o| OperatorView {
                        index: o.index,
                        laser: o.laser,
                        owner: o.owner,
                        who: o.who,
                        crew: o.crew,
                        lead: o.lead,
                        hold: o.hold,
                        effect: o.effect.into(),
                        jumps: o.jumps.into_iter().map(jump_view).collect(),
                    })
                    .collect(),
                total: y
                    .total
                    .into_iter()
                    .map(|t| TotalRowView {
                        label: t.label,
                        throttles: t.throttles,
                        big: t.big,
                        usual: t.usual,
                        shares: t.shares,
                    })
                    .collect(),
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
            crew: r
                .crew
                .into_iter()
                .map(|c| CrewRowView {
                    id: c.id,
                    owner: c.owner,
                    name: c.name,
                    eval: c.eval.into(),
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
    let rock = rock_input(rock);
    let crew = load_state().crew();
    tauri::async_runtime::spawn_blocking(move || {
        mod_mining::solve(&catalog, &loadout, &rock, &crew)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
    .map(Into::into)
    .map_err(|e| AppError::Config(e.to_string()))
}

fn rock_input(rock: RockView) -> mod_mining::RockInput {
    mod_mining::RockInput {
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
    }
}

/// The loadout's class along the mass axis for this rock's composition.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MassBandView {
    /// The axis' end, tonnes.
    pub top: f32,
    /// The heaviest breakable mass of this composition.
    pub max_mass: f32,
    pub segments: Vec<BandSegmentView>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BandSegmentView {
    pub from: f32,
    pub to: f32,
    pub class: RockClassView,
}

/// Where along the mass axis a rock like this one is too much power, easy,
/// manageable, hard or out of reach for the loadout: the calculator's
/// "what if it weighs" band.
#[tauri::command]
#[specta::specta]
pub(crate) async fn mining_mass_band(
    app: AppHandle,
    channel: String,
    loadout: LoadoutView,
    rock: RockView,
) -> Result<MassBandView, AppError> {
    let catalog = catalog_for(&app, &channel).await?;
    let loadout: mod_mining::LoadoutInput = loadout.into();
    let rock = rock_input(rock);
    tauri::async_runtime::spawn_blocking(move || mod_mining::mass_band(&catalog, &loadout, &rock))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .map(|b| MassBandView {
            top: b.top,
            max_mass: b.max_mass,
            segments: b
                .segments
                .into_iter()
                .map(|s| BandSegmentView {
                    from: s.from,
                    to: s.to,
                    class: s.class.into(),
                })
                .collect(),
        })
        .map_err(|e| AppError::Config(e.to_string()))
}

// ── Saved loadouts ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SavedLoadoutView {
    pub id: String,
    pub name: String,
    pub loadout: LoadoutView,
    /// The crew mate it belongs to; `null` for yours.
    pub owner: Option<String>,
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
                owner: s.owner,
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

/// Save `loadout` as `name`, yours or a crew mate's (`owner`). With `id` it
/// edits that saved loadout; otherwise one of the same name and owner is
/// replaced.
#[tauri::command]
#[specta::specta]
pub(crate) fn mining_save_loadout(
    id: Option<String>,
    name: String,
    owner: Option<String>,
    loadout: LoadoutView,
) -> Result<MiningStateView, AppError> {
    if name.trim().is_empty() {
        return Err(AppError::Config("give the loadout a name".into()));
    }
    let _guard = STATE_LOCK.lock().unwrap();
    let mut state = load_state();
    state.save(
        id.as_deref(),
        &name,
        owner.as_deref(),
        loadout.into(),
        || uuid::Uuid::new_v4().to_string(),
    );
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
    /// The places are known (the cook could read them); else areas carry
    /// their record names.
    pub places: bool,
}

/// The rock generator's areas: where each ship rock type spawns, how often,
/// and its ore odds.
#[tauri::command]
#[specta::specta]
pub(crate) async fn mining_areas(
    app: AppHandle,
    channel: String,
) -> Result<MiningAreasView, AppError> {
    let cooked = cooked_for(&app, &channel).await?;
    let places = cooked.mining.places.is_some();
    let areas = tauri::async_runtime::spawn_blocking(move || -> Result<Vec<AreaView>, AppError> {
        let catalog = catalog_of(&cooked)?;
        let src = sources(&cooked)?;
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
    Ok(MiningAreasView { areas, places })
}

fn catalog_of(cooked: &svc_data::CookedData) -> Result<Catalog, AppError> {
    Catalog::build(&cooked.mining, &cooked.locale).ok_or_else(|| {
        AppError::Config(
            "this build's game data has no mining rules — reload it in Settings → Game data".into(),
        )
    })
}

fn sources(cooked: &svc_data::CookedData) -> Result<mod_mining::Sources<'_>, AppError> {
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
        places: cooked.mining.places.as_ref(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum RockClassView {
    Easy,
    Manageable,
    Hard,
    TooMuchPower,
    Impossible,
}

impl From<mod_mining::RockClass> for RockClassView {
    fn from(c: mod_mining::RockClass) -> Self {
        match c {
            mod_mining::RockClass::Easy => Self::Easy,
            mod_mining::RockClass::Manageable => Self::Manageable,
            mod_mining::RockClass::Hard => Self::Hard,
            mod_mining::RockClass::TooMuchPower => Self::TooMuchPower,
            mod_mining::RockClass::Impossible => Self::Impossible,
        }
    }
}

impl From<RockClassView> for mod_mining::RockClass {
    fn from(c: RockClassView) -> Self {
        match c {
            RockClassView::Easy => Self::Easy,
            RockClassView::Manageable => Self::Manageable,
            RockClassView::Hard => Self::Hard,
            RockClassView::TooMuchPower => Self::TooMuchPower,
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
    MostOre {
        element: String,
    },
    BestQuality {
        element: String,
    },
    Class {
        class: RockClassView,
    },
    /// The roll closest to this mass, tonnes.
    Mass {
        mass: f32,
    },
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
    pub easy: f32,
    pub manageable: f32,
    pub hard: f32,
    pub too_much_power: f32,
    pub impossible: f32,
}

impl From<mod_mining::ClassShares> for ClassSharesView {
    fn from(c: mod_mining::ClassShares) -> Self {
        Self {
            easy: c.easy,
            manageable: c.manageable,
            hard: c.hard,
            too_much_power: c.too_much_power,
            impossible: c.impossible,
        }
    }
}

/// One bar of a rock type's mass distribution.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MassBucketView {
    pub from: f32,
    pub to: f32,
    pub count: u32,
    /// With a loadout: the class most of its rolls have.
    pub class: Option<RockClassView>,
}

/// A rock type at an area with its class shares for the loadout.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RockClassesView {
    pub area: String,
    pub rock_type: String,
    pub classes: ClassSharesView,
}

/// Every rollable rock type at every area with the loadout's class shares:
/// the rock finder's lists, so each row shows how the ship fares.
#[tauri::command]
#[specta::specta]
pub(crate) async fn mining_class_table(
    app: AppHandle,
    channel: String,
    loadout: LoadoutView,
) -> Result<Vec<RockClassesView>, AppError> {
    let cooked = cooked_for(&app, &channel).await?;
    let loadout: mod_mining::LoadoutInput = loadout.into();
    tauri::async_runtime::spawn_blocking(move || -> Result<Vec<RockClassesView>, AppError> {
        let catalog = catalog_of(&cooked)?;
        let src = sources(&cooked)?;
        Ok(mod_mining::class_table(&src, &catalog, &loadout)
            .map_err(|e| AppError::Config(e.to_string()))?
            .into_iter()
            .map(|r| RockClassesView {
                area: r.area,
                rock_type: r.rock_type,
                classes: r.classes.into(),
            })
            .collect())
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
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
    /// The sample's masses as bars, lightest first.
    pub masses: Vec<MassBucketView>,
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
    let cooked = cooked_for(&app, &channel).await?;
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
            PickView::Mass { mass } => mod_mining::Pick::Mass { mass },
        },
        place: request.place,
    };
    let loadout: Option<mod_mining::LoadoutInput> = loadout.map(Into::into);
    tauri::async_runtime::spawn_blocking(move || -> Result<GeneratedView, AppError> {
        let catalog = catalog_of(&cooked)?;
        let src = sources(&cooked)?;
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
            classes: g.classes.map(Into::into),
            pick_share: g.pick_share,
            masses: g
                .masses
                .into_iter()
                .map(|b| MassBucketView {
                    from: b.from,
                    to: b.to,
                    count: b.count,
                    class: b.class.map(Into::into),
                })
                .collect(),
        })
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

/// A place in the generator's tree, or a field standing for many places.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlaceNodeView {
    /// `StarMapObject` GUID; the provider's GUID for a field.
    pub id: String,
    pub name: String,
    /// `"body"`, `"place"`, `"field"`.
    pub kind: String,
    /// Area (provider) ids with ship rocks here; empty for a planet that
    /// only parents its moons.
    pub areas: Vec<String>,
    /// For a field, how many places it stands for.
    pub count: u32,
    pub children: Vec<PlaceNodeView>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlaceSystemView {
    pub name: String,
    pub nodes: Vec<PlaceNodeView>,
}

fn node_view(n: mod_mining::PlaceNode) -> PlaceNodeView {
    PlaceNodeView {
        id: n.id,
        name: n.name,
        kind: n.kind,
        areas: n.areas,
        count: n.count,
        children: n.children.into_iter().map(node_view).collect(),
    }
}

/// The rock finder's place tree: systems, planets with their moons, the
/// other places and fields.
#[tauri::command]
#[specta::specta]
pub(crate) async fn mining_places(
    app: AppHandle,
    channel: String,
) -> Result<Vec<PlaceSystemView>, AppError> {
    let cooked = cooked_for(&app, &channel).await?;
    tauri::async_runtime::spawn_blocking(move || -> Result<Vec<PlaceSystemView>, AppError> {
        let catalog = catalog_of(&cooked)?;
        let src = sources(&cooked)?;
        Ok(mod_mining::place_tree(&src, &catalog)
            .into_iter()
            .map(|s| PlaceSystemView {
                name: s.name,
                nodes: s.nodes.into_iter().map(node_view).collect(),
            })
            .collect())
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}
