//! Ignored-by-default check of the rock generator against real game data:
//! the areas with their rock types and ore odds, and real rolls of one rock
//! type (average, smallest, largest, most ore, best quality, a class for the
//! Golem). Shares svc-data's real-data cache.
//! ```text
//! STARLUME_REAL_P4K="C:\Games\StarCitizen\LIVE\Data.p4k" \
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

    let places = cooked.mining.places.as_ref();
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
        places,
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
    // The place tree: systems, planets with moons, places and fields.
    let tree = mod_mining::place_tree(&src, &catalog);
    fn show(n: &mod_mining::PlaceNode, depth: usize) {
        eprintln!(
            "TREE {}{} [{}]{}{}",
            "  ".repeat(depth),
            n.name,
            n.kind,
            if n.areas.len() > 1 {
                format!(" {} areas", n.areas.len())
            } else {
                String::new()
            },
            if n.count > 0 {
                format!(" ({} places)", n.count)
            } else {
                String::new()
            }
        );
        for c in &n.children {
            show(c, depth + 1);
        }
    }
    for sys in &tree {
        eprintln!("TREE {}", sys.name);
        for n in &sys.nodes {
            show(n, 1);
        }
    }
    let stanton = tree.iter().find(|s| s.name == "Stanton").expect("Stanton");
    let hurston = stanton
        .nodes
        .iter()
        .find(|n| n.name == "Hurston")
        .expect("Hurston");
    assert!(
        hurston.children.iter().any(|c| c.name == "Aberdeen"),
        "moons nest under their planet"
    );
    assert!(
        !tree
            .iter()
            .flat_map(|s| &s.nodes)
            .any(|n| n.name.contains("Akiro")),
        "leftovers stay hidden"
    );
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
            who: None,
            crew: None,
            distance_m: None,
        }],
        gadget: None,
        lead: None,
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
            "hard",
            Pick::Class {
                class: RockClass::Hard,
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
        match generate(&src, &catalog, &request, Some(&golem), &|| false) {
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
                        " | easy {:.0} manageable {:.0} hard {:.0} too much power {:.0} impossible {:.0}",
                        c.easy * 100.0,
                        c.manageable * 100.0,
                        c.hard * 100.0,
                        c.too_much_power * 100.0,
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

    // A pick by mass lands near it; the histogram covers the sample.
    let target = mass("average") * 1.5;
    let g = generate(
        &src,
        &catalog,
        &GenerateRequest {
            area: area.id.clone(),
            rock_type: rock.rock_type.clone(),
            pick: Pick::Mass { mass: target },
            place: None,
        },
        Some(&golem),
        &|| false,
    )
    .expect("mass pick");
    assert_eq!(g.masses.len(), 36);
    assert_eq!(g.masses.iter().map(|b| b.count).sum::<u32>(), g.rolled);
    assert!(g.masses.iter().any(|b| b.class.is_some()));
    eprintln!("  by mass     {:>8.0} t (asked {target:.0} t)", g.rock.mass);

    // The finder's class table: every rollable rock type at every area.
    let start = Instant::now();
    let table = mod_mining::class_table(&src, &catalog, &golem, &|| false).expect("class table");
    eprintln!(
        "class table: {} rows in {} ms",
        table.len(),
        start.elapsed().as_millis()
    );
    assert!(!table.is_empty());
    for r in &table {
        let c = r.classes;
        let sum = c.easy + c.manageable + c.hard + c.too_much_power + c.impossible;
        assert!(
            (sum - 1.0).abs() < 1e-3,
            "{} shares sum to {sum}",
            r.rock_type
        );
    }
}
