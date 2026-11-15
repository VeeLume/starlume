//! Refining reference data: the star-map places that have a refinery, and
//! the refining methods a refinery offers. Reference-catalog framework data
//! (CONTRIBUTING module rule 4): Plans places its refine steps from it and
//! never needs the raw Datacore.
//!
//! The places come from sc-holotable's walk of every object container for
//! entities of a refinery class, joined to the star-map place each belongs
//! to (the station a refinery deck is part of). That walk needs the live
//! p4k, so it runs once with the cook; [`RefiningIndex::placements`] is
//! `None` when the cook could not open it.
//!
//! Not in the game data, so not here either: how much ore a refinery turns
//! into how much material, and how long it takes. The methods carry only a
//! speed and a quality label with their names.

use sc_holotable::asset::{AssetSource, Datacore, Guid};
use serde::{Deserialize, Serialize};

pub use sc_holotable::refining::{
    Refineries, RefineryPlacement, RefineryPlacements, RefiningProcess, RefiningProcesses,
};

/// The cooked refining bundle.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RefiningIndex {
    /// Where refineries are placed, by refinery class, with the star-map
    /// places that have one. `None`: the cook had no live p4k to walk.
    #[serde(default)]
    pub placements: Option<RefineryPlacements>,
    /// The refining methods (raw locale keys for name and detail).
    #[serde(default)]
    pub processes: RefiningProcesses,
}

impl RefiningIndex {
    /// The star-map places (`StarMapObject` GUIDs, the keys of the location
    /// records) that have a refinery, in a stable order. Empty when the
    /// placements were not cooked.
    pub fn places(&self) -> Vec<Guid> {
        let mut places: Vec<Guid> = self
            .placements
            .iter()
            .flat_map(RefineryPlacements::places)
            .copied()
            .collect();
        places.sort_by_key(Guid::to_string);
        places
    }
}

/// Build the refining bundle off the cook's parse. `assets` is the live
/// p4k when the cook could open it; the placements walk reads every object
/// container in it.
pub(crate) fn build_refining(datacore: &Datacore, assets: Option<&AssetSource>) -> RefiningIndex {
    let processes = RefiningProcesses::build(datacore);
    let placements = match assets {
        Some(assets) => RefineryPlacements::cook(assets, &Refineries::build(datacore))
            .inspect_err(|e| tracing::warn!("refinery placements: {e}"))
            .ok(),
        None => {
            tracing::warn!("no p4k for the refinery placements; Plans places no refine steps");
            None
        }
    };
    RefiningIndex {
        placements,
        processes,
    }
}
