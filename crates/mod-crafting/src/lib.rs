//! Crafting module: your side of the blueprint catalog. The catalog itself
//! is reference data (svc-data's `BlueprintIndex`, CONTRIBUTING module
//! rule 4); this crate holds what you marked. The shell persists it and
//! does the I/O (module rule 1).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// What can be marked on a blueprint, each independently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    /// Owned, said by hand: for anyone not reading their owned set from the
    /// game, and for a blueprint learned since the last read.
    Owned,
    /// You want the blueprint.
    WantBlueprint,
    /// You want a crafted copy of the item.
    WantItem,
}

/// Blueprint GUIDs by what you marked them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CraftingState {
    pub owned: BTreeSet<String>,
    pub want_blueprint: BTreeSet<String>,
    pub want_item: BTreeSet<String>,
}

impl CraftingState {
    /// Flip `mark` on `blueprint`. Owning a blueprint settles wanting it.
    pub fn toggle(&mut self, mark: Mark, blueprint: &str) {
        let set = match mark {
            Mark::Owned => &mut self.owned,
            Mark::WantBlueprint => &mut self.want_blueprint,
            Mark::WantItem => &mut self.want_item,
        };
        if !set.remove(blueprint) {
            set.insert(blueprint.to_string());
            if mark == Mark::Owned {
                self.want_blueprint.remove(blueprint);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mark_flips() {
        let mut s = CraftingState::default();
        s.toggle(Mark::WantItem, "a");
        assert!(s.want_item.contains("a"));
        s.toggle(Mark::WantItem, "a");
        assert!(s.want_item.is_empty());
    }

    #[test]
    fn owning_a_blueprint_settles_wanting_it_but_not_the_item() {
        let mut s = CraftingState::default();
        s.toggle(Mark::WantBlueprint, "a");
        s.toggle(Mark::WantItem, "a");
        s.toggle(Mark::Owned, "a");
        assert!(s.want_blueprint.is_empty());
        assert!(s.want_item.contains("a"));
    }

    #[test]
    fn unmarking_owned_brings_no_want_back() {
        let mut s = CraftingState::default();
        s.toggle(Mark::Owned, "a");
        s.toggle(Mark::Owned, "a");
        assert_eq!(s, CraftingState::default());
    }
}
