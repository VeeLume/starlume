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
            off: false,
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

/// A gas pocket (`ARC L3-A`) is not a target on the star map: an ore site
/// there is flown to by its Lagrange point.
#[test]
#[ignore = "needs a real Data.p4k — set STARLUME_REAL_P4K"]
fn ore_sites_send_pockets_to_their_lagrange_point() {
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
    let pocket = |name: &str| {
        name.rsplit_once('-').is_some_and(|(point, letter)| {
            point.contains(" L")
                && letter.len() == 1
                && letter.chars().all(|c| c.is_ascii_uppercase())
        })
    };
    let mut moved: Vec<String> = Vec::new();
    for element in &catalog.element_names {
        for site in mod_mining::ore_sites(&src, &catalog, &element.id, 0) {
            for target in &site.travel {
                assert!(
                    !pocket(&target.name),
                    "{} is flown to as {}",
                    site.name,
                    target.name
                );
                if !site.places.iter().any(|p| p.id == target.id) {
                    let line = format!("{} -> {}", site.name, target.name);
                    if !moved.contains(&line) {
                        moved.push(line);
                    }
                }
            }
        }
    }
    for line in moved.iter().take(20) {
        eprintln!("{line}");
    }
    assert!(
        moved.iter().any(|m| m.ends_with("ARC L3")),
        "no site moved to ARC L3"
    );
}

/// The share of an area's ship rocks that hold an ore at a quality, for a
/// handful of ores: printed as a table (the figures to compare when the
/// odds' method changes), and never rising with the quality asked.
#[test]
#[ignore = "needs a real Data.p4k — set STARLUME_REAL_P4K"]
fn ore_site_odds_fall_with_the_quality_asked() {
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
    let wanted = [
        "quantanium",
        "titanium",
        "gold",
        "copper",
        "riccite",
        "stileron",
        "iron",
        "tungsten",
    ];
    let qualities = [0u16, 300, 500, 700, 900];
    let mut seen = 0;
    for element in &catalog.element_names {
        let name = element.name.to_lowercase();
        if !wanted.iter().any(|w| name.starts_with(w)) {
            continue;
        }
        let start = Instant::now();
        let by_quality: Vec<Vec<mod_mining::OreSite>> = qualities
            .iter()
            .map(|q| mod_mining::ore_sites(&src, &catalog, &element.id, *q))
            .collect();
        let ms = start.elapsed().as_millis();
        let Some(best) = by_quality[0].first() else {
            continue;
        };
        seen += 1;
        // The area that holds it most often, and the one best at Q700.
        let mut areas = vec![best.area.clone()];
        if let Some(high) = by_quality[3].first().filter(|s| s.area != best.area) {
            areas.push(high.area.clone());
        }
        for area in areas {
            let row: Vec<String> = qualities
                .iter()
                .zip(&by_quality)
                .map(|(q, sites)| {
                    let find = sites
                        .iter()
                        .find(|s| s.area == area)
                        .map_or(0.0, |s| s.find);
                    format!("Q{q}+ {:.2}%", find * 100.0)
                })
                .collect();
            let name = &by_quality
                .iter()
                .flatten()
                .find(|s| s.area == area)
                .unwrap()
                .name;
            eprintln!("ODDS {} @ {}: {}", element.name, name, row.join(" | "));
        }
        eprintln!(
            "ODDS {}: sites per quality {:?}, {ms} ms",
            element.name,
            by_quality.iter().map(Vec::len).collect::<Vec<_>>()
        );
        for pair in by_quality.windows(2) {
            for site in &pair[1] {
                let lower = pair[0]
                    .iter()
                    .find(|s| s.area == site.area)
                    .map_or(0.0, |s| s.find);
                assert!(
                    site.find <= lower + 1e-4,
                    "{} at {}: {} above {lower} at the lower quality",
                    element.name,
                    site.name,
                    site.find
                );
            }
        }
        for site in by_quality.iter().flatten() {
            assert!((0.0..=1.0001).contains(&site.find), "{}", site.find);
        }
    }
    assert!(seen > 0, "none of the ores is in the catalog");
}
