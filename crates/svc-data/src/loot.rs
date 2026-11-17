//! Where items drop: per item, the container fills that can yield it and
//! the places those containers sit. Reference framework (CONTRIBUTING
//! module rule 4), for Plans' Looting.
//!
//! Built in the cook from sc-holotable's simulated drops
//! ([`sc_holotable::loot::Drops`]) and written beside it; loaded only when
//! something asks, so it costs nothing while Plans is closed. Only drops in
//! a container placed on the star map are kept (NPC loot and mission
//! content have no place to send you), at most [`PER_ITEM`] per item, and
//! place names are resolved here so the file is self-contained.

use std::collections::HashMap;

use sc_holotable::Foundations;
use sc_holotable::asset::{AssetSource, Datacore, Guid, LocaleMap, RecordCollection};
use sc_holotable::loot::{Drops, LootPlacements, LootTables, Lootables};
use serde::{Deserialize, Serialize};

/// How many drops an item keeps, most likely first.
pub const PER_ITEM: usize = 8;

/// Simulated fills per lootable config: a chance below about `1 / RUNS`
/// may be missed.
const RUNS: u32 = 2_000;

/// The loot of one build.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LootIndex {
    /// Every place a kept drop names; drops point into it.
    pub places: Vec<LootPlace>,
    /// By item record GUID, most likely first.
    pub items: HashMap<String, Vec<LootDrop>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LootPlace {
    /// The `StarMapObject` GUID.
    pub guid: String,
    pub name: String,
}

/// One container fill that can yield the item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LootDrop {
    /// The chance one fill yields at least one.
    pub chance: f32,
    /// How many one fill yields on average.
    pub expected: f32,
    /// The places it sits ([`LootIndex::places`] index) with how many such
    /// containers each has, most first.
    pub places: Vec<(u32, u32)>,
}

impl LootIndex {
    pub fn get(&self, item: &str) -> &[LootDrop] {
        self.items.get(item).map_or(&[], Vec::as_slice)
    }

    pub fn place(&self, index: u32) -> &LootPlace {
        &self.places[index as usize]
    }
}

/// Simulate the loot tables and keep what Plans can send you to. Empty
/// without the p4k: the containers' placements are read from it.
pub(crate) fn build_loot(
    datacore: &Datacore,
    foundations: &Foundations,
    locale: &LocaleMap,
    assets: Option<&AssetSource>,
) -> LootIndex {
    let Some(assets) = assets else {
        return LootIndex::default();
    };
    let tables = LootTables::build(datacore, &foundations.items, &foundations.tags);
    let lootables = Lootables::build(datacore, &tables);
    let placements = match LootPlacements::cook(assets, &lootables) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("loot placements unreadable ({e}); no loot index");
            return LootIndex::default();
        }
    };
    let drops = Drops::build(&tables, &lootables, &placements, &foundations.items, RUNS);
    let name_of = |g: &Guid| {
        let location = foundations.locations.get(g)?;
        let name = location.display_name(locale)?;
        (!name.is_empty() && !name.starts_with('@')).then(|| name.to_owned())
    };
    index(&drops, name_of)
}

/// Fold `drops` into the index, naming places with `name_of` (an unnamed
/// place is dropped; a drop left with none is too).
fn index(drops: &Drops, name_of: impl Fn(&Guid) -> Option<String>) -> LootIndex {
    let mut out = LootIndex::default();
    let mut slot: HashMap<Guid, Option<u32>> = HashMap::new();
    for (item, list) in drops.iter() {
        let mut kept = Vec::new();
        for drop in list {
            let source = drops.source(drop);
            let places: Vec<(u32, u32)> = source
                .places
                .iter()
                .filter_map(|(place, n)| {
                    let at = *slot.entry(*place).or_insert_with(|| {
                        let name = name_of(place)?;
                        out.places.push(LootPlace {
                            guid: place.to_string(),
                            name,
                        });
                        Some(out.places.len() as u32 - 1)
                    });
                    Some((at?, *n as u32))
                })
                .collect();
            if places.is_empty() {
                continue;
            }
            kept.push(LootDrop {
                chance: drop.chance as f32,
                expected: drop.expected as f32,
                places,
            });
        }
        kept.sort_by(|a, b| b.chance.total_cmp(&a.chance));
        kept.truncate(PER_ITEM);
        if !kept.is_empty() {
            out.items.insert(item.to_string(), kept);
        }
    }
    tracing::info!(
        items = out.items.len(),
        places = out.places.len(),
        "loot index built"
    );
    out
}
