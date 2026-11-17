//! Plans: the goals you set, and the board derived from them (model,
//! ledger, steps and session in `crates/mod-plans`).
//!
//! The plans are one file of the player's own work. Everything else on the
//! board is derived on each read from what other parts of the app already
//! hold: the cached inventory and blueprints (svc-dossier), saved loadouts
//! (Ships), the UEX price cache, the blueprint catalog, the mining tables,
//! the quantum-travel places, the places with a refinery and the session's
//! last place. Nothing here
//! touches the network: a board without a price cache simply has no buy
//! steps with prices.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use mod_plans::ledger::{LedgerRow, Stack};
use mod_plans::session::{Ordered, OrderedVisit, Router, Site, StopSpec, Tracks, TripLeg};
use mod_plans::steps::{
    CraftedCopy, Grant, HeldPart, Inputs, LoadoutFacts, Offer, OreSite, OreSites, PartFacts,
    PlanBoard, PlanPlace, PlanStep, QualityBand, RecipeFacts, ShipRead, SlotNeed,
};
use mod_plans::{CraftGoal, Goal, LoadoutGoal, Plan, PlansState};
use svc_data::RecordCollection;
use tauri::{AppHandle, Emitter, Manager};

use crate::AppState;
use crate::data::cooked_for;
use crate::error::AppError;

pub(crate) const MODULE_ID: &str = "plans";

/// Emitted after the plans changed (payload-free; the frontend re-reads).
pub const PLANS_CHANGED_EVENT: &str = "plans:changed";

/// Serializes read-modify-writes across commands.
static STATE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn state_path() -> std::path::PathBuf {
    app_kit::app_data_root().join("plans").join("plans.json")
}

/// Read the plans file. A missing file is an empty start; a file that
/// cannot be read or does not parse is an error, never an empty state: the
/// plans are the player's own work, and "empty" would be written back over
/// them by the next change.
fn read_state_at(path: &std::path::Path) -> Result<PlansState, AppError> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(PlansState::default()),
        Err(e) => {
            return Err(AppError::Internal(format!(
                "your plans could not be read ({e}); nothing was changed"
            )));
        }
    };
    serde_json::from_slice(&bytes).map_err(|e| {
        AppError::Internal(format!(
            "your plans file does not parse ({e}); nothing was changed"
        ))
    })
}

/// Read, change and write the plans file. Nothing is written when the read
/// failed.
fn change_at(
    path: &std::path::Path,
    f: impl FnOnce(&mut PlansState),
) -> Result<Vec<Plan>, AppError> {
    let mut state = read_state_at(path)?;
    f(&mut state);
    app_kit::save_json(path, &state)?;
    Ok(state.plans)
}

/// The plans, for showing. A file that cannot be read shows as no plans
/// (logged); it is left as it is, and a change then reports the error.
fn load_state() -> PlansState {
    let _lock = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    read_state_at(&state_path()).unwrap_or_else(|e| {
        tracing::warn!("plans: {e}");
        PlansState::default()
    })
}

fn change(app: &AppHandle, f: impl FnOnce(&mut PlansState)) -> Result<Vec<Plan>, AppError> {
    let plans = {
        let _lock = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        change_at(&state_path(), f)?
    };
    let _ = app.emit(PLANS_CHANGED_EVENT, ());
    Ok(plans)
}

/// Your plans, in order. A file read: no game data needed, so Crafting and
/// Ships can mark what is already planned.
#[tauri::command]
#[specta::specta]
pub(crate) fn plans_list() -> Vec<Plan> {
    load_state().plans
}

/// Plan a number of one item, crafted at a quality. Planning the same
/// blueprint again changes nothing.
#[tauri::command]
#[specta::specta]
pub(crate) fn plans_add_craft(
    app: AppHandle,
    blueprint: String,
    count: u32,
    quality: u32,
) -> Result<Vec<Plan>, AppError> {
    change(&app, |state| {
        state.add(
            "",
            Goal::Craft(CraftGoal::uniform(blueprint, count, quality)),
        );
    })
}

/// Plan a craft with a minimum quality per material slot (`slots`, in the
/// recipe's slot order; empty: `quality` for every slot). With `project`
/// it goes into that project, else it is a plan of its own. A blueprint
/// that is already there is left as it is.
#[tauri::command]
#[specta::specta]
pub(crate) fn plans_plan_craft(
    app: AppHandle,
    project: Option<String>,
    blueprint: String,
    count: u32,
    quality: u32,
    slots: Vec<u32>,
) -> Result<Vec<Plan>, AppError> {
    change(&app, |state| {
        let craft = CraftGoal::with_slots(blueprint, count, quality, &slots);
        match project {
            Some(id) => {
                state.add_to_project(&id, craft);
            }
            None => {
                state.add("", Goal::Craft(craft));
            }
        }
    })
}

/// Change a planned craft's count and minimums: plan `id`'s own craft, or
/// the entry for `blueprint` of a project. `slots` as in
/// [`plans_plan_craft`].
#[tauri::command]
#[specta::specta]
pub(crate) fn plans_update_craft(
    app: AppHandle,
    id: String,
    blueprint: String,
    count: u32,
    quality: u32,
    slots: Vec<u32>,
) -> Result<Vec<Plan>, AppError> {
    change(&app, |state| {
        if let Some(craft) = state.craft_mut(&id, &blueprint) {
            craft.set(count, quality, &slots);
        }
    })
}

/// Take a craft out of a project.
#[tauri::command]
#[specta::specta]
pub(crate) fn plans_remove_craft(
    app: AppHandle,
    id: String,
    blueprint: String,
) -> Result<Vec<Plan>, AppError> {
    change(&app, |state| state.remove_from_project(&id, &blueprint))
}

/// Start an empty project: a named group of crafts that is served as one
/// plan.
#[tauri::command]
#[specta::specta]
pub(crate) fn plans_add_project(app: AppHandle, name: String) -> Result<Vec<Plan>, AppError> {
    change(&app, |state| {
        state.add_project(&name);
    })
}

/// Name a plan; an empty name goes back to the one its goal gives it.
#[tauri::command]
#[specta::specta]
pub(crate) fn plans_rename(
    app: AppHandle,
    id: String,
    name: String,
) -> Result<Vec<Plan>, AppError> {
    change(&app, |state| state.rename(&id, &name))
}

#[tauri::command]
#[specta::specta]
pub(crate) fn plans_set_notes(
    app: AppHandle,
    id: String,
    notes: String,
) -> Result<Vec<Plan>, AppError> {
    change(&app, |state| state.set_notes(&id, &notes))
}

/// Park a plan (it stays in the list and reserves nothing), or take it
/// back up.
#[tauri::command]
#[specta::specta]
pub(crate) fn plans_set_parked(
    app: AppHandle,
    id: String,
    parked: bool,
) -> Result<Vec<Plan>, AppError> {
    change(&app, |state| state.set_parked(&id, parked))
}

/// Move a plan to just before another (`before` absent: to the end). The
/// order is who the ledger serves first.
#[tauri::command]
#[specta::specta]
pub(crate) fn plans_move(
    app: AppHandle,
    id: String,
    before: Option<String>,
) -> Result<Vec<Plan>, AppError> {
    change(&app, |state| state.move_before(&id, before.as_deref()))
}

/// Plan to have a saved loadout fitted on its ship.
#[tauri::command]
#[specta::specta]
pub(crate) fn plans_add_loadout(app: AppHandle, loadout: String) -> Result<Vec<Plan>, AppError> {
    change(&app, |state| {
        state.add("", Goal::Loadout(LoadoutGoal { loadout }));
    })
}

#[tauri::command]
#[specta::specta]
pub(crate) fn plans_remove(app: AppHandle, id: String) -> Result<Vec<Plan>, AppError> {
    change(&app, |state| state.remove(&id))
}

/// Make a plan the focus: it is served first by the ledger.
#[tauri::command]
#[specta::specta]
pub(crate) fn plans_focus(app: AppHandle, id: String) -> Result<Vec<Plan>, AppError> {
    change(&app, |state| state.focus(&id))
}

#[tauri::command]
#[specta::specta]
pub(crate) fn plans_set_craft(
    app: AppHandle,
    id: String,
    count: u32,
    quality: u32,
) -> Result<Vec<Plan>, AppError> {
    change(&app, |state| state.set_craft(&id, count, quality))
}

/// A saved loadout that can be planned.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LoadoutChoiceView {
    pub id: String,
    pub name: String,
    /// The ship's name.
    pub ship: String,
}

/// Every saved loadout, with its ship named: what "New plan" offers beside
/// the recipes.
#[tauri::command]
#[specta::specta]
pub(crate) async fn plans_loadout_choices(
    app: AppHandle,
    channel: String,
) -> Result<Vec<LoadoutChoiceView>, AppError> {
    let cooked = cooked_for(&app, &channel).await?;
    tauri::async_runtime::spawn_blocking(move || {
        crate::ships::saved_loadouts()
            .into_iter()
            .map(|s| LoadoutChoiceView {
                ship: mod_ships::ship_name(&cooked, &s.ship).unwrap_or_else(|| s.ship.clone()),
                id: s.id,
                name: s.name,
            })
            .collect()
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))
}

/// Where a plan's goal lives in the app, for the board's "open" link.
#[derive(Debug, Clone, Default, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanTargetView {
    pub plan: String,
    /// A loadout plan: the ship's GUID and the saved loadout's id.
    pub ship: Option<String>,
    pub loadout: Option<String>,
    /// A craft project: the blueprint's family key on the Crafting page.
    pub family: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlansBoardView {
    pub plans: Vec<PlanBoard>,
    pub targets: Vec<PlanTargetView>,
    pub steps: Vec<PlanStep>,
    pub ledger: Vec<LedgerRow>,
    /// One per choice, in the order the card offers them.
    /// One track per kind of work with its next trip, and the one the next
    /// session suggests.
    pub tracks: Tracks,
    /// The last known place, as the session log names it; `None` when no
    /// session has one or the map does not know it.
    pub from: Option<String>,
    /// The ship whose quantum drive times the legs (the first loadout
    /// plan's); `None`: legs carry distances only.
    pub drive_ship: Option<String>,
    /// When the inventory was read from the game, unix seconds; `None`: it
    /// never was, and nothing you hold is counted.
    pub inventory_at: Option<f64>,
    /// When the UEX prices were fetched, unix seconds.
    pub prices_at: Option<f64>,
}

/// The board: every plan with its stages, the open steps, the ledger and
/// the tracks. Derived on each call; nothing is stored.
#[tauri::command]
#[specta::specta]
pub(crate) async fn plans_board(
    app: AppHandle,
    channel: String,
) -> Result<PlansBoardView, AppError> {
    let cooked = cooked_for(&app, &channel).await?;
    let blueprints = crate::crafting::index_for(&app, &channel).await.ok();
    let last_place = last_place(&app).await;
    let state = app.state::<AppState>();
    let inventory = state.dossier.cached_inventory();
    let owned_blueprints: HashSet<String> = state
        .dossier
        .cached_blueprints()
        .map(|o| o.blueprint_ids.into_iter().collect())
        .unwrap_or_default();
    let uex = state.uex.clone();
    let plans = load_state();

    tauri::async_runtime::spawn_blocking(move || {
        let gathered = Gathered {
            cooked: &cooked,
            blueprints: blueprints.as_deref(),
            inventory: inventory.as_ref(),
            owned_blueprints,
            hand_owned: crate::crafting::hand_owned().into_iter().collect(),
            prices: uex.snapshot(),
            prices_at: uex.fetched_at(),
            last_place,
        };
        gathered.board(&plans)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))
}

/// The session log's last place, as the game names it internally: from the
/// current session, else from the newest finished one. `None` while the
/// module is off or no session has one.
async fn last_place(app: &AppHandle) -> Option<String> {
    let of = |session: &mod_session::Session| {
        session.timeline.iter().rev().find_map(|e| match &e.kind {
            mod_session::EntryKind::Location { location } => Some(location.clone()),
            _ => None,
        })
    };
    let current = crate::session::session_state(app.clone()).await.current;
    if let Some(place) = current.as_ref().and_then(of) {
        return Some(place);
    }
    let history = crate::session::session_history(app.clone()).await.ok()?;
    for summary in history.into_iter().take(3) {
        let kept = crate::session::session_get(app.clone(), summary.id).await;
        if let Some(place) = kept.ok().flatten().as_ref().and_then(of) {
            return Some(place);
        }
    }
    None
}

/// The site ids of the places with a refinery.
fn refinery_ids(cooked: &svc_data::CookedData) -> Vec<String> {
    mod_ships::refinery_places(cooked)
        .into_iter()
        .map(|p| p.guid)
        .collect()
}

/// Seconds an arrival costs on top of the travel: landing or docking and
/// leaving again. One value for every place until the session log can
/// measure them; sc-travel takes the drive's cooldown when that is longer.
const ARRIVAL_SECONDS: f64 = 120.0;

/// Orders Plans' trips on holotable's travel graph with one ship's drive:
/// the fewest seconds, crossing systems through every jump link (none is
/// left out yet; a temporary link becomes a setting when one closes).
struct TravelRouter<'a> {
    travel: &'a svc_data::ships::Travel,
    drive: &'a svc_data::ships::Quantum,
}

impl Router for TravelRouter<'_> {
    fn order(&self, start: Option<&str>, stops: &[StopSpec]) -> Option<Ordered> {
        use svc_data::ships::Tour;
        let guid = |id: &str| id.parse::<svc_data::Guid>().ok();
        let mut tour = match start.and_then(guid) {
            Some(g) => Tour::new(g),
            None => Tour::anywhere(),
        };
        tour.overhead(ARRIVAL_SECONDS).end_anywhere();
        let mut ids = Vec::with_capacity(stops.len());
        for s in stops {
            let places: Vec<svc_data::Guid> = s.sites.iter().filter_map(|id| guid(id)).collect();
            if places.is_empty() {
                return None;
            }
            ids.push(tour.visit_any(&places).dwell(s.dwell).id());
        }
        for (i, s) in stops.iter().enumerate() {
            for &before in &s.after {
                tour.require(*ids.get(before)?, ids[i]);
            }
        }
        let plan = self.travel.order(&tour, self.drive, |_| true)?;
        let index = |id: svc_data::ships::StopId| ids.iter().position(|x| *x == id);
        let visits = plan
            .visits
            .iter()
            .map(|v| {
                let hops = &v.route.hops;
                Some(OrderedVisit {
                    stop: index(v.stop)?,
                    site: v.place.to_string(),
                    at: v.at,
                    leg: (!hops.is_empty()).then(|| TripLeg {
                        gm: v.route.metres() / 1e9,
                        seconds: Some(v.route.seconds()),
                        crosses: hops
                            .iter()
                            .any(|h| matches!(h.kind, svc_data::ships::HopKind::Tunnel)),
                    }),
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Ordered {
            visits,
            estimate: plan.is_estimate(),
        })
    }
}

struct Gathered<'a> {
    cooked: &'a Arc<svc_data::CookedData>,
    blueprints: Option<&'a svc_data::BlueprintIndex>,
    inventory: Option<&'a svc_dossier::inventory::Inventory>,
    owned_blueprints: HashSet<String>,
    hand_owned: HashSet<String>,
    prices: svc_uex::Snapshot,
    prices_at: Option<i64>,
    last_place: Option<String>,
}

/// The quantum-travel places, and how names and locations resolve to them.
struct Sites {
    sites: Vec<Site>,
    ids: HashSet<String>,
    by_name: HashMap<String, String>,
}

impl Sites {
    fn new(cooked: &svc_data::CookedData) -> Self {
        let sites: Vec<Site> = mod_ships::places(cooked)
            .places
            .into_iter()
            .map(|p| Site {
                id: p.guid,
                name: p.name,
                system: p.system,
                position: p.position,
                arrival: p.arrival,
            })
            .collect();
        let ids = sites.iter().map(|s| s.id.clone()).collect();
        let mut by_name = HashMap::new();
        for s in &sites {
            // The first of a name wins: the list is sorted by system.
            by_name
                .entry(svc_uex::norm(&s.name))
                .or_insert_with(|| s.id.clone());
        }
        // Shown with its system when another system has a place of the
        // same name: each gateway station is in two systems ("Pyro Gateway
        // (Stanton)", "Pyro Gateway (Nyx)"). Lookups above keep the bare
        // name.
        let shared: Vec<bool> = sites
            .iter()
            .map(|s| {
                sites
                    .iter()
                    .any(|o| o.name == s.name && o.system != s.system)
            })
            .collect();
        let mut sites = sites;
        for (s, shared) in sites.iter_mut().zip(shared) {
            if shared {
                s.name = format!("{} ({})", s.name, s.system);
            }
        }
        Self {
            sites,
            ids,
            by_name,
        }
    }

    /// The place itself when travel can go there, else the nearest parent
    /// it can go to.
    fn of_location(&self, cooked: &svc_data::CookedData, guid: &str) -> Option<String> {
        if self.ids.contains(guid) {
            return Some(guid.to_owned());
        }
        let parsed: svc_data::Guid = guid.parse().ok()?;
        cooked
            .holotable
            .locations
            .as_ref()?
            .ancestors(&parsed)
            .map(|l| l.guid.to_string())
            .find(|g| self.ids.contains(g))
    }

    fn place(&self, id: &str) -> Option<PlanPlace> {
        let site = self.sites.iter().find(|s| s.id == id)?;
        Some(PlanPlace {
            name: site.name.clone(),
            site: Some(site.id.clone()),
        })
    }

    fn system_of(&self, id: &str) -> Option<&str> {
        self.sites
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.system.as_str())
    }

    /// The first of `names` that is a place, most specific first.
    fn of_names<'n>(&self, names: impl IntoIterator<Item = &'n str>) -> Option<String> {
        names
            .into_iter()
            .filter(|n| !n.is_empty())
            .find_map(|n| self.by_name.get(&svc_uex::norm(n)).cloned())
    }

    fn of_place(&self, cooked: &svc_data::CookedData, place: &mod_items::Place) -> PlanPlace {
        let site = (!place.key.is_empty())
            .then(|| self.of_location(cooked, &place.key))
            .flatten()
            .or_else(|| {
                self.of_names(
                    std::iter::once(place.name.as_str())
                        .chain(place.parents.iter().map(String::as_str)),
                )
            });
        PlanPlace {
            name: place.name.clone(),
            site,
        }
    }
}

impl Gathered<'_> {
    fn board(&self, state: &PlansState) -> PlansBoardView {
        let cooked = self.cooked;
        let sites = Sites::new(cooked);
        let finder = self.inventory.map(|inv| mod_items::finder(inv, cooked));
        let mut inputs = Inputs::default();
        let mut targets: Vec<PlanTargetView> = Vec::new();
        let mut drive: Option<(String, svc_data::ships::Quantum)> = None;

        if let Some(finder) = &finder {
            for row in &finder.items {
                if row.ownership == mod_items::OwnershipState::Bricked {
                    continue;
                }
                if let Some(r) = &row.resource {
                    // As Crafting counts stock: SCU for cargo, pieces else.
                    let bulk = row.kind.item_type == "Cargo";
                    let stack = f64::from(row.stack);
                    inputs.stock.push(Stack {
                        material: r.name.clone(),
                        quality: u32::from(r.quality),
                        amount: if bulk { r.scu * stack } else { stack },
                        bulk,
                        place: row.place.name.clone(),
                    });
                    continue;
                }
                // A part is free to fit when it is not on a ship, not in
                // one, and yours to keep.
                let free = !row.fitted
                    && row.holders.is_empty()
                    && row.ownership != mod_items::OwnershipState::Rented;
                if free {
                    let at = sites.of_place(cooked, &row.place);
                    let held = inputs.parts.entry(row.kind.guid.clone()).or_default();
                    match held.iter_mut().find(|h| h.at == at) {
                        Some(h) => h.count += row.stack.max(1),
                        None => held.push(HeldPart {
                            at,
                            count: row.stack.max(1),
                        }),
                    }
                }
            }
        }

        let saved = crate::ships::saved_loadouts();
        let bands = cooked.quality_bands();
        for plan in &state.plans {
            let mut target = PlanTargetView {
                plan: plan.id.clone(),
                ..PlanTargetView::default()
            };
            match &plan.goal {
                Goal::Loadout(goal) => {
                    if let Some(saved) = saved.iter().find(|s| s.id == goal.loadout) {
                        let ship = mod_ships::ship_name(cooked, &saved.ship)
                            .unwrap_or_else(|| saved.ship.clone());
                        target.ship = Some(saved.ship.clone());
                        target.loadout = Some(saved.id.clone());
                        if drive.is_none() {
                            drive = self.drive_of(saved).map(|d| (ship.clone(), d));
                        }
                        // A loadout this build cannot resolve derives nothing:
                        // the plan says so instead of reading as done.
                        if let Some(facts) = self.loadout_facts(saved, ship, &sites) {
                            for bp in facts.parts.iter().filter_map(|p| p.blueprint.as_ref()) {
                                self.recipe_into(&mut inputs, bp, &bands, finder.as_ref(), &sites);
                            }
                            inputs.loadouts.insert(goal.loadout.clone(), facts);
                        }
                    }
                }
                Goal::Craft(goal) => {
                    target.family = self
                        .blueprints
                        .and_then(|i| i.get(&goal.blueprint))
                        .map(|b| b.family.clone());
                    self.recipe_into(
                        &mut inputs,
                        &goal.blueprint,
                        &bands,
                        finder.as_ref(),
                        &sites,
                    );
                }
                // Each craft of a project carries its own family
                // (`PlanCraft::family`): the project has no one recipe.
                Goal::Project(project) => {
                    for goal in &project.crafts {
                        self.recipe_into(
                            &mut inputs,
                            &goal.blueprint,
                            &bands,
                            finder.as_ref(),
                            &sites,
                        );
                    }
                }
                Goal::Unknown => {}
            }
            targets.push(target);
        }

        let start = self.last_place.as_deref().and_then(|internal| {
            let place = cooked.place_names().get(internal)?;
            let site = sites.of_names(
                std::iter::once(place.name.as_str())
                    .chain(place.parents.iter().map(String::as_str)),
            )?;
            Some((place.name, site))
        });
        let ores = self.ores(&sites, start.as_ref().map(|(_, site)| site.as_str()));
        let mut board = mod_plans::steps::derive(state, &inputs, &ores);
        let router = drive.as_ref().map(|(_, quantum)| TravelRouter {
            travel: &cooked.ships.travel,
            drive: quantum,
        });
        let tracks = mod_plans::session::tracks(
            &board.steps,
            &sites.sites,
            start.as_ref().map(|(_, site)| site.as_str()),
            router.as_ref().map(|r| r as &dyn Router),
        );
        // The run's ore goes to the refinery nearest the mining trip's
        // last stop, in that stop's system.
        let refineries = refinery_ids(cooked);
        let near = |site: &str| {
            mod_plans::session::nearest(&sites.sites, site, refineries.iter().map(String::as_str))
                .and_then(|id| sites.place(id))
        };
        mod_plans::session::refine_after_mining(&mut board.steps, &tracks, &near);
        // The steps list follows the tracks and their trips' order.
        mod_plans::session::sort_steps(&mut board.steps, &tracks);

        PlansBoardView {
            plans: board.plans,
            targets,
            steps: board.steps,
            ledger: board.ledger,
            tracks,
            from: start.as_ref().map(|(name, _)| name.clone()),
            drive_ship: drive.map(|(ship, _)| ship),
            inventory_at: self.inventory.map(|i| i.fetched_at as f64),
            prices_at: self.prices_at.map(|t| t as f64),
        }
    }

    /// The parts a saved loadout swaps in, with what is fitted on the copy
    /// of the ship that carries most of them, and where each can be had.
    fn loadout_facts(
        &self,
        saved: &mod_ships::SavedLoadout,
        ship: String,
        sites: &Sites,
    ) -> Option<LoadoutFacts> {
        let cooked = self.cooked;
        let resolve = |edits: &[mod_ships::Edit]| {
            mod_ships::loadout(
                cooked,
                &mod_ships::LoadoutInput {
                    ship: saved.ship.clone(),
                    edits: edits.to_vec(),
                },
            )
            .ok()
        };
        let mut facts = LoadoutFacts {
            name: saved.name.clone(),
            ship,
            read: ShipRead::NoRead,
            parts: Vec::new(),
        };
        let wanted = resolve(&saved.edits)?;
        let stock = resolve(&[])?;

        // The swaps that hold in the end: the last edit of a port, when the
        // resolved loadout still carries its part there (an edit under a
        // part that was swapped later is gone) and it is not what stock has.
        let mut paths: Vec<&Vec<String>> = Vec::new();
        for edit in saved.edits.iter().rev() {
            if !paths.contains(&&edit.path) {
                paths.push(&edit.path);
            }
        }
        paths.reverse();
        let item_at = |loadout: &svc_data::ships::Loadout, path: &[String]| {
            let path: Vec<&str> = path.iter().map(String::as_str).collect();
            loadout
                .at(&path)
                .and_then(|m| m.item)
                .map(|g| g.to_string())
        };
        let wants: Vec<(&Vec<String>, String)> = paths
            .into_iter()
            .filter_map(|path| {
                let item = item_at(&wanted, path)?;
                (item_at(&stock, path).as_ref() != Some(&item)).then_some((path, item))
            })
            .collect();

        // What is fitted: the copy whose ports match most.
        let copies = self
            .inventory
            .and_then(|inv| mod_ships::owned_loadouts(cooked, inv, &saved.ship).ok())
            .unwrap_or_default();
        let live = copies
            .iter()
            .filter_map(|c| c.live.as_ref().ok())
            .filter_map(|edits| resolve(edits))
            .max_by_key(|l| {
                wants
                    .iter()
                    .filter(|(path, item)| item_at(l, path).as_ref() == Some(item))
                    .count()
            });
        facts.read = match (&live, copies.is_empty()) {
            (Some(_), _) => ShipRead::Fitted,
            (None, false) => ShipRead::NoRead,
            // No inventory read at all says nothing about owning the ship.
            (None, true) if self.inventory.is_none() => ShipRead::NoRead,
            (None, true) => ShipRead::NotOwned,
        };

        let index = svc_uex::Index::new(&self.prices);
        for (path, item) in wants {
            let name = cooked.item_name(&item).unwrap_or_else(|| item.clone());
            let offers = index
                .item(&item, &name)
                .buy
                .into_iter()
                .map(|o| Offer {
                    at: self.terminal_place(&o, sites),
                    shop: o.terminal,
                    price: o.price,
                })
                .collect();
            facts.parts.push(PartFacts {
                fitted: live
                    .as_ref()
                    .is_some_and(|l| item_at(l, path).as_ref() == Some(&item)),
                blueprint: self.blueprints.and_then(|i| {
                    i.blueprints
                        .iter()
                        .find(|b| b.entity_guid.as_deref() == Some(item.as_str()))
                        .map(|b| b.guid.clone())
                }),
                item,
                name,
                offers,
            });
        }
        Some(facts)
    }

    /// Where a UEX offer is: its city, station or outpost when the map
    /// knows it, else the moon or planet it is on. UEX words the place
    /// smallest first and ends on the system.
    fn terminal_place(&self, offer: &svc_uex::Offer, sites: &Sites) -> PlanPlace {
        let mut names: Vec<&str> = offer.place.split(" \u{b7} ").collect();
        if names.len() > 1 {
            names.pop();
        }
        names.retain(|n| !n.is_empty());
        PlanPlace {
            name: names
                .first()
                .map_or_else(|| offer.terminal.clone(), |n| (*n).to_owned()),
            site: sites.of_names(names.iter().copied()),
        }
    }

    /// A blueprint's recipe, with whether you own it, the missions that
    /// grant it, the qualities each material comes in and the crafted
    /// copies you hold.
    fn recipe_into(
        &self,
        inputs: &mut Inputs,
        blueprint: &str,
        bands: &svc_data::quality::QualityBands,
        finder: Option<&mod_items::Finder>,
        sites: &Sites,
    ) {
        if inputs.recipes.contains_key(blueprint) {
            return;
        }
        let Some(entry) = self.blueprints.and_then(|i| i.get(blueprint)) else {
            return;
        };
        // The stock quality: a material at it neither helps nor hurts.
        let Some(neutral) = self
            .blueprints
            .map(|i| i.default_quality.clamp(0, 1000) as u32)
        else {
            return;
        };
        let grants = entry
            .granted_by
            .iter()
            .map(|g| Grant {
                title: g.title.clone(),
                at: self
                    .cooked
                    .missions
                    .iter()
                    .find(|m| m.mission_id == g.mission_id)
                    .map(|m| {
                        m.locations
                            .iter()
                            .map(|region| {
                                let name = if region.name.is_empty() {
                                    &region.system
                                } else {
                                    &region.name
                                };
                                PlanPlace {
                                    name: name.clone(),
                                    site: sites.of_names([region.name.as_str()]),
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            })
            .collect();
        // Crafted copies that are free to use (not on a ship, not in one,
        // yours to keep), with what went into each: whether a copy reaches
        // a craft's minimums is that craft's call, since the same blueprint
        // can be planned more than once at different ones.
        let crafted = match (&entry.entity_guid, finder) {
            (Some(entity), Some(finder)) => finder
                .items
                .iter()
                .filter(|row| {
                    row.kind.guid == *entity
                        && !row.fitted
                        && row.holders.is_empty()
                        && row.ownership == mod_items::OwnershipState::Owned
                })
                .filter_map(|row| {
                    let made = row.crafted.as_ref()?;
                    Some(CraftedCopy {
                        count: row.stack.max(1),
                        ingredients: made
                            .ingredients
                            .iter()
                            .map(|i| (i.name.clone().unwrap_or_default(), u32::from(i.quality)))
                            .collect(),
                    })
                })
                .collect(),
            _ => Vec::new(),
        };
        inputs.recipes.insert(
            blueprint.to_owned(),
            RecipeFacts {
                name: entry.name.clone(),
                entity: entry.entity_guid.clone(),
                owned: self.owned_blueprints.contains(blueprint)
                    || self.hand_owned.contains(blueprint),
                craft_seconds: entry.craft_seconds,
                slots: entry
                    .slots
                    .iter()
                    .map(|s| SlotNeed {
                        name: s.name.clone(),
                        moves: s.modifiers.iter().filter_map(|m| m.name.clone()).collect(),
                        bands: bands
                            .of(&s.material)
                            .iter()
                            .map(|b| QualityBand {
                                from: b.start,
                                to: b.end,
                                value: b.value,
                            })
                            .collect(),
                        material: s.material.clone(),
                        amount: s
                            .scu
                            .map(f64::from)
                            .or(s.count.map(f64::from))
                            .unwrap_or(0.0),
                        bulk: s.scu.is_some(),
                        min_quality: s.min_quality.clamp(0, 1000) as u32,
                        aim: if s.modifiers.is_empty() {
                            s.min_quality.clamp(0, 1000) as u32
                        } else {
                            neutral.max(s.min_quality.clamp(0, 1000) as u32)
                        },
                    })
                    .collect(),
                grants,
                crafted,
                family: Some(entry.family.clone()),
            },
        );
    }

    /// The first loadout plan's quantum drive, its crafted parts as crafted:
    /// what the trips' legs are costed with.
    fn drive_of(&self, saved: &mod_ships::SavedLoadout) -> Option<svc_data::ships::Quantum> {
        let recipes = svc_data::Recipes::new(self.blueprints, &self.cooked.crafting);
        mod_ships::quantum_drive(
            self.cooked,
            &recipes,
            &mod_ships::LoadoutInput {
                ship: saved.ship.clone(),
                edits: saved.edits.clone(),
            },
        )
    }

    /// Where a material is ship-mined at a quality: its ore's areas from
    /// the mining tables, with the game's own quality odds per place. A
    /// recipe names the refined material, the tables the ore, so both names
    /// lead to the element; asked by the refined name, the answer carries
    /// the ore's name and the refinery nearest to each place.
    ///
    /// `start` is the site of the last known place.
    fn ores<'s>(
        &'s self,
        sites: &'s Sites,
        start: Option<&'s str>,
    ) -> impl Fn(&str, u32) -> OreSites + 's {
        let cooked = self.cooked;
        let system = start.and_then(|site| sites.system_of(site));
        let refineries = refinery_ids(cooked);
        let mining = crate::mining::catalog_of(cooked)
            .ok()
            .zip(crate::mining::sources(cooked).ok());
        // Material name → (element, the ore's name when the material is
        // its refined version).
        let mut elements: HashMap<String, (String, Option<String>)> = HashMap::new();
        if let (Some((catalog, _)), Some(resources)) =
            (&mining, cooked.holotable.resources.as_ref())
        {
            for named in &catalog.element_names {
                let Some(raw) = catalog.elements.get(&named.value).and_then(|e| e.resource) else {
                    continue;
                };
                let ore = cooked.resource_name(&raw.to_string());
                // The ore's own name first: it is never "its refined version".
                if let Some(name) = &ore {
                    elements
                        .entry(name.to_lowercase())
                        .or_insert_with(|| (named.id.clone(), None));
                }
                let refined = resources
                    .refined_version_of(&raw)
                    .and_then(|r| cooked.resource_name(&r.guid.to_string()));
                if let Some(name) = refined {
                    elements
                        .entry(name.to_lowercase())
                        .or_insert_with(|| (named.id.clone(), ore.clone()));
                }
            }
        }
        move |material: &str, quality: u32| {
            let (Some((catalog, src)), Some((element, ore))) =
                (&mining, elements.get(&material.to_lowercase()))
            else {
                return OreSites::default();
            };
            let found = |q: u32| mod_mining::ore_sites(src, catalog, element, q.min(1000) as u16);
            let at_quality = found(quality);
            let mined = !at_quality.is_empty() || (quality > 0 && !found(0).is_empty());
            // The refinery nearest to a place; to you when the place is
            // unknown or its system has none.
            // The refinery nearest to a place on the map, in its system: the
            // area its ore is mined and refined in. None for a place off
            // the map (a ring without a marker): where you stand says
            // nothing about where the ring is.
            let refine_at = |site: Option<&str>| {
                site.and_then(|from| {
                    mod_plans::session::nearest(
                        &sites.sites,
                        from,
                        refineries.iter().map(String::as_str),
                    )
                })
                .and_then(|id| sites.place(id))
            };
            let mut found: Vec<OreSite> = at_quality
                .into_iter()
                .map(|s| {
                    // A gas pocket or a claim is flown to by its Lagrange
                    // point, and named by it: the pocket's own name
                    // ("ARC L3-A") is nowhere on the star map.
                    let reach: Vec<(String, &mod_mining::Place)> = s
                        .travel
                        .iter()
                        .filter_map(|p| sites.of_location(cooked, &p.id).map(|site| (site, p)))
                        .collect();
                    // An area at several places (one table at ARC L3, CRU L5
                    // and MIC L4): the one nearest to you when you are in
                    // its system, else the first.
                    let at = start
                        .and_then(|from| {
                            mod_plans::session::nearest(
                                &sites.sites,
                                from,
                                reach.iter().map(|(site, _)| site.as_str()),
                            )
                        })
                        .and_then(|id| reach.iter().find(|(site, _)| site == id))
                        .or(reach.first());
                    // Named by where you fly: the point for a pocket, the
                    // chosen place for an area at several, else the area.
                    let name = at
                        .filter(|(_, p)| {
                            s.travel.len() > 1 || !s.places.iter().any(|q| q.id == p.id)
                        })
                        .map(|(_, p)| p.name.clone());
                    let site = at.map(|(site, _)| site.clone());
                    OreSite {
                        refine_at: refine_at(site.as_deref()),
                        at: PlanPlace {
                            site,
                            name: name.unwrap_or(s.name),
                        },
                        find: s.find,
                    }
                })
                .collect();
            // Best first, but a place in the system you are in before one
            // a jump point away (the sort is stable).
            if let Some(system) = system {
                found.sort_by_key(|o| {
                    o.at.site.as_deref().and_then(|id| sites.system_of(id)) != Some(system)
                });
            }
            OreSites {
                mined,
                sites: found,
                ore: ore.clone(),
                refineries: refineries.len() as u32,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn craft(blueprint: &str) -> Goal {
        Goal::Craft(CraftGoal::uniform(blueprint, 1, 0))
    }

    #[test]
    fn a_plans_file_from_before_projects_loads_and_takes_the_new_fields() {
        // The dev plans file as the page before this change wrote it.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plans.json");
        std::fs::write(
            &path,
            br#"{
  "plans": [
    {"id": "p2", "name": "", "goal": {"kind": "craft", "blueprint": "bp-a", "count": 2, "quality": 700}},
    {"id": "p3", "name": "", "goal": {"kind": "loadout", "loadout": "lo-1"}}
  ],
  "next_id": 4
}"#,
        )
        .unwrap();
        let plans = change_at(&path, |s| {
            let kit = s.add_project("Kit");
            s.add_to_project(&kit, CraftGoal::with_slots("bp-a", 1, 0, &[500, 750, 750]));
            s.set_parked("p3", true);
        })
        .unwrap();
        assert_eq!(plans.len(), 3);
        assert_eq!(plans[2].id, "p5");
        // The old craft plan still asks Q700 of every slot.
        let Goal::Craft(old) = &plans[0].goal else {
            panic!("a craft plan");
        };
        assert_eq!((old.count, old.min_of(0), old.min_of(2)), (2, 700, 700));
        let again = read_state_at(&path).unwrap();
        assert_eq!(again.plans, plans);
        assert!(again.plans[1].parked);
    }

    #[test]
    fn a_missing_file_is_an_empty_start_and_the_first_change_creates_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plans").join("plans.json");
        assert_eq!(read_state_at(&path).unwrap(), PlansState::default());
        let plans = change_at(&path, |s| {
            s.add("", craft("bp-a"));
        })
        .unwrap();
        assert_eq!(plans.len(), 1);
        assert_eq!(read_state_at(&path).unwrap().plans, plans);
    }

    #[test]
    fn a_file_that_does_not_parse_is_an_error_and_is_left_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plans.json");
        let kept = br#"{"plans":[{"id":"p1","goal":{"kind":"craft","blueprint":"bp-a"}}"#;
        std::fs::write(&path, kept).unwrap();
        let result = change_at(&path, |s| {
            s.add("", craft("bp-b"));
        });
        assert!(result.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), kept);
    }

    #[test]
    fn a_file_that_cannot_be_read_is_an_error_and_nothing_is_written() {
        // A directory where the file should be: the read fails with
        // something other than "not found".
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plans.json");
        std::fs::create_dir(&path).unwrap();
        let result = change_at(&path, |s| {
            s.add("", craft("bp-a"));
        });
        assert!(result.is_err());
        assert!(path.is_dir());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
