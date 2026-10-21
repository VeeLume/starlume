//! Ignored-by-default timing of the comfort solve (every rock that spawns,
//! one loadout) against real game data: a Golem, a Prospector and a MOLE
//! with one, two and three heads firing. Shares svc-data's real-data cache.
//! Run it in a release-speed profile; `profiling` keeps line tables for a
//! sampling profiler:
//! ```text
//! STARLUME_REAL_P4K="C:\Games\StarCitizen\LIVE\Data.p4k" \
//!   cargo test -p mod-mining --profile profiling --test real_comfort -- --ignored --nocapture
//! ```

use std::time::Instant;

use mod_mining::{Catalog, LaserInput, LoadoutInput, Sources, comfort};
use svc_data::{DataService, InstallRef};

#[test]
#[ignore = "needs a real Data.p4k — set STARLUME_REAL_P4K"]
fn comfort_timings() {
    let p4k =
        std::env::var("STARLUME_REAL_P4K").expect("set STARLUME_REAL_P4K to a real Data.p4k path");
    let install = InstallRef {
        channel_key: "real-test".into(),
        p4k_path: p4k.into(),
        build_id: "real-test-build".into(),
        version: "real-test".into(),
    };
    let service = DataService::new(std::env::temp_dir().join("starlume-svcdata-real-test"));
    let start = Instant::now();
    let cooked = service.load(&install, |_| {}).expect("load real install");
    eprintln!("load: {} ms", start.elapsed().as_millis());

    let start = Instant::now();
    let catalog = Catalog::build(&cooked.mining, &cooked.locale).unwrap();
    let h = &cooked.holotable;
    let src = Sources {
        index: &cooked.mining,
        providers: h.gathering.as_ref().unwrap(),
        tags: h.tags.as_ref().unwrap(),
        resources: h.resources.as_ref().unwrap(),
        locations: h.locations.as_ref().unwrap(),
        locale: &cooked.locale,
        places: cooked.mining.places.as_ref(),
    };
    eprintln!("catalog + sources: {} ms", start.elapsed().as_millis());

    let laser = |name: &str| {
        catalog
            .lasers
            .iter()
            .find(|l| l.name.contains(name))
            .unwrap_or_else(|| panic!("no laser named {name}"))
            .id
            .clone()
    };
    let head = |name: &str, off: bool| LaserInput {
        laser: laser(name),
        off,
        ..Default::default()
    };
    let ship = |lasers: Vec<LaserInput>| LoadoutInput {
        lasers,
        gadget: None,
        lead: None,
    };
    let mole = |on: usize| {
        ship(
            ["Arbor MH2", "Hofstede-S2", "Hofstede-S2"]
                .iter()
                .enumerate()
                .map(|(i, n)| head(n, i >= on))
                .collect(),
        )
    };
    let loadouts = [
        ("Golem (Pitman)", ship(vec![head("Pitman", false)])),
        (
            "Prospector (Arbor MH1)",
            ship(vec![head("Arbor MH1", false)]),
        ),
        ("MOLE, 1 head", mole(1)),
        ("MOLE, 2 heads", mole(2)),
        ("MOLE, 3 heads", mole(3)),
    ];
    for (label, loadout) in &loadouts {
        let start = Instant::now();
        let rows = comfort(&src, &catalog, loadout, &|| false).expect("comfort");
        eprintln!(
            "{label:<24} {:>6} ms  ({} distributions)",
            start.elapsed().as_millis(),
            rows.len()
        );
        assert!(!rows.is_empty());
    }
}
