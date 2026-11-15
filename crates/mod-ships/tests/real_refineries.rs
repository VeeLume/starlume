//! Ignored-by-default check against real game data: the refineries the cook
//! found resolve to named places quantum travel can go.
//!
//! Shares svc-data's real-data cache; a cook-rev bump recooks from the
//! extract snapshot (tier 2):
//! ```text
//! STARLUME_REAL_P4K="C:\Games\StarCitizen\LIVE\Data.p4k" \
//!   cargo test -p mod-ships --release --test real_refineries -- --ignored --nocapture
//! ```

use mod_ships::{places, refinery_places};
use svc_data::{DataService, InstallRef, RecordCollection};

#[test]
#[ignore = "needs a real Data.p4k — set STARLUME_REAL_P4K"]
fn refineries_resolve_to_named_quantum_travel_places() {
    let p4k =
        std::env::var("STARLUME_REAL_P4K").expect("set STARLUME_REAL_P4K to a real Data.p4k path");
    let install = InstallRef {
        channel_key: "real-test".into(),
        p4k_path: p4k.into(),
        build_id: "real-test-build".into(),
        version: "real-test".into(),
    };
    let service = DataService::new(std::env::temp_dir().join("starlume-svcdata-real-test"));
    let cooked = service
        .load(&install, |stage| eprintln!("stage: {stage:?}"))
        .expect("load real install");

    let refining = &cooked.refining;
    let placements = refining
        .placements
        .as_ref()
        .expect("the cook had the p4k: placements are cooked");
    let socpaks: usize = placements.values().map(Vec::len).sum();
    let unplaced = placements
        .values()
        .flatten()
        .filter(|p| p.places.is_empty())
        .count();
    let raw = refining.places();
    eprintln!(
        "{} refinery classes in {socpaks} object containers ({unplaced} of them belong to no place), {} star-map places, {} refining methods",
        placements.len(),
        raw.len(),
        refining.processes.len()
    );
    assert!(!raw.is_empty(), "no place has a refinery");

    // Every place with a refinery is on the star map, and leads to a place
    // quantum travel can go: itself, or one of its parents.
    let star_map = cooked.holotable.locations.as_ref().unwrap();
    let travel = places(&cooked).places;
    let mut unresolved = Vec::new();
    for place in &raw {
        let named = star_map
            .get(place)
            .and_then(|l| l.display_name(&cooked.locale))
            .unwrap_or("?");
        let reached = std::iter::once(*place)
            .chain(star_map.ancestors(place).map(|l| l.guid))
            .find_map(|g| travel.iter().find(|p| p.guid == g.to_string()));
        match reached {
            Some(p) => eprintln!("REFINERY {named} -> {} ({}, {})", p.name, p.kind, p.system),
            None => unresolved.push(named.to_string()),
        }
    }
    assert!(
        unresolved.is_empty(),
        "refineries at places travel cannot reach: {unresolved:?}"
    );

    let resolved = refinery_places(&cooked);
    eprintln!(
        "{} of {} refinery places resolve to {} quantum-travel places",
        raw.len() - unresolved.len(),
        raw.len(),
        resolved.len()
    );
    assert!(!resolved.is_empty());
    for p in &resolved {
        assert!(!p.name.trim().is_empty(), "{} has no name", p.guid);
        assert!(!p.system.is_empty(), "{} has no system", p.name);
    }
    // The refinery every miner knows.
    assert!(
        resolved.iter().any(|p| p.name.contains("ARC-L1")),
        "ARC-L1 is not among {:?}",
        resolved.iter().map(|p| &p.name).collect::<Vec<_>>()
    );
}
