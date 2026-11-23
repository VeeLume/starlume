//! "What if this part were crafted": a loadout edit can mark the part it
//! fits as crafted, at one quality for every material (`Edit::quality`)
//! or one per material slot (`Edit::qualities`), and the page's figures
//! take what the recipe changes.
//!
//! The holotable sheets are keyed by item class, and two ports often hold
//! the same class with only one crafted, so a crafted part is applied per
//! port path, after the sheets are read: its power plant's segments and
//! its cooler's coolant before the network settles, its shield generator
//! before the shields combine, its drive's speed and fuel, its radar's
//! aim-assist distances and its gun's damage. What each crafting property
//! moves (svc-data's [`GameplayStat`], by record name):
//!
//! | Property | Moves |
//! |---|---|
//! | `PowerGeneration` (Power Pips, added) | the plant's power segments |
//! | `CoolantGeneration` (Coolant Rating) | the cooler's coolant |
//! | `ShieldMaxHealth` | the generator's capacity |
//! | `QuantumSpeed` | the drive's speed |
//! | `QuantumFuelRequirement` (Quantum Fuel Burn) | fuel per distance |
//! | `RadarMinAimAssist` / `RadarMaxAimAssist` | the aim-assist distances |
//! | `WeaponDamage` (Impact Force) | the gun's damage, so its DPS |
//! | `Integrity` | no figure on the page: shown on the part only |
//!
//! Quantum Fuel Burn is the one inferred pairing: the recipe's base row is
//! the drive's legacy `quantumFuelRequirement`, which the client no longer
//! reads; the page's fuel figure is the drive's travelling consumption,
//! and the factor is laid on that.

use std::collections::HashMap;

use svc_data::crafting::GameplayStat;
use svc_data::ships::Loadout;
use svc_data::{Change, Crafted, Guid, Recipes};

use crate::view::LoadoutInput;

/// The crafted marks in effect, by `/`-joined port path: the last edit of
/// a port carries a quality, and the port still holds what it fitted (a
/// later swap above it may have taken the part away).
#[derive(Debug, Default)]
pub(crate) struct CraftedParts {
    by_path: HashMap<String, Crafted>,
}

impl CraftedParts {
    pub(crate) fn resolve(recipes: &Recipes, input: &LoadoutInput, loadout: &Loadout) -> Self {
        let mut last = HashMap::new();
        for e in &input.edits {
            last.insert(e.path.join("/"), e);
        }
        let by_path = last
            .into_iter()
            .filter_map(|(path, e)| {
                let quality = e.quality?;
                let item = e.item.as_deref()?;
                let guid: Guid = item.parse().ok()?;
                let refs: Vec<&str> = e.path.iter().map(String::as_str).collect();
                if loadout.at(&refs).and_then(|m| m.item) != Some(guid) {
                    return None;
                }
                let at: Vec<i32> = if e.qualities.is_empty() {
                    vec![quality]
                } else {
                    e.qualities.clone()
                };
                Some((path, recipes.crafted_at(&guid.to_string(), &at)?))
            })
            .collect();
        Self { by_path }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.by_path.is_empty()
    }

    pub(crate) fn get(&self, path: &str) -> Option<&Crafted> {
        self.by_path.get(path)
    }

    /// What the part at `path` does to `stat`; none when it isn't crafted.
    pub(crate) fn change(&self, path: &str, stat: &GameplayStat) -> Change {
        self.by_path.get(path).map_or(Change::NONE, |c| c.of(stat))
    }

    /// `value` of the part at `path`, crafted.
    pub(crate) fn apply(&self, path: &str, stat: &GameplayStat, value: f32) -> f32 {
        self.change(path, stat).apply(f64::from(value)) as f32
    }
}
