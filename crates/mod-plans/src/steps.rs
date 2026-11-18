//! The steps your inventory leaves open, per plan.
//!
//! Nothing here is stored: a step exists while the account's state says it
//! is still to do, and is gone once the game shows it done. Each step is
//! phrased as an action with its reason. A figure is only stated where the
//! shell handed in a real one (a UEX price, a recipe's craft time, the share
//! of rocks that hold an ore); what the data cannot give is left out: how
//! much ore a refinery turns into how much material, and how long it takes.

use std::collections::HashMap;

use serde::Serialize;

use crate::ledger::{self, Demand, Fill, LedgerRow, Stack};
use crate::model::{CraftGoal, Goal, PartSource, Plan, PlansState};

/// A place by its display name, and the quantum-travel place it resolves to
/// when the shell could tell (the id is opaque here).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PlanPlace {
    pub name: String,
    pub site: Option<String>,
}

/// Everything the derivation reads, gathered by the shell.
#[derive(Debug, Clone, Default)]
pub struct Inputs {
    /// Materials you hold.
    pub stock: Vec<Stack>,
    /// Parts you hold that are free to fit (not on a ship, not rented), by
    /// item GUID.
    pub parts: HashMap<String, Vec<HeldPart>>,
    /// By saved loadout id.
    pub loadouts: HashMap<String, LoadoutFacts>,
    /// By blueprint GUID.
    pub recipes: HashMap<String, RecipeFacts>,
    /// The mining ship you would fly and its ore hold: a system's ore is
    /// split into loads of this size. `None`: one load per system.
    pub hold: Option<OreHold>,
    /// Your standing and the missions, for rank and mission plans.
    pub contracts: crate::contracts::ContractInputs,
}

/// A mining ship's ore hold, its pods as fitted.
#[derive(Debug, Clone, PartialEq)]
pub struct OreHold {
    /// The ship's name ("MISC Prospector").
    pub ship: String,
    pub scu: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HeldPart {
    pub at: PlanPlace,
    pub count: u32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LoadoutFacts {
    pub name: String,
    pub ship: String,
    pub read: ShipRead,
    pub parts: Vec<PartFacts>,
}

/// Whether the snapshot shows what is fitted on a copy of the ship.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ShipRead {
    /// A copy you own was read; `PartFacts::fitted` is what it carries.
    Fitted,
    /// You own the ship, but the snapshot has no read of its ports.
    #[default]
    NoRead,
    NotOwned,
}

/// One part the loadout swaps in.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PartFacts {
    pub item: String,
    pub name: String,
    pub fitted: bool,
    /// Shops that sell it, cheapest first.
    pub offers: Vec<Offer>,
    /// The blueprint that crafts it, when there is one.
    pub blueprint: Option<String>,
    /// What players ask for it on the UEX marketplace, when anyone lists it.
    pub market: Option<MarketOffer>,
    /// Where it drops in containers placed on the map, when anywhere.
    pub loot: Option<LootOffer>,
}

/// Where a part drops: the best container fill and how widely it spreads.
#[derive(Debug, Clone, PartialEq)]
pub struct LootOffer {
    /// The chance the best fill yields one, 0 to 1.
    pub chance: f64,
    /// Where that fill sits most often.
    pub at: PlanPlace,
    /// How many places it drops at, over every fill.
    pub places: u32,
}

/// The player market's ask for a part: the average of the listings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarketOffer {
    /// aUEC.
    pub price: f64,
    pub listings: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Offer {
    pub shop: String,
    pub at: PlanPlace,
    /// aUEC.
    pub price: f64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RecipeFacts {
    /// The crafted item's name.
    pub name: String,
    /// You own the blueprint.
    pub owned: bool,
    pub craft_seconds: Option<f32>,
    pub slots: Vec<SlotNeed>,
    pub grants: Vec<Grant>,
    /// The crafted item's GUID: crafted copies of it are taken out of the
    /// free parts, so one copy serves one plan.
    pub entity: Option<String>,
    /// Crafted copies of the item you hold that are free to use, with what
    /// went into each: a copy counts for a craft whose minimums it reaches.
    pub crafted: Vec<CraftedCopy>,
    /// The blueprint's family key on the Crafting page.
    pub family: Option<String>,
}

/// Crafted copies of one item made from the same materials.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CraftedCopy {
    pub count: u32,
    /// The material and quality of each ingredient.
    pub ingredients: Vec<(String, u32)>,
}

impl CraftedCopy {
    /// Whether every ingredient reaches the minimum of the slots that take
    /// its material (the strictest of them; a material no slot names has to
    /// reach the highest minimum).
    fn reaches(&self, slots: &[SlotNeed], mins: &[u32]) -> bool {
        let top = mins.iter().copied().max().unwrap_or(0);
        self.ingredients.iter().all(|(material, quality)| {
            let need = slots
                .iter()
                .zip(mins)
                .filter(|(s, _)| s.material == *material)
                .map(|(_, min)| *min)
                .max()
                .unwrap_or(top);
            *quality >= need
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SlotNeed {
    /// "Frame", "Emitter"; `None` for a slot the game has not named.
    pub name: Option<String>,
    pub material: String,
    /// SCU for bulk, pieces else, for one craft.
    pub amount: f64,
    pub bulk: bool,
    /// The recipe's own minimum for the slot.
    pub min_quality: u32,
    /// The minimum a plan aims for when nobody set one: the stock quality
    /// (Q500, where a material neither helps nor hurts) for a slot that
    /// moves stats, since anything below makes the item worse; the recipe's
    /// own minimum for a slot that moves none (Valerie, 2026-11-16). Never
    /// below `min_quality`.
    pub aim: u32,
    /// The stats the slot's quality moves ("Integrity"), for showing beside
    /// its minimum.
    pub moves: Vec<String>,
    /// The qualities the material comes in, lowest first; empty when the
    /// data gives none.
    pub bands: Vec<QualityBand>,
}

/// One quality a material comes in: the game sorts a mined resource's
/// quality into bands and gives every roll from `from` to `to` the same
/// `value`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct QualityBand {
    pub from: u32,
    pub to: u32,
    pub value: u32,
}

/// A mission that grants a blueprint.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Grant {
    pub title: String,
    pub at: Vec<PlanPlace>,
    /// CIG's duration for the mission, the one its payout assumes
    /// (`time_to_complete`), in minutes; `None` when it has none.
    pub minutes: Option<f32>,
}

/// Where an ore can be ship-mined at a quality.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OreSite {
    pub at: PlanPlace,
    /// The share of the ship rocks there that hold it at that quality.
    pub find: f32,
    /// The refinery nearest to this place, in its system: the area its ore
    /// is mined and refined in, one load. `None` off the map.
    pub refine_at: Option<PlanPlace>,
    /// The place's system: a load is the ore mined in one system, never
    /// flown through a jump point to a refinery. `None` off the map.
    pub system: Option<String>,
    /// SCU of the ore at the quality in a rock there that holds it so;
    /// `None` when unknown.
    pub ore_scu: Option<f32>,
    /// SCU of such a whole rock: breaking one puts all of it in the hold.
    /// `None` when unknown.
    pub rock_scu: Option<f32>,
}

/// What mining knows about a material at a quality.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OreSites {
    /// Best first; empty when no place rolls it at that quality.
    pub sites: Vec<OreSite>,
    /// The material is ship-mined at all, at any quality.
    pub mined: bool,
    /// The ore's name when the material is what a refinery makes of it: the
    /// rocks hold the ore, the recipe wants the refined material.
    pub ore: Option<String>,
    /// How many places have a refinery.
    pub refineries: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum StepKind {
    /// Fit a part you hold.
    Fit,
    Buy,
    Mine,
    /// Take mined ore to a refinery: the recipe wants the refined material.
    Refine,
    /// Get a material no ship-mining place gives.
    Gather,
    Craft,
    /// Earn a blueprint from a mission.
    Earn,
    /// A part no shop sells and no blueprint makes.
    Find,
    /// Loot a part from containers in the world.
    Loot,
    /// Earn rep with a faction, up to its next rank.
    Rep,
    /// Run a mission.
    Run,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PlanStep {
    pub id: String,
    pub kind: StepKind,
    /// The action: "Fit the … you own".
    pub title: String,
    /// Its reason, or what it waits on.
    pub detail: String,
    /// The plans it moves, in plan order.
    pub plans: Vec<String>,
    pub blocked: bool,
    pub at: Option<PlanPlace>,
    /// Time the step itself takes, where the data gives one: a recipe's
    /// craft time; for an earn step, CIG's estimate for the shortest
    /// mission that grants the blueprint.
    pub seconds: Option<f64>,
    /// aUEC.
    pub cost: Option<f64>,
    /// Mining: the share of the rocks at the place that hold the ore at the
    /// wanted quality. Looting: the chance the best container fill yields
    /// the part.
    pub odds: Option<f32>,
    /// How many places it can be done at (mining places, shops, missions,
    /// refineries, places the part drops).
    pub sources: u32,
    /// The steps this one follows (their ids): refining comes after the
    /// mining that gives the ore. A trip never puts it before them.
    pub after: Vec<String>,
    /// Parts it buys, fits or loots; 0 for any other step.
    pub parts: u32,
    /// Mining: the rocks it takes, where the data gives the ore a rock
    /// holds.
    pub rocks: Option<Rocks>,
    /// Mining: the SCU it mines of a bulk material, as ore where a refinery
    /// makes the material (the shortfall over `REFINE_YIELD`); 0 for any
    /// other step.
    pub scu: f64,
}

/// The share of mined ore a refinery gives back as the material. The game
/// data has no yield (the yield types have no records); this is Valerie's
/// in-game figure (2026-11-17), to stand until a better source exists.
pub const REFINE_YIELD: f64 = 0.5;

/// The rocks a mining step takes (Valerie, 2026-11-17: a count of rocks
/// in place of a mining time the data cannot give).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Rocks {
    /// Rocks holding the ore at the quality, to break for the amount: the
    /// SCU wanted over the ore such a rock holds.
    pub to_break: u32,
    /// Rocks to scan to find those: `to_break` over the share of the rocks
    /// there that hold it at the quality.
    pub to_scan: u32,
}

/// The rocks `scu` of an ore takes at a place where a share `find` of the
/// rocks hold it, `ore_scu` each; `None` when either is unknown.
fn rocks(scu: f64, find: f32, ore_scu: Option<f32>) -> Option<Rocks> {
    let ore = f64::from(ore_scu.filter(|o| *o > 0.0)?);
    if find <= 0.0 || scu <= 0.0 {
        return None;
    }
    let to_break = (scu / ore).ceil().max(1.0);
    let to_scan = (to_break / f64::from(find)).ceil().max(to_break);
    Some(Rocks {
        to_break: to_break as u32,
        to_scan: to_scan as u32,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum PlanKind {
    Loadout,
    Craft,
    Project,
    Rank,
    Mission,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum StageState {
    Done,
    Current,
    Ahead,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PlanStage {
    pub name: String,
    pub sub: String,
    pub state: StageState,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PlanTile {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PlanBoard {
    pub id: String,
    pub kind: PlanKind,
    /// The ship, or the item.
    pub title: String,
    /// The loadout's name, or the count and quality.
    pub subtitle: String,
    /// The user's name for the plan, else title and subtitle.
    pub name: String,
    /// 0..=1.
    pub progress: f32,
    pub done: bool,
    /// The goal's loadout or blueprint is gone.
    pub missing: bool,
    /// Nothing can be done for it right now.
    pub waiting: bool,
    /// What comes next, or what it waits for.
    pub note: String,
    pub stages: Vec<PlanStage>,
    pub tiles: Vec<PlanTile>,
    /// Set aside: it reserves nothing and has no steps.
    pub parked: bool,
    pub notes: String,
    /// The crafts in it: one for a craft plan, each entry of a project,
    /// none for a loadout.
    pub crafts: Vec<PlanCraft>,
}

/// One craft of a plan, with the minimum quality of each material.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PlanCraft {
    pub blueprint: String,
    /// The crafted item's name.
    pub name: String,
    /// The blueprint's family key on the Crafting page.
    pub family: Option<String>,
    pub count: u32,
    /// Crafted copies you hold that count for it.
    pub crafted: u32,
    /// How many the reserved stock covers right now.
    pub now: u32,
    /// You own the blueprint.
    pub owned: bool,
    /// This build has no such blueprint.
    pub missing: bool,
    /// The one quality every slot is set to; `None` when they differ.
    pub quality: Option<u32>,
    pub slots: Vec<PlanSlot>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PlanSlot {
    pub name: Option<String>,
    pub material: String,
    /// SCU for bulk, pieces else, for one craft.
    pub amount: f64,
    pub bulk: bool,
    /// The minimum in force: the plan's, or the recipe's own when higher.
    pub min: u32,
    /// The recipe's own minimum: the plan cannot go below it.
    pub floor: u32,
    /// Your stock covers this slot for every craft still to make.
    pub held: bool,
    /// The stats this slot's quality moves.
    pub moves: Vec<String>,
    /// The qualities the material comes in, lowest first; empty when the
    /// data gives none. A minimum is picked from these.
    pub bands: Vec<QualityBand>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Board {
    pub plans: Vec<PlanBoard>,
    /// Ready steps first, each group in plan order.
    pub steps: Vec<PlanStep>,
    pub ledger: Vec<LedgerRow>,
    /// The parts the plans still need and where each can come from: the
    /// Components panel.
    pub parts: Vec<PartChoice>,
}

/// A part the plans still need (not fitted, no free copy held), with its
/// sources and the one the plan takes.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PartChoice {
    pub item: String,
    pub name: String,
    pub count: u32,
    pub plans: Vec<String>,
    /// `None`: no source at all (no shop, no listing, no blueprint).
    pub chosen: Option<PartSource>,
    /// The sources it has, in the panel's order (Store, Market, Craft).
    pub options: Vec<SourceOption>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SourceOption {
    pub source: PartSource,
    /// aUEC each; `None` for a craft.
    pub price: Option<f64>,
    /// The shop, the listings, or the blueprint's state.
    pub detail: String,
}

/// The sources a part has, in the panel's order.
fn sources_of(part: &PartFacts, inputs: &Inputs) -> Vec<PartSource> {
    let mut out = Vec::new();
    if !part.offers.is_empty() {
        out.push(PartSource::Store);
    }
    if part.market.is_some() {
        out.push(PartSource::Market);
    }
    if part
        .blueprint
        .as_ref()
        .is_some_and(|bp| inputs.recipes.contains_key(bp))
    {
        out.push(PartSource::Craft);
    }
    if part.loot.is_some() {
        out.push(PartSource::Loot);
    }
    out
}

/// The source a part is taken from: its own choice, else the preferred
/// one, else a shop, a craft, the market, loot, in that order (a shop
/// before a craft keeps what plans did before there was a choice; loot is
/// luck, so it comes last).
fn choose(state: &PlansState, part: &PartFacts, inputs: &Inputs) -> Option<PartSource> {
    let has = sources_of(part, inputs);
    state
        .sources
        .get(&part.item)
        .copied()
        .filter(|s| has.contains(s))
        .or(Some(state.prefer).filter(|s| has.contains(s)))
        .or_else(|| {
            [
                PartSource::Store,
                PartSource::Craft,
                PartSource::Market,
                PartSource::Loot,
            ]
            .into_iter()
            .find(|s| has.contains(s))
        })
}

/// One craft a plan wants: a craft project, or a loadout part only a
/// blueprint makes.
struct Craft<'a> {
    plan: &'a str,
    blueprint: &'a str,
    recipe: &'a RecipeFacts,
    /// How many the plan wants.
    count: u32,
    /// Still to craft.
    left: u32,
    /// Each recipe slot's minimum: the plan's, or the slot's own when
    /// higher.
    mins: Vec<u32>,
    /// The one quality the plan sets for every slot; `None` when they
    /// differ.
    quality: Option<u32>,
    /// Its plan is parked: it asks for nothing.
    parked: bool,
    /// (index into the demand list, amount for one craft).
    demands: Vec<(usize, f64)>,
    /// Each slot's demand, as an index into the demand list.
    slot_demand: Vec<Option<usize>>,
}

type Pool<'a> = HashMap<&'a str, Vec<(PlanPlace, u32)>>;

/// A plan's craft, with the crafted copies you hold counted towards it and
/// taken: a loadout ahead of it that fits a copy has it, not both. `used`
/// counts the copies already taken, per blueprint.
fn start_craft<'a>(
    plan: &'a Plan,
    goal: &'a CraftGoal,
    inputs: &'a Inputs,
    pool: &mut Pool<'a>,
    used: &mut HashMap<&'a str, Vec<u32>>,
) -> Option<Craft<'a>> {
    let recipe = inputs.recipes.get(&goal.blueprint)?;
    let mins: Vec<u32> = recipe
        .slots
        .iter()
        .enumerate()
        .map(|(i, slot)| slot.min_quality.max(goal.min_of(i)))
        .collect();
    let mut have = 0;
    if !plan.parked {
        let used = used
            .entry(goal.blueprint.as_str())
            .or_insert_with(|| vec![0; recipe.crafted.len()]);
        for (copy, taken) in recipe.crafted.iter().zip(used.iter_mut()) {
            if have >= goal.count {
                break;
            }
            if !copy.reaches(&recipe.slots, &mins) {
                continue;
            }
            let wanted = copy.count.saturating_sub(*taken).min(goal.count - have);
            let got = match recipe.entity.as_deref() {
                Some(entity) => take_many(pool.get_mut(entity), wanted),
                None => wanted,
            };
            *taken += got;
            have += got;
        }
    }
    Some(Craft {
        plan: &plan.id,
        blueprint: &goal.blueprint,
        recipe,
        count: goal.count,
        left: goal.count - have,
        slot_demand: vec![None; mins.len()],
        mins,
        quality: goal.is_uniform().then_some(goal.quality),
        parked: plan.parked,
        demands: Vec::new(),
    })
}

struct Part<'a> {
    facts: &'a PartFacts,
    /// A free copy reserved for it.
    held: Option<PlanPlace>,
    /// Where it comes from when it is still needed.
    source: Option<PartSource>,
}

struct LoadoutWork<'a> {
    plan: &'a str,
    facts: &'a LoadoutFacts,
    parts: Vec<Part<'a>>,
    parked: bool,
}

/// Derive every plan's steps, stages and figures, and the ledger across
/// them. `ores` answers where a material can be ship-mined at a quality.
pub fn derive(state: &PlansState, inputs: &Inputs, ores: &dyn Fn(&str, u32) -> OreSites) -> Board {
    // Free parts are counted once, like stock: reserved in plan order.
    let mut pool: Pool = inputs
        .parts
        .iter()
        .map(|(k, v)| {
            (
                k.as_str(),
                v.iter().map(|h| (h.at.clone(), h.count)).collect(),
            )
        })
        .collect();

    let mut loadouts: Vec<LoadoutWork> = Vec::new();
    let mut crafts: Vec<Craft> = Vec::new();
    let mut demands: Vec<Demand> = Vec::new();
    let mut used: HashMap<&str, Vec<u32>> = HashMap::new();

    for plan in &state.plans {
        match &plan.goal {
            Goal::Loadout(goal) => {
                let Some(facts) = inputs.loadouts.get(&goal.loadout) else {
                    continue;
                };
                let mut work = LoadoutWork {
                    plan: &plan.id,
                    facts,
                    parts: Vec::new(),
                    parked: plan.parked,
                };
                for part in &facts.parts {
                    let held = (!part.fitted && !plan.parked)
                        .then(|| take(pool.get_mut(part.item.as_str())))
                        .flatten();
                    let needed = !part.fitted && !plan.parked && held.is_none();
                    let source = needed.then(|| choose(state, part, inputs)).flatten();
                    if let (Some(PartSource::Craft), Some(bp)) = (source, &part.blueprint)
                        && let Some(recipe) = inputs.recipes.get(bp)
                    {
                        match crafts
                            .iter_mut()
                            .find(|c| c.plan == plan.id && c.blueprint == bp)
                        {
                            Some(c) => {
                                c.count += 1;
                                c.left += 1;
                            }
                            None => crafts.push(Craft {
                                plan: &plan.id,
                                blueprint: bp,
                                recipe,
                                count: 1,
                                left: 1,
                                mins: recipe
                                    .slots
                                    .iter()
                                    .map(|s| s.aim.max(s.min_quality))
                                    .collect(),
                                quality: Some(0),
                                parked: false,
                                demands: Vec::new(),
                                slot_demand: vec![None; recipe.slots.len()],
                            }),
                        }
                    }
                    work.parts.push(Part {
                        facts: part,
                        held,
                        source,
                    });
                }
                loadouts.push(work);
            }
            Goal::Craft(goal) => {
                crafts.extend(start_craft(plan, goal, inputs, &mut pool, &mut used));
            }
            Goal::Project(project) => {
                for goal in &project.crafts {
                    crafts.extend(start_craft(plan, goal, inputs, &mut pool, &mut used));
                }
            }
            Goal::Rank(_) | Goal::Mission(_) | Goal::Unknown => {}
        }
    }

    // Demands in plan order: `crafts` was filled in that order.
    // A parked plan asks for nothing: what it would need stays free.
    for craft in &mut crafts {
        if craft.left == 0 || craft.parked {
            continue;
        }
        for (n, slot) in craft.recipe.slots.iter().enumerate() {
            // Each slot at its own minimum: two slots of one material at
            // different minimums are two demands.
            let quality = craft.mins[n];
            let at = craft.demands.iter().position(|(i, _)| {
                let d: &Demand = &demands[*i];
                d.material == slot.material && d.min_quality == quality && d.bulk == slot.bulk
            });
            match at {
                Some(at) => {
                    let (i, each) = &mut craft.demands[at];
                    *each += slot.amount;
                    demands[*i].amount += slot.amount * f64::from(craft.left);
                    craft.slot_demand[n] = Some(*i);
                }
                None => {
                    craft.slot_demand[n] = Some(demands.len());
                    craft.demands.push((demands.len(), slot.amount));
                    demands.push(Demand {
                        plan: craft.plan.to_owned(),
                        material: slot.material.clone(),
                        min_quality: quality,
                        amount: slot.amount * f64::from(craft.left),
                        bulk: slot.bulk,
                    });
                }
            }
        }
    }
    let ledger = ledger::reserve(&inputs.stock, &demands);

    let mut ready: Vec<PlanStep> = Vec::new();
    let mut blocked: Vec<PlanStep> = Vec::new();
    let mut push = |step: PlanStep| {
        if step.blocked {
            blocked.push(step)
        } else {
            ready.push(step)
        }
    };

    // Fit and buy, per loadout.
    let mut fits: Vec<(PlanStep, u32)> = Vec::new();
    let mut finds: Vec<PlanStep> = Vec::new();
    let mut buys: Vec<PlanStep> = Vec::new();
    let mut buy_lines: Vec<Vec<(String, u32)>> = Vec::new();
    let mut market: Option<(PlanStep, Vec<(String, u32)>)> = None;
    // One step per looted part, whichever plans want it: (step, count).
    let mut loots: Vec<(PlanStep, u32)> = Vec::new();
    let mut parts: Vec<PartChoice> = Vec::new();
    for work in loadouts.iter().filter(|w| !w.parked) {
        for (n, part) in work.parts.iter().enumerate() {
            let facts = part.facts;
            if facts.fitted {
                continue;
            }
            if part.held.is_none() {
                note_part(&mut parts, work.plan, part, inputs);
            }
            if let Some(at) = &part.held {
                // Two ports wanting the same part from the same place are
                // one step, counted.
                let id = format!("fit:{}:{}:{}", work.plan, facts.item, at.name);
                if let Some((_, count)) = fits.iter_mut().find(|(s, _)| s.id == id) {
                    *count += 1;
                    continue;
                }
                let no_ship = work.facts.read == ShipRead::NotOwned;
                fits.push((
                    PlanStep {
                        id,
                        kind: StepKind::Fit,
                        title: facts.name.clone(),
                        detail: if no_ship {
                            format!("Waits on the ship: you own no {}", work.facts.ship)
                        } else {
                            String::new()
                        },
                        plans: vec![work.plan.to_owned()],
                        blocked: no_ship,
                        at: Some(at.clone()),
                        seconds: None,
                        cost: None,
                        odds: None,
                        sources: 1,
                        after: Vec::new(),
                        parts: 0,
                        rocks: None,
                        scu: 0.0,
                    },
                    1,
                ));
            } else if let (Some(PartSource::Store), Some(offer)) =
                (part.source, facts.offers.first())
            {
                let at = match buys
                    .iter()
                    .position(|b| b.id == format!("buy:{}", offer.shop))
                {
                    Some(at) => at,
                    None => {
                        buys.push(PlanStep {
                            id: format!("buy:{}", offer.shop),
                            kind: StepKind::Buy,
                            title: String::new(),
                            detail: String::new(),
                            plans: Vec::new(),
                            blocked: false,
                            at: Some(offer.at.clone()),
                            seconds: None,
                            cost: Some(0.0),
                            odds: None,
                            sources: u32::MAX,
                            after: Vec::new(),
                            parts: 0,
                            rocks: None,
                            scu: 0.0,
                        });
                        buy_lines.push(Vec::new());
                        buys.len() - 1
                    }
                };
                let step = &mut buys[at];
                step.cost = Some(step.cost.unwrap_or(0.0) + offer.price);
                step.sources = step.sources.min(facts.offers.len() as u32);
                if !step.plans.iter().any(|p| p == work.plan) {
                    step.plans.push(work.plan.to_owned());
                }
                match buy_lines[at]
                    .iter_mut()
                    .find(|(name, _)| *name == facts.name)
                {
                    Some((_, count)) => *count += 1,
                    None => buy_lines[at].push((facts.name.clone(), 1)),
                }
            } else if let (Some(PartSource::Market), Some(offer)) = (part.source, facts.market) {
                // Every part bought from players is one step: the market is
                // no place to fly to.
                let (step, lines) = market.get_or_insert_with(|| {
                    (
                        PlanStep {
                            id: "market".to_owned(),
                            kind: StepKind::Buy,
                            title: String::new(),
                            detail: String::new(),
                            plans: Vec::new(),
                            blocked: false,
                            at: None,
                            seconds: None,
                            cost: Some(0.0),
                            odds: None,
                            sources: 0,
                            after: Vec::new(),
                            parts: 0,
                            rocks: None,
                            scu: 0.0,
                        },
                        Vec::new(),
                    )
                });
                step.cost = Some(step.cost.unwrap_or(0.0) + offer.price);
                step.sources += offer.listings;
                if !step.plans.iter().any(|p| p == work.plan) {
                    step.plans.push(work.plan.to_owned());
                }
                match lines.iter_mut().find(|(name, _)| *name == facts.name) {
                    Some((_, count)) => *count += 1,
                    None => lines.push((facts.name.clone(), 1)),
                }
            } else if let (Some(PartSource::Loot), Some(offer)) = (part.source, &facts.loot) {
                let id = format!("loot:{}", facts.item);
                match loots.iter_mut().find(|(s, _)| s.id == id) {
                    Some((step, count)) => {
                        *count += 1;
                        if !step.plans.iter().any(|p| p == work.plan) {
                            step.plans.push(work.plan.to_owned());
                        }
                    }
                    None => loots.push((
                        PlanStep {
                            id,
                            kind: StepKind::Loot,
                            title: facts.name.clone(),
                            detail: loot_detail(offer),
                            plans: vec![work.plan.to_owned()],
                            blocked: false,
                            at: Some(offer.at.clone()),
                            seconds: None,
                            cost: None,
                            odds: Some(offer.chance as f32),
                            sources: offer.places,
                            after: Vec::new(),
                            parts: 0,
                            rocks: None,
                            scu: 0.0,
                        },
                        1,
                    )),
                }
            } else if part.source.is_none() {
                finds.push(PlanStep {
                    id: format!("find:{}:{}:{n}", work.plan, facts.item),
                    kind: StepKind::Find,
                    title: format!("Find the {}", facts.name),
                    detail: "No shop is known to sell it and no blueprint makes it".to_owned(),
                    plans: vec![work.plan.to_owned()],
                    blocked: false,
                    at: None,
                    seconds: None,
                    cost: None,
                    odds: None,
                    sources: 0,
                    after: Vec::new(),
                    parts: 0,
                    rocks: None,
                    scu: 0.0,
                });
            }
        }
    }
    for (mut step, count) in fits {
        step.parts = count;
        let name = std::mem::take(&mut step.title);
        // No "It is at …" line: the row's right column names the place.
        (step.title, step.detail) = match (count, step.blocked) {
            (1, false) => (format!("Fit the {name} you own"), String::new()),
            (1, true) => (format!("Fit the {name} you own"), step.detail),
            (n, false) => (format!("Fit {n} × {name} you own"), String::new()),
            (n, true) => (format!("Fit {n} × {name} you own"), step.detail),
        };
        push(step);
    }
    for (mut step, lines) in buys.into_iter().zip(buy_lines) {
        let count: u32 = lines.iter().map(|(_, c)| c).sum();
        step.parts = count;
        let shop = step.id.trim_start_matches("buy:").to_owned();
        // The total is the row's figure, so the detail names only the parts.
        if let [(name, 1)] = lines.as_slice() {
            step.title = format!("Buy the {name}");
            step.detail = shop;
        } else {
            step.title = format!("Buy {count} parts at {shop}");
            let list: Vec<String> = lines
                .iter()
                .map(|(name, c)| format!("{c} × {name}"))
                .collect();
            step.detail = and(&list);
        }
        push(step);
    }

    if let Some((mut step, lines)) = market {
        let count: u32 = lines.iter().map(|(_, c)| c).sum();
        step.parts = count;
        if let [(name, 1)] = lines.as_slice() {
            step.title = format!("Buy the {name} from a player");
        } else {
            step.title = format!("Buy {count} parts from players");
            step.detail = and(&lines
                .iter()
                .map(|(name, c)| format!("{c} × {name}"))
                .collect::<Vec<_>>());
        }
        push(step);
    }

    for (mut step, count) in loots {
        step.parts = count;
        step.title = match count {
            1 => format!("Loot the {}", step.title),
            n => format!("Loot {n} × {}", step.title),
        };
        push(step);
    }

    finds.into_iter().for_each(&mut push);

    // Blueprints to earn: one step a blueprint, whichever plans want it.
    let mut earns: Vec<PlanStep> = Vec::new();
    for craft in crafts
        .iter()
        .filter(|c| c.left > 0 && !c.parked && !c.recipe.owned)
    {
        let id = format!("earn:{}", craft.blueprint);
        if let Some(step) = earns.iter_mut().find(|s| s.id == id) {
            if !step.plans.iter().any(|p| p == craft.plan) {
                step.plans.push(craft.plan.to_owned());
            }
            continue;
        }
        let grants = &craft.recipe.grants;
        earns.push(PlanStep {
            id,
            kind: StepKind::Earn,
            title: format!("Earn the {} blueprint", craft.recipe.name),
            detail: match grants.as_slice() {
                [] => "No mission is known to grant it".to_owned(),
                [one] => format!("One mission grants it: {}", one.title),
                many => format!("{} missions grant it", many.len()),
            },
            plans: vec![craft.plan.to_owned()],
            blocked: grants.is_empty(),
            at: grants
                .iter()
                .flat_map(|g| &g.at)
                .find(|w| w.site.is_some())
                .or_else(|| grants.iter().flat_map(|g| &g.at).next())
                .cloned(),
            // The shortest granting mission, by CIG's own estimate.
            seconds: grants
                .iter()
                .filter_map(|g| g.minutes)
                .min_by(f32::total_cmp)
                .map(|m| f64::from(m) * 60.0),
            cost: None,
            odds: None,
            sources: grants.len() as u32,
            after: Vec::new(),
            parts: 0,
            rocks: None,
            scu: 0.0,
        });
    }
    earns.into_iter().for_each(&mut push);

    // Ranks and missions: the path to each.
    crate::contracts::steps(&state.plans, &inputs.contracts)
        .into_iter()
        .for_each(&mut push);

    // Materials still short: one step a ledger row, their places chosen
    // together so the run stays in as few refinery areas as it can.
    let short: Vec<(&crate::ledger::LedgerRow, OreSites)> = ledger
        .rows
        .iter()
        .filter(|r| r.short() > 1e-6)
        .map(|r| (r, ores(&r.material, r.min_quality)))
        .collect();
    let picks = choose_sites(
        &short
            .iter()
            .map(|(_, f)| f.sites.as_slice())
            .collect::<Vec<_>>(),
    );
    let mut refines: Vec<ToRefine> = Vec::new();
    for ((row, found), pick) in short.iter().zip(picks) {
        let row = *row;
        let best = pick.map(|i| &found.sites[i]);
        // A refined material is mined as ore, about twice its weight.
        let mined = if found.ore.is_some() {
            row.short() / REFINE_YIELD
        } else {
            row.short()
        };
        let amount = amount(mined, row.bulk);
        let grade = quality_suffix(row.min_quality);
        let id = format!("mine:{}:{}:{}", row.material, row.min_quality, row.bulk);
        let base = PlanStep {
            id,
            kind: StepKind::Mine,
            title: String::new(),
            detail: String::new(),
            // Only the plans still short of it: one the stock covers is not
            // moved by mining more.
            plans: demands
                .iter()
                .zip(&ledger.fills)
                .fold(Vec::new(), |mut plans, (d, fill)| {
                    let same = d.material == row.material
                        && d.min_quality == row.min_quality
                        && d.bulk == row.bulk;
                    if same && !fill.covered() && !plans.contains(&d.plan) {
                        plans.push(d.plan.clone());
                    }
                    plans
                }),
            blocked: false,
            at: None,
            seconds: None,
            cost: None,
            odds: None,
            sources: found.sites.len() as u32,
            after: Vec::new(),
            parts: 0,
            rocks: None,
            scu: 0.0,
        };
        // Bulk only: a counted material is no share of a rock.
        let rocks = best
            .filter(|_| row.bulk)
            .and_then(|b| rocks(mined, b.find, b.ore_scu));
        // A refined material is mined as its ore and goes to its area's
        // refinery (below).
        if let (Some(best), Some(ore)) = (best, found.ore.as_ref()) {
            refines.push(ToRefine {
                ore: ore.clone(),
                material: row.material.clone(),
                mine: base.id.clone(),
                plans: base.plans.clone(),
                near: best.refine_at.clone(),
                system: best.system.clone(),
                // Everything collected takes space: a rock broken for the
                // ore goes into the hold whole.
                scu: match (rocks, best.rock_scu.filter(|r| *r > 0.0)) {
                    (Some(r), Some(rock)) => f64::from(r.to_break) * f64::from(rock),
                    _ if row.bulk => mined,
                    _ => 0.0,
                },
                refineries: found.refineries,
            });
        }
        push(match best {
            Some(best) => PlanStep {
                title: format!("Mine {amount} {}{grade}", row.material),
                detail: format!(
                    "{} of the ship rocks there hold it{}",
                    share(best.find),
                    if row.min_quality > 1 {
                        " at this quality"
                    } else {
                        ""
                    }
                ),
                at: Some(best.at.clone()),
                odds: Some(best.find),
                rocks,
                scu: if row.bulk { mined } else { 0.0 },
                ..base
            },
            None => PlanStep {
                kind: StepKind::Gather,
                // Not mined: the material itself, as the plans need it.
                title: format!(
                    "Get {} {}{grade}",
                    self::amount(row.short(), row.bulk),
                    row.material
                ),
                detail: if found.mined {
                    format!(
                        "No ship mining place rolls it at Q{} or better",
                        row.min_quality
                    )
                } else {
                    "No ship mining place holds it".to_owned()
                },
                ..base
            },
        });
    }
    refine_steps(&refines, inputs.hold.as_ref())
        .into_iter()
        .for_each(&mut push);

    // Crafts: what the reserved stock covers now, and what still waits.
    let mut craft_now: HashMap<(&str, &str), u32> = HashMap::new();
    for craft in crafts.iter().filter(|c| c.left > 0 && !c.parked) {
        let fills: Vec<(&Demand, &Fill, f64)> = craft
            .demands
            .iter()
            .map(|(i, each)| (&demands[*i], &ledger.fills[*i], *each))
            .collect();
        let can = if craft.recipe.owned {
            fills
                .iter()
                .filter(|(_, _, each)| *each > 0.0)
                .map(|(_, fill, each)| ((fill.reserved + 1e-6) / each).floor() as u32)
                .min()
                .unwrap_or(craft.left)
                .min(craft.left)
        } else {
            0
        };
        craft_now.insert((craft.plan, craft.blueprint), can);
        // Named only when it is one quality: per-material minimums are on
        // the plan itself.
        let grade = match craft.quality {
            Some(q) if q > 0 => format!(" at Q{q}"),
            _ => String::new(),
        };
        let name = &craft.recipe.name;
        let titled = |n: u32| match n {
            1 => format!("Craft the {name}{grade}"),
            n => format!("Craft {n} × {name}{grade}"),
        };
        if can > 0 {
            push(PlanStep {
                id: format!("craft:{}:{}", craft.plan, craft.blueprint),
                kind: StepKind::Craft,
                title: titled(can),
                detail: "Materials held".to_owned(),
                plans: vec![craft.plan.to_owned()],
                blocked: false,
                at: None,
                seconds: craft
                    .recipe
                    .craft_seconds
                    .map(|s| f64::from(s) * f64::from(can)),
                cost: None,
                odds: None,
                sources: 0,
                after: Vec::new(),
                parts: 0,
                rocks: None,
                scu: 0.0,
            });
        }
        let rest = craft.left - can;
        if rest > 0 {
            let mut needs: Vec<String> = Vec::new();
            if !craft.recipe.owned {
                needs.push("the blueprint".to_owned());
            }
            for (demand, fill, each) in &fills {
                // Short for the crafts that are not covered yet.
                if fill.reserved + 1e-6 < each * f64::from(craft.left) {
                    needs.push(demand.material.clone());
                }
            }
            push(PlanStep {
                id: format!("craft-wait:{}:{}", craft.plan, craft.blueprint),
                kind: StepKind::Craft,
                title: titled(rest),
                detail: format!("Needs {}", and(&needs)),
                plans: vec![craft.plan.to_owned()],
                blocked: true,
                at: None,
                seconds: craft
                    .recipe
                    .craft_seconds
                    .map(|s| f64::from(s) * f64::from(rest)),
                cost: None,
                odds: None,
                sources: 0,
                after: Vec::new(),
                parts: 0,
                rocks: None,
                scu: 0.0,
            });
        }
    }

    let mut steps = ready;
    steps.append(&mut blocked);

    let plans = state
        .plans
        .iter()
        .map(|plan| {
            let progress_of = |goal: &CraftGoal| {
                let craft = crafts
                    .iter()
                    .find(|c| c.plan == plan.id && c.blueprint == goal.blueprint)?;
                let now = craft_now
                    .get(&(craft.plan, craft.blueprint))
                    .copied()
                    .unwrap_or(0);
                Some(craft_progress(craft, now, &demands, &ledger.fills))
            };
            let mine: Vec<&PlanStep> = steps
                .iter()
                .filter(|s| s.plans.contains(&plan.id))
                .collect();
            let mut board = match &plan.goal {
                Goal::Loadout(_) => match loadouts.iter().find(|w| w.plan == plan.id) {
                    Some(work) => loadout_board(work, &mine),
                    None => missing(
                        PlanKind::Loadout,
                        "Loadout",
                        "This saved loadout is gone, or its ship is not in this build",
                    ),
                },
                Goal::Craft(goal) => match progress_of(goal) {
                    Some(progress) => craft_board(progress, &mine),
                    None => missing(
                        PlanKind::Craft,
                        "Craft project",
                        "This build has no such blueprint",
                    ),
                },
                Goal::Project(project) => project_board(
                    project
                        .crafts
                        .iter()
                        .map(|goal| {
                            progress_of(goal).unwrap_or_else(|| CraftProgress::missing(goal))
                        })
                        .collect(),
                    &mine,
                ),
                Goal::Rank(goal) => crate::contracts::rank_board(goal, &inputs.contracts),
                Goal::Mission(goal) => crate::contracts::mission_board(goal, &inputs.contracts),
                Goal::Unknown => missing(PlanKind::Unknown, "Plan", "Made by a newer Starlume"),
            };
            board.id = plan.id.clone();
            board.parked = plan.parked;
            board.notes = plan.notes.clone();
            if !plan.name.is_empty() {
                board.name = plan.name.clone();
                if board.kind == PlanKind::Project {
                    board.title = plan.name.clone();
                }
            }
            let empty = board.kind == PlanKind::Project && board.crafts.is_empty();
            if plan.parked {
                // Nothing is reserved for it, so nothing can be said about
                // how far it is.
                board.waiting = false;
                board.note = "Parked: it reserves nothing".to_owned();
            } else if empty {
                board.waiting = true;
                board.note = "No crafts in it yet".to_owned();
            } else if !board.missing {
                // A loadout with nothing left to fit is still not done on
                // a ship you do not own.
                let no_ship = loadouts
                    .iter()
                    .any(|w| w.plan == plan.id && w.facts.read == ShipRead::NotOwned);
                board.done = mine.is_empty() && !no_ship;
                board.waiting = !board.done && mine.iter().all(|s| s.blocked);
                board.note = if board.done {
                    "Done".to_owned()
                } else if let Some(next) = mine.iter().find(|s| !s.blocked) {
                    format!("Next: {}", lower_first(&next.title))
                } else {
                    match mine.first() {
                        Some(s) if s.detail.starts_with("Waits") => s.detail.clone(),
                        Some(s) => format!("Waits: {}", lower_first(&s.detail)),
                        None => format!("Waits on the ship: you own no {}", board.title),
                    }
                };
                if no_ship && mine.is_empty() {
                    board.progress = board.progress.min(0.5);
                }
            }
            board
        })
        .collect();

    Board {
        plans,
        steps,
        ledger: ledger.rows,
        parts,
    }
}

/// Count a still-needed part into the Components panel's list.
fn note_part(parts: &mut Vec<PartChoice>, plan: &str, part: &Part, inputs: &Inputs) {
    let facts = part.facts;
    if let Some(row) = parts.iter_mut().find(|p| p.item == facts.item) {
        row.count += 1;
        if !row.plans.iter().any(|p| p == plan) {
            row.plans.push(plan.to_owned());
        }
        return;
    }
    let options = sources_of(facts, inputs)
        .into_iter()
        .map(|source| match source {
            PartSource::Store => {
                let offer = &facts.offers[0];
                SourceOption {
                    source,
                    price: Some(offer.price),
                    detail: offer.at.name.clone(),
                }
            }
            PartSource::Market => {
                let m = facts.market.expect("a market source has an offer");
                SourceOption {
                    source,
                    price: Some(m.price),
                    detail: match m.listings {
                        1 => "1 listing".to_owned(),
                        n => format!("{n} listings"),
                    },
                }
            }
            PartSource::Loot => {
                let offer = facts.loot.as_ref().expect("a loot source has an offer");
                SourceOption {
                    source,
                    price: None,
                    detail: format!("{} at {}", percent(offer.chance), offer.at.name),
                }
            }
            PartSource::Craft => {
                let owned = facts
                    .blueprint
                    .as_ref()
                    .and_then(|bp| inputs.recipes.get(bp))
                    .is_some_and(|r| r.owned);
                SourceOption {
                    source,
                    price: None,
                    detail: if owned {
                        "blueprint owned".to_owned()
                    } else {
                        "blueprint to earn".to_owned()
                    },
                }
            }
        })
        .collect();
    parts.push(PartChoice {
        item: facts.item.clone(),
        name: facts.name.clone(),
        count: 1,
        plans: vec![plan.to_owned()],
        chosen: part.source,
        options,
    });
}

/// Take up to `n` free copies; returns how many there were.
fn take_many(mut pool: Option<&mut Vec<(PlanPlace, u32)>>, n: u32) -> u32 {
    (0..n)
        .take_while(|_| take(pool.as_deref_mut()).is_some())
        .count() as u32
}

fn take(pool: Option<&mut Vec<(PlanPlace, u32)>>) -> Option<PlanPlace> {
    let entry = pool?.iter_mut().find(|(_, count)| *count > 0)?;
    entry.1 -= 1;
    Some(entry.0.clone())
}

pub(crate) fn missing(kind: PlanKind, title: &str, note: &str) -> PlanBoard {
    PlanBoard {
        id: String::new(),
        kind,
        title: title.to_owned(),
        subtitle: String::new(),
        name: title.to_owned(),
        progress: 0.0,
        done: false,
        missing: true,
        waiting: true,
        note: note.to_owned(),
        stages: Vec::new(),
        tiles: Vec::new(),
        parked: false,
        notes: String::new(),
        crafts: Vec::new(),
    }
}

/// Stages of a loadout, each from what the account shows: the parts a shop
/// sells, the parts only a blueprint makes, the parts with no known source,
/// then all of them on the ship.
fn loadout_board(work: &LoadoutWork, steps: &[&PlanStep]) -> PlanBoard {
    let parts = &work.parts;
    let total = parts.len() as u32;
    let have = |p: &Part| p.facts.fitted || p.held.is_some();
    let class = |want: u8| {
        let of: Vec<&Part> = parts
            .iter()
            .filter(|p| {
                let class = if !p.facts.offers.is_empty() {
                    0
                } else if p.facts.blueprint.is_some() {
                    1
                } else {
                    2
                };
                class == want
            })
            .collect();
        (
            of.iter().filter(|p| have(p)).count() as u32,
            of.len() as u32,
        )
    };
    let fitted = parts.iter().filter(|p| p.facts.fitted).count() as u32;
    let held = parts.iter().filter(|p| have(p)).count() as u32;
    let counts = [
        ("Store", class(0)),
        ("Crafted", class(1)),
        ("Loot", class(2)),
        ("Fitted", (fitted, total)),
    ];
    let cost: f64 = steps.iter().filter_map(|s| s.cost).sum();
    let open = steps.len();
    PlanBoard {
        id: String::new(),
        kind: PlanKind::Loadout,
        title: work.facts.ship.clone(),
        subtitle: work.facts.name.clone(),
        name: format!("{}, {}", work.facts.ship, work.facts.name),
        progress: if total == 0 {
            1.0
        } else {
            (held + fitted) as f32 / (2 * total) as f32
        },
        done: false,
        missing: false,
        waiting: false,
        note: String::new(),
        stages: stages(&counts),
        tiles: vec![
            tile(format!("{fitted} of {total}"), "parts fitted"),
            tile(format!("{held} of {total}"), "parts in hand"),
            tile(
                open.to_string(),
                if open == 1 { "step open" } else { "steps open" },
            ),
            match work.facts.read {
                ShipRead::NotOwned => tile("not owned".to_owned(), "the ship"),
                ShipRead::NoRead => tile("no read".to_owned(), "of its ports"),
                ShipRead::Fitted => tile(auec_figure(cost), "aUEC to buy"),
            },
        ],
        parked: false,
        notes: String::new(),
        crafts: Vec::new(),
    }
}

/// How far one craft is, and how the board shows it.
struct CraftProgress {
    view: PlanCraft,
    /// The blueprint is owned, or nothing is left to craft.
    blueprint: bool,
    /// Demands the stock covers, of how many.
    covered: u32,
    rows: u32,
    /// 0..=1.
    progress: f64,
}

impl CraftProgress {
    /// A project's craft whose blueprint this build does not have.
    fn missing(goal: &CraftGoal) -> Self {
        Self {
            view: PlanCraft {
                blueprint: goal.blueprint.clone(),
                name: "A blueprint this build does not have".to_owned(),
                count: goal.count,
                missing: true,
                quality: goal.is_uniform().then_some(goal.quality),
                ..PlanCraft::default()
            },
            blueprint: false,
            covered: 0,
            rows: 0,
            progress: 0.0,
        }
    }
}

fn craft_progress(craft: &Craft, now: u32, demands: &[Demand], fills: &[Fill]) -> CraftProgress {
    let recipe = craft.recipe;
    let count = craft.count;
    let crafted = count.saturating_sub(craft.left);
    let (covered, rows) = craft.demands.iter().fold((0u32, 0u32), |(c, n), (i, _)| {
        (c + u32::from(fills[*i].covered()), n + 1)
    });
    let coverage: f64 = if craft.parked {
        0.0
    } else if craft.left == 0 || craft.demands.is_empty() {
        1.0
    } else {
        craft
            .demands
            .iter()
            .map(|(i, _)| {
                if demands[*i].amount > 0.0 {
                    fills[*i].reserved / demands[*i].amount
                } else {
                    1.0
                }
            })
            .sum::<f64>()
            / craft.demands.len() as f64
    };
    let blueprint = recipe.owned || craft.left == 0;
    let share = f64::from(crafted) / f64::from(count.max(1));
    CraftProgress {
        view: PlanCraft {
            blueprint: craft.blueprint.to_owned(),
            name: recipe.name.clone(),
            family: recipe.family.clone(),
            count,
            crafted,
            now,
            owned: recipe.owned,
            missing: false,
            quality: craft.quality,
            slots: recipe
                .slots
                .iter()
                .enumerate()
                .map(|(n, slot)| PlanSlot {
                    name: slot.name.clone(),
                    material: slot.material.clone(),
                    amount: slot.amount,
                    bulk: slot.bulk,
                    min: craft.mins[n],
                    floor: slot.min_quality,
                    held: craft.left == 0
                        || craft.slot_demand[n].is_some_and(|i| fills[i].covered()),
                    moves: slot.moves.clone(),
                    bands: slot.bands.clone(),
                })
                .collect(),
        },
        blueprint,
        covered: if craft.left == 0 { 1 } else { covered },
        rows: if craft.left == 0 { 1 } else { rows.max(1) },
        progress: (f64::from(u8::from(blueprint)) + coverage) / 2.0 * (1.0 - share) + share,
    }
}

/// "2 to craft at Q700", or the span when the materials differ.
fn craft_subtitle(craft: &PlanCraft) -> String {
    let count = craft.count;
    match craft.quality {
        Some(0) => format!("{count} to craft"),
        Some(q) => format!("{count} to craft at Q{q}"),
        None => {
            let mins = craft.slots.iter().map(|s| s.min);
            let (low, high) = (mins.clone().min().unwrap_or(0), mins.max().unwrap_or(0));
            format!("{count} to craft, materials from Q{low} to Q{high}")
        }
    }
}

fn open_tile(open: usize) -> PlanTile {
    tile(
        open.to_string(),
        if open == 1 { "step open" } else { "steps open" },
    )
}

fn craft_board(progress: CraftProgress, steps: &[&PlanStep]) -> PlanBoard {
    let craft = &progress.view;
    let counts = [
        ("Blueprint", (u32::from(progress.blueprint), 1)),
        ("Materials", (progress.covered, progress.rows)),
        ("Crafted", (craft.crafted, craft.count)),
    ];
    let mut stages = stages(&counts);
    stages[0].sub = if progress.blueprint {
        "owned".to_owned()
    } else {
        "to earn".to_owned()
    };
    let subtitle = craft_subtitle(craft);
    PlanBoard {
        id: String::new(),
        kind: PlanKind::Craft,
        title: craft.name.clone(),
        name: format!("{}, {subtitle}", craft.name),
        subtitle,
        progress: progress.progress as f32,
        done: false,
        missing: false,
        waiting: false,
        note: String::new(),
        stages,
        tiles: vec![
            tile(format!("{} of {}", craft.crafted, craft.count), "crafted"),
            tile(craft.now.to_string(), "craftable now"),
            tile(
                format!("{} of {}", progress.covered, progress.rows),
                "materials held",
            ),
            open_tile(steps.len()),
        ],
        parked: false,
        notes: String::new(),
        crafts: vec![progress.view],
    }
}

/// A project as one plan: its crafts' blueprints, materials and copies
/// summed, its progress their mean.
fn project_board(crafts: Vec<CraftProgress>, steps: &[&PlanStep]) -> PlanBoard {
    let sum = |f: &dyn Fn(&CraftProgress) -> u32| crafts.iter().map(f).sum::<u32>();
    let total = crafts.len() as u32;
    let blueprints = sum(&|c| u32::from(c.blueprint));
    let (covered, rows) = (sum(&|c| c.covered), sum(&|c| c.rows));
    let (crafted, wanted) = (sum(&|c| c.view.crafted), sum(&|c| c.view.count));
    let now = sum(&|c| c.view.now);
    let subtitle = match total {
        1 => "1 craft".to_owned(),
        n => format!("{n} crafts"),
    };
    PlanBoard {
        id: String::new(),
        kind: PlanKind::Project,
        title: "Project".to_owned(),
        name: "Project".to_owned(),
        subtitle,
        progress: if crafts.is_empty() {
            0.0
        } else {
            (crafts.iter().map(|c| c.progress).sum::<f64>() / crafts.len() as f64) as f32
        },
        done: false,
        missing: false,
        waiting: false,
        note: String::new(),
        stages: stages(&[
            ("Blueprints", (blueprints, total)),
            ("Materials", (covered, rows)),
            ("Crafted", (crafted, wanted)),
        ]),
        tiles: vec![
            tile(format!("{crafted} of {wanted}"), "crafted"),
            tile(now.to_string(), "craftable now"),
            tile(format!("{covered} of {rows}"), "materials held"),
            open_tile(steps.len()),
        ],
        parked: false,
        notes: String::new(),
        crafts: crafts.into_iter().map(|c| c.view).collect(),
    }
}

/// A stage is done when all of its share is in; the first that is not is
/// the current one. A stage with nothing in it is done and says so.
fn stages(counts: &[(&str, (u32, u32))]) -> Vec<PlanStage> {
    let mut current_seen = false;
    counts
        .iter()
        .map(|(name, (done, of))| {
            let complete = done >= of;
            let state = if complete {
                StageState::Done
            } else if !current_seen {
                current_seen = true;
                StageState::Current
            } else {
                StageState::Ahead
            };
            PlanStage {
                name: (*name).to_owned(),
                sub: if *of == 0 {
                    "none".to_owned()
                } else {
                    format!("{done} / {of}")
                },
                state,
            }
        })
        .collect()
}

pub(crate) fn tile(value: String, label: &str) -> PlanTile {
    PlanTile {
        value,
        label: label.to_owned(),
    }
}

/// A minimum of 1 is the recipe's "any quality": only a real bar is named.
fn quality_suffix(quality: u32) -> String {
    if quality > 1 {
        format!(", Q{quality}+")
    } else {
        String::new()
    }
}

/// "12 SCU", "0.06 SCU", "3 ×".
fn amount(value: f64, bulk: bool) -> String {
    if !bulk {
        return format!("{} ×", value.ceil() as u64);
    }
    let text = if value >= 10.0 {
        format!("{value:.0}")
    } else if value >= 1.0 {
        format!("{value:.1}")
    } else if value >= 0.01 {
        format!("{value:.2}")
    } else {
        return "under 0.01 SCU".to_owned();
    };
    let text = if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.')
    } else {
        &text
    };
    format!("{text} SCU")
}

/// The share of rocks that hold an ore at a quality, to the whole percent:
/// "38%". The quality odds in it are exact (the game's own distribution);
/// which parts a rock holds is still counted over sampled rolls of its
/// composition, which moves the figure by about a point at most, so no
/// decimals are shown.
fn share(share: f32) -> String {
    let pct = share * 100.0;
    if pct > 0.0 && pct < 1.0 {
        "Under 1%".to_owned()
    } else {
        format!("{pct:.0}%")
    }
}

pub(crate) fn auec_figure(value: f64) -> String {
    let digits = format!("{:.0}", value.round());
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// A chance as the page shows it: whole percent, one decimal below 10 %,
/// "<0.1%" below that.
pub(crate) fn percent(chance: f64) -> String {
    let p = chance * 100.0;
    if p >= 10.0 {
        format!("{p:.0}%")
    } else if p >= 0.1 {
        format!("{p:.1}%")
    } else {
        "<0.1%".to_owned()
    }
}

/// "drops in 3 places, 4.0% at the best".
fn loot_detail(offer: &LootOffer) -> String {
    let places = match offer.places {
        1 => "1 place".to_owned(),
        n => format!("{n} places"),
    };
    format!("drops in {places}, {} at the best", percent(offer.chance))
}

/// "a", "a and b", "a, b and c".
fn and(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// One ore on its way to the refinery.
struct ToRefine {
    ore: String,
    material: String,
    /// The mining step that gives it.
    mine: String,
    plans: Vec<String>,
    /// Its area's refinery: the one nearest to where it is mined.
    near: Option<PlanPlace>,
    /// The system it is mined in: its load.
    system: Option<String>,
    /// The hold it takes, SCU: the whole rocks it is mined from where the
    /// data gives them, else the ore itself (0 for a material not counted
    /// in SCU).
    scu: f64,
    refineries: u32,
}

/// How much rarer than at its best place an ore may be at a place in an
/// area the run already goes to and still be mined there: half as often.
/// Rarer than that, it goes where it is easier (Valerie, 2026-11-17: keep
/// the run together, but not at any odds).
const CLOSE_ENOUGH: f32 = 0.5;

/// The area a place belongs to: its refinery's site. `None` off the map.
fn area(site: &OreSite) -> Option<&str> {
    site.refine_at.as_ref().and_then(|p| p.site.as_deref())
}

/// Each ore's place (an index into its sites), chosen so the run needs as
/// few refinery areas as it can. Each round takes the area that most ores
/// still open can be mined in at [`CLOSE_ENOUGH`] odds, and mines them there
/// at their best place in it; on a tie, the area listed first (the shell
/// lists the system you are in first). An ore no area takes goes to its
/// first place. `None` for an ore with no place.
fn choose_sites(ores: &[&[OreSite]]) -> Vec<Option<usize>> {
    let fair: Vec<Vec<usize>> = ores
        .iter()
        .map(|sites| {
            let best = sites.iter().map(|s| s.find).fold(0.0_f32, f32::max);
            (0..sites.len())
                .filter(|&i| sites[i].find >= best * CLOSE_ENOUGH)
                .collect()
        })
        .collect();
    let mut pick: Vec<Option<usize>> = vec![None; ores.len()];
    loop {
        // Areas in the order first seen, with how many open ores they take.
        let mut counts: Vec<(&str, usize)> = Vec::new();
        for (o, sites) in ores.iter().enumerate().filter(|(o, _)| pick[*o].is_none()) {
            let mut seen: Vec<&str> = Vec::new();
            for a in fair[o].iter().filter_map(|&i| area(&sites[i])) {
                if seen.contains(&a) {
                    continue;
                }
                seen.push(a);
                match counts.iter_mut().find(|(k, _)| *k == a) {
                    Some((_, n)) => *n += 1,
                    None => counts.push((a, 1)),
                }
            }
        }
        let Some(top) = counts
            .iter()
            .fold(None::<&(&str, usize)>, |best, c| match best {
                Some(b) if b.1 >= c.1 => Some(b),
                _ => Some(c),
            })
            .map(|(a, _)| *a)
        else {
            break;
        };
        for (o, sites) in ores.iter().enumerate() {
            if pick[o].is_some() {
                continue;
            }
            pick[o] = fair[o]
                .iter()
                .copied()
                .filter(|&i| area(&sites[i]) == Some(top))
                .max_by(|&a, &b| sites[a].find.total_cmp(&sites[b].find));
        }
    }
    for (o, p) in pick.iter_mut().enumerate() {
        if p.is_none() && !ores[o].is_empty() {
            *p = Some(0);
        }
    }
    pick
}

/// The prefix of a refine step's id; the rest is its load's system.
pub const REFINE: &str = "refine:";

/// A run's ore is refined in the system it is mined in, one refine step a
/// load, after the mining steps that feed it (Valerie, 2026-11-16/17:
/// nobody splits a run over refineries across the system, nor carries it
/// through a jump point). A load is what the mining ship's hold takes
/// ([`Inputs::hold`]); without one, a system's ore is one load. Ore mined
/// off the map is a load of its own, at a refinery not known yet. Its
/// refinery here is the first ore's area's.
/// [`crate::session::refine_after_mining`] moves each step beside the
/// mining trip's last stop of its load once the trips are ordered. No
/// amount of ore is named and no time: the game data has neither a yield
/// nor a duration.
fn refine_steps(ores: &[ToRefine], hold: Option<&OreHold>) -> Vec<PlanStep> {
    let mut systems: Vec<(Option<&str>, Vec<&ToRefine>)> = Vec::new();
    for o in ores {
        let key = o.system.as_deref();
        match systems.iter_mut().find(|(k, _)| *k == key) {
            Some((_, list)) => list.push(o),
            None => systems.push((key, vec![o])),
        }
    }
    let mut out = Vec::new();
    for (system, ores) in systems {
        let at = ores.iter().find_map(|o| o.near.as_ref());
        for (n, (load, holds)) in loads(&ores, hold).into_iter().enumerate() {
            let mut plans: Vec<String> = Vec::new();
            for p in load.iter().flat_map(|o| &o.plans) {
                if !plans.contains(p) {
                    plans.push(p.clone());
                }
            }
            let materials = and(&load.iter().map(|o| o.material.clone()).collect::<Vec<_>>());
            let mut step = PlanStep {
                id: format!("{REFINE}{}:{n}", system.unwrap_or("?")),
                kind: StepKind::Refine,
                title: match load.as_slice() {
                    [one] => format!("Refine {}", one.ore),
                    _ => "Refine the ore".to_owned(),
                },
                // The materials when they are not in the title, and how
                // many holds of the ship it is.
                detail: match (hold, load.len()) {
                    (None, 1) => String::new(),
                    (None, _) => materials,
                    (Some(h), 1) => {
                        format!("{} {} {}", count_word(holds), h.ship, loads_word(holds))
                    }
                    (Some(h), _) => format!(
                        "{materials}, {} {} {}",
                        count_word(holds),
                        h.ship,
                        loads_word(holds)
                    ),
                },
                plans,
                blocked: false,
                at: None,
                seconds: None,
                cost: None,
                odds: None,
                sources: load[0].refineries,
                after: load.iter().map(|o| o.mine.clone()).collect(),
                parts: 0,
                rocks: None,
                scu: 0.0,
            };
            place_refine(&mut step, at.cloned());
            out.push(step);
        }
    }
    out
}

/// One system's ore in loads of the hold, each with how many holds it
/// fills: most ore first, each into the first load it still fits (an ore
/// more than one hold holds is its own load, filling several). Without a
/// hold, all of it is one load.
fn loads<'a>(ores: &[&'a ToRefine], hold: Option<&OreHold>) -> Vec<(Vec<&'a ToRefine>, u32)> {
    let Some(cap) = hold.map(|h| h.scu).filter(|c| *c > 0.0) else {
        return vec![(ores.to_vec(), 1)];
    };
    let mut by_size = ores.to_vec();
    by_size.sort_by(|a, b| b.scu.total_cmp(&a.scu));
    let mut out: Vec<(Vec<&ToRefine>, f64, u32)> = Vec::new();
    for o in by_size {
        if o.scu > cap {
            out.push((vec![o], o.scu, (o.scu / cap).ceil() as u32));
            continue;
        }
        match out
            .iter_mut()
            .find(|(_, used, holds)| *holds == 1 && used + o.scu <= cap + 1e-9)
        {
            Some((load, used, _)) => {
                load.push(o);
                *used += o.scu;
            }
            None => out.push((vec![o], o.scu, 1)),
        }
    }
    // Each load lists its ores in the order they were asked for.
    out.into_iter()
        .map(|(mut load, _, holds)| {
            load.sort_by_key(|o| ores.iter().position(|x| std::ptr::eq(*x, *o)));
            (load, holds)
        })
        .collect()
}

fn count_word(n: u32) -> String {
    if n == 1 {
        "one".to_owned()
    } else {
        n.to_string()
    }
}

fn loads_word(n: u32) -> &'static str {
    if n == 1 { "load" } else { "loads" }
}

/// Put the refine step at `at` (`None`: a refinery not known yet).
pub(crate) fn place_refine(step: &mut PlanStep, at: Option<PlanPlace>) {
    let what = step
        .title
        .split_once(" at ")
        .map_or(step.title.as_str(), |(what, _)| what)
        .to_owned();
    step.title = match &at {
        Some(at) => format!("{what} at {}", at.name),
        None => format!("{what} at a refinery"),
    };
    step.at = at;
}

fn lower_first(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::model::{CraftGoal, LoadoutGoal, ProjectGoal};

    pub(crate) fn at(name: &str) -> PlanPlace {
        PlanPlace {
            name: name.into(),
            site: Some(format!("site-{name}")),
        }
    }

    fn no_ores(_: &str, _: u32) -> OreSites {
        OreSites::default()
    }

    fn part(item: &str) -> PartFacts {
        PartFacts {
            item: item.into(),
            name: format!("Part {item}"),
            ..PartFacts::default()
        }
    }

    fn loadout_state() -> PlansState {
        let mut state = PlansState::default();
        state.add(
            "",
            Goal::Loadout(LoadoutGoal {
                loadout: "lo-1".into(),
            }),
        );
        state
    }

    fn loadout(parts: Vec<PartFacts>) -> Inputs {
        let mut inputs = Inputs::default();
        inputs.loadouts.insert(
            "lo-1".into(),
            LoadoutFacts {
                name: "Test kit".into(),
                ship: "Test Ship".into(),
                read: ShipRead::Fitted,
                parts,
            },
        );
        inputs
    }

    fn recipe(owned: bool, slots: Vec<SlotNeed>) -> RecipeFacts {
        RecipeFacts {
            name: "Widget".into(),
            owned,
            craft_seconds: Some(60.0),
            slots,
            ..RecipeFacts::default()
        }
    }

    fn slot(material: &str, amount: f64, min_quality: u32) -> SlotNeed {
        SlotNeed {
            material: material.into(),
            amount,
            bulk: true,
            min_quality,
            ..SlotNeed::default()
        }
    }

    fn copies(count: u32, ingredients: &[(&str, u32)]) -> Vec<CraftedCopy> {
        vec![CraftedCopy {
            count,
            ingredients: ingredients
                .iter()
                .map(|(m, q)| ((*m).to_owned(), *q))
                .collect(),
        }]
    }

    fn stack(material: &str, quality: u32, amount: f64) -> Stack {
        Stack {
            material: material.into(),
            quality,
            amount,
            bulk: true,
            place: "Depot One".into(),
        }
    }

    fn craft_state(count: u32, quality: u32) -> PlansState {
        let mut state = PlansState::default();
        state.add("", Goal::Craft(CraftGoal::uniform("bp-1", count, quality)));
        state
    }

    /// A recipe shaped like the one the per-slot minimums were asked for:
    /// a frame that moves one stat, two slots that move another.
    fn repeater() -> RecipeFacts {
        let mut frame = slot("Metal", 3.0, 1);
        frame.name = Some("Frame".into());
        frame.moves = vec!["Integrity".into()];
        frame.bands = [(0, 399, 346), (400, 599, 588), (700, 799, 796)]
            .map(|(from, to, value)| QualityBand { from, to, value })
            .to_vec();
        let mut emitter = slot("Gem", 75.0, 1);
        emitter.bulk = false;
        emitter.moves = vec!["Impact Force".into()];
        let mut sink = slot("Ice", 1.5, 1);
        sink.moves = vec!["Impact Force".into()];
        recipe(true, vec![frame, emitter, sink])
    }

    fn slots_state(mins: &[u32]) -> PlansState {
        let mut state = PlansState::default();
        state.add("", Goal::Craft(CraftGoal::with_slots("bp-1", 1, 0, mins)));
        state
    }

    fn project(state: &mut PlansState, name: &str, crafts: Vec<CraftGoal>) -> String {
        state.add(name, Goal::Project(ProjectGoal { crafts }))
    }

    #[test]
    fn a_part_you_hold_is_a_fit_step_at_the_place_it_sits() {
        let mut inputs = loadout(vec![part("a")]);
        inputs.parts.insert(
            "a".into(),
            vec![HeldPart {
                at: at("Depot One"),
                count: 1,
            }],
        );
        let board = derive(&loadout_state(), &inputs, &no_ores);
        assert_eq!(board.steps.len(), 1);
        let step = &board.steps[0];
        assert_eq!(step.kind, StepKind::Fit);
        assert_eq!(step.title, "Fit the Part a you own");
        assert_eq!(step.detail, "");
        assert_eq!(step.at, Some(at("Depot One")));
        assert!(!step.blocked);
    }

    #[test]
    fn one_held_copy_serves_one_port_and_the_second_is_bought() {
        let mut second = part("a");
        second.offers = vec![Offer {
            shop: "Shop One".into(),
            at: at("Town"),
            price: 1200.0,
        }];
        let mut inputs = loadout(vec![second.clone(), second]);
        inputs.parts.insert(
            "a".into(),
            vec![HeldPart {
                at: at("Depot One"),
                count: 1,
            }],
        );
        let board = derive(&loadout_state(), &inputs, &no_ores);
        let kinds: Vec<StepKind> = board.steps.iter().map(|s| s.kind).collect();
        assert_eq!(kinds, [StepKind::Fit, StepKind::Buy]);
        assert_eq!(board.steps[1].title, "Buy the Part a");
        assert_eq!(board.steps[1].detail, "Shop One");
    }

    #[test]
    fn parts_from_one_shop_are_one_buy_step_with_the_total() {
        let offer = |price| {
            vec![Offer {
                shop: "Shop One".into(),
                at: at("Town"),
                price,
            }]
        };
        let mut a = part("a");
        a.offers = offer(20_000.0);
        let mut b = part("b");
        b.offers = offer(21_300.0);
        let board = derive(&loadout_state(), &loadout(vec![a.clone(), a, b]), &no_ores);
        assert_eq!(board.steps.len(), 1);
        assert_eq!(board.steps[0].title, "Buy 3 parts at Shop One");
        assert_eq!(board.steps[0].detail, "2 × Part a and 1 × Part b");
        assert_eq!(board.steps[0].cost, Some(61_300.0));
    }

    /// A part a shop sells, players list and a blueprint makes.
    fn three_sources() -> Inputs {
        let mut a = part("a");
        a.offers = vec![Offer {
            shop: "Shop One".into(),
            at: at("Town"),
            price: 20_000.0,
        }];
        a.market = Some(MarketOffer {
            price: 15_000.0,
            listings: 3,
        });
        a.blueprint = Some("bp-a".into());
        let mut inputs = loadout(vec![a]);
        inputs
            .recipes
            .insert("bp-a".into(), recipe(true, vec![slot("Iron", 1.0, 0)]));
        inputs
    }

    #[test]
    fn a_part_comes_from_the_shop_unless_a_source_is_picked() {
        let inputs = three_sources();
        let mut state = loadout_state();
        let kinds = |board: &Board| board.steps.iter().map(|s| s.kind).collect::<Vec<_>>();
        assert_eq!(kinds(&derive(&state, &inputs, &no_ores)), [StepKind::Buy]);

        // Prefer crafting: the buy becomes the craft and its materials.
        state.set_prefer(PartSource::Craft);
        let board = derive(&state, &inputs, &no_ores);
        assert!(board.steps.iter().all(|s| s.kind != StepKind::Buy));
        assert!(board.steps.iter().any(|s| s.kind == StepKind::Craft));

        // The part's own pick wins over the preference.
        state.set_source("a", Some(PartSource::Market));
        let board = derive(&state, &inputs, &no_ores);
        assert_eq!(kinds(&board), [StepKind::Buy]);
        let step = &board.steps[0];
        assert_eq!(step.title, "Buy the Part a from a player");
        assert_eq!(step.at, None);
        assert_eq!(step.cost, Some(15_000.0));
    }

    #[test]
    fn the_components_panel_lists_each_needed_part_with_its_sources() {
        let board = derive(&loadout_state(), &three_sources(), &no_ores);
        let [row] = board.parts.as_slice() else {
            panic!("one part, got {}", board.parts.len());
        };
        assert_eq!(row.chosen, Some(PartSource::Store));
        let options: Vec<(PartSource, Option<f64>, &str)> = row
            .options
            .iter()
            .map(|o| (o.source, o.price, o.detail.as_str()))
            .collect();
        assert_eq!(
            options,
            [
                (PartSource::Store, Some(20_000.0), "Town"),
                (PartSource::Market, Some(15_000.0), "3 listings"),
                (PartSource::Craft, None, "blueprint owned"),
            ]
        );
        // A part with a free copy held is not needed.
        let mut inputs = three_sources();
        inputs.parts.insert(
            "a".into(),
            vec![HeldPart {
                at: at("Depot"),
                count: 1,
            }],
        );
        assert!(derive(&loadout_state(), &inputs, &no_ores).parts.is_empty());
    }

    /// `three_sources` plus a drop at Ruin Station.
    fn four_sources() -> Inputs {
        let mut a = part("a");
        a.offers = vec![Offer {
            shop: "Shop One".into(),
            at: at("Town"),
            price: 20_000.0,
        }];
        a.loot = Some(LootOffer {
            chance: 0.04,
            at: at("Ruin Station"),
            places: 3,
        });
        loadout(vec![a])
    }

    #[test]
    fn rocks_to_break_and_to_scan_follow_the_ore_a_rock_holds() {
        assert_eq!(
            rocks(4.6, 0.28, Some(1.2)),
            Some(Rocks {
                to_break: 4,
                to_scan: 15
            })
        );
        // A little ore is still one rock; every rock holding it is one scan.
        assert_eq!(
            rocks(0.2, 1.0, Some(3.0)),
            Some(Rocks {
                to_break: 1,
                to_scan: 1
            })
        );
        assert_eq!(rocks(4.6, 0.28, None), None);
        assert_eq!(rocks(4.6, 0.0, Some(1.2)), None);
    }

    #[test]
    fn a_mine_step_counts_its_rocks_and_a_buy_its_parts() {
        let mut inputs = Inputs::default();
        inputs
            .recipes
            .insert("bp-1".into(), recipe(true, vec![slot("Ore A", 2.0, 1)]));
        let ores = |_: &str, _: u32| OreSites {
            sites: vec![OreSite {
                at: at("Moon One"),
                find: 0.5,
                ore_scu: Some(0.4),
                ..OreSite::default()
            }],
            mined: true,
            ..OreSites::default()
        };
        let board = derive(&craft_state(1, 0), &inputs, &ores);
        let mine = board
            .steps
            .iter()
            .find(|s| s.kind == StepKind::Mine)
            .unwrap();
        assert_eq!(
            mine.rocks,
            Some(Rocks {
                to_break: 5,
                to_scan: 10
            })
        );
        assert_eq!(mine.scu, 2.0);

        // A refined material is mined as its ore, at the refining yield.
        let refined = |m: &str, q: u32| OreSites {
            ore: Some("Ore A (Ore)".into()),
            refineries: 1,
            ..ores(m, q)
        };
        let board = derive(&craft_state(1, 0), &inputs, &refined);
        let mine = board
            .steps
            .iter()
            .find(|s| s.kind == StepKind::Mine)
            .unwrap();
        assert_eq!(mine.scu, 4.0);
        assert_eq!(mine.title, "Mine 4 SCU Ore A");
        assert_eq!(mine.rocks.map(|r| r.to_break), Some(10));

        let board = derive(&loadout_state(), &three_sources(), &no_ores);
        assert_eq!(board.steps[0].kind, StepKind::Buy);
        assert_eq!(board.steps[0].parts, 1);
    }

    #[test]
    fn a_looted_part_is_one_step_at_its_best_place() {
        let inputs = four_sources();
        let mut state = loadout_state();
        let board = derive(&state, &inputs, &no_ores);
        let row = &board.parts[0];
        assert_eq!(
            row.chosen,
            Some(PartSource::Store),
            "loot is never the default"
        );
        let loot = row.options.last().unwrap();
        assert_eq!(
            (loot.source, loot.price, loot.detail.as_str()),
            (PartSource::Loot, None, "4.0% at Ruin Station")
        );

        state.set_source("a", Some(PartSource::Loot));
        let board = derive(&state, &inputs, &no_ores);
        let [step] = board.steps.as_slice() else {
            panic!("one step, got {:?}", board.steps);
        };
        assert_eq!(step.kind, StepKind::Loot);
        assert_eq!(step.title, "Loot the Part a");
        assert_eq!(step.detail, "drops in 3 places, 4.0% at the best");
        assert_eq!(step.at, Some(at("Ruin Station")));
        assert_eq!(step.odds, Some(0.04));
        assert_eq!(
            crate::session::TrackKind::of(step.kind),
            Some(crate::session::TrackKind::Looting)
        );
    }

    #[test]
    fn two_ports_wanting_one_part_from_one_place_are_one_fit_step() {
        let mut inputs = loadout(vec![part("a"), part("a")]);
        inputs.parts.insert(
            "a".into(),
            vec![HeldPart {
                at: at("Depot One"),
                count: 2,
            }],
        );
        let board = derive(&loadout_state(), &inputs, &no_ores);
        assert_eq!(board.steps.len(), 1);
        assert_eq!(board.steps[0].title, "Fit 2 × Part a you own");
        assert_eq!(board.steps[0].detail, "");
    }

    #[test]
    fn a_minimum_quality_of_one_is_any_quality_and_is_not_named() {
        let mut inputs = Inputs::default();
        inputs
            .recipes
            .insert("bp-1".into(), recipe(true, vec![slot("Ore A", 2.0, 1)]));
        let ores = |_: &str, _: u32| OreSites {
            sites: vec![OreSite {
                system: None,
                at: at("Moon One"),
                find: 0.5,
                ..OreSite::default()
            }],
            mined: true,
            ..OreSites::default()
        };
        let board = derive(&craft_state(1, 0), &inputs, &ores);
        let mine = board
            .steps
            .iter()
            .find(|s| s.kind == StepKind::Mine)
            .unwrap();
        assert_eq!(mine.title, "Mine 2 SCU Ore A");
        assert_eq!(mine.detail, "50% of the ship rocks there hold it");
    }

    #[test]
    fn a_fitted_part_leaves_no_step_and_a_fitted_loadout_is_done() {
        let mut a = part("a");
        a.fitted = true;
        let board = derive(&loadout_state(), &loadout(vec![a]), &no_ores);
        assert!(board.steps.is_empty());
        assert!(board.plans[0].done);
        assert_eq!(board.plans[0].progress, 1.0);
    }

    #[test]
    fn a_part_with_no_shop_and_no_blueprint_is_to_find() {
        let board = derive(&loadout_state(), &loadout(vec![part("a")]), &no_ores);
        assert_eq!(board.steps[0].kind, StepKind::Find);
        assert_eq!(board.steps[0].at, None);
    }

    #[test]
    fn fitting_waits_on_a_ship_you_do_not_own() {
        let mut inputs = loadout(vec![part("a")]);
        inputs.loadouts.get_mut("lo-1").unwrap().read = ShipRead::NotOwned;
        inputs.parts.insert(
            "a".into(),
            vec![HeldPart {
                at: at("Depot One"),
                count: 1,
            }],
        );
        let board = derive(&loadout_state(), &inputs, &no_ores);
        assert!(board.steps[0].blocked);
        assert_eq!(
            board.steps[0].detail,
            "Waits on the ship: you own no Test Ship"
        );
        assert!(board.plans[0].waiting);
    }

    #[test]
    fn a_part_only_a_blueprint_makes_becomes_a_craft_with_its_materials() {
        let mut a = part("a");
        a.blueprint = Some("bp-1".into());
        let mut inputs = loadout(vec![a]);
        inputs
            .recipes
            .insert("bp-1".into(), recipe(true, vec![slot("Ore A", 2.0, 500)]));
        inputs.stock = vec![stack("Ore A", 600, 2.0)];
        let board = derive(&loadout_state(), &inputs, &no_ores);
        assert_eq!(board.steps.len(), 1);
        assert_eq!(board.steps[0].kind, StepKind::Craft);
        assert_eq!(board.steps[0].title, "Craft the Widget");
        assert_eq!(board.steps[0].detail, "Materials held");
        assert_eq!(board.steps[0].seconds, Some(60.0));
    }

    #[test]
    fn a_loadout_craft_aims_at_the_neutral_quality_only_where_stats_move() {
        let mut a = part("a");
        a.blueprint = Some("bp-1".into());
        let mut inputs = loadout(vec![a]);
        let stat = SlotNeed {
            aim: 500,
            ..slot("Ore A", 2.0, 1)
        };
        let plain = SlotNeed {
            aim: 1,
            ..slot("Ore B", 1.0, 1)
        };
        inputs
            .recipes
            .insert("bp-1".into(), recipe(true, vec![stat, plain]));
        // Q400 is below the neutral point: it does not fill the stat slot.
        inputs.stock = vec![stack("Ore A", 400, 2.0), stack("Ore B", 10, 1.0)];
        let board = derive(&loadout_state(), &inputs, &no_ores);
        let mins: Vec<(&str, u32)> = board
            .ledger
            .iter()
            .map(|r| (r.material.as_str(), r.min_quality))
            .collect();
        assert_eq!(mins, [("Ore A", 500), ("Ore B", 1)]);
    }

    #[test]
    fn a_craft_without_blueprint_or_material_is_blocked_and_says_on_what() {
        let mut inputs = Inputs::default();
        let mut widget = recipe(false, vec![slot("Ore A", 2.0, 500), slot("Ore B", 1.0, 0)]);
        widget.grants = vec![
            Grant {
                title: "Run One".into(),
                at: vec![at("Planet One")],
                minutes: Some(45.0),
            },
            Grant {
                title: "Run Two".into(),
                at: vec![],
                minutes: Some(20.0),
            },
        ];
        inputs.recipes.insert("bp-1".into(), widget);
        inputs.stock = vec![stack("Ore B", 100, 5.0)];
        let ores = |material: &str, _: u32| match material {
            "Ore A" => OreSites {
                sites: vec![OreSite {
                    system: None,
                    at: at("Moon One"),
                    find: 0.42,
                    ..OreSite::default()
                }],
                mined: true,
                ..OreSites::default()
            },
            _ => OreSites::default(),
        };
        let board = derive(&craft_state(1, 700), &inputs, &ores);
        let by_kind = |kind| board.steps.iter().find(|s| s.kind == kind).unwrap();

        let earn = by_kind(StepKind::Earn);
        assert_eq!(earn.title, "Earn the Widget blueprint");
        assert_eq!(earn.detail, "2 missions grant it");
        assert_eq!(earn.at, Some(at("Planet One")));
        assert_eq!(earn.seconds, Some(20.0 * 60.0), "the shortest mission");

        // The plan's Q700 lifts both slots: Ore B at Q100 no longer counts.
        let mine = by_kind(StepKind::Mine);
        assert_eq!(mine.title, "Mine 2 SCU Ore A, Q700+");
        assert_eq!(
            mine.detail,
            "42% of the ship rocks there hold it at this quality"
        );
        assert_eq!(mine.at, Some(at("Moon One")));
        let gather = by_kind(StepKind::Gather);
        assert_eq!(gather.title, "Get 1 SCU Ore B, Q700+");

        let craft = by_kind(StepKind::Craft);
        assert!(craft.blocked);
        assert_eq!(craft.title, "Craft the Widget at Q700");
        assert_eq!(craft.detail, "Needs the blueprint, Ore A and Ore B");
        // Blocked steps come after the ready ones.
        assert_eq!(board.steps.last().unwrap().id, craft.id);
    }

    #[test]
    fn stock_for_part_of_the_count_splits_the_craft_in_ready_and_waiting() {
        let mut inputs = Inputs::default();
        inputs
            .recipes
            .insert("bp-1".into(), recipe(true, vec![slot("Ore A", 2.0, 0)]));
        inputs.stock = vec![stack("Ore A", 500, 5.0)];
        let board = derive(&craft_state(3, 0), &inputs, &no_ores);
        let crafts: Vec<(&str, bool)> = board
            .steps
            .iter()
            .filter(|s| s.kind == StepKind::Craft)
            .map(|s| (s.title.as_str(), s.blocked))
            .collect();
        assert_eq!(
            crafts,
            [("Craft 2 × Widget", false), ("Craft the Widget", true)]
        );
        // 5 of 6 SCU held: 1 SCU is still to get.
        assert_eq!(board.ledger[0].short(), 1.0);
    }

    #[test]
    fn crafted_copies_you_hold_count_towards_the_project() {
        let mut inputs = Inputs::default();
        let mut widget = recipe(true, vec![slot("Ore A", 2.0, 0)]);
        widget.crafted = copies(2, &[]);
        inputs.recipes.insert("bp-1".into(), widget);
        let board = derive(&craft_state(2, 0), &inputs, &no_ores);
        assert!(board.steps.is_empty());
        assert!(board.ledger.is_empty());
        assert!(board.plans[0].done);
        assert_eq!(board.plans[0].note, "Done");
    }

    #[test]
    fn a_mining_step_moves_only_the_plans_still_short_and_the_first_is_served_first() {
        let mut state = craft_state(1, 0);
        state.add("", Goal::Craft(CraftGoal::uniform("bp-2", 1, 0)));
        let mut inputs = Inputs::default();
        inputs
            .recipes
            .insert("bp-1".into(), recipe(true, vec![slot("Ore A", 2.0, 0)]));
        inputs
            .recipes
            .insert("bp-2".into(), recipe(true, vec![slot("Ore A", 3.0, 0)]));
        inputs.stock = vec![stack("Ore A", 500, 2.0)];
        let ores = |_: &str, _: u32| OreSites {
            sites: vec![OreSite {
                system: None,
                at: at("Moon One"),
                find: 0.5,
                ..OreSite::default()
            }],
            mined: true,
            ..OreSites::default()
        };
        let board = derive(&state, &inputs, &ores);
        let mine = board
            .steps
            .iter()
            .find(|s| s.kind == StepKind::Mine)
            .unwrap();
        assert_eq!(mine.title, "Mine 3 SCU Ore A");
        assert_eq!(mine.plans, ["p2"]);
        // The first plan got the stock and can craft; the second waits.
        assert!(!board.plans[0].waiting);
        assert_eq!(board.plans[0].note, "Next: craft the Widget");
        assert_eq!(board.plans[1].tiles[1].value, "0");
    }

    /// A recipe that wants 2 SCU of "Metal", mined as "Metal (Ore)".
    fn refined(refine_at: Option<PlanPlace>, quality: u32) -> Board {
        let mut inputs = Inputs::default();
        inputs
            .recipes
            .insert("bp-1".into(), recipe(true, vec![slot("Metal", 2.0, 0)]));
        let ores = move |_: &str, _: u32| OreSites {
            sites: vec![OreSite {
                system: None,
                ore_scu: None,
                rock_scu: None,
                at: at("Moon One"),
                find: 0.25,
                refine_at: refine_at.clone(),
            }],
            mined: true,
            ore: Some("Metal (Ore)".into()),
            refineries: 4,
        };
        derive(&craft_state(1, quality), &inputs, &ores)
    }

    #[test]
    fn a_refined_material_is_mined_as_ore_and_then_refined_at_the_nearest_refinery() {
        let board = refined(Some(at("Station R")), 700);
        let kinds: Vec<StepKind> = board.steps.iter().map(|s| s.kind).collect();
        assert_eq!(kinds, [StepKind::Mine, StepKind::Refine, StepKind::Craft]);
        let (mine, refine) = (&board.steps[0], &board.steps[1]);
        assert_eq!(refine.title, "Refine Metal (Ore) at Station R");
        assert_eq!(refine.at, Some(at("Station R")));
        assert_eq!(refine.after, std::slice::from_ref(&mine.id));
        assert_eq!(refine.plans, mine.plans);
        assert_eq!(refine.sources, 4);
        assert!(!refine.blocked);
        // Mining comes first in the plan's note: twice the 2 SCU wanted,
        // as ore refines at about half.
        assert_eq!(board.plans[0].note, "Next: mine 4 SCU Metal, Q700+");
    }

    /// A place an ore is mined at: (place, find, its refinery, its
    /// system).
    type AreaSite = (
        &'static str,
        f32,
        Option<&'static str>,
        Option<&'static str>,
    );

    /// Ores and the places they are mined at.
    fn area_ores(table: Vec<(&'static str, Vec<AreaSite>)>) -> Board {
        let mut inputs = Inputs::default();
        inputs.recipes.insert(
            "bp-1".into(),
            recipe(true, table.iter().map(|(m, _)| slot(m, 1.0, 0)).collect()),
        );
        let ores = move |material: &str, _: u32| {
            let sites = &table.iter().find(|(m, _)| *m == material).unwrap().1;
            OreSites {
                sites: sites
                    .iter()
                    .map(|(place, find, refinery, system)| OreSite {
                        system: system.map(str::to_owned),
                        ore_scu: None,
                        rock_scu: None,
                        at: if *place == "The Ring" {
                            PlanPlace {
                                name: (*place).into(),
                                site: None,
                            }
                        } else {
                            at(place)
                        },
                        find: *find,
                        refine_at: refinery.map(at),
                    })
                    .collect(),
                mined: true,
                ore: Some(format!("{material} (Ore)")),
                refineries: 4,
            }
        };
        derive(&craft_state(1, 0), &inputs, &ores)
    }

    fn mined_at(board: &Board, material: &str) -> String {
        board
            .steps
            .iter()
            .find(|s| s.kind == StepKind::Mine && s.title.contains(material))
            .and_then(|s| s.at.as_ref())
            .map(|p| p.name.clone())
            .unwrap_or_default()
    }

    fn refine_titles(board: &Board) -> Vec<(String, String)> {
        board
            .steps
            .iter()
            .filter(|s| s.kind == StepKind::Refine)
            .map(|s| (s.title.clone(), s.detail.clone()))
            .collect()
    }

    /// Iron is best at Moon A but nearly as common at Moon B, where Gold
    /// is: both are mined around Station B, one load. Tin is rare there
    /// (well under half its best), so it is not forced and makes a load of
    /// its own.
    #[test]
    fn ores_keep_to_as_few_areas_as_their_odds_allow() {
        let board = area_ores(vec![
            (
                "Iron",
                vec![
                    ("Moon A", 0.30, Some("Station A"), Some("Here")),
                    ("Moon B", 0.20, Some("Station B"), Some("Here")),
                ],
            ),
            (
                "Gold",
                vec![("Moon B", 0.25, Some("Station B"), Some("Here"))],
            ),
            (
                "Tin",
                vec![
                    ("Moon C", 0.40, Some("Station C"), Some("There")),
                    ("Moon B", 0.10, Some("Station B"), Some("Here")),
                ],
            ),
        ]);
        assert_eq!(mined_at(&board, "Iron"), "Moon B");
        assert_eq!(mined_at(&board, "Gold"), "Moon B");
        assert_eq!(mined_at(&board, "Tin"), "Moon C");
        assert_eq!(
            refine_titles(&board),
            [
                (
                    "Refine the ore at Station B".to_owned(),
                    "Iron and Gold".to_owned()
                ),
                ("Refine Tin (Ore) at Station C".to_owned(), String::new()),
            ]
        );
        let refine = board
            .steps
            .iter()
            .find(|s| s.title.ends_with("Station B"))
            .unwrap();
        assert_eq!(refine.after, ["mine:Iron:0:true", "mine:Gold:0:true"]);
    }

    /// Two areas in one system are one load: the playtest's Nyx run split
    /// 1.4 SCU over two refineries.
    #[test]
    fn a_system_is_one_load() {
        let board = area_ores(vec![
            (
                "Iron",
                vec![("Moon A", 0.30, Some("Station A"), Some("Here"))],
            ),
            (
                "Tin",
                vec![("Moon B", 0.30, Some("Station B"), Some("Here"))],
            ),
        ]);
        assert_eq!(
            refine_titles(&board),
            [(
                "Refine the ore at Station A".to_owned(),
                "Iron and Tin".to_owned()
            )]
        );
    }

    /// Ores of the given SCU mined in one system, with the ship's hold.
    fn held(amounts: Vec<(&'static str, f64)>, hold: Option<f64>) -> Board {
        let mut inputs = Inputs {
            hold: hold.map(|scu| OreHold {
                ship: "Prospector".into(),
                scu,
            }),
            ..Inputs::default()
        };
        inputs.recipes.insert(
            "bp-1".into(),
            recipe(true, amounts.iter().map(|(m, a)| slot(m, *a, 0)).collect()),
        );
        let ores = |material: &str, _: u32| OreSites {
            sites: vec![OreSite {
                at: at("Moon A"),
                find: 0.3,
                refine_at: Some(at("Station A")),
                system: Some("Here".into()),
                ore_scu: None,
                rock_scu: None,
            }],
            mined: true,
            ore: Some(format!("{material} (Ore)")),
            refineries: 4,
        };
        derive(&craft_state(1, 0), &inputs, &ores)
    }

    /// A 32 SCU hold takes Iron and Gold (30) in one load; Tin no longer
    /// fits and is the next. An ore more than a hold holds is its own load
    /// of several holds.
    /// A rock broken for its ore goes into the hold whole: 1 SCU refined is
    /// 2 SCU of ore, 2 rocks of 1.5 at the quality, 2 × 20 SCU in the hold,
    /// two 32 SCU loads where the ore alone would fill one.
    #[test]
    fn a_load_counts_the_whole_rocks_it_is_mined_from() {
        let mut inputs = Inputs {
            hold: Some(OreHold {
                ship: "Prospector".into(),
                scu: 32.0,
            }),
            ..Inputs::default()
        };
        inputs
            .recipes
            .insert("bp-1".into(), recipe(true, vec![slot("Iron", 1.0, 0)]));
        let ores = |material: &str, _: u32| OreSites {
            sites: vec![OreSite {
                at: at("Moon A"),
                find: 0.5,
                refine_at: Some(at("Station A")),
                system: Some("Here".into()),
                ore_scu: Some(1.5),
                rock_scu: Some(20.0),
            }],
            mined: true,
            ore: Some(format!("{material} (Ore)")),
            refineries: 1,
        };
        let board = derive(&craft_state(1, 0), &inputs, &ores);
        let mine = board
            .steps
            .iter()
            .find(|s| s.kind == StepKind::Mine)
            .unwrap();
        assert_eq!(mine.scu, 2.0);
        assert_eq!(mine.rocks.map(|r| r.to_break), Some(2));
        assert_eq!(
            refine_titles(&board),
            [(
                "Refine Iron (Ore) at Station A".to_owned(),
                "2 Prospector loads".to_owned()
            )]
        );
    }

    #[test]
    fn a_system_ore_splits_into_loads_of_the_hold() {
        let board = held(
            vec![("Iron", 10.0), ("Gold", 5.0), ("Tin", 4.0)],
            Some(32.0),
        );
        assert_eq!(
            refine_titles(&board),
            [
                (
                    "Refine the ore at Station A".to_owned(),
                    "Iron and Gold, one Prospector load".to_owned()
                ),
                (
                    "Refine Tin (Ore) at Station A".to_owned(),
                    "one Prospector load".to_owned()
                ),
            ]
        );
        let board = held(vec![("Iron", 35.0), ("Gold", 1.0)], Some(32.0));
        assert_eq!(
            refine_titles(&board),
            [
                (
                    "Refine Iron (Ore) at Station A".to_owned(),
                    "3 Prospector loads".to_owned()
                ),
                (
                    "Refine Gold (Ore) at Station A".to_owned(),
                    "one Prospector load".to_owned()
                ),
            ]
        );
        // Without a hold, the system's ore is one load.
        let board = held(vec![("Iron", 35.0), ("Gold", 1.0)], None);
        assert_eq!(
            refine_titles(&board),
            [(
                "Refine the ore at Station A".to_owned(),
                "Iron and Gold".to_owned()
            )]
        );
    }

    /// Ore from a place off the map (a ring without a marker) is a load of
    /// its own, at a refinery not known yet: where you stand says nothing
    /// about where the ring is.
    #[test]
    fn ore_off_the_map_is_its_own_load() {
        let board = area_ores(vec![
            (
                "Iron",
                vec![("Moon A", 0.30, Some("Station A"), Some("Here"))],
            ),
            ("Tin", vec![("The Ring", 0.30, None, None)]),
        ]);
        assert_eq!(
            refine_titles(&board),
            [
                ("Refine Iron (Ore) at Station A".to_owned(), String::new()),
                ("Refine Tin (Ore) at a refinery".to_owned(), String::new()),
            ]
        );
    }

    #[test]
    fn a_refine_step_states_no_yield_no_time_and_no_odds() {
        let board = refined(Some(at("Station R")), 0);
        let refine = &board.steps[1];
        assert_eq!(refine.seconds, None);
        assert_eq!(refine.cost, None);
        assert_eq!(refine.odds, None);
        assert_eq!(refine.detail, "");
        assert!(!refine.title.chars().any(|c| c.is_ascii_digit()));
    }

    #[test]
    fn without_a_known_refinery_the_refine_step_has_no_place() {
        let board = refined(None, 0);
        let refine = &board.steps[1];
        assert_eq!(refine.title, "Refine Metal (Ore) at a refinery");
        assert_eq!(refine.at, None);
    }

    #[test]
    fn an_ore_the_recipe_takes_as_it_is_and_one_no_place_gives_have_no_refine_step() {
        // "Ore A" is asked for by its own name: no `ore` is handed in.
        let mut inputs = Inputs::default();
        inputs
            .recipes
            .insert("bp-1".into(), recipe(true, vec![slot("Ore A", 2.0, 0)]));
        let raw = |_: &str, _: u32| OreSites {
            sites: vec![OreSite {
                system: None,
                at: at("Moon One"),
                find: 0.5,
                ..OreSite::default()
            }],
            mined: true,
            ..OreSites::default()
        };
        let board = derive(&craft_state(1, 0), &inputs, &raw);
        assert!(board.steps.iter().all(|s| s.kind != StepKind::Refine));
        // Refined, but no place rolls it at the quality: there is nothing
        // to refine yet.
        let none = |_: &str, _: u32| OreSites {
            mined: true,
            ore: Some("Ore A (Ore)".into()),
            refineries: 4,
            ..OreSites::default()
        };
        let board = derive(&craft_state(1, 900), &inputs, &none);
        assert!(board.steps.iter().all(|s| s.kind != StepKind::Refine));
        assert!(board.steps.iter().any(|s| s.kind == StepKind::Gather));
    }

    #[test]
    fn loadout_stages_come_from_where_each_part_can_be_had() {
        let mut sold = part("a");
        sold.offers = vec![Offer {
            shop: "Shop One".into(),
            at: at("Town"),
            price: 10.0,
        }];
        sold.fitted = true;
        let mut made = part("b");
        made.blueprint = Some("bp-1".into());
        let mut inputs = loadout(vec![sold, made]);
        inputs.recipes.insert("bp-1".into(), recipe(false, vec![]));
        let board = derive(&loadout_state(), &inputs, &no_ores);
        let stages: Vec<(&str, &str, StageState)> = board.plans[0]
            .stages
            .iter()
            .map(|s| (s.name.as_str(), s.sub.as_str(), s.state))
            .collect();
        assert_eq!(
            stages,
            [
                ("Store", "1 / 1", StageState::Done),
                ("Crafted", "0 / 1", StageState::Current),
                ("Loot", "none", StageState::Done),
                ("Fitted", "1 / 2", StageState::Ahead),
            ]
        );
    }

    #[test]
    fn a_plan_whose_goal_is_gone_says_so_and_derives_nothing() {
        let board = derive(&loadout_state(), &Inputs::default(), &no_ores);
        assert!(board.plans[0].missing);
        assert!(board.steps.is_empty());
    }

    #[test]
    fn one_crafted_copy_serves_one_plan_the_first_that_wants_it() {
        // A loadout that fits the part, then a project to craft one of it.
        let mut made = part("a");
        made.blueprint = Some("bp-1".into());
        let mut state = loadout_state();
        state.add("", Goal::Craft(CraftGoal::uniform("bp-1", 1, 0)));
        let mut inputs = loadout(vec![made]);
        let mut widget = recipe(true, vec![slot("Ore A", 2.0, 0)]);
        widget.entity = Some("a".into());
        widget.crafted = copies(1, &[]);
        inputs.recipes.insert("bp-1".into(), widget);
        inputs.parts.insert(
            "a".into(),
            vec![HeldPart {
                at: at("Depot One"),
                count: 1,
            }],
        );
        let board = derive(&state, &inputs, &no_ores);
        // The loadout takes the copy; the project still has one to craft.
        assert_eq!(board.steps[0].kind, StepKind::Fit);
        assert!(!board.plans[1].done);
        assert_eq!(board.plans[1].tiles[0].value, "0 of 1");
    }

    #[test]
    fn a_loadout_with_nothing_to_fit_is_not_done_on_a_ship_you_do_not_own() {
        let mut inputs = loadout(vec![]);
        inputs.loadouts.get_mut("lo-1").unwrap().read = ShipRead::NotOwned;
        let board = derive(&loadout_state(), &inputs, &no_ores);
        assert!(!board.plans[0].done);
        assert!(board.plans[0].waiting);
        assert_eq!(
            board.plans[0].note,
            "Waits on the ship: you own no Test Ship"
        );
    }

    #[test]
    fn a_blueprint_no_mission_grants_blocks_its_plan_and_says_so() {
        let mut inputs = Inputs::default();
        inputs.recipes.insert("bp-1".into(), recipe(false, vec![]));
        let board = derive(&craft_state(1, 0), &inputs, &no_ores);
        let earn = board
            .steps
            .iter()
            .find(|s| s.kind == StepKind::Earn)
            .unwrap();
        assert!(earn.blocked);
        assert!(board.plans[0].waiting);
        assert_eq!(
            board.plans[0].note,
            "Waits: no mission is known to grant it"
        );
    }

    #[test]
    fn a_craft_has_no_place_so_a_session_cannot_stop_for_it() {
        // No data says where crafting is done: the step is listed, the trip
        // leaves it out and says why nothing is planned.
        let mut inputs = Inputs::default();
        inputs
            .recipes
            .insert("bp-1".into(), recipe(true, vec![slot("Ore A", 2.0, 0)]));
        inputs.stock = vec![stack("Ore A", 500, 2.0)];
        let board = derive(&craft_state(1, 0), &inputs, &no_ores);
        assert_eq!(board.steps[0].at, None);
        let t = crate::session::tracks(&board.steps, &[], None, None);
        let craft = &t.tracks[0];
        assert_eq!(craft.kind, crate::session::TrackKind::Crafting);
        assert!(craft.trip.stops.is_empty());
        assert_eq!(craft.trip.unplaced, 1);
    }

    #[test]
    fn each_slot_asks_at_its_own_minimum() {
        let mut inputs = Inputs::default();
        inputs.recipes.insert("bp-1".into(), repeater());
        inputs.stock = vec![
            // Enough metal at the frame's 500, not at the 750 of the others.
            stack("Metal", 588, 3.0),
            // Ice below its 750 does not count; the gems are simply missing.
            stack("Ice", 700, 9.0),
        ];
        let asked = std::cell::RefCell::new(Vec::new());
        let ores = |material: &str, quality: u32| {
            asked.borrow_mut().push((material.to_owned(), quality));
            OreSites {
                sites: vec![OreSite {
                    system: None,
                    at: at("Moon One"),
                    find: 0.2,
                    ..OreSite::default()
                }],
                mined: true,
                ..OreSites::default()
            }
        };
        let board = derive(&slots_state(&[500, 750, 750]), &inputs, &ores);

        let rows: Vec<(&str, u32, f64)> = board
            .ledger
            .iter()
            .map(|r| (r.material.as_str(), r.min_quality, r.short()))
            .collect();
        assert_eq!(
            rows,
            [("Metal", 500, 0.0), ("Gem", 750, 75.0), ("Ice", 750, 1.5)]
        );
        // The mining odds are asked for at each slot's own minimum, and
        // only for what is short.
        assert_eq!(
            *asked.borrow(),
            [("Gem".to_owned(), 750), ("Ice".to_owned(), 750)]
        );
        let titles: Vec<&str> = board.steps.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(
            titles,
            [
                "Mine 75 × Gem, Q750+",
                "Mine 1.5 SCU Ice, Q750+",
                "Craft the Widget"
            ]
        );
        assert_eq!(board.steps[2].detail, "Needs Gem and Ice");
        // The next session flies what the steps say: per-slot places.
        let t = crate::session::tracks(&board.steps, &[], None, None);
        assert!(t.tracks.iter().all(|t| t.trip.stops.is_empty()));
    }

    #[test]
    fn a_craft_plan_shows_each_slots_minimum_and_what_it_moves() {
        let mut inputs = Inputs::default();
        inputs.recipes.insert("bp-1".into(), repeater());
        inputs.stock = vec![stack("Metal", 588, 3.0)];
        let board = derive(&slots_state(&[500, 750, 750]), &inputs, &no_ores);
        let plan = &board.plans[0];
        assert_eq!(plan.subtitle, "1 to craft, materials from Q500 to Q750");
        let craft = &plan.crafts[0];
        assert_eq!(craft.quality, None);
        let slots: Vec<(Option<&str>, &str, u32, bool, &str)> = craft
            .slots
            .iter()
            .map(|s| {
                (
                    s.name.as_deref(),
                    s.material.as_str(),
                    s.min,
                    s.held,
                    s.moves[0].as_str(),
                )
            })
            .collect();
        assert_eq!(
            slots,
            [
                (Some("Frame"), "Metal", 500, true, "Integrity"),
                (None, "Gem", 750, false, "Impact Force"),
                (None, "Ice", 750, false, "Impact Force"),
            ]
        );
        let values: Vec<u32> = craft.slots[0].bands.iter().map(|b| b.value).collect();
        assert_eq!(values, [346, 588, 796]);
        assert_eq!(craft.slots[0].floor, 1);
    }

    #[test]
    fn a_slots_own_minimum_holds_when_the_plan_asks_for_less() {
        let mut inputs = Inputs::default();
        inputs.recipes.insert(
            "bp-1".into(),
            recipe(true, vec![slot("Ore A", 1.0, 800), slot("Ore B", 1.0, 0)]),
        );
        let board = derive(&slots_state(&[500, 600]), &inputs, &no_ores);
        let mins: Vec<u32> = board.ledger.iter().map(|r| r.min_quality).collect();
        assert_eq!(mins, [800, 600]);
        assert_eq!(board.plans[0].crafts[0].slots[0].floor, 800);
    }

    #[test]
    fn two_slots_of_one_material_at_different_minimums_are_two_demands() {
        let mut inputs = Inputs::default();
        inputs.recipes.insert(
            "bp-1".into(),
            recipe(true, vec![slot("Ore A", 2.0, 0), slot("Ore A", 1.0, 0)]),
        );
        // The Q900 stack must stay for the slot that needs it.
        inputs.stock = vec![stack("Ore A", 900, 1.0), stack("Ore A", 550, 2.0)];
        let board = derive(&slots_state(&[500, 850]), &inputs, &no_ores);
        assert_eq!(board.ledger.len(), 2);
        assert!(board.ledger.iter().all(|r| r.short() == 0.0));
        assert_eq!(board.steps[0].detail, "Materials held");
        // At one quality they are one demand, as before.
        let board = derive(&craft_state(1, 500), &inputs, &no_ores);
        assert_eq!(board.ledger.len(), 1);
        assert_eq!(board.ledger[0].need, 3.0);
    }

    #[test]
    fn a_crafted_copy_counts_when_it_reaches_each_slots_minimum() {
        let mut inputs = Inputs::default();
        let mut widget = repeater();
        widget.crafted = vec![
            CraftedCopy {
                count: 1,
                ingredients: vec![
                    ("Metal".into(), 588),
                    ("Gem".into(), 700),
                    ("Ice".into(), 800),
                ],
            },
            CraftedCopy {
                count: 1,
                ingredients: vec![
                    ("Metal".into(), 510),
                    ("Gem".into(), 760),
                    ("Ice".into(), 800),
                ],
            },
        ];
        inputs.recipes.insert("bp-1".into(), widget);
        let mut state = PlansState::default();
        state.add(
            "",
            Goal::Craft(CraftGoal::with_slots("bp-1", 2, 0, &[500, 750, 750])),
        );
        let board = derive(&state, &inputs, &no_ores);
        // The first copy's gems are below 750: only the second counts.
        assert_eq!(board.plans[0].tiles[0].value, "1 of 2");
        // At one quality for all, the frame's Q510 rules that copy out too.
        let board = derive(&craft_state(2, 750), &inputs, &no_ores);
        assert_eq!(board.plans[0].tiles[0].value, "0 of 2");
    }

    #[test]
    fn a_project_reads_as_one_plan_with_its_crafts_inside() {
        let mut state = PlansState::default();
        let kit = project(
            &mut state,
            "Mining kit",
            vec![
                CraftGoal::uniform("bp-1", 2, 0),
                CraftGoal::with_slots("bp-2", 1, 0, &[600]),
                CraftGoal::uniform("bp-gone", 1, 0),
            ],
        );
        let mut inputs = Inputs::default();
        inputs
            .recipes
            .insert("bp-1".into(), recipe(true, vec![slot("Ore A", 2.0, 0)]));
        let mut second = recipe(false, vec![slot("Ore B", 1.0, 0)]);
        second.name = "Gadget".into();
        inputs.recipes.insert("bp-2".into(), second);
        inputs.stock = vec![stack("Ore A", 500, 4.0)];
        let board = derive(&state, &inputs, &no_ores);

        assert_eq!(board.plans.len(), 1);
        let plan = &board.plans[0];
        assert_eq!(plan.kind, PlanKind::Project);
        assert_eq!(
            (plan.title.as_str(), plan.name.as_str()),
            ("Mining kit", "Mining kit")
        );
        assert_eq!(plan.subtitle, "3 crafts");
        assert!(!plan.done);
        // Its own next step, from all of its crafts.
        assert_eq!(plan.note, "Next: get 1 SCU Ore B, Q600+");
        let crafts: Vec<(&str, u32, u32, bool)> = plan
            .crafts
            .iter()
            .map(|c| (c.name.as_str(), c.count, c.now, c.missing))
            .collect();
        assert_eq!(
            crafts,
            [
                ("Widget", 2, 2, false),
                ("Gadget", 1, 0, false),
                ("A blueprint this build does not have", 1, 0, true),
            ]
        );
        let stages: Vec<(&str, &str)> = plan
            .stages
            .iter()
            .map(|s| (s.name.as_str(), s.sub.as_str()))
            .collect();
        assert_eq!(
            stages,
            [
                ("Blueprints", "1 / 3"),
                ("Materials", "1 / 2"),
                ("Crafted", "0 / 4")
            ]
        );
        // Every step of its crafts is the project's.
        assert!(
            board
                .steps
                .iter()
                .all(|s| s.plans == std::slice::from_ref(&kit))
        );
        assert_eq!(board.ledger[1].min_quality, 600);
    }

    #[test]
    fn an_empty_project_is_not_done() {
        let mut state = PlansState::default();
        project(&mut state, "Later", vec![]);
        let board = derive(&state, &Inputs::default(), &no_ores);
        let plan = &board.plans[0];
        assert!(!plan.done && plan.waiting);
        assert_eq!(plan.note, "No crafts in it yet");
        assert_eq!(plan.subtitle, "0 crafts");
    }

    /// Two projects that both want 2 SCU of Ore A, and stock for one.
    fn rivals() -> (PlansState, Inputs, String, String) {
        let mut state = PlansState::default();
        let one = project(&mut state, "One", vec![CraftGoal::uniform("bp-1", 1, 0)]);
        let two = project(&mut state, "Two", vec![CraftGoal::uniform("bp-1", 1, 0)]);
        let mut inputs = Inputs::default();
        inputs
            .recipes
            .insert("bp-1".into(), recipe(true, vec![slot("Ore A", 2.0, 0)]));
        inputs.stock = vec![stack("Ore A", 500, 2.0)];
        (state, inputs, one, two)
    }

    #[test]
    fn projects_are_served_in_their_order_and_reordering_changes_who_gets_the_stock() {
        let (mut state, inputs, one, two) = rivals();
        let board = derive(&state, &inputs, &no_ores);
        // The same blueprint in both: the first project gets the stock.
        assert_eq!(board.plans[0].crafts[0].now, 1);
        assert_eq!(board.plans[1].crafts[0].now, 0);
        assert_eq!(board.ledger[0].plans, [one.clone(), two.clone()]);

        state.move_before(&two, Some(&one));
        let board = derive(&state, &inputs, &no_ores);
        assert_eq!(board.plans[0].id, two);
        assert_eq!(board.plans[0].crafts[0].now, 1);
        assert_eq!(board.plans[1].crafts[0].now, 0);
    }

    #[test]
    fn a_parked_project_reserves_nothing_and_has_no_steps() {
        let (mut state, inputs, one, two) = rivals();
        state.set_parked(&one, true);
        let board = derive(&state, &inputs, &no_ores);

        // It is still shown, in its place, with its crafts.
        let parked = &board.plans[0];
        assert_eq!(parked.id, one);
        assert!(parked.parked && !parked.done && !parked.waiting);
        assert_eq!(parked.note, "Parked: it reserves nothing");
        assert_eq!(parked.crafts[0].name, "Widget");
        assert!(!parked.crafts[0].slots[0].held);
        assert_eq!(parked.progress, 0.5);
        // The stock goes to the project behind it.
        assert_eq!(board.plans[1].crafts[0].now, 1);
        assert_eq!(board.ledger.len(), 1);
        assert_eq!(board.ledger[0].plans, std::slice::from_ref(&two));
        assert_eq!((board.ledger[0].need, board.ledger[0].held), (2.0, 2.0));
        assert!(
            board
                .steps
                .iter()
                .all(|s| s.plans == std::slice::from_ref(&two))
        );

        // Taken back up, it is served first again.
        state.set_parked(&one, false);
        let board = derive(&state, &inputs, &no_ores);
        assert_eq!(board.plans[0].crafts[0].now, 1);
        assert_eq!(board.plans[1].crafts[0].now, 0);
    }

    #[test]
    fn a_parked_loadout_takes_no_part_and_a_parked_craft_no_copy() {
        let mut state = loadout_state();
        state.add("", Goal::Craft(CraftGoal::uniform("bp-1", 1, 0)));
        let first = state.plans[0].id.clone();
        state.set_parked(&first, true);
        let mut inputs = loadout(vec![part("a")]);
        let mut widget = recipe(true, vec![slot("Ore A", 2.0, 0)]);
        widget.entity = Some("a".into());
        widget.crafted = copies(1, &[]);
        inputs.recipes.insert("bp-1".into(), widget);
        inputs.parts.insert(
            "a".into(),
            vec![HeldPart {
                at: at("Depot One"),
                count: 1,
            }],
        );
        let board = derive(&state, &inputs, &no_ores);
        // The parked loadout fits nothing, so the copy counts for the craft.
        assert!(board.steps.is_empty());
        assert!(board.plans[1].done);
        assert!(!board.plans[0].done);
    }

    #[test]
    fn amounts_read_like_the_game_shows_them() {
        assert_eq!(amount(12.0, true), "12 SCU");
        assert_eq!(amount(2.5, true), "2.5 SCU");
        assert_eq!(amount(0.06, true), "0.06 SCU");
        assert_eq!(amount(0.001, true), "under 0.01 SCU");
        assert_eq!(amount(3.0, false), "3 ×");
        assert_eq!(auec_figure(41_300.0), "41,300");
        assert_eq!(auec_figure(950.0), "950");
    }
}
