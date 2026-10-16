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
    Difficulty, Entry, Laser, MiningGadget, ModifiedRock, Player, Rock, Ruleset, Setup, SimConfig,
    difficulty,
};

use crate::catalog::{Catalog, is_active};
use crate::guide::{ThrottleGuide, throttle_guide};

/// Player-model attempts per evaluated setup.
pub const SIM_RUNS: u32 = 100;
/// How many setups the module search simulates, strongest margin first.
const KIT_SIMULATED: usize = 8;
/// Rows per suggestion table.
const GADGET_ROWS: usize = 4;
const KIT_ROWS: usize = 3;
const SUPPORT_ROWS: usize = 6;

/// The ship (or ships): lasers with their modules, and the gadget on the rock.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LoadoutInput {
    pub lasers: Vec<LaserInput>,
    /// Gadget id; `None` for no gadget.
    pub gadget: Option<String>,
}

/// One laser: its id, its fitted module ids, and which ship it is on
/// (lasers on one ship split the instability wave; a MOLE is one ship).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LaserInput {
    pub laser: String,
    pub modules: Vec<String>,
    #[serde(default)]
    pub owner: u32,
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

/// The loadout as given.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct YourSetup {
    pub eval: Eval,
    /// What the scanner should read with the beam on (it truncates
    /// resistance to whole percent).
    pub scanner_resistance_pct: f32,
    pub scanner_instability: f32,
    /// What to do at each charge level: the throttle scale, the wave's
    /// reach, the way back from an overshoot.
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

/// Solve `rock` against `loadout`.
pub fn solve(
    catalog: &Catalog,
    loadout: &LoadoutInput,
    rock: &RockInput,
) -> Result<Report, SolveError> {
    let rs = &catalog.ruleset;
    let rock = rock_of(catalog, rock)?;
    let lasers = lasers_of(catalog, loadout)?;
    let gadgets: Vec<MiningGadget> = match &loadout.gadget {
        None => vec![],
        Some(id) => vec![
            catalog
                .gadget(id)
                .ok_or_else(|| SolveError::UnknownGadget(id.clone()))?
                .value
                .clone(),
        ],
    };
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
        });
    }

    let mine = on(lasers.clone(), gadgets.clone());
    let modified = ModifiedRock::new(rs, &rock, &mine);
    let yours = YourSetup {
        eval: Eval::of(rs, &rock, &mine, true),
        scanner_resistance_pct: (modified.resistance * 100.0).floor(),
        scanner_instability: modified.instability,
        guide: throttle_guide(&modified, &gadget_only, rs),
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
    by_gadget.truncate(GADGET_ROWS);

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

    Ok(Report {
        rock: facts,
        yours: Some(yours),
        gadgets: by_gadget,
        passive_kits,
        active_kits,
        support,
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
