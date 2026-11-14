//! The steps your inventory leaves open, per plan.
//!
//! Nothing here is stored: a step exists while the account's state says it
//! is still to do, and is gone once the game shows it done. Each step is
//! phrased as an action with its reason. A figure is only stated where the
//! shell handed in a real one (a UEX price, a recipe's craft time, the share
//! of rocks that hold an ore); what the data cannot give is left out.

use std::collections::HashMap;

use serde::Serialize;

use crate::ledger::{self, Demand, Fill, LedgerRow, Stack};
use crate::model::{Goal, PlansState};

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
    /// Crafted copies of the item you already hold.
    pub crafted_held: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SlotNeed {
    pub material: String,
    /// SCU for bulk, pieces else, for one craft.
    pub amount: f64,
    pub bulk: bool,
    pub min_quality: u32,
}

/// A mission that grants a blueprint.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Grant {
    pub title: String,
    pub at: Vec<PlanPlace>,
}

/// Where an ore can be ship-mined at a quality.
#[derive(Debug, Clone, PartialEq)]
pub struct OreSite {
    pub at: PlanPlace,
    /// The share of the ship rocks there that hold it at that quality.
    pub find: f32,
}

/// What mining knows about a material at a quality.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OreSites {
    /// Best first; empty when no place rolls it at that quality.
    pub sites: Vec<OreSite>,
    /// The material is ship-mined at all, at any quality.
    pub mined: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum StepKind {
    /// Fit a part you hold.
    Fit,
    Buy,
    Mine,
    /// Get a material no ship-mining place gives.
    Gather,
    Craft,
    /// Earn a blueprint from a mission.
    Earn,
    /// A part no shop sells and no blueprint makes.
    Find,
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
    /// Time the step itself takes, where the data gives one (a recipe's
    /// craft time).
    pub seconds: Option<f64>,
    /// aUEC.
    pub cost: Option<f64>,
    /// Mining: the share of the rocks at the place that hold the ore at the
    /// wanted quality.
    pub odds: Option<f32>,
    /// How many places it can be done at (mining places, shops, missions).
    pub sources: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum PlanKind {
    Loadout,
    Craft,
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
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Board {
    pub plans: Vec<PlanBoard>,
    /// Ready steps first, each group in plan order.
    pub steps: Vec<PlanStep>,
    pub ledger: Vec<LedgerRow>,
}

/// One craft a plan wants: a craft project, or a loadout part only a
/// blueprint makes.
struct Craft<'a> {
    plan: &'a str,
    blueprint: &'a str,
    recipe: &'a RecipeFacts,
    /// Still to craft.
    left: u32,
    quality: u32,
    /// (index into the demand list, amount for one craft).
    demands: Vec<(usize, f64)>,
}

struct Part<'a> {
    facts: &'a PartFacts,
    /// A free copy reserved for it.
    held: Option<PlanPlace>,
}

struct LoadoutWork<'a> {
    plan: &'a str,
    facts: &'a LoadoutFacts,
    parts: Vec<Part<'a>>,
}

/// Derive every plan's steps, stages and figures, and the ledger across
/// them. `ores` answers where a material can be ship-mined at a quality.
pub fn derive(state: &PlansState, inputs: &Inputs, ores: &dyn Fn(&str, u32) -> OreSites) -> Board {
    // Free parts are counted once, like stock: reserved in plan order.
    let mut pool: HashMap<&str, Vec<(PlanPlace, u32)>> = inputs
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
                };
                for part in &facts.parts {
                    let held = (!part.fitted)
                        .then(|| take(pool.get_mut(part.item.as_str())))
                        .flatten();
                    let lacks = !part.fitted && held.is_none() && part.offers.is_empty();
                    if let (true, Some(bp)) = (lacks, &part.blueprint)
                        && let Some(recipe) = inputs.recipes.get(bp)
                    {
                        match crafts
                            .iter_mut()
                            .find(|c| c.plan == plan.id && c.blueprint == bp)
                        {
                            Some(c) => c.left += 1,
                            None => crafts.push(Craft {
                                plan: &plan.id,
                                blueprint: bp,
                                recipe,
                                left: 1,
                                quality: 0,
                                demands: Vec::new(),
                            }),
                        }
                    }
                    work.parts.push(Part { facts: part, held });
                }
                loadouts.push(work);
            }
            Goal::Craft(goal) => {
                let Some(recipe) = inputs.recipes.get(&goal.blueprint) else {
                    continue;
                };
                crafts.push(Craft {
                    plan: &plan.id,
                    blueprint: &goal.blueprint,
                    recipe,
                    left: goal.count.saturating_sub(recipe.crafted_held),
                    quality: goal.quality,
                    demands: Vec::new(),
                });
            }
            Goal::Unknown => {}
        }
    }

    // Demands in plan order: `crafts` was filled in that order.
    for craft in &mut crafts {
        if craft.left == 0 {
            continue;
        }
        for slot in &craft.recipe.slots {
            let quality = slot.min_quality.max(craft.quality);
            let at = craft.demands.iter().position(|(i, _)| {
                let d: &Demand = &demands[*i];
                d.material == slot.material && d.min_quality == quality && d.bulk == slot.bulk
            });
            match at {
                Some(at) => {
                    let (i, each) = &mut craft.demands[at];
                    *each += slot.amount;
                    demands[*i].amount += slot.amount * f64::from(craft.left);
                }
                None => {
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
    for work in &loadouts {
        for (n, part) in work.parts.iter().enumerate() {
            let facts = part.facts;
            if facts.fitted {
                continue;
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
                    },
                    1,
                ));
            } else if let Some(offer) = facts.offers.first() {
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
            } else if facts
                .blueprint
                .as_ref()
                .is_none_or(|bp| !inputs.recipes.contains_key(bp))
            {
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
                });
            }
        }
    }
    for (mut step, count) in fits {
        let name = std::mem::take(&mut step.title);
        let place = step.at.as_ref().map(sits_at).unwrap_or_default();
        (step.title, step.detail) = match (count, step.blocked) {
            (1, false) => (format!("Fit the {name} you own"), format!("It is {place}")),
            (1, true) => (format!("Fit the {name} you own"), step.detail),
            (n, false) => (
                format!("Fit {n} × {name} you own"),
                format!("They are {place}"),
            ),
            (n, true) => (format!("Fit {n} × {name} you own"), step.detail),
        };
        push(step);
    }
    for (mut step, lines) in buys.into_iter().zip(buy_lines) {
        let count: u32 = lines.iter().map(|(_, c)| c).sum();
        let shop = step.id.trim_start_matches("buy:").to_owned();
        let price = auec(step.cost.unwrap_or(0.0));
        if let [(name, 1)] = lines.as_slice() {
            step.title = format!("Buy the {name}");
            step.detail = format!("{shop} sells it for {price} (UEX)");
        } else {
            step.title = format!("Buy {count} parts at {shop}");
            let list: Vec<String> = lines
                .iter()
                .map(|(name, c)| format!("{c} × {name}"))
                .collect();
            step.detail = format!("{} for {price} (UEX)", and(&list));
        }
        push(step);
    }

    finds.into_iter().for_each(&mut push);

    // Blueprints to earn: one step a blueprint, whichever plans want it.
    let mut earns: Vec<PlanStep> = Vec::new();
    for craft in crafts.iter().filter(|c| c.left > 0 && !c.recipe.owned) {
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
            blocked: false,
            at: grants
                .iter()
                .flat_map(|g| &g.at)
                .find(|w| w.site.is_some())
                .or_else(|| grants.iter().flat_map(|g| &g.at).next())
                .cloned(),
            seconds: None,
            cost: None,
            odds: None,
            sources: grants.len() as u32,
        });
    }
    earns.into_iter().for_each(&mut push);

    // Materials still short: one step a ledger row.
    for row in ledger.rows.iter().filter(|r| r.short() > 1e-6) {
        let found = ores(&row.material, row.min_quality);
        let amount = amount(row.short(), row.bulk);
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
        };
        push(match found.sites.first() {
            Some(best) => PlanStep {
                title: format!("Mine {amount} {}{grade}", row.material),
                detail: format!(
                    "{} of the ship rocks there hold it{}",
                    percent(best.find),
                    if row.min_quality > 1 {
                        " at this quality"
                    } else {
                        ""
                    }
                ),
                at: Some(best.at.clone()),
                odds: Some(best.find),
                ..base
            },
            None => PlanStep {
                kind: StepKind::Gather,
                title: format!("Get {amount} {}{grade}", row.material),
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

    // Crafts: what the reserved stock covers now, and what still waits.
    let mut craft_now: HashMap<(&str, &str), u32> = HashMap::new();
    for craft in crafts.iter().filter(|c| c.left > 0) {
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
        let grade = if craft.quality > 0 {
            format!(" at Q{}", craft.quality)
        } else {
            String::new()
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
            });
        }
    }

    let mut steps = ready;
    steps.append(&mut blocked);

    let plans = state
        .plans
        .iter()
        .map(|plan| {
            let mine: Vec<&PlanStep> = steps
                .iter()
                .filter(|s| s.plans.contains(&plan.id))
                .collect();
            let mut board = match &plan.goal {
                Goal::Loadout(_) => match loadouts.iter().find(|w| w.plan == plan.id) {
                    Some(work) => loadout_board(work, &mine),
                    None => missing(PlanKind::Loadout, "Loadout", "This saved loadout is gone"),
                },
                Goal::Craft(goal) => match crafts
                    .iter()
                    .find(|c| c.plan == plan.id && c.blueprint == goal.blueprint)
                {
                    Some(craft) => {
                        let now = craft_now
                            .get(&(craft.plan, craft.blueprint))
                            .copied()
                            .unwrap_or(0);
                        craft_board(craft, goal.count, now, &demands, &ledger.fills, &mine)
                    }
                    None => missing(
                        PlanKind::Craft,
                        "Craft project",
                        "This build has no such blueprint",
                    ),
                },
                Goal::Unknown => missing(PlanKind::Unknown, "Plan", "Made by a newer Starlume"),
            };
            board.id = plan.id.clone();
            if !plan.name.is_empty() {
                board.name = plan.name.clone();
            }
            if !board.missing {
                board.done = mine.is_empty();
                board.waiting = !board.done && mine.iter().all(|s| s.blocked);
                board.note = if board.done {
                    "Done".to_owned()
                } else if let Some(next) = mine.iter().find(|s| !s.blocked) {
                    format!("Next: {}", lower_first(&next.title))
                } else {
                    mine.first()
                        .map(|s| format!("Waits: {}", lower_first(&s.detail)))
                        .unwrap_or_default()
                };
            }
            board
        })
        .collect();

    Board {
        plans,
        steps,
        ledger: ledger.rows,
    }
}

fn take(pool: Option<&mut Vec<(PlanPlace, u32)>>) -> Option<PlanPlace> {
    let entry = pool?.iter_mut().find(|(_, count)| *count > 0)?;
    entry.1 -= 1;
    Some(entry.0.clone())
}

fn missing(kind: PlanKind, title: &str, note: &str) -> PlanBoard {
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
    }
}

fn craft_board(
    craft: &Craft,
    count: u32,
    now: u32,
    demands: &[Demand],
    fills: &[Fill],
    steps: &[&PlanStep],
) -> PlanBoard {
    let recipe = craft.recipe;
    let crafted = count.saturating_sub(craft.left);
    let (covered, rows) = craft.demands.iter().fold((0u32, 0u32), |(c, n), (i, _)| {
        (c + u32::from(fills[*i].covered()), n + 1)
    });
    let coverage: f64 = if craft.left == 0 || craft.demands.is_empty() {
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
    let counts = [
        ("Blueprint", (u32::from(blueprint), 1)),
        (
            "Materials",
            if craft.left == 0 {
                (1, 1)
            } else {
                (covered, rows.max(1))
            },
        ),
        ("Crafted", (crafted, count)),
    ];
    let mut stages = stages(&counts);
    stages[0].sub = if blueprint {
        "owned".to_owned()
    } else {
        "to earn".to_owned()
    };
    let subtitle = match craft.quality {
        0 => format!("{count} to craft"),
        q => format!("{count} to craft at Q{q}"),
    };
    let open = steps.len();
    PlanBoard {
        id: String::new(),
        kind: PlanKind::Craft,
        title: recipe.name.clone(),
        name: format!("{}, {subtitle}", recipe.name),
        subtitle,
        progress: ((f64::from(u8::from(blueprint)) + coverage) / 2.0
            * (1.0 - f64::from(crafted) / f64::from(count.max(1)))
            + f64::from(crafted) / f64::from(count.max(1))) as f32,
        done: false,
        missing: false,
        waiting: false,
        note: String::new(),
        stages,
        tiles: vec![
            tile(format!("{crafted} of {count}"), "crafted"),
            tile(now.to_string(), "craftable now"),
            tile(format!("{covered} of {rows}"), "materials held"),
            tile(
                open.to_string(),
                if open == 1 { "step open" } else { "steps open" },
            ),
        ],
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

fn tile(value: String, label: &str) -> PlanTile {
    PlanTile {
        value,
        label: label.to_owned(),
    }
}

/// "sit at Levski", "are on you": where held parts are, after "It" or
/// "They" (the caller picks; one part "sits", so the s is added there).
fn sits_at(at: &PlanPlace) -> String {
    match at.name.as_str() {
        "On you" => "on you".to_owned(),
        name => format!("at {name}"),
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

fn percent(share: f32) -> String {
    let pct = share * 100.0;
    if pct > 0.0 && pct < 1.0 {
        "Under 1%".to_owned()
    } else {
        format!("{pct:.0}%")
    }
}

fn auec_figure(value: f64) -> String {
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

fn auec(value: f64) -> String {
    format!("{} aUEC", auec_figure(value))
}

/// "a", "a and b", "a, b and c".
fn and(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
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
    use crate::model::{CraftGoal, LoadoutGoal};

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
        }
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
        state.add(
            "",
            Goal::Craft(CraftGoal {
                blueprint: "bp-1".into(),
                count,
                quality,
            }),
        );
        state
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
        assert_eq!(step.detail, "It is at Depot One");
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
        assert_eq!(
            board.steps[1].detail,
            "Shop One sells it for 1,200 aUEC (UEX)"
        );
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
        assert_eq!(
            board.steps[0].detail,
            "2 × Part a and 1 × Part b for 61,300 aUEC (UEX)"
        );
        assert_eq!(board.steps[0].cost, Some(61_300.0));
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
        assert_eq!(board.steps[0].detail, "They are at Depot One");
    }

    #[test]
    fn a_minimum_quality_of_one_is_any_quality_and_is_not_named() {
        let mut inputs = Inputs::default();
        inputs
            .recipes
            .insert("bp-1".into(), recipe(true, vec![slot("Ore A", 2.0, 1)]));
        let ores = |_: &str, _: u32| OreSites {
            sites: vec![OreSite {
                at: at("Moon One"),
                find: 0.5,
            }],
            mined: true,
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
    fn a_craft_without_blueprint_or_material_is_blocked_and_says_on_what() {
        let mut inputs = Inputs::default();
        let mut widget = recipe(false, vec![slot("Ore A", 2.0, 500), slot("Ore B", 1.0, 0)]);
        widget.grants = vec![
            Grant {
                title: "Run One".into(),
                at: vec![at("Planet One")],
            },
            Grant {
                title: "Run Two".into(),
                at: vec![],
            },
        ];
        inputs.recipes.insert("bp-1".into(), widget);
        inputs.stock = vec![stack("Ore B", 100, 5.0)];
        let ores = |material: &str, _: u32| match material {
            "Ore A" => OreSites {
                sites: vec![OreSite {
                    at: at("Moon One"),
                    find: 0.42,
                }],
                mined: true,
            },
            _ => OreSites::default(),
        };
        let board = derive(&craft_state(1, 700), &inputs, &ores);
        let by_kind = |kind| board.steps.iter().find(|s| s.kind == kind).unwrap();

        let earn = by_kind(StepKind::Earn);
        assert_eq!(earn.title, "Earn the Widget blueprint");
        assert_eq!(earn.detail, "2 missions grant it");
        assert_eq!(earn.at, Some(at("Planet One")));

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
        widget.crafted_held = 2;
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
        state.add(
            "",
            Goal::Craft(CraftGoal {
                blueprint: "bp-2".into(),
                count: 1,
                quality: 0,
            }),
        );
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
                at: at("Moon One"),
                find: 0.5,
            }],
            mined: true,
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
    fn amounts_read_like_the_game_shows_them() {
        assert_eq!(amount(12.0, true), "12 SCU");
        assert_eq!(amount(2.5, true), "2.5 SCU");
        assert_eq!(amount(0.06, true), "0.06 SCU");
        assert_eq!(amount(0.001, true), "under 0.01 SCU");
        assert_eq!(amount(3.0, false), "3 ×");
        assert_eq!(auec(41_300.0), "41,300 aUEC");
        assert_eq!(auec(950.0), "950 aUEC");
    }
}
