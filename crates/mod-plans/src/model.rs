//! What is persisted: the plans, in the order they are served.

use serde::{Deserialize, Serialize};

/// Every plan, first one first: the order is the priority. The first plan is
/// the focus, and the ledger reserves stock in this order.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlansState {
    pub plans: Vec<Plan>,
    /// Feeds the next plan id; never reused, so a removed plan's id stays
    /// gone.
    pub next_id: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default, rename_all = "camelCase")]
pub struct Plan {
    pub id: String,
    /// The user's name for it; empty means "name it after the goal".
    pub name: String,
    pub goal: Goal,
}

/// What a plan is for. New kinds (an armour set, a blueprint collection) are
/// new variants; a file written by a newer build keeps loading, its unknown
/// goals read as [`Goal::Unknown`] and derive no steps.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Goal {
    /// A saved loadout to have fitted on its ship.
    Loadout(LoadoutGoal),
    /// A number of one item to have crafted, at a quality.
    Craft(CraftGoal),
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
    /// The quality every material has to reach, 0..=1000. A slot's own
    /// minimum still holds when it is higher.
    pub quality: u32,
}

impl Default for CraftGoal {
    fn default() -> Self {
        Self {
            blueprint: String::new(),
            count: 1,
            quality: 0,
        }
    }
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
        });
        id
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

    /// Change a craft project's count and quality. Other kinds are left
    /// alone.
    pub fn set_craft(&mut self, id: &str, count: u32, quality: u32) {
        if let Some(Plan {
            goal: Goal::Craft(craft),
            ..
        }) = self.plans.iter_mut().find(|p| p.id == id)
        {
            craft.count = count.max(1);
            craft.quality = quality.min(1000);
        }
    }

    pub fn get(&self, id: &str) -> Option<&Plan> {
        self.plans.iter().find(|p| p.id == id)
    }
}

/// Whether two goals aim at the same thing (the same loadout, the same
/// blueprint), whatever their count or quality.
fn same_target(a: &Goal, b: &Goal) -> bool {
    match (a, b) {
        (Goal::Loadout(a), Goal::Loadout(b)) => a.loadout == b.loadout,
        (Goal::Craft(a), Goal::Craft(b)) => a.blueprint == b.blueprint,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn craft(blueprint: &str) -> Goal {
        Goal::Craft(CraftGoal {
            blueprint: blueprint.into(),
            count: 2,
            quality: 750,
        })
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
        assert_eq!(
            state
                .plans
                .iter()
                .map(|p| p.id.as_str())
                .collect::<Vec<_>>(),
            [a, b]
        );
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
        assert_eq!(
            state.plans.iter().map(|p| p.id.clone()).collect::<Vec<_>>(),
            [c, a, b]
        );
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
                quality: 1000
            }))
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
                quality: 0
            })
        );
        assert_eq!(state.plans[1].name, "Kit");
        assert_eq!(state.plans[2].goal, Goal::Unknown);
        assert_eq!(state.plans[3].goal, Goal::Unknown);
    }
}
