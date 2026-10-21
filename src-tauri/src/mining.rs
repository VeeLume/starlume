//! Mining — the shell side of the module: the calculator's IPC surface and
//! the module's persisted state.
//!
//! The catalog filters and the solver are `mod-mining`; the reference data is
//! svc-data's cooked mining index. Nothing here touches the network, so no
//! online gate applies. A solve runs a few hundred player-model attempts per
//! candidate setup, so it runs on the blocking pool.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

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

pub(crate) fn load_state() -> MiningState {
    app_kit::load_json(&state_path())
}

fn save_state(state: &MiningState) -> Result<(), AppError> {
    Ok(app_kit::save_json(&state_path(), state)?)
}

/// The newest request of a heavy kind (the mass band, the class table, a
/// finder roll). The page only keeps the newest answer, so an older one
/// still running stops between steps instead of holding the cores the
/// newest needs (the simulated tries run on every core).
struct Latest(AtomicU64);

impl Latest {
    /// Take the newest ticket; the check is true once a newer one is taken.
    fn ticket(&'static self) -> impl Fn() -> bool + Sync + Send + 'static {
        let mine = self.0.fetch_add(1, Ordering::SeqCst) + 1;
        move || self.0.load(Ordering::SeqCst) != mine
    }
}

static BAND: Latest = Latest(AtomicU64::new(0));
static ROLL: Latest = Latest(AtomicU64::new(0));

pub(crate) async fn catalog_for(app: &AppHandle, channel: &str) -> Result<Catalog, AppError> {
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
    /// Beam distance to the rock's surface, m; `null`: within optimal range.
    pub distance_m: Option<f32>,
    /// The head holds fire today (a MOLE seat nobody is in): it stays on
    /// the ship but puts nothing on the rock. The calculator's only; saved
    /// loadouts have every head on.
    #[serde(default)]
    pub off: bool,
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
                    distance_m: l.distance_m,
                    off: l.off,
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
                    distance_m: l.distance_m,
                    off: l.off,
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
    /// On a planet or moon (`true`) or in space; `null`: not known (rated
    /// as in space). How the beam's distance drifts depends on it.
    #[serde(default)]
    pub surface: Option<bool>,
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
    /// The window is narrower than a person misjudges the charge by:
    /// never rated easy.
    pub narrow_window: bool,
    /// The share of the rock's ore the simulated cracks kept on average
    /// (overcharges drop pieces and their ore); `null` without a simulation.
    pub ore_kept: Option<f32>,
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
            narrow_window: e.narrow_window,
            ore_kept: e.ore_kept,
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
    /// Your lasers' range and, for a rock that is too much power, how far
    /// to back off.
    pub range: Option<RangeAdviceView>,
    /// The crack's explosion.
    pub blast: BlastView,
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
    /// The window's bottom with the beam off (a pulse's cut drops the
    /// lasers' window modifiers).
    pub window_low_off: f32,
    pub throttle_minimum: f32,
    /// `"safe"`, `"okay"`, `"dangerous"`, `"tooMuchPower"`; `null` when it
    /// cannot crack.
    pub category: Option<String>,
    pub decline: Option<f32>,
    pub balance: Option<f32>,
    pub safe_climb: Option<f32>,
    pub full_until: Option<f32>,
    pub climb: Vec<ClimbStepView>,
    /// Keeping the window: the rise setting for its lower half and the fall
    /// setting for its upper half; `keepLow` is `null` when not even the
    /// gentlest rise keeps it (`unholdable`).
    pub keep_low: Option<f32>,
    pub keep_high: Option<f32>,
    /// Room left at the gentlest rise (share of the bar); negative: the
    /// window can't be kept.
    pub keep_margin: Option<f32>,
    /// The throttle in the window for one-throttle uses (the rise setting,
    /// else the fall setting).
    pub hold_throttle: Option<f32>,
    /// The window can't be kept long enough to crack it cleanly.
    pub unholdable: bool,
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
            window_low_off: g.window_low_off,
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
            keep_low: g.keep_low,
            keep_high: g.keep_high,
            keep_margin: g.keep_margin,
            hold_throttle: g.hold_throttle,
            unholdable: g.unholdable,
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

/// The explosion when the rock cracks (every crack blasts; an overcharge
/// grows it). Radii from the rock's centre; `*ClearM` from its surface,
/// as the HUD's distance gauge measures. The hull takes the full damage
/// anywhere inside the radius.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BlastView {
    pub clean_min_m: f32,
    pub clean_max_m: f32,
    pub clean_damage: f32,
    pub full_min_m: f32,
    pub full_max_m: f32,
    pub full_damage: f32,
    pub rock_radius_m: f32,
    pub clean_clear_m: f32,
    pub full_clear_m: f32,
}

impl From<mod_mining::Blast> for BlastView {
    fn from(b: mod_mining::Blast) -> Self {
        Self {
            clean_min_m: b.clean_min_m,
            clean_max_m: b.clean_max_m,
            clean_damage: b.clean_damage,
            full_min_m: b.full_min_m,
            full_max_m: b.full_max_m,
            full_damage: b.full_damage,
            rock_radius_m: b.rock_radius_m,
            clean_clear_m: b.clean_clear_m,
            full_clear_m: b.full_clear_m,
        }
    }
}

/// Your lasers' range: full power up to `fullM`, none from `zeroM`.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RangeAdviceView {
    pub full_m: f32,
    pub zero_m: f32,
    /// Power at the given distance, as a share of full power.
    pub power_share: f32,
    /// For a rock that is too much power: distances past optimal range that
    /// bring the balance throttle back into the lasers' range.
    pub back_off: Option<BackOffView>,
}

#[derive(Debug, Clone, Copy, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BackOffView {
    pub from_m: f32,
    pub to_m: f32,
    /// Where to sit: `marginM` either way stays in the band.
    pub best_m: f32,
    /// The band is narrower than the margin: the distance needs watching.
    pub tight: bool,
    /// The margin the advice keeps for the distance wandering (ship drift,
    /// a turning rock), m.
    pub margin_m: f32,
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
                range: y.range.map(|a| RangeAdviceView {
                    full_m: a.full_m,
                    zero_m: a.zero_m,
                    power_share: a.power_share,
                    back_off: a.back_off.map(|b| BackOffView {
                        from_m: b.from_m,
                        to_m: b.to_m,
                        best_m: b.best_m,
                        tight: b.tight,
                        margin_m: mod_mining::player::SIT_MARGIN_M,
                    }),
                }),
                blast: y.blast.into(),
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

/// How each loadout does on the rock (each with its own gadget): the
/// picker's options for a slot, the loadout list, a try-out against the
/// saved loadout. `null` for a loadout without lasers or with parts this
/// build lacks.
#[tauri::command]
#[specta::specta]
pub(crate) async fn mining_compare(
    app: AppHandle,
    channel: String,
    rock: RockView,
    loadouts: Vec<LoadoutView>,
) -> Result<Vec<Option<EvalView>>, AppError> {
    let catalog = catalog_for(&app, &channel).await?;
    let loadouts: Vec<mod_mining::LoadoutInput> = loadouts.into_iter().map(Into::into).collect();
    let rock = rock_input(rock);
    tauri::async_runtime::spawn_blocking(move || mod_mining::compare(&catalog, &rock, &loadouts))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .map(|evals| evals.into_iter().map(|e| e.map(Into::into)).collect())
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
        surface: rock.surface,
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
    /// Loss-free share of the simulated tries; `null` where breakability or
    /// power decide the class.
    pub share: Option<f32>,
}

/// Where along the mass axis a rock like this one is too much power, easy,
/// manageable, hard or out of reach for the loadout: the calculator's
/// "what if it weighs" band. `heaviest`: the rock type's heaviest roll, when
/// the rock came from the finder (the axis' end; else twice the mass).
#[tauri::command]
#[specta::specta]
pub(crate) async fn mining_mass_band(
    app: AppHandle,
    channel: String,
    loadout: LoadoutView,
    rock: RockView,
    heaviest: Option<f32>,
) -> Result<MassBandView, AppError> {
    // first, before any await: tickets follow the page's request order
    let superseded = BAND.ticket();
    let catalog = catalog_for(&app, &channel).await?;
    let loadout: mod_mining::LoadoutInput = loadout.into();
    let rock = rock_input(rock);
    tauri::async_runtime::spawn_blocking(move || {
        mod_mining::mass_band(&catalog, &loadout, &rock, heaviest, &superseded)
    })
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
                share: s.share,
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
    /// The saved loadout (its id) your part of `current` came from; the
    /// calculator's edits are a try-out against it until saved.
    pub based_on: Option<String>,
    pub saved: Vec<SavedLoadoutView>,
}

fn state_view(s: MiningState) -> MiningStateView {
    MiningStateView {
        current: s.current.into(),
        based_on: s.based_on,
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

/// Remember the calculator's loadout (it opens with it next time) and the
/// saved loadout your part of it came from (`null`: none).
#[tauri::command]
#[specta::specta]
pub(crate) fn mining_set_current(
    loadout: LoadoutView,
    based_on: Option<String>,
) -> Result<MiningStateView, AppError> {
    let _guard = STATE_LOCK.lock().unwrap();
    let mut state = load_state();
    state.current = loadout.into();
    state.based_on = based_on.filter(|id| state.saved.iter().any(|s| &s.id == id));
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
    /// Its distribution: rows with the same key roll the same rocks, so the
    /// finder shows them once with their places.
    pub dist: String,
    /// Lightest, most common and heaviest roll; `null` when not rollable.
    pub mass: Option<MassRangeView>,
}

/// A distribution's masses in tonnes.
#[derive(Debug, Clone, Copy, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MassRangeView {
    pub min: f32,
    pub mode: f32,
    pub max: f32,
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
                        dist: r.dist,
                        mass: r.mass.map(|m| MassRangeView {
                            min: m.min,
                            mode: m.mode,
                            max: m.max,
                        }),
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
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

/// A distribution with a loadout (design pass 5): its class shares and its
/// masses as bars counted by class — the Loadouts tab's comfort zones and
/// scores, and the finder's class split.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ComfortView {
    /// The distribution key ([`AreaRockView::dist`]).
    pub dist: String,
    pub classes: ClassSharesView,
    pub bars: Vec<ComfortBarView>,
}

/// One bar of a distribution's masses, its rolls counted by class.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ComfortBarView {
    pub from: f32,
    pub to: f32,
    /// Too much power, easy, manageable, hard, can't break.
    pub counts: Vec<u32>,
}

/// Every distribution that spawns, with the loadout: class shares and mass
/// bars by class. A solve takes a few seconds, so results are kept on disk
/// per build and loadout (the disk is the product: a list of saved
/// loadouts opens with its scores at once); a newer build drops the older
/// build's. Not superseded: the Loadouts tab queues several (the list, a
/// sweep) and every result is worth keeping.
#[tauri::command]
#[specta::specta]
pub(crate) async fn mining_comfort(
    app: AppHandle,
    channel: String,
    loadout: LoadoutView,
) -> Result<Vec<ComfortView>, AppError> {
    use sha2::{Digest, Sha256};
    let install = crate::data::install_for(&app, &channel).await?;
    // The build's changelist (a bare build_id can be "None" on every patch)
    // and the solver's version: a solver change leaves results for its own
    // version, never the old ones.
    let build = format!("{}-s{}", install.build_key(), mod_mining::SOLVER_VERSION);
    let root = app_kit::app_data_root().join("mining-comfort");
    let dir = root.join(&build);
    let json = serde_json::to_vec(&loadout).map_err(|e| AppError::Internal(e.to_string()))?;
    let hash = Sha256::digest(&json);
    let file = dir.join(format!(
        "{}.json",
        hash.iter()
            .take(12)
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    ));
    if let Ok(bytes) = std::fs::read(&file)
        && let Ok(cached) = serde_json::from_slice::<Vec<ComfortView>>(&bytes)
    {
        return Ok(cached);
    }
    let cooked = cooked_for(&app, &channel).await?;
    let input: mod_mining::LoadoutInput = loadout.into();
    let out =
        tauri::async_runtime::spawn_blocking(move || -> Result<Vec<ComfortView>, AppError> {
            let catalog = catalog_of(&cooked)?;
            let src = sources(&cooked)?;
            Ok(mod_mining::comfort(&src, &catalog, &input, &|| false)
                .map_err(|e| AppError::Config(e.to_string()))?
                .into_iter()
                .map(|c| ComfortView {
                    dist: c.dist,
                    classes: c.classes.into(),
                    bars: c
                        .bars
                        .into_iter()
                        .map(|b| ComfortBarView {
                            from: b.from,
                            to: b.to,
                            counts: b.counts.to_vec(),
                        })
                        .collect(),
                })
                .collect())
        })
        .await
        .map_err(|e| AppError::Internal(e.to_string()))??;
    // Only this build's results stay.
    if let Ok(entries) = std::fs::read_dir(&root) {
        for e in entries.flatten() {
            if e.path() != dir {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
    }
    if std::fs::create_dir_all(&dir).is_ok()
        && let Ok(bytes) = serde_json::to_vec(&out)
        && let Err(e) = app_kit::atomic_write(&file, &bytes)
    {
        tracing::warn!("mining comfort cache: {e}");
    }
    Ok(out)
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedView {
    /// Tonnes, as the scanner shows it.
    pub mass: f32,
    /// On a planet or moon (the rock's mesh); `null`: not known.
    pub surface: Option<bool>,
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
    // first, before any await: tickets follow the page's request order
    let superseded = ROLL.ticket();
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
        let g = mod_mining::generate(&src, &catalog, &request, loadout.as_ref(), &superseded)
            .map_err(|e| AppError::Config(e.to_string()))?;
        Ok(GeneratedView {
            mass: g.rock.mass,
            surface: g.rock.surface,
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
    /// `"body"`, `"place"`, `"lpoint"` (a Lagrange point, under its
    /// planet), `"field"`.
    pub kind: String,
    /// Area (provider) ids with ship rocks here; empty for a planet that
    /// only parents its moons.
    pub areas: Vec<String>,
    /// For a field, how many places it stands for; for a Lagrange point,
    /// its mining claims.
    pub count: u32,
    /// A Lagrange point's rest stop, as the game names it.
    pub rest_stop: Option<String>,
    /// A Lagrange point's tables by what they spawn at: its own (no title),
    /// gas pockets, every claim, yield claims by name.
    pub groups: Vec<PlaceGroupView>,
    pub children: Vec<PlaceNodeView>,
}

/// Spawn tables at a Lagrange point, by what they spawn at.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlaceGroupView {
    pub title: Option<String>,
    pub detail: Option<String>,
    pub areas: Vec<String>,
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
        rest_stop: n.rest_stop,
        groups: n
            .groups
            .into_iter()
            .map(|g| PlaceGroupView {
                title: g.title,
                detail: g.detail,
                areas: g.areas,
            })
            .collect(),
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
