//! What the calculator offers: the ship ruleset and the lasers, modules,
//! gadgets and elements a ship miner can name, with in-game names.
//!
//! Built per query from the cooked [`MiningIndex`] and the build's locale
//! (cheap: a few hundred entries). The filters are the calculator's choice,
//! not data facts, which is why they live here and not in the cook:
//!
//! - test, template and MPUV records carry real display names and nothing
//!   typed marks them, so they go by record name;
//! - only entries with an in-game name are offered;
//! - size-0 lasers are FPS and ROC heads, on other rulesets;
//! - an active module without a duration is a base record or the ROC's;
//! - elements are the ones ship rocks spawn, with a density.

use serde::Serialize;
use svc_data::craft::eval_property;
use svc_data::crafting::GameplayStat;
use svc_data::mining::{
    Activation, Elements, MiningGadget, MiningLaserStats, MiningModule, Ruleset, Shape,
};
use svc_data::{BlueprintSlot, Change, Guid, LocaleMap, MiningIndex, Recipes};

/// The ship mining ruleset's record name.
pub const SHIP_RULESET: &str = "MiningGlobalParamsShip";

/// A pickable entry: an id, its in-game name and its record name.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Named<T> {
    /// Record GUID, hex-string form.
    pub id: String,
    pub name: String,
    pub record_name: String,
    #[serde(skip)]
    pub value: T,
}

/// The calculator's catalog for one build.
#[derive(Debug, Clone)]
pub struct Catalog {
    pub ruleset: Ruleset,
    pub elements: Elements,
    pub element_names: Vec<Named<Guid>>,
    /// Each element's localisation key (by id), to resolve its name in the
    /// localisation the game actually shows (the scan reader).
    pub element_keys: Vec<(String, Option<String>)>,
    pub lasers: Vec<Named<LaserItem>>,
    pub modules: Vec<Named<MiningModule>>,
    pub gadgets: Vec<Named<MiningGadget>>,
    /// The meshes ship rocks spawn with (asteroid_01..03, mining_rock_01),
    /// each with its fracture pieces: what an overcharged crack loses.
    /// Empty when the cook could not open the p4k.
    pub rock_shapes: Vec<Shape>,
}

/// A ship mining laser.
#[derive(Debug, Clone, PartialEq)]
pub struct LaserItem {
    pub size: i32,
    pub stats: MiningLaserStats,
    /// Its recipe, when a blueprint makes it ([`Catalog::with_recipes`]).
    pub craft: Option<LaserCraft>,
}

/// What crafting a laser changes. A mining laser's recipe moves two
/// properties: Integrity, which nothing here reads, and Impact Force
/// (`WeaponDamage`), whose sheet row is fire action 0's `damagePerSecond`:
/// the fracturing beam's energy, its power. That is what is laid on. The
/// extraction beam is left as it is: the recipe's sheet shows only the
/// first beam, and nothing says the game scales the other one too.
/// Mining modules' recipes change nothing at any quality (4.x).
#[derive(Debug, Clone, PartialEq)]
pub struct LaserCraft {
    /// The blueprint (`CraftingBlueprintRecord` GUID): what Plans takes.
    pub blueprint: String,
    slots: Vec<BlueprintSlot>,
    /// The recipe's `WeaponDamage` properties.
    power: Vec<String>,
}

impl LaserCraft {
    /// What crafting at `quality` does to the fracturing beam's power.
    pub fn power_at(&self, quality: i32) -> Change {
        let q = [f64::from(quality.clamp(0, svc_data::craft::MAX_QUALITY))];
        self.power
            .iter()
            .map(|p| eval_property(p, &self.slots, &q))
            .fold(Change::NONE, |acc, c| Change {
                factor: acc.factor + (c.factor - 1.0),
                added: acc.added + c.added,
            })
    }
}

impl LaserItem {
    /// Its stats as sold, or crafted at `quality` when a recipe makes it.
    pub fn stats_at(&self, quality: Option<i32>) -> MiningLaserStats {
        let mut stats = self.stats.clone();
        let (Some(q), Some(craft)) = (quality, &self.craft) else {
            return stats;
        };
        let fracture = stats
            .fracture_beam()
            .and_then(|f| stats.beams.iter().position(|b| std::ptr::eq(b, f)));
        if let Some(i) = fracture {
            let beam = &mut stats.beams[i];
            beam.energy_per_second =
                craft.power_at(q).apply(f64::from(beam.energy_per_second)) as f32;
        }
        stats
    }

    /// Its power (the fracturing beam's energy per second) at `quality`.
    pub fn power_at(&self, quality: Option<i32>) -> f32 {
        self.stats_at(quality)
            .fracture_beam()
            .map_or(0.0, |b| b.energy_per_second)
    }
}

fn is_junk(record_name: &str) -> bool {
    let r = record_name.to_ascii_lowercase();
    r.contains("test") || r.contains("template") || r.contains("mpuv")
}

fn name(locale: &LocaleMap, key: Option<&str>) -> Option<String> {
    key.and_then(|k| locale.resolve_name(k))
        .filter(|t| !t.starts_with('@'))
        .map(str::to_string)
}

fn by_name<T>(mut v: Vec<Named<T>>) -> Vec<Named<T>> {
    v.sort_by(|a, b| a.name.cmp(&b.name).then(a.record_name.cmp(&b.record_name)));
    v
}

impl Catalog {
    /// `None` when the build has no ship mining ruleset.
    pub fn build(index: &MiningIndex, locale: &LocaleMap) -> Option<Self> {
        let ruleset = index
            .rulesets
            .iter()
            .find(|r| r.record_name == SHIP_RULESET)?
            .ruleset
            .clone();

        let element_names: Vec<Named<Guid>> = index
            .element_names
            .iter()
            .filter(|e| e.rulesets.iter().any(|r| r == SHIP_RULESET))
            .filter(|e| {
                use svc_data::RecordCollection;
                index
                    .elements
                    .get(&e.guid)
                    .is_some_and(|el| el.density.is_some())
            })
            .map(|e| Named {
                id: e.guid.to_string(),
                name: name(locale, e.name_key.as_deref()).unwrap_or_else(|| e.record_name.clone()),
                record_name: e.record_name.clone(),
                value: e.guid,
            })
            .collect();

        let element_keys = index
            .element_names
            .iter()
            .filter(|e| {
                element_names
                    .iter()
                    .any(|n: &Named<Guid>| n.value == e.guid)
            })
            .map(|e| (e.guid.to_string(), e.name_key.clone()))
            .collect();

        let lasers = index
            .lasers
            .iter()
            .filter(|l| l.size >= 1 && !is_junk(&l.record_name))
            .filter_map(|l| {
                Some(Named {
                    id: l.guid.to_string(),
                    name: name(locale, l.name_key.as_deref())?,
                    record_name: l.record_name.clone(),
                    value: LaserItem {
                        size: l.size,
                        stats: l.stats.clone(),
                        craft: None,
                    },
                })
            })
            .collect();

        let modules = index
            .modules
            .iter()
            .filter(|m| !is_junk(&m.record_name))
            .filter(|m| {
                !matches!(
                    m.module.activation,
                    Activation::Active {
                        duration_s: None,
                        ..
                    }
                )
            })
            .filter_map(|m| {
                Some(Named {
                    id: m.module.guid.to_string(),
                    name: name(locale, m.module.name_key.as_ref().map(|k| k.as_str()))?,
                    record_name: m.record_name.clone(),
                    value: m.module.clone(),
                })
            })
            .collect();

        let gadgets = index
            .gadgets
            .iter()
            .filter(|g| !is_junk(&g.record_name))
            .filter_map(|g| {
                Some(Named {
                    id: g.gadget.guid.to_string(),
                    name: name(locale, g.gadget.name_key.as_ref().map(|k| k.as_str()))?,
                    record_name: g.record_name.clone(),
                    value: g.gadget.clone(),
                })
            })
            .collect();

        // The meshes ship rocks spawn with, each once.
        let mut rock_shapes: Vec<Shape> = Vec::new();
        {
            use svc_data::RecordCollection;
            for rock in index
                .rock_types
                .values()
                .filter(|r| r.ruleset == Some(ruleset.guid))
            {
                for g in &rock.geometries {
                    if let Some(shape) = index.shapes.get(&g.path)
                        && !rock_shapes.iter().any(|s| s.path == shape.path)
                    {
                        rock_shapes.push(shape.clone());
                    }
                }
            }
        }
        Some(Self {
            ruleset,
            elements: index.elements.clone(),
            element_names: by_name(element_names),
            element_keys,
            lasers: by_name(lasers),
            modules: by_name(modules),
            gadgets: by_name(gadgets),
            rock_shapes,
        })
    }

    /// Each laser with its recipe, when a blueprint makes it: a laser
    /// marked crafted then solves with the recipe's power.
    pub fn with_recipes(mut self, recipes: &Recipes) -> Self {
        for l in &mut self.lasers {
            let Some(bp) = recipes.of(&l.id) else {
                continue;
            };
            let mut power: Vec<String> = Vec::new();
            for m in bp.slots.iter().flat_map(|s| &s.modifiers) {
                if let Some(p) = m.property.as_deref()
                    && recipes.stat_of(p) == Some(GameplayStat::WeaponDamage)
                    && !power.iter().any(|x| x == p)
                {
                    power.push(p.to_owned());
                }
            }
            l.value.craft = Some(LaserCraft {
                blueprint: bp.guid.clone(),
                slots: bp.slots.clone(),
                power,
            });
        }
        self
    }

    pub fn laser(&self, id: &str) -> Option<&Named<LaserItem>> {
        self.lasers.iter().find(|l| l.id == id)
    }

    pub fn module(&self, id: &str) -> Option<&Named<MiningModule>> {
        self.modules.iter().find(|m| m.id == id)
    }

    pub fn gadget(&self, id: &str) -> Option<&Named<MiningGadget>> {
        self.gadgets.iter().find(|g| g.id == id)
    }

    pub fn element(&self, id: &str) -> Option<&Named<Guid>> {
        self.element_names.iter().find(|e| e.id == id)
    }
}

/// Whether a module only works while activated.
pub fn is_active(module: &MiningModule) -> bool {
    matches!(module.activation, Activation::Active { .. })
}

#[cfg(test)]
mod tests {
    use super::*;
    use svc_data::mining::{MiningBeam, MiningModifiers};
    use svc_data::{ModifierRange, ModifierTransform, SlotModifier};

    fn beam(hit: &str, eps: f32) -> MiningBeam {
        MiningBeam {
            fire_action: 0,
            hit_type: hit.into(),
            energy_per_second: eps,
            full_damage_range: 60.0,
            zero_damage_range: 180.0,
        }
    }

    /// The Arbor MH1's recipe shape: two slots of Impact Force at
    /// ×0.80–1.20 each.
    fn arbor() -> LaserItem {
        let slot = || BlueprintSlot {
            name: None,
            material: "Ore".into(),
            scu: Some(1.0),
            count: None,
            min_quality: 0,
            modifiers: vec![SlotModifier {
                property: Some("impact".into()),
                name: Some("Impact Force".into()),
                transform: ModifierTransform::Raw,
                ranges: vec![ModifierRange {
                    additive: false,
                    start_quality: 0,
                    end_quality: 1000,
                    at_start: 0.8,
                    at_end: 1.2,
                }],
            }],
        };
        LaserItem {
            size: 1,
            stats: MiningLaserStats {
                beams: vec![beam("ElectricArc", 2340.0), beam("Extraction", 1850.0)],
                throttle_minimum: 0.1,
                throttle_lerp_speed: 1.0,
                uses_power_throttle: false,
                modifiers: MiningModifiers::default(),
                inert_filter: None,
                module_slots: 1,
            },
            craft: Some(LaserCraft {
                blueprint: "bp".into(),
                slots: vec![slot(), slot()],
                power: vec!["impact".into()],
            }),
        }
    }

    #[test]
    fn a_crafted_laser_powers_its_fracturing_beam_only() {
        let l = arbor();
        assert_eq!(l.power_at(None), 2340.0);
        // Two +20% slots at Q1000: +40%.
        assert!((l.power_at(Some(1000)) - 3276.0).abs() < 0.01);
        assert_eq!(l.power_at(Some(500)), 2340.0);
        let crafted = l.stats_at(Some(1000));
        assert_eq!(crafted.extraction_beam().unwrap().energy_per_second, 1850.0);
    }

    #[test]
    fn a_laser_no_recipe_makes_stays_as_sold() {
        let l = LaserItem {
            craft: None,
            ..arbor()
        };
        assert_eq!(l.power_at(Some(1000)), 2340.0);
    }
}
