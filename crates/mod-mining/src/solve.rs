//! The calculator: a scanned rock against a loadout, and what would do
//! better — the best gadget, module swaps, one support Prospector.
//!
//! Ported from sc-holotable's `mining_calc` example (the solver is not in
//! the lib yet; Firefly "Mining 4" plans it there after the loadout model).
//! Each candidate is a [`ModifiedRock`] for the breakability margin and,
//! when it can crack, our [`difficulty`] over [`SIM_RUNS`] player-model
//! attempts. Ranking: a clean crack first, then margin.

use serde::{Deserialize, Serialize};
use svc_data::mining::{
    Difficulty, Entry, Laser, LaserPower, MiningGadget, ModifiedRock, Player, Rock, Ruleset, Setup,
    SimConfig, difficulty,
};

use crate::catalog::{Catalog, is_active};
use crate::guide::{JumpRow, ThrottleGuide, throttle_guide, throttle_guide_lead, wave_lift};

/// Player-model attempts per evaluated setup.
pub const SIM_RUNS: u32 = 100;
/// How many setups the module search simulates, strongest margin first.
const KIT_SIMULATED: usize = 8;
/// Rows per suggestion table (gadgets: all of them, ranked).
const KIT_ROWS: usize = 3;
const SUPPORT_ROWS: usize = 6;

/// The ship (or ships): lasers with their modules, and the gadget on the rock.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LoadoutInput {
    pub lasers: Vec<LaserInput>,
    /// Gadget id; `None` for no gadget.
    pub gadget: Option<String>,
    /// Which laser (its index) leads a crew: it follows the steps while the
    /// others hold. `None`: the first laser you work yourself.
    #[serde(default)]
    pub lead: Option<u32>,
}

/// One laser: its id, its fitted module ids, and which ship it is on
/// (lasers on one ship split the instability wave; a MOLE is one ship).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LaserInput {
    pub laser: String,
    pub modules: Vec<String>,
    #[serde(default)]
    pub owner: u32,
    /// Who works it: a crew mate (their ship brought along) or a MOLE
    /// seat's name. `None`: you.
    #[serde(default)]
    pub who: Option<String>,
    /// The crew loadout (its id) the laser came from, so taking that crew
    /// mate off the rock removes exactly their lasers.
    #[serde(default)]
    pub crew: Option<String>,
}

/// A crew mate's saved loadout, offered as support: their ship beside
/// yours on the rock.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CrewShip {
    /// The saved loadout's id.
    pub id: String,
    /// The crew mate's name.
    pub owner: String,
    /// The loadout's name ("Prospector").
    pub name: String,
    pub loadout: LoadoutInput,
}

/// What the scanner shows: the mass and the composition rows.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RockInput {
    /// Mass in tonnes (kg in the game's units: the scanner's "MASS").
    pub mass: f32,
    /// The composition rows, inert left out (inert is the remainder).
    pub entries: Vec<EntryInput>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EntryInput {
    /// Element id.
    pub element: String,
    /// The row's percentage, 0..100.
    pub percent: f32,
    pub quality: Option<u16>,
}

/// Why a solve could not run.
#[derive(Debug, Clone, PartialEq)]
pub enum SolveError {
    UnknownLaser(String),
    UnknownModule(String),
    UnknownGadget(String),
    UnknownElement(String),
    TooManyModules {
        laser: String,
        slots: u32,
    },
    NoMass,
    NoElements,
    /// More than 100 % in the composition.
    OverFull,
    /// An element without a density (the catalog never offers one).
    NoDensity,
}

impl std::fmt::Display for SolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownLaser(id) => write!(f, "unknown laser {id}"),
            Self::UnknownModule(id) => write!(f, "unknown module {id}"),
            Self::UnknownGadget(id) => write!(f, "unknown gadget {id}"),
            Self::UnknownElement(id) => write!(f, "unknown element {id}"),
            Self::TooManyModules { laser, slots } => {
                write!(f, "{laser} has {slots} module slots")
            }
            Self::NoMass => write!(f, "the rock needs a mass"),
            Self::NoElements => write!(f, "the rock needs at least one element"),
            Self::OverFull => write!(f, "the composition adds up to more than 100 %"),
            Self::NoDensity => write!(f, "an element has no density"),
        }
    }
}

impl std::error::Error for SolveError {}

/// Everything the page shows for one rock and loadout.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    pub rock: RockFacts,
    /// `None` without a laser.
    pub yours: Option<YourSetup>,
    /// Gadgets with the loadout as it is, best first ("no gadget" included).
    pub gadgets: Vec<GadgetRow>,
    /// Module kits for the one laser (single-laser ships only), passive.
    pub passive_kits: Vec<KitRow>,
    /// Module kits with an active module running.
    pub active_kits: Vec<KitRow>,
    /// One Prospector beside the loadout and gadget as they are.
    pub support: Vec<SupportRow>,
    /// Each crew ship beside the loadout and gadget as they are, best first.
    pub crew: Vec<CrewRow>,
}

/// The rock on its own.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RockFacts {
    pub mass: f32,
    pub scu: f32,
    pub resistance: f32,
    pub instability: f32,
    pub window_low: f32,
    pub window_high: f32,
    /// With the gadget on but no beam: what the scanner shows before the
    /// beam lands.
    pub gadget_resistance: f32,
    pub gadget_instability: f32,
}

/// What lasers put on the rock: their power and their change to the rock's
/// resistance, instability and window, as fractions (`-0.3`: 30 % less).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct Effect {
    pub power: f32,
    pub resistance: f32,
    pub instability: f32,
    pub window: f32,
}

/// One laser and whoever works it: what it brings and what it does.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Operator {
    /// The laser's index in the loadout.
    pub index: u32,
    pub laser: String,
    /// The ship it is on.
    pub owner: u32,
    pub who: Option<String>,
    pub crew: Option<String>,
    /// The lead follows the guide's steps; the others hold.
    pub lead: bool,
    /// A support's throttle through the climb and the window.
    pub hold: Option<f32>,
    pub effect: Effect,
    /// How far one wave lifts the charge at this operator's throttles, the
    /// others at theirs.
    pub jumps: Vec<JumpRow>,
}

/// One moment of the plan for everyone: each operator's throttle, the
/// wave's lift, and each operator's share of it (their share of the gain).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TotalRow {
    pub label: Option<String>,
    pub throttles: Vec<f32>,
    pub big: f32,
    pub usual: f32,
    pub shares: Vec<f32>,
}

/// The loadout as given.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct YourSetup {
    pub eval: Eval,
    /// The lasers without the gadget: what the ship (or crew) puts on the rock.
    pub effect: Effect,
    /// One per laser, in loadout order.
    pub operators: Vec<Operator>,
    /// The wave for everyone at the plan's moments (the lead's jump rows).
    pub total: Vec<TotalRow>,
    /// What the scanner should read with the beam on (it truncates
    /// resistance to whole percent).
    pub scanner_resistance_pct: f32,
    pub scanner_instability: f32,
    /// What to do at each charge level: the throttle scale, the wave's
    /// reach, the way back from an overshoot. With a crew, the lead's.
    pub guide: ThrottleGuide,
}

/// How a setup does on the rock.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Eval {
    /// The heaviest breakable mass over the rock's mass: ≥ 1 cracks.
    pub margin: f32,
    /// Fracturing energy per second at full throttle.
    pub power: f32,
    /// Energy per second the rock needs.
    pub need: f32,
    pub resistance: f32,
    pub instability: f32,
    pub window_low: f32,
    pub window_high: f32,
    /// The throttle that holds the bar still at the window's centre.
    pub balance_throttle: Option<f32>,
    /// `None` when the rock cannot be cracked.
    pub difficulty: Option<Difficulty>,
}

impl Eval {
    fn of(ruleset: &Ruleset, rock: &Rock, setup: &Setup, sim: bool) -> Self {
        let m = ModifiedRock::new(ruleset, rock, setup);
        let difficulty = (sim && m.is_breakable(ruleset)).then(|| {
            difficulty(
                &m,
                ruleset,
                &Player::default(),
                &SimConfig::default(),
                SIM_RUNS,
            )
        });
        Self {
            margin: m.max_mass(ruleset) / rock.mass,
            power: m.power(),
            need: rock.mass * ruleset.decay_per_mass / (1.0 - m.resistance).max(1e-6),
            resistance: m.resistance,
            instability: m.instability,
            window_low: m.window.low,
            window_high: m.window.high,
            balance_throttle: m.balance_throttle(ruleset),
            difficulty,
        }
    }

    /// A clean crack first, then margin.
    fn score(&self) -> (f32, f32) {
        (self.difficulty.map_or(-1.0, |d| d.controlled), self.margin)
    }
}

fn better(a: &Eval, b: &Eval) -> std::cmp::Ordering {
    let (a, b) = (a.score(), b.score());
    b.0.total_cmp(&a.0).then(b.1.total_cmp(&a.1))
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GadgetRow {
    /// `None` for "no gadget".
    pub gadget: Option<String>,
    pub eval: Eval,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct KitRow {
    /// Module ids, in slot order.
    pub modules: Vec<String>,
    /// The shortest active duration in the kit, when it runs out before the
    /// median fracture.
    pub runs_out_s: Option<f32>,
    pub eval: Eval,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SupportRow {
    pub laser: String,
    pub modules: Vec<String>,
    pub eval: Eval,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CrewRow {
    /// The crew loadout's id.
    pub id: String,
    pub owner: String,
    pub name: String,
    pub eval: Eval,
}

/// Every multiset of up to `k` indices below `n`.
fn kits(n: usize, k: usize) -> Vec<Vec<usize>> {
    let mut out = vec![vec![]];
    let mut frontier = vec![vec![]];
    for _ in 0..k {
        frontier = frontier
            .into_iter()
            .flat_map(|v: Vec<usize>| {
                let start = v.last().copied().unwrap_or(0);
                (start..n).map(move |i| {
                    let mut w = v.clone();
                    w.push(i);
                    w
                })
            })
            .collect();
        out.extend(frontier.iter().cloned());
    }
    out
}

fn rock_of(catalog: &Catalog, input: &RockInput) -> Result<Rock, SolveError> {
    if input.mass.is_nan() || input.mass <= 0.0 {
        return Err(SolveError::NoMass);
    }
    let entries = input
        .entries
        .iter()
        .filter(|e| e.percent > 0.0)
        .map(|e| {
            let element = catalog
                .element(&e.element)
                .ok_or_else(|| SolveError::UnknownElement(e.element.clone()))?;
            Ok(Entry {
                element: element.value,
                fraction: e.percent / 100.0,
                quality: e.quality,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if entries.is_empty() {
        return Err(SolveError::NoElements);
    }
    if entries.iter().map(|e| e.fraction).sum::<f32>() > 1.0 + 1e-3 {
        return Err(SolveError::OverFull);
    }
    Rock::from_mass(&catalog.ruleset, &catalog.elements, &entries, input.mass)
        .ok_or(SolveError::NoDensity)
}

/// The loadout's lasers (with modules) and gadgets, from the catalog.
pub(crate) fn setup_of(
    catalog: &Catalog,
    loadout: &LoadoutInput,
) -> Result<(Vec<Laser>, Vec<MiningGadget>), SolveError> {
    let gadgets = match &loadout.gadget {
        None => vec![],
        Some(id) => vec![
            catalog
                .gadget(id)
                .ok_or_else(|| SolveError::UnknownGadget(id.clone()))?
                .value
                .clone(),
        ],
    };
    Ok((lasers_of(catalog, loadout)?, gadgets))
}

fn lasers_of(catalog: &Catalog, input: &LoadoutInput) -> Result<Vec<Laser>, SolveError> {
    input
        .lasers
        .iter()
        .map(|l| {
            let item = catalog
                .laser(&l.laser)
                .ok_or_else(|| SolveError::UnknownLaser(l.laser.clone()))?;
            if l.modules.len() > item.value.stats.module_slots as usize {
                return Err(SolveError::TooManyModules {
                    laser: item.name.clone(),
                    slots: item.value.stats.module_slots,
                });
            }
            let mut laser = Laser::new(item.value.stats.clone());
            laser.owner = l.owner;
            laser.modules = l
                .modules
                .iter()
                .map(|id| {
                    catalog
                        .module(id)
                        .map(|m| m.value.clone())
                        .ok_or_else(|| SolveError::UnknownModule(id.clone()))
                })
                .collect::<Result<_, _>>()?;
            Ok(laser)
        })
        .collect()
}

/// `lasers`' change to the bare `rock`, no gadget.
fn effect_of(ruleset: &Ruleset, rock: &Rock, lasers: Vec<Laser>) -> Effect {
    let m = ModifiedRock::new(
        ruleset,
        rock,
        &Setup {
            lasers,
            gadgets: vec![],
        },
    );
    let rel = |with: f32, bare: f32| if bare > 0.0 { with / bare - 1.0 } else { 0.0 };
    Effect {
        power: m.power(),
        resistance: rel(m.resistance, rock.resistance),
        instability: rel(m.instability, rock.instability),
        window: rel(m.window.size(), rock.window.size()),
    }
}

/// `rock` with every laser but `free` folded in as fixed gain at its
/// throttle (a laser whose minimum is 1 contributes its power at any
/// throttle), so the guide's throttle is `free`'s alone.
fn folded(rock: &ModifiedRock, free: usize, throttles: &[f32]) -> ModifiedRock {
    let mut m = rock.clone();
    for (i, l) in m.lasers.iter_mut().enumerate() {
        if i != free {
            *l = LaserPower {
                power: l.power * throttles[i].clamp(l.throttle_minimum, 1.0),
                throttle_minimum: 1.0,
            };
        }
    }
    m
}

/// The crew plan: the lead follows the steps, the supports hold one
/// throttle through the climb and the window. The supports' throttle puts
/// the lead's balance mid-way through the lead's range, so the lead can
/// climb past it and drop below it.
struct Plan {
    lead: usize,
    /// Each laser's hold (the lead's entry is unused).
    holds: Vec<f32>,
    guide: ThrottleGuide,
}

fn plan(ruleset: &Ruleset, rock: &ModifiedRock, beam_off: &ModifiedRock, lead: usize) -> Plan {
    let lasers = &rock.lasers;
    let total: f32 = lasers.iter().map(|l| l.power).sum();
    let full = rock.gain_rate(ruleset, 1.0);
    let decay = full - rock.net_rate(ruleset, 1.0);
    let lead_power = lasers[lead].power;
    let lead_min = lasers[lead].throttle_minimum;
    let support_power = total - lead_power;
    let mut holds = vec![1.0; lasers.len()];
    if support_power > 0.0 && full > 0.0 {
        // Gain per unit of energy, then the energy the supports cover with
        // the lead at the middle of its range when the bar holds still.
        let k = full / total;
        let target = (lead_min + 1.0) / 2.0;
        let s = (decay / k - lead_power * target) / support_power;
        for (i, l) in lasers.iter().enumerate() {
            if i != lead {
                holds[i] = s.clamp(l.throttle_minimum, 1.0);
            }
        }
    }
    let guide = if lasers.len() > 1 {
        throttle_guide_lead(&folded(rock, lead, &holds), beam_off, ruleset, lead_min)
    } else {
        throttle_guide(rock, beam_off, ruleset)
    };
    Plan { lead, holds, guide }
}

/// The lead's throttle while the supports hold: the guide's hold, else its
/// balance, else the minimum.
fn lead_hold(g: &ThrottleGuide) -> f32 {
    g.hold_throttle
        .or(g.balance)
        .unwrap_or(g.throttle_minimum)
        .min(1.0)
}

/// Each operator (one per laser) with their effect, role and wave rows,
/// and the wave for everyone at the lead's jump rows.
fn operators(
    ruleset: &Ruleset,
    rock: &Rock,
    modified: &ModifiedRock,
    lasers: &[Laser],
    inputs: &[LaserInput],
    plan: &Plan,
) -> (Vec<Operator>, Vec<TotalRow>) {
    let n = lasers.len();
    let mut at = plan.holds.clone();
    at[plan.lead] = lead_hold(&plan.guide);
    let ops = (0..n)
        .map(|i| {
            let input = &inputs[i];
            let lead = i == plan.lead;
            let jumps = if lead {
                plan.guide.jumps.clone()
            } else {
                // Around the support's hold, everyone else at theirs.
                let rock_i = folded(modified, i, &at);
                let min = modified.lasers[i].throttle_minimum;
                let hold = plan.holds[i];
                let mut rows: Vec<JumpRow> = Vec::new();
                for (t, label) in [
                    (hold - 0.05, None),
                    (hold, Some("hold")),
                    (hold + 0.05, None),
                ] {
                    let t = t.clamp(min, 1.0);
                    if rows.iter().any(|r| (r.throttle - t).abs() < 0.005) {
                        continue;
                    }
                    let (big, usual) = wave_lift(&rock_i, ruleset, t);
                    rows.push(JumpRow {
                        throttle: t,
                        label: label.map(str::to_string),
                        big,
                        usual,
                    });
                }
                rows
            };
            Operator {
                index: i as u32,
                laser: input.laser.clone(),
                owner: input.owner,
                who: input.who.clone(),
                crew: input.crew.clone(),
                lead,
                hold: (!lead && n > 1).then(|| plan.holds[i]),
                effect: effect_of(ruleset, rock, vec![lasers[i].clone()]),
                jumps,
            }
        })
        .collect();
    let total = plan
        .guide
        .jumps
        .iter()
        .map(|j| {
            let mut throttles = plan.holds.clone();
            throttles[plan.lead] = j.throttle;
            let throttles: Vec<f32> = throttles
                .iter()
                .zip(&modified.lasers)
                .map(|(t, l)| t.clamp(l.throttle_minimum, 1.0))
                .collect();
            let gains: Vec<f32> = modified
                .lasers
                .iter()
                .zip(&throttles)
                .map(|(l, t)| l.power * t)
                .collect();
            let sum: f32 = gains.iter().sum();
            TotalRow {
                label: j.label.clone(),
                throttles,
                big: j.big,
                usual: j.usual,
                shares: gains
                    .iter()
                    .map(|g| if sum > 0.0 { g / sum } else { 0.0 })
                    .collect(),
            }
        })
        .collect();
    (ops, total)
}

/// The lead's laser index: the loadout's pick, else the first laser you
/// work yourself, else the first.
fn lead_of(loadout: &LoadoutInput) -> usize {
    loadout
        .lead
        .map(|i| i as usize)
        .filter(|i| *i < loadout.lasers.len())
        .or_else(|| loadout.lasers.iter().position(|l| l.who.is_none()))
        .unwrap_or(0)
}

/// Graded masses along the band (each a short player-model simulation).
const BAND_SAMPLES: usize = 40;

/// The loadout's class along the mass axis for this composition: where a
/// rock like this one is too much power, easy, manageable, hard, or out of
/// reach — graded by our player model, like the outcome.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MassBand {
    /// The axis' end, tonnes.
    pub top: f32,
    /// The heaviest breakable mass of this composition.
    pub max_mass: f32,
    pub segments: Vec<BandSegment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct BandSegment {
    pub from: f32,
    pub to: f32,
    pub class: crate::generate::RockClass,
}

/// The class band for `rock`'s composition and `loadout`, from the empty
/// rock to about twice the heavier of the scanned mass and the limit.
pub fn mass_band(
    catalog: &Catalog,
    loadout: &LoadoutInput,
    rock: &RockInput,
) -> Result<MassBand, SolveError> {
    let rs = &catalog.ruleset;
    let scanned = rock_of(catalog, rock)?;
    let (lasers, gadgets) = setup_of(catalog, loadout)?;
    let setup = Setup { lasers, gadgets };
    let max_mass = ModifiedRock::new(rs, &scanned, &setup).max_mass(rs);
    let reach = scanned.mass.max(if max_mass.is_finite() {
        max_mass
    } else {
        scanned.mass
    });
    // A round axis end: the next 1,000 t (500 t below 5,000 t).
    let step = if reach * 1.6 < 5000.0 { 500.0 } else { 1000.0 };
    let top = ((reach * 1.6) / step).ceil().max(1.0) * step;
    let mut segments: Vec<BandSegment> = Vec::new();
    for k in 0..BAND_SAMPLES {
        let from = top * k as f32 / BAND_SAMPLES as f32;
        let to = top * (k + 1) as f32 / BAND_SAMPLES as f32;
        let at = RockInput {
            mass: (from + to) / 2.0,
            entries: rock.entries.clone(),
        };
        let class = crate::generate::grade(
            rs,
            &ModifiedRock::new(rs, &rock_of(catalog, &at)?, &setup),
            crate::generate::GRADE_RUNS,
        );
        match segments.last_mut() {
            Some(s) if s.class == class => s.to = to,
            _ => segments.push(BandSegment { from, to, class }),
        }
    }
    Ok(MassBand {
        top,
        max_mass,
        segments,
    })
}

/// Solve `rock` against `loadout`.
pub fn solve(
    catalog: &Catalog,
    loadout: &LoadoutInput,
    rock: &RockInput,
    crew: &[CrewShip],
) -> Result<Report, SolveError> {
    let rs = &catalog.ruleset;
    let rock = rock_of(catalog, rock)?;
    let (lasers, gadgets) = setup_of(catalog, loadout)?;
    let on = |lasers: Vec<Laser>, gadgets: Vec<MiningGadget>| Setup { lasers, gadgets };

    let gadget_only = ModifiedRock::new(rs, &rock, &on(vec![], gadgets.clone()));
    let facts = RockFacts {
        mass: rock.mass,
        scu: rock.scu,
        resistance: rock.resistance,
        instability: rock.instability,
        window_low: rock.window.low,
        window_high: rock.window.high,
        gadget_resistance: gadget_only.resistance,
        gadget_instability: gadget_only.instability,
    };
    if lasers.is_empty() {
        return Ok(Report {
            rock: facts,
            yours: None,
            gadgets: vec![],
            passive_kits: vec![],
            active_kits: vec![],
            support: vec![],
            crew: vec![],
        });
    }

    let mine = on(lasers.clone(), gadgets.clone());
    let modified = ModifiedRock::new(rs, &rock, &mine);
    let crew_plan = plan(rs, &modified, &gadget_only, lead_of(loadout));
    let (operators, total) = operators(rs, &rock, &modified, &lasers, &loadout.lasers, &crew_plan);
    let yours = YourSetup {
        eval: Eval::of(rs, &rock, &mine, true),
        effect: effect_of(rs, &rock, lasers.clone()),
        operators,
        total,
        scanner_resistance_pct: (modified.resistance * 100.0).floor(),
        scanner_instability: modified.instability,
        guide: crew_plan.guide,
    };

    // Gadgets with the loadout as it is.
    let mut by_gadget: Vec<GadgetRow> = std::iter::once(None)
        .chain(catalog.gadgets.iter().map(Some))
        .map(|g| GadgetRow {
            gadget: g.map(|g| g.id.clone()),
            eval: Eval::of(
                rs,
                &rock,
                &on(
                    lasers.clone(),
                    g.map(|g| g.value.clone()).into_iter().collect(),
                ),
                true,
            ),
        })
        .collect();
    by_gadget.sort_by(|a, b| better(&a.eval, &b.eval));

    // Module swaps, single-laser ships only.
    let (mut passive_kits, mut active_kits) = (vec![], vec![]);
    if let [laser] = lasers.as_slice() {
        let fitted = |kit: &[usize]| {
            let mut l = laser.clone();
            l.modules = kit
                .iter()
                .map(|&i| catalog.modules[i].value.clone())
                .collect();
            on(vec![l], gadgets.clone())
        };
        let all = kits(catalog.modules.len(), laser.stats.module_slots as usize);
        for (active, out) in [(false, &mut passive_kits), (true, &mut active_kits)] {
            // Simulate only the strongest few by margin, then rank by the sim.
            let mut rows: Vec<(&Vec<usize>, f32)> = all
                .iter()
                .filter(|kit| kit.iter().any(|&i| is_active(&catalog.modules[i].value)) == active)
                .map(|kit| (kit, Eval::of(rs, &rock, &fitted(kit), false).margin))
                .collect();
            rows.sort_by(|a, b| b.1.total_cmp(&a.1));
            let mut rows: Vec<KitRow> = rows
                .into_iter()
                .take(KIT_SIMULATED)
                .map(|(kit, _)| {
                    let eval = Eval::of(rs, &rock, &fitted(kit), true);
                    // An active module that runs out before the median fracture.
                    let shortest = kit
                        .iter()
                        .filter_map(|&i| match catalog.modules[i].value.activation {
                            svc_data::mining::Activation::Active { duration_s, .. } => duration_s,
                            svc_data::mining::Activation::Passive => None,
                        })
                        .reduce(f32::min);
                    let runs_out_s = match (shortest, eval.difficulty.and_then(|d| d.median_time_s))
                    {
                        (Some(d), Some(t)) if t > d => Some(d),
                        _ => None,
                    };
                    KitRow {
                        modules: kit.iter().map(|&i| catalog.modules[i].id.clone()).collect(),
                        runs_out_s,
                        eval,
                    }
                })
                .collect();
            rows.sort_by(|a, b| better(&a.eval, &b.eval));
            rows.truncate(KIT_ROWS);
            *out = rows;
        }
    }

    // Support: one Prospector (any size-1 laser but the Golem's Pitman),
    // bare and with the smallest passive kit that cracks with some margin.
    let passive: Vec<usize> = (0..catalog.modules.len())
        .filter(|&i| !is_active(&catalog.modules[i].value))
        .collect();
    let support_owner = lasers.iter().map(|l| l.owner).max().unwrap_or(0) + 1;
    let mut support: Vec<SupportRow> = Vec::new();
    for l in catalog.lasers.iter().filter(|l| {
        l.value.size == 1
            && !l.name.to_ascii_lowercase().contains("golem")
            && !l.record_name.to_ascii_lowercase().contains("golem")
    }) {
        let with = |kit: &[usize]| {
            let mut p = Laser::new(l.value.stats.clone());
            p.modules = kit
                .iter()
                .map(|&i| catalog.modules[i].value.clone())
                .collect();
            p.owner = support_owner;
            let mut all = lasers.clone();
            all.push(p);
            on(all, gadgets.clone())
        };
        let mut best: Option<(f32, Vec<usize>)> = None;
        for kit in kits(passive.len(), l.value.stats.module_slots as usize) {
            let kit: Vec<usize> = kit.iter().map(|&i| passive[i]).collect();
            let e = Eval::of(rs, &rock, &with(&kit), false);
            // The smallest kit that cracks with some margin, else the strongest.
            let key = if e.margin >= 1.1 {
                10.0 - kit.len() as f32 - e.margin / 100.0
            } else {
                e.margin
            };
            if best.as_ref().is_none_or(|(b, _)| key > *b) {
                best = Some((key, kit));
            }
        }
        for kit in std::iter::once(vec![]).chain(best.map(|b| b.1)) {
            let modules: Vec<String> = kit.iter().map(|&i| catalog.modules[i].id.clone()).collect();
            if support
                .iter()
                .any(|r| r.laser == l.id && r.modules == modules)
            {
                continue;
            }
            support.push(SupportRow {
                laser: l.id.clone(),
                modules,
                eval: Eval::of(rs, &rock, &with(&kit), true),
            });
        }
    }
    support.sort_by(|a, b| better(&a.eval, &b.eval));
    support.truncate(SUPPORT_ROWS);

    // Crew: each crew ship (all its lasers on one more ship) beside yours.
    // A crew loadout naming parts this build lacks is left out.
    let mut crew_rows: Vec<CrewRow> = crew
        .iter()
        .filter_map(|c| {
            let theirs = lasers_of(catalog, &c.loadout).ok()?;
            let mut all = lasers.clone();
            all.extend(theirs.into_iter().map(|mut l| {
                l.owner += support_owner;
                l
            }));
            Some(CrewRow {
                id: c.id.clone(),
                owner: c.owner.clone(),
                name: c.name.clone(),
                eval: Eval::of(rs, &rock, &on(all, gadgets.clone()), true),
            })
        })
        .collect();
    crew_rows.sort_by(|a, b| better(&a.eval, &b.eval));

    Ok(Report {
        rock: facts,
        yours: Some(yours),
        gadgets: by_gadget,
        passive_kits,
        active_kits,
        support,
        crew: crew_rows,
    })
}

#[cfg(test)]
mod tests {
    use super::kits;

    #[test]
    fn kits_are_multisets_up_to_k() {
        // n = 3, k = 2: {} + 3 singles + 6 pairs.
        let all = kits(3, 2);
        assert_eq!(all.len(), 10);
        assert!(all.contains(&vec![1, 1]));
        assert!(!all.contains(&vec![2, 1]));
    }
}
