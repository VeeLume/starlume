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
use svc_data::mining::{
    Activation, Elements, MiningGadget, MiningLaserStats, MiningModule, Ruleset,
};
use svc_data::{Guid, LocaleMap, MiningIndex};

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
    pub lasers: Vec<Named<LaserItem>>,
    pub modules: Vec<Named<MiningModule>>,
    pub gadgets: Vec<Named<MiningGadget>>,
}

/// A ship mining laser.
#[derive(Debug, Clone, PartialEq)]
pub struct LaserItem {
    pub size: i32,
    pub stats: MiningLaserStats,
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

        let element_names = index
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

        Some(Self {
            ruleset,
            elements: index.elements.clone(),
            element_names: by_name(element_names),
            lasers: by_name(lasers),
            modules: by_name(modules),
            gadgets: by_name(gadgets),
        })
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
