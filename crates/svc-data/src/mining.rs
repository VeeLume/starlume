//! Mining reference data — sc-mining's rulesets and elements plus the
//! mining lasers, modules and gadgets, cooked into one serde bundle.
//! Reference-catalog framework data (CONTRIBUTING module rule 4): mod-mining
//! solves rocks against it, and never needs the raw Datacore.
//!
//! The sc-mining and item-crate types are serde already, so they cross into
//! the snapshot as they are; only the record name (for search keys and the
//! junk filter) and the raw locale keys are added beside them.
//!
//! The rock spawn side (rock types, compositions, and the rock meshes'
//! fracture trees) feeds the rock generator. The meshes need the live p4k:
//! the cook passes it when it can open it (a live parse has it open; a
//! recook from the extract snapshot opens it for this pass), and leaves
//! [`MiningIndex::shapes`] empty when it cannot.
//!
//! Everything named is kept; which entries a picker offers (test records,
//! FPS lasers, base records) is the consumer's choice.
//!
//! sc-mining's maths and the mining item types are re-exported here, so
//! mod-mining solves against the snapshot without its own sc-holotable pin
//! (the one-pin rule).

use std::collections::{BTreeSet, HashMap};

use sc_holotable::asset::{AssetSource, Datacore, Guid};
use sc_holotable::items::Items;
use sc_holotable::resources::{Quality, Resources};
use sc_holotable::ship_weapons::ShipWeapons;
use serde::{Deserialize, Serialize};

pub use sc_holotable::mining::*;
pub use sc_holotable::mining_items::*;
pub use sc_holotable::ship_weapons::{MiningBeam, MiningLaserStats};

/// The cooked mining bundle.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MiningIndex {
    /// Every `MiningGlobalParams` record (ship, ground vehicle, FPS), sorted
    /// by record name.
    pub rulesets: Vec<RulesetEntry>,
    /// Every `MineableElement`, densities included — `Rock::from_mass`
    /// reads them.
    pub elements: Elements,
    /// The elements with a display name and the rulesets whose rocks spawn
    /// them, sorted by record name.
    pub element_names: Vec<ElementEntry>,
    /// Ship weapons with mining stats, sorted by record name.
    pub lasers: Vec<LaserEntry>,
    /// Mining modules, sorted by record name.
    pub modules: Vec<ModuleEntry>,
    /// Mining gadgets, sorted by record name.
    pub gadgets: Vec<GadgetEntry>,
    /// Every mineable rock type (`HarvestablePreset`): ruleset, composition,
    /// scale range, meshes.
    #[serde(default)]
    pub rock_types: RockTypes,
    /// Rock type record names, sorted by record name.
    #[serde(default)]
    pub rock_type_names: Vec<RockTypeEntry>,
    /// `HarvestableProviderPreset` record names (`"HPP_Nyx_KeegerBelt"`),
    /// sorted: the label for a provider no place is known for.
    #[serde(default)]
    pub provider_names: Vec<RockTypeEntry>,
    /// Every `MineableComposition`: the parts a rock rolls.
    #[serde(default)]
    pub compositions: Compositions,
    /// Each rock mesh's volume and fracture tree. Empty when the cook could
    /// not open the p4k.
    #[serde(default)]
    pub shapes: Shapes,
    /// The shared quality records (distributions, location overrides,
    /// quantization) a rock's quality roll reads beside `Resources`.
    #[serde(default)]
    pub quality: Quality,
}

/// A record's name beside its GUID (rock types, providers).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RockTypeEntry {
    pub guid: Guid,
    pub record_name: String,
}

/// One `MiningGlobalParams` record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RulesetEntry {
    /// DCB record name (`"MiningGlobalParamsShip"`).
    pub record_name: String,
    pub ruleset: Ruleset,
}

/// One mineable element, as a picker lists it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ElementEntry {
    pub guid: Guid,
    /// DCB record name (`"Bexalite_Ore"`).
    pub record_name: String,
    /// The resource's raw `@`-key, when the element names one.
    pub name_key: Option<String>,
    /// Record names of the rulesets whose rock types list this element in
    /// their composition — which kind of mining finds it.
    pub rulesets: Vec<String>,
}

/// One mining laser.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LaserEntry {
    pub guid: Guid,
    pub record_name: String,
    pub name_key: Option<String>,
    /// Item size: 0 for FPS and ROC heads, 1 and 2 for ships.
    pub size: i32,
    pub grade: i32,
    pub stats: MiningLaserStats,
}

/// One mining module.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModuleEntry {
    pub record_name: String,
    pub module: MiningModule,
}

/// One mining gadget.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GadgetEntry {
    pub record_name: String,
    pub gadget: MiningGadget,
}

/// Cook the mining index from one parsed `Datacore`; `items` and
/// `resources` come from the foundations build the caller already paid for,
/// `assets` (the live p4k, when the cook could open it) feeds the meshes.
pub(crate) fn build_mining(
    datacore: &Datacore,
    items: &Items,
    resources: &Resources,
    assets: Option<&AssetSource>,
) -> MiningIndex {
    let db = datacore.db();
    let record_name = |guid: &Guid| {
        db.record(guid)
            .and_then(|r| {
                r.name()
                    .map(|n| n.rsplit('.').next().unwrap_or(n).to_string())
            })
            .unwrap_or_default()
    };
    let key = |k: Option<&sc_holotable::asset::LocaleKey>| k.map(|k| k.as_str().to_string());

    let mut rulesets: Vec<RulesetEntry> = Rulesets::build(datacore, resources)
        .iter()
        .map(|(g, r)| RulesetEntry {
            record_name: record_name(g),
            ruleset: r.clone(),
        })
        .collect();
    rulesets.sort_by(|a, b| a.record_name.cmp(&b.record_name));
    let ruleset_names: HashMap<Guid, &str> = rulesets
        .iter()
        .map(|r| (r.ruleset.guid, r.record_name.as_str()))
        .collect();

    // Which rulesets' rocks spawn each element: rock type → composition →
    // parts. Typed GUID joins throughout.
    let compositions = Compositions::build(datacore);
    let rock_types = RockTypes::build(datacore);
    let mut found_by: HashMap<Guid, BTreeSet<String>> = HashMap::new();
    for rock in rock_types.values() {
        let (Some(ruleset), Some(composition)) = (
            rock.ruleset.and_then(|g| ruleset_names.get(&g)),
            rock.composition.and_then(|c| compositions.get(&c)),
        ) else {
            continue;
        };
        for element in composition.parts.iter().filter_map(|p| p.element) {
            found_by
                .entry(element)
                .or_default()
                .insert((*ruleset).to_string());
        }
    }

    let elements = Elements::build(datacore, resources);
    let mut element_names: Vec<ElementEntry> = elements
        .iter()
        .map(|(g, e)| ElementEntry {
            guid: *g,
            record_name: record_name(g),
            name_key: e
                .resource
                .and_then(|r| resources.get(&r))
                .map(|r| r.name_key.as_str().to_string()),
            rulesets: found_by
                .remove(g)
                .map(|s| s.into_iter().collect())
                .unwrap_or_default(),
        })
        .collect();
    element_names.sort_by(|a, b| a.record_name.cmp(&b.record_name));

    let mut lasers: Vec<LaserEntry> = ShipWeapons::build(datacore, items)
        .iter()
        .filter_map(|(g, w)| {
            Some(LaserEntry {
                guid: *g,
                record_name: record_name(g),
                name_key: key(w.name_key.as_ref()),
                size: w.size,
                grade: w.grade,
                stats: w.mining.clone()?,
            })
        })
        .collect();
    lasers.sort_by(|a, b| a.record_name.cmp(&b.record_name));

    let mut modules: Vec<ModuleEntry> = MiningModules::build(datacore, items)
        .iter()
        .map(|(g, m)| ModuleEntry {
            record_name: record_name(g),
            module: m.clone(),
        })
        .collect();
    modules.sort_by(|a, b| a.record_name.cmp(&b.record_name));

    let mut gadgets: Vec<GadgetEntry> = MiningGadgets::build(datacore, items)
        .iter()
        .map(|(g, m)| GadgetEntry {
            record_name: record_name(g),
            gadget: m.clone(),
        })
        .collect();
    gadgets.sort_by(|a, b| a.record_name.cmp(&b.record_name));

    let mut rock_type_names: Vec<RockTypeEntry> = rock_types
        .iter()
        .map(|(g, _)| RockTypeEntry {
            guid: *g,
            record_name: record_name(g),
        })
        .collect();
    rock_type_names.sort_by(|a, b| a.record_name.cmp(&b.record_name));
    let mut provider_names: Vec<RockTypeEntry> = datacore
        .records()
        .records
        .harvestable
        .harvestable_provider_preset
        .keys()
        .map(|g| RockTypeEntry {
            guid: *g,
            record_name: record_name(g),
        })
        .collect();
    provider_names.sort_by(|a, b| a.record_name.cmp(&b.record_name));
    // A dozen meshes, about half a second.
    let shapes = match assets {
        Some(assets) => Shapes::build(assets, &rock_types),
        None => {
            tracing::warn!("no p4k for the rock meshes; the rock generator stays empty");
            Shapes::new()
        }
    };

    MiningIndex {
        rulesets,
        elements,
        element_names,
        lasers,
        modules,
        gadgets,
        rock_types,
        rock_type_names,
        provider_names,
        compositions,
        shapes,
        quality: Quality::build(datacore.records()),
    }
}
