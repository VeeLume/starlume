//! What is persisted: the plans, in the order they are served.

use serde::{Deserialize, Serialize};

/// Every plan, first one first: the order is the priority. The first plan is
/// the focus, and the ledger reserves stock in this order.
///
/// Read and written through its own `Serialize` / `Deserialize`: a goal of a
/// kind this build does not know is kept as it was read and written back
/// unchanged, so a file can go to an older build and return.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlansState {
    pub plans: Vec<Plan>,
    /// Feeds the next plan id; never reused, so a removed plan's id stays
    /// gone.
    pub next_id: u64,
    /// Where a part comes from when it has a choice: the Components panel's
    /// Prefer.
    pub prefer: PartSource,
    /// A part's own source (item GUID → source), over [`Self::prefer`].
    pub sources: std::collections::BTreeMap<String, PartSource>,
}

/// Where a part comes from (decision 'Plans pick one source per need with a
/// switch', 2026-11-16).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum PartSource {
    /// An in-game shop (UEX's prices and places).
    #[default]
    Store,
    /// Another player, on the UEX marketplace.
    Market,
    /// Crafted from its blueprint.
    Craft,
    /// Looted from a container placed in the world (sc-loot's simulated
    /// drops).
    Loot,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default, rename_all = "camelCase")]
pub struct Plan {
    pub id: String,
    /// The user's name for it; empty means "name it after the goal".
    pub name: String,
    pub goal: Goal,
    /// Set aside: shown, but it reserves nothing and derives no steps.
    pub parked: bool,
    /// The user's own words about it.
    pub notes: String,
    /// The goal as the file had it, when its kind is one this build does
    /// not know: written back in place of [`Goal::Unknown`].
    #[serde(skip)]
    #[cfg_attr(feature = "specta", specta(skip))]
    pub kept: Option<serde_json::Value>,
}

/// What a plan is for. New kinds (an armour set, a blueprint collection) are
/// new variants. A file written by a newer build still loads: a goal of a
/// kind this build does not know reads as [`Goal::Unknown`] and derives no
/// steps; its fields ride along in [`Plan::kept`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Goal {
    /// A saved loadout to have fitted on its ship.
    Loadout(LoadoutGoal),
    /// A number of one item to have crafted, at a quality.
    Craft(CraftGoal),
    /// A named group of crafts, served together at the plan's place in the
    /// order.
    Project(ProjectGoal),
    #[default]
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default, rename_all = "camelCase")]
pub struct LoadoutGoal {
    /// The saved loadout's id in the Ships module.
    pub loadout: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default, rename_all = "camelCase")]
pub struct CraftGoal {
    /// The blueprint's record GUID.
    pub blueprint: String,
    pub count: u32,
    /// The quality every material has to reach, 0..=1000, where `slots`
    /// says nothing; with `slots` set it is their highest. A slot's own
    /// minimum still holds when it is higher.
    pub quality: u32,
    /// The minimum quality of each material slot, in the recipe's slot
    /// order. Empty: every slot at `quality` (how files from before
    /// per-slot minimums read). A slot past the end is at `quality` too.
    pub slots: Vec<u32>,
}

impl Default for CraftGoal {
    fn default() -> Self {
        Self {
            blueprint: String::new(),
            count: 1,
            quality: 0,
            slots: Vec::new(),
        }
    }
}

impl CraftGoal {
    /// A craft with one quality for every material.
    pub fn uniform(blueprint: impl Into<String>, count: u32, quality: u32) -> Self {
        let mut goal = Self {
            blueprint: blueprint.into(),
            ..Self::default()
        };
        goal.set(count, quality, &[]);
        goal
    }

    /// A craft with a minimum per slot; no slots means `quality` for all.
    pub fn with_slots(
        blueprint: impl Into<String>,
        count: u32,
        quality: u32,
        slots: &[u32],
    ) -> Self {
        let mut goal = Self {
            blueprint: blueprint.into(),
            ..Self::default()
        };
        goal.set(count, quality, slots);
        goal
    }

    /// The minimum the plan sets for slot `index`.
    pub fn min_of(&self, index: usize) -> u32 {
        self.slots.get(index).copied().unwrap_or(self.quality)
    }

    /// Whether every slot is at one quality.
    pub fn is_uniform(&self) -> bool {
        self.slots.iter().all(|q| *q == self.quality)
    }

    /// Take a new count and minimums. With `slots` given they are the
    /// per-slot minimums and `quality` becomes their highest; without, every
    /// slot is at `quality`.
    pub fn set(&mut self, count: u32, quality: u32, slots: &[u32]) {
        self.count = count.max(1);
        self.slots = slots.iter().map(|q| (*q).min(1000)).collect();
        self.quality = match self.slots.iter().max() {
            Some(top) => *top,
            None => quality.min(1000),
        };
        if self.is_uniform() {
            self.slots.clear();
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default, rename_all = "camelCase")]
pub struct ProjectGoal {
    /// In the order they are served; one entry a blueprint.
    pub crafts: Vec<CraftGoal>,
}

impl PlansState {
    /// Append a plan and return its id. A goal that is already planned is
    /// not added twice: its existing id comes back.
    pub fn add(&mut self, name: &str, goal: Goal) -> String {
        if let Some(existing) = self.plans.iter().find(|p| same_target(&p.goal, &goal)) {
            return existing.id.clone();
        }
        self.next_id += 1;
        let id = format!("p{}", self.next_id);
        self.plans.push(Plan {
            id: id.clone(),
            name: name.trim().to_owned(),
            goal,
            ..Plan::default()
        });
        id
    }

    /// Start an empty project and return its id.
    pub fn add_project(&mut self, name: &str) -> String {
        self.add(name, Goal::Project(ProjectGoal::default()))
    }

    pub fn remove(&mut self, id: &str) {
        self.plans.retain(|p| p.id != id);
    }

    /// Make a plan the focus: it moves to the front, the rest keep their
    /// order.
    pub fn focus(&mut self, id: &str) {
        if let Some(at) = self.plans.iter().position(|p| p.id == id) {
            let plan = self.plans.remove(at);
            self.plans.insert(0, plan);
        }
    }

    /// Move a plan to just before another (`None`: to the end). The order
    /// is the priority, so this is also who is served first.
    pub fn move_before(&mut self, id: &str, before: Option<&str>) {
        if before == Some(id) {
            return;
        }
        let Some(at) = self.plans.iter().position(|p| p.id == id) else {
            return;
        };
        let plan = self.plans.remove(at);
        let to = before
            .and_then(|b| self.plans.iter().position(|p| p.id == b))
            .unwrap_or(self.plans.len());
        self.plans.insert(to, plan);
    }

    pub fn rename(&mut self, id: &str, name: &str) {
        if let Some(plan) = self.plans.iter_mut().find(|p| p.id == id) {
            plan.name = name.trim().to_owned();
        }
    }

    pub fn set_notes(&mut self, id: &str, notes: &str) {
        if let Some(plan) = self.plans.iter_mut().find(|p| p.id == id) {
            plan.notes = notes.trim().to_owned();
        }
    }

    /// Park a plan, or take it back up. A parked plan keeps its place.
    /// Where parts come from when they have a choice; a part's own source
    /// gives way to it.
    pub fn set_prefer(&mut self, source: PartSource) {
        self.prefer = source;
        self.sources.clear();
    }

    /// A part's own source (`None`: follow [`Self::prefer`]).
    pub fn set_source(&mut self, item: &str, source: Option<PartSource>) {
        match source {
            Some(s) => {
                self.sources.insert(item.to_owned(), s);
            }
            None => {
                self.sources.remove(item);
            }
        }
    }

    pub fn set_parked(&mut self, id: &str, parked: bool) {
        if let Some(plan) = self.plans.iter_mut().find(|p| p.id == id) {
            plan.parked = parked;
        }
    }

    /// Change a craft project's count and quality: one quality for every
    /// material. Other kinds are left alone.
    pub fn set_craft(&mut self, id: &str, count: u32, quality: u32) {
        if let Some(Plan {
            goal: Goal::Craft(craft),
            ..
        }) = self.plans.iter_mut().find(|p| p.id == id)
        {
            craft.set(count, quality, &[]);
        }
    }

    /// Put a craft into a project. A blueprint the project already has is
    /// not added twice. Returns whether the project exists.
    pub fn add_to_project(&mut self, id: &str, craft: CraftGoal) -> bool {
        let Some(Plan {
            goal: Goal::Project(project),
            ..
        }) = self.plans.iter_mut().find(|p| p.id == id)
        else {
            return false;
        };
        if !project
            .crafts
            .iter()
            .any(|c| c.blueprint == craft.blueprint)
        {
            project.crafts.push(craft);
        }
        true
    }

    /// The craft of `blueprint` in plan `id`: the plan's own goal, or the
    /// entry of a project.
    pub fn craft_mut(&mut self, id: &str, blueprint: &str) -> Option<&mut CraftGoal> {
        match &mut self.plans.iter_mut().find(|p| p.id == id)?.goal {
            Goal::Craft(craft) if craft.blueprint == blueprint => Some(craft),
            Goal::Project(project) => project.crafts.iter_mut().find(|c| c.blueprint == blueprint),
            _ => None,
        }
    }

    /// Take a craft out of a project. The project stays, even when empty.
    pub fn remove_from_project(&mut self, id: &str, blueprint: &str) {
        if let Some(Plan {
            goal: Goal::Project(project),
            ..
        }) = self.plans.iter_mut().find(|p| p.id == id)
        {
            project.crafts.retain(|c| c.blueprint != blueprint);
        }
    }

    pub fn get(&self, id: &str) -> Option<&Plan> {
        self.plans.iter().find(|p| p.id == id)
    }
}

/// Whether two goals aim at the same thing (the same loadout, the same
/// blueprint on its own), whatever their count or quality. A project is
/// never the same as another: the same blueprint may sit in several.
fn same_target(a: &Goal, b: &Goal) -> bool {
    match (a, b) {
        (Goal::Loadout(a), Goal::Loadout(b)) => a.loadout == b.loadout,
        (Goal::Craft(a), Goal::Craft(b)) => a.blueprint == b.blueprint,
        _ => false,
    }
}

/// The file's shape: plans as they are, each goal still raw.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct FileState {
    plans: Vec<serde_json::Value>,
    next_id: u64,
    prefer: PartSource,
    sources: std::collections::BTreeMap<String, PartSource>,
}

impl<'de> Deserialize<'de> for PlansState {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let file = FileState::deserialize(deserializer)?;
        let plans = file
            .plans
            .into_iter()
            .map(|raw| {
                let mut plan: Plan =
                    serde_json::from_value(raw.clone()).map_err(serde::de::Error::custom)?;
                if plan.goal == Goal::Unknown {
                    plan.kept = raw.get("goal").filter(|g| g.is_object()).cloned();
                }
                Ok(plan)
            })
            .collect::<Result<_, D::Error>>()?;
        Ok(Self {
            plans,
            next_id: file.next_id,
            prefer: file.prefer,
            sources: file.sources,
        })
    }
}

impl Serialize for PlansState {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let plans = self
            .plans
            .iter()
            .map(|plan| {
                let mut raw = serde_json::to_value(plan).map_err(serde::ser::Error::custom)?;
                if let (Goal::Unknown, Some(kept)) = (&plan.goal, &plan.kept) {
                    raw["goal"] = kept.clone();
                }
                Ok(raw)
            })
            .collect::<Result<_, S::Error>>()?;
        FileState {
            plans,
            next_id: self.next_id,
            prefer: self.prefer,
            sources: self.sources.clone(),
        }
        .serialize(serializer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn craft(blueprint: &str) -> Goal {
        Goal::Craft(CraftGoal::uniform(blueprint, 2, 750))
    }

    fn ids(state: &PlansState) -> Vec<&str> {
        state.plans.iter().map(|p| p.id.as_str()).collect()
    }

    #[test]
    fn the_sources_survive_the_file_and_prefer_resets_the_picks() {
        let mut state = PlansState::default();
        state.set_source("item-a", Some(PartSource::Market));
        let json = serde_json::to_string(&state).unwrap();
        let back: PlansState = serde_json::from_str(&json).unwrap();
        assert_eq!(back.sources.get("item-a"), Some(&PartSource::Market));
        assert_eq!(back.prefer, PartSource::Store);
        // An older file without them reads as the defaults.
        let old: PlansState = serde_json::from_str(r#"{"plans":[],"next_id":3}"#).unwrap();
        assert_eq!(
            (old.prefer, old.sources.len(), old.next_id),
            (PartSource::Store, 0, 3)
        );
        state.set_prefer(PartSource::Craft);
        assert!(state.sources.is_empty());
    }

    #[test]
    fn plans_keep_the_order_they_were_added_in() {
        let mut state = PlansState::default();
        let a = state.add("", craft("bp-a"));
        let b = state.add(
            "",
            Goal::Loadout(LoadoutGoal {
                loadout: "lo-1".into(),
            }),
        );
        assert_eq!(ids(&state), [a, b]);
    }

    #[test]
    fn the_same_goal_is_not_planned_twice() {
        let mut state = PlansState::default();
        let first = state.add("", craft("bp-a"));
        let again = state.add("other name", craft("bp-a"));
        assert_eq!(first, again);
        assert_eq!(state.plans.len(), 1);
    }

    #[test]
    fn focus_moves_a_plan_to_the_front_and_keeps_the_rest() {
        let mut state = PlansState::default();
        let a = state.add("", craft("bp-a"));
        let b = state.add("", craft("bp-b"));
        let c = state.add("", craft("bp-c"));
        state.focus(&c);
        assert_eq!(ids(&state), [c, a, b]);
    }

    #[test]
    fn a_plan_moves_before_another_or_to_the_end() {
        let mut state = PlansState::default();
        let a = state.add("", craft("bp-a"));
        let b = state.add("", craft("bp-b"));
        let c = state.add("", craft("bp-c"));
        state.move_before(&c, Some(&b));
        assert_eq!(ids(&state), [&a, &c, &b]);
        state.move_before(&a, None);
        assert_eq!(ids(&state), [&c, &b, &a]);
        // Before itself, or a plan that is not there: nothing moves.
        state.move_before(&b, Some(&b));
        state.move_before("p99", Some(&c));
        assert_eq!(ids(&state), [&c, &b, &a]);
    }

    #[test]
    fn a_removed_plans_id_is_not_handed_out_again() {
        let mut state = PlansState::default();
        let a = state.add("", craft("bp-a"));
        state.remove(&a);
        let b = state.add("", craft("bp-b"));
        assert_ne!(a, b);
    }

    #[test]
    fn a_craft_project_takes_a_new_count_and_quality() {
        let mut state = PlansState::default();
        let a = state.add("", craft("bp-a"));
        state.set_craft(&a, 0, 4000);
        assert_eq!(
            state.get(&a).map(|p| &p.goal),
            Some(&Goal::Craft(CraftGoal {
                blueprint: "bp-a".into(),
                count: 1,
                quality: 1000,
                slots: vec![],
            }))
        );
    }

    #[test]
    fn slot_minimums_are_kept_per_slot_and_the_quality_is_their_highest() {
        let goal = CraftGoal::with_slots("bp-a", 1, 0, &[500, 750, 750]);
        assert_eq!(goal.slots, [500, 750, 750]);
        assert_eq!(goal.quality, 750);
        assert_eq!((goal.min_of(0), goal.min_of(1)), (500, 750));
        // A slot the plan does not list (the recipe grew one) is at the
        // highest, never below what was asked.
        assert_eq!(goal.min_of(3), 750);
        assert!(!goal.is_uniform());
    }

    #[test]
    fn slots_all_at_one_quality_are_stored_as_that_quality() {
        let goal = CraftGoal::with_slots("bp-a", 1, 0, &[600, 600]);
        assert_eq!((goal.quality, goal.slots.len()), (600, 0));
        assert_eq!(goal.min_of(1), 600);
    }

    #[test]
    fn the_single_quality_sets_every_slot() {
        let mut state = PlansState::default();
        let a = state.add(
            "",
            Goal::Craft(CraftGoal::with_slots("bp-a", 1, 0, &[500, 750])),
        );
        state.set_craft(&a, 3, 800);
        let Some(Goal::Craft(goal)) = state.get(&a).map(|p| &p.goal) else {
            panic!("a craft plan");
        };
        assert_eq!((goal.count, goal.min_of(0), goal.min_of(1)), (3, 800, 800));
    }

    #[test]
    fn a_project_holds_a_blueprint_once_and_two_projects_may_share_one() {
        let mut state = PlansState::default();
        let own = state.add("", craft("bp-a"));
        let one = state.add_project("Kit one");
        let two = state.add_project("Kit two");
        assert_ne!(one, two);
        assert!(state.add_to_project(&one, CraftGoal::uniform("bp-a", 1, 0)));
        assert!(state.add_to_project(&one, CraftGoal::uniform("bp-a", 5, 900)));
        assert!(state.add_to_project(&two, CraftGoal::uniform("bp-a", 2, 0)));
        assert!(!state.add_to_project(&own, CraftGoal::uniform("bp-b", 1, 0)));
        let crafts = |id: &str| match &state.get(id).unwrap().goal {
            Goal::Project(p) => p.crafts.clone(),
            _ => panic!("a project"),
        };
        assert_eq!(crafts(&one), [CraftGoal::uniform("bp-a", 1, 0)]);
        assert_eq!(crafts(&two), [CraftGoal::uniform("bp-a", 2, 0)]);
        assert_eq!(state.plans.len(), 3);
    }

    #[test]
    fn a_craft_is_reached_on_its_own_and_inside_a_project() {
        let mut state = PlansState::default();
        let own = state.add("", craft("bp-a"));
        let kit = state.add_project("Kit");
        state.add_to_project(&kit, CraftGoal::uniform("bp-a", 1, 0));
        state
            .craft_mut(&kit, "bp-a")
            .unwrap()
            .set(4, 0, &[500, 750]);
        assert!(state.craft_mut(&own, "bp-b").is_none());
        assert_eq!(state.craft_mut(&own, "bp-a").unwrap().count, 2);
        assert_eq!(state.craft_mut(&kit, "bp-a").unwrap().slots, [500, 750]);
        state.remove_from_project(&kit, "bp-a");
        assert!(state.craft_mut(&kit, "bp-a").is_none());
        assert!(state.get(&kit).is_some());
    }

    #[test]
    fn a_plan_is_renamed_parked_and_annotated() {
        let mut state = PlansState::default();
        let a = state.add_project("Kit");
        state.rename(&a, "  Mining kit ");
        state.set_notes(&a, "for the Prospector\n");
        state.set_parked(&a, true);
        let plan = state.get(&a).unwrap();
        assert_eq!(
            (plan.name.as_str(), plan.notes.as_str(), plan.parked),
            ("Mining kit", "for the Prospector", true)
        );
    }

    #[test]
    fn the_file_shape_is_stable_and_tolerant() {
        let json = r#"{"plans":[
            {"id":"p1","goal":{"kind":"craft","blueprint":"bp-a"}},
            {"id":"p2","name":"Kit","goal":{"kind":"loadout","loadout":"lo-1"}},
            {"id":"p3","goal":{"kind":"armourSet","pieces":4}},
            {"id":"p4"}
        ]}"#;
        let state: PlansState = serde_json::from_str(json).unwrap();
        assert_eq!(
            state.plans[0].goal,
            Goal::Craft(CraftGoal {
                blueprint: "bp-a".into(),
                count: 1,
                quality: 0,
                slots: vec![],
            })
        );
        assert_eq!(state.plans[1].name, "Kit");
        assert_eq!(state.plans[2].goal, Goal::Unknown);
        assert_eq!(state.plans[3].goal, Goal::Unknown);
    }

    #[test]
    fn a_file_from_before_slot_minimums_reads_as_every_slot_at_its_quality() {
        // What the Plans page wrote before this change, verbatim.
        let json = r#"{
          "plans": [
            {"id": "p1", "name": "", "goal": {"kind": "craft", "blueprint": "bp-a", "count": 2, "quality": 700}},
            {"id": "p3", "name": "", "goal": {"kind": "loadout", "loadout": "lo-1"}}
          ],
          "next_id": 4
        }"#;
        let state: PlansState = serde_json::from_str(json).unwrap();
        let Goal::Craft(goal) = &state.plans[0].goal else {
            panic!("a craft plan");
        };
        assert!(goal.slots.is_empty());
        assert_eq!((goal.min_of(0), goal.min_of(2)), (700, 700));
        assert!(!state.plans[0].parked);
        assert_eq!(state.plans[0].notes, "");
        assert_eq!(state.next_id, 4);
    }

    #[test]
    fn a_goal_of_an_unknown_kind_is_written_back_as_it_was_read() {
        let json = r#"{"plans":[
            {"id":"p1","name":"Set","goal":{"kind":"armourSet","pieces":4,"tint":"red"}},
            {"id":"p2","goal":{"kind":"craft","blueprint":"bp-a"}}
        ],"next_id":2}"#;
        let mut state: PlansState = serde_json::from_str(json).unwrap();
        // A change elsewhere, then a save: the unknown goal keeps its fields.
        state.rename("p2", "Mine");
        state.focus("p2");
        let written = serde_json::to_value(&state).unwrap();
        assert_eq!(
            written["plans"][1]["goal"],
            serde_json::json!({"kind": "armourSet", "pieces": 4, "tint": "red"})
        );
        assert_eq!(written["plans"][1]["name"], "Set");
        // And it reads back the same.
        let again: PlansState = serde_json::from_value(written).unwrap();
        assert_eq!(again, state);
    }

    #[test]
    fn what_is_written_reads_back_the_same() {
        let mut state = PlansState::default();
        let kit = state.add_project("Kit");
        state.add_to_project(&kit, CraftGoal::with_slots("bp-a", 2, 0, &[500, 750, 750]));
        state.set_parked(&kit, true);
        state.set_notes(&kit, "later");
        state.add("", craft("bp-b"));
        let text = serde_json::to_string(&state).unwrap();
        assert!(text.contains(r#""slots":[500,750,750]"#));
        assert!(!text.contains("kept"));
        assert_eq!(serde_json::from_str::<PlansState>(&text).unwrap(), state);
    }
}
