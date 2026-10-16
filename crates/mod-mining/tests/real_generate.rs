//! Ignored-by-default check of the rock generator against real game data:
//! the areas with their rock types and ore odds, and real rolls of one rock
//! type (average, smallest, largest, most ore, best quality, a class for the
//! Golem). Shares svc-data's real-data cache. With `STARLUME_BUILD_PLACES=1`
//! it first runs the heavy mining-places pass (minutes) so areas get names.
//! ```text
//! STARLUME_REAL_P4K="C:\Games\StarCitizen\LIVE\Data.p4k" [STARLUME_BUILD_PLACES=1] \
//!   cargo test -p mod-mining --release --test real_generate -- --ignored --nocapture
//! ```

use std::time::Instant;

use mod_mining::{
    Catalog, GenerateRequest, LaserInput, LoadoutInput, Pick, RockClass, Sources, areas, generate,
};
use svc_data::{DataService, InstallRef, RecordCollection};

#[test]
#[ignore = "needs a real Data.p4k — set STARLUME_REAL_P4K"]
fn generator_rolls_real_rocks() {
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
    eprintln!(
        "cooked: {} rock types, {} compositions, {} meshes",
        cooked.mining.rock_types.len(),
        cooked.mining.compositions.len(),
        cooked.mining.shapes.len()
    );
    assert!(!cooked.mining.shapes.is_empty(), "no rock meshes cooked");

    if std::env::var("STARLUME_BUILD_PLACES").is_ok() {
        let start = Instant::now();
        service
            .build_mining_places(&install)
            .expect("mining places");
        eprintln!("mining places built in {} s", start.elapsed().as_secs());
    }
    let places = service.mining_places(&install);
    eprintln!("places known: {}", places.is_some());

    let catalog = Catalog::build(&cooked.mining, &cooked.locale).unwrap();
    let h = &cooked.holotable;
    let src = Sources {
        index: &cooked.mining,
        providers: h.gathering.as_ref().unwrap(),
        tags: h.tags.as_ref().unwrap(),
        resources: h.resources.as_ref().unwrap(),
        locations: h.locations.as_ref().unwrap(),
        locale: &cooked.locale,
        places: places.as_ref(),
    };
    let start = Instant::now();
    let all = areas(&src, &catalog);
    eprintln!("{} areas in {} ms", all.len(), start.elapsed().as_millis());
    let element_name = |id: &str| {
        catalog
            .element_names
            .iter()
            .find(|e| e.id == id)
            .map_or(id.to_string(), |e| e.name.clone())
    };
    for a in all.iter().take(60) {
        eprintln!(
            "{}{} ({} places): {}",
            a.name,
            if a.from_record { " [record]" } else { "" },
            a.places.len(),
            a.rocks
                .iter()
                .map(|r| format!(
                    "{} {:.0} %{}",
                    r.name,
                    r.share * 100.0,
                    if r.rollable { "" } else { " (no mesh)" }
                ))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    // An area of several places named by record: what kinds its places are.
    for a in all
        .iter()
        .filter(|a| a.places.len() > 1 && a.places.len() < 6)
    {
        for p in &a.places {
            let kind = src
                .locations
                .get(&p.id.parse().unwrap())
                .map(|l| format!("{:?}", l.kind));
            eprintln!("  {} place {} {:?}", a.name, p.name, kind);
        }
    }
    assert!(
        all.iter()
            .all(|a| a.rocks.iter().map(|r| r.share).sum::<f32>() > 0.95),
        "an area's rock shares should add up to ~100 %"
    );
    assert!(all.len() >= 20, "expected most providers as areas");
    assert!(
        all.iter().any(|a| a.name.starts_with("Glaciem Ring")),
        "the Nyx belts go by their names"
    );
    assert!(
        !all.iter()
            .any(|a| a.name.contains("Lagrange G") || a.name.contains("Akiro")),
        "leftovers are hidden"
    );
    assert!(all.iter().flat_map(|a| &a.rocks).any(|r| r.rollable));

    // One rollable rock type with ores: the first of the first area.
    let (area, rock) = all
        .iter()
        .find_map(|a| {
            a.rocks
                .iter()
                .find(|r| r.rollable && !r.ores.is_empty())
                .map(|r| (a, r))
        })
        .unwrap();
    let ore = &rock.ores[0];
    eprintln!(
        "\nrolling {} at {}: top ore {} ({:.0} % of rocks, {:.1} % share when present)",
        rock.name,
        area.name,
        element_name(&ore.element),
        ore.chance * 100.0,
        ore.mean_share * 100.0
    );
    let golem = LoadoutInput {
        lasers: vec![LaserInput {
            laser: catalog
                .lasers
                .iter()
                .find(|l| l.name.contains("Pitman"))
                .unwrap()
                .id
                .clone(),
            modules: vec![],
            owner: 0,
        }],
        gadget: None,
    };
    let picks = [
        ("average", Pick::Average),
        ("most common", Pick::MostCommon),
        ("smallest", Pick::Smallest),
        ("largest", Pick::Largest),
        (
            "most ore",
            Pick::MostOre {
                element: ore.element.clone(),
            },
        ),
        (
            "best quality",
            Pick::BestQuality {
                element: ore.element.clone(),
            },
        ),
        (
            "dangerous",
            Pick::Class {
                class: RockClass::Dangerous,
            },
        ),
    ];
    let mut masses = Vec::new();
    for (label, pick) in picks {
        let start = Instant::now();
        let request = GenerateRequest {
            area: area.id.clone(),
            rock_type: rock.rock_type.clone(),
            pick,
            place: None,
        };
        match generate(&src, &catalog, &request, Some(&golem)) {
            Ok(g) => {
                eprintln!(
                    "  {label:<12} {:>8.0} t {:>6.2} SCU | {} | {} ms{}",
                    g.rock.mass,
                    g.scu,
                    g.rock
                        .entries
                        .iter()
                        .map(|e| format!(
                            "{} {:.2} %{}",
                            element_name(&e.element),
                            e.percent,
                            e.quality.map_or(String::new(), |q| format!(" q{q}"))
                        ))
                        .collect::<Vec<_>>()
                        .join(", "),
                    start.elapsed().as_millis(),
                    g.classes.map_or(String::new(), |c| format!(
                        " | safe {:.0} okay {:.0} dangerous {:.0} too strong {:.0} impossible {:.0}",
                        c.safe * 100.0,
                        c.okay * 100.0,
                        c.dangerous * 100.0,
                        c.too_strong * 100.0,
                        c.impossible * 100.0
                    ))
                );
                masses.push((label, g.rock.mass));
                if let Some(share) = g.pick_share {
                    eprintln!(
                        "  {:<12} its ore mix in {:.0} % of rolls",
                        "",
                        share * 100.0
                    );
                }
            }
            Err(e) => eprintln!("  {label:<12} {e}"),
        }
    }
    let mass = |l: &str| {
        masses
            .iter()
            .find(|(x, _)| *x == l)
            .map(|(_, m)| *m)
            .unwrap()
    };
    assert!(mass("smallest") <= mass("average") && mass("average") <= mass("largest"));
}
