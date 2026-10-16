//! Mining places: which game places (`StarMapObject`s) each harvestable
//! provider applies to — the location half of the rock generator.
//!
//! sc-gathering's [`ProviderLocations`] joins provider ↔ place through the
//! placement graph, which needs the live p4k and is heavy: walking every
//! object container took 200 s and the join another 60 s on LIVE 4.10, at a
//! multi-GB peak. So it is not part of the cook. The shell runs
//! [`crate::DataService::build_mining_places`] as a background activity once
//! per build (only while the mining module is enabled, never alongside a
//! parse), and only the join (~48 KB) is kept, persisted beside the cooked
//! snapshot and read from disk per query — nothing stays resident.
//!
//! Known gaps (holotable, 2026-10-16): 14 of the 49 providers get no place
//! this way (the Nyx belts, Yela's ring, the Pyro clusters, a few special
//! fields); some mining claims are realized under two providers, so
//! `provider_at` is ambiguous for them and `locations_of` lists them under
//! both.

use std::path::Path;

use anyhow::Context;
use sc_holotable::asset::{AssetSource, ProcessedSnapshot};
use sc_holotable::gathering::ProviderLocations;
use sc_holotable::locations::ObjectContainers;

use crate::InstallRef;
use crate::cache;

pub(crate) const PLACES_NAME: &str = "mining-places.cook";

/// Our revision of the places file; composes the upstream schema so either
/// bump invalidates it.
pub const PLACES_COOK_VERSION: u32 = 1000 + ProviderLocations::COOK_SCHEMA_VERSION;

/// Walk the placement graph and join it to the providers. Blocking and
/// heavy: the caller runs it on its own thread.
pub(crate) fn build(p4k: &Path) -> anyhow::Result<ProviderLocations> {
    let assets = AssetSource::open(p4k).with_context(|| format!("opening {}", p4k.display()))?;
    let containers = ObjectContainers::cook(&assets).context("reading the object containers")?;
    let places =
        ProviderLocations::cook(&assets, &containers).context("joining places to providers")?;
    // Only the join is kept; the placement graph and the p4k go here.
    drop(containers);
    drop(assets);
    Ok(places)
}

/// The persisted join for this install's build, if one was written.
pub(crate) fn load(dir: &Path, install: &InstallRef) -> Option<ProviderLocations> {
    let path = dir.join(PLACES_NAME);
    if !path.exists() {
        return None;
    }
    match ProcessedSnapshot::<ProviderLocations>::load(&path, PLACES_COOK_VERSION) {
        Ok(snap) if cache::same_build(&snap.meta.build_id, install) => Some(snap.into_index()),
        Ok(_) => None,
        Err(e) => {
            tracing::info!("mining places unusable ({e}); they will be rebuilt");
            None
        }
    }
}

pub(crate) fn save(
    dir: &Path,
    install: &InstallRef,
    places: ProviderLocations,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    ProcessedSnapshot::new(cache::meta_for(install), PLACES_COOK_VERSION, places)
        .save(&dir.join(PLACES_NAME))
        .context("saving the mining places")
}
