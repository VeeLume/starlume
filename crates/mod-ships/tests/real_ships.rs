//! Ignored-by-default check against real game data: cook (or reuse) the
//! snapshot and walk the stock RSI Constellation Mk V Centaurus as slots.
//!
//! Shares svc-data's real-data cache; a cook-rev bump recooks from the
//! extract snapshot (tier 2):
//! ```text
//! STARLUME_REAL_P4K="C:\Games\StarCitizen\LIVE\Data.p4k" \
//!   cargo test -p mod-ships --test real_ships -- --ignored --nocapture
//! ```

use std::time::Instant;

use mod_ships::{LoadoutInput, Mode, PowerInput, Slot, System, candidates, ships, slots, view};
use svc_data::ships::{Loadout, RecordCollection};
use svc_data::{CookedData, DataService, InstallRef};

fn cooked() -> std::sync::Arc<CookedData> {
    let p4k =
        std::env::var("STARLUME_REAL_P4K").expect("set STARLUME_REAL_P4K to a real Data.p4k path");
    let install = InstallRef {
        channel_key: "real-test".into(),
        p4k_path: p4k.into(),
        build_id: "real-test-build".into(),
        version: "real-test".into(),
    };
    let service = DataService::new(std::env::temp_dir().join("starlume-svcdata-real-test"));
    service
        .load(&install, |stage| eprintln!("stage: {stage:?}"))
        .expect("load real install")
}

fn print(slots: &[Slot], depth: usize) {
    for s in slots {
        eprintln!(
            "{:indent$}{:?} {} [{}{}] S{}-{} {} {} / {} S{} G{}",
            "",
            s.system,
            s.path.last().unwrap(),
            if s.editable { "edit" } else { "lock" },
            s.label
                .as_deref()
                .map(|l| format!(" '{l}'"))
                .unwrap_or_default(),
            s.min_size,
            s.max_size,
            s.name.as_deref().unwrap_or("-"),
            s.item_type.as_deref().unwrap_or("-"),
            s.sub_type.as_deref().unwrap_or("-"),
            s.size,
            s.grade,
            indent = depth * 2
        );
        print(&s.children, depth + 1);
    }
}

#[test]
#[ignore = "needs a real Data.p4k — set STARLUME_REAL_P4K"]
fn centaurus_stock_slots() {
    let t = Instant::now();
    let cooked = cooked();
    eprintln!("loaded in {:?}", t.elapsed());
    let rows = ships(&cooked);
    eprintln!("{} ships", rows.len());
    let paths = cooked.holotable.paths.as_ref().unwrap();
    for (g, v) in cooked.ships.vehicles.iter() {
        let name = paths.get(g).map(|p| p.name.as_str()).unwrap_or("?");
        if name.contains("Mk5") || name.contains("Centaurus") {
            eprintln!("  {name} {:?} {g}", v.listing);
        }
    }
    assert!(rows.len() > 200, "ship list too short");
    let centaurus = rows
        .iter()
        .find(|r| r.record_name == "RSI_Constellation_Mk5_Centaurus")
        .expect("Centaurus listed");
    eprintln!("{centaurus:?}");
    assert_eq!(centaurus.manufacturer_code.as_deref(), Some("RSI"));
    // sc-vehicles' Storage matches erkul's 240 SCU for the Centaurus.
    assert_eq!(centaurus.cargo, Some(240.0), "{centaurus:?}");
    let mut careers: std::collections::BTreeMap<String, usize> = Default::default();
    for r in rows.iter().filter(|r| r.base.is_none()) {
        *careers
            .entry(r.career.clone().unwrap_or_default())
            .or_default() += 1;
    }
    eprintln!("careers: {careers:?}");
    let editions = rows.iter().filter(|r| r.base.is_some()).count();
    eprintln!("{editions} editions under a base hull");
    for r in rows.iter().filter(|r| r.name.contains("Lightning")) {
        let base = r
            .base
            .as_ref()
            .and_then(|b| rows.iter().find(|x| &x.guid == b));
        eprintln!("  {} → base {:?}", r.name, base.map(|b| &b.name));
    }
    let (guid, _) = cooked
        .ships
        .by_record_name("RSI_Constellation_Mk5_Centaurus")
        .unwrap();
    let loadout = Loadout::stock(&cooked.ships.vehicles, guid).expect("stock loadout");
    let tree = slots(&cooked, &loadout);
    print(&tree, 0);
    let systems: std::collections::BTreeSet<System> = tree.iter().map(|s| s.system).collect();
    eprintln!("systems: {systems:?}");
    assert!(systems.contains(&System::PowerPlants));
    assert!(systems.contains(&System::Coolers));
    assert!(systems.contains(&System::Shields));
    eprintln!(
        "index: {} vehicles, {} containers, {} profiles, {} axes",
        cooked.ships.vehicles.len(),
        cooked.ships.containers.len(),
        cooked.ships.profiles.len(),
        cooked.ships.axes.len()
    );

    // The page's view, stock, power-on in SCM then NAV (spviewer, LIVE
    // 4.10.197: supply 24, coolant 82, cooling 33.65 / 82 in SCM).
    let input = LoadoutInput {
        ship: guid.to_string(),
        edits: Vec::new(),
    };
    for mode in [Mode::Scm, Mode::Nav] {
        let v = view(
            &cooked,
            &input,
            &PowerInput {
                mode,
                ..Default::default()
            },
        )
        .unwrap();
        let p = v.power.as_ref().expect("network rules");
        eprintln!(
            "\n{mode:?}: supply {} used {:.2}, cooling {:?} ({:.2}/{:.2}), EM {:.0}, IR {:.0}, shield {:.0}/s",
            p.supply,
            p.used,
            p.cooling,
            p.coolant_asked,
            p.coolant_made,
            p.em,
            p.ir,
            p.shield_regen
        );
        for c in &p.columns {
            eprintln!(
                "  {:<40} {:<22} {:>5.2} / {:>2} min {:?}{}{}",
                c.id,
                c.item_type,
                c.level,
                c.max,
                c.minimums,
                if c.off { " off" } else { "" },
                if c.excluded { " excluded" } else { "" }
            );
        }
        assert_eq!(p.supply, 24);
        assert!((p.coolant_made - 82.0).abs() < 1e-6);
        if mode == Mode::Scm {
            assert!(
                (p.coolant_asked - 33.65).abs() < 1e-3,
                "{}",
                p.coolant_asked
            );
        }
    }
    // Switching a part off frees its pips and moves no other column (a
    // power-on without the part used to deal its pips to the shields).
    let levels = |input: &PowerInput| -> Vec<(String, f64)> {
        let v = view(
            &cooked,
            &LoadoutInput {
                ship: guid.to_string(),
                edits: Vec::new(),
            },
            input,
        )
        .unwrap();
        v.power
            .unwrap()
            .columns
            .into_iter()
            .map(|c| (c.id, c.level))
            .collect()
    };
    let before = levels(&PowerInput::default());
    let after = levels(&PowerInput {
        off: vec!["hardpoint_cooler_02".into()],
        ..Default::default()
    });
    for ((id, a), (_, b)) in before.iter().zip(&after) {
        if id == "item:hardpoint_cooler_02" {
            assert!(
                *b == 0.0 && *a > 0.0,
                "the cooler did not switch off: {a} to {b}"
            );
        } else {
            assert!(
                (a - b).abs() < 1e-6,
                "{id}: {a} became {b} with a cooler off"
            );
        }
    }
    // A cooler runs at its minimum (3 of 4) or above: asking for 2 gets 3.
    let two = levels(&PowerInput {
        levels: vec![("item:hardpoint_cooler_01".into(), 2)],
        ..Default::default()
    });
    let c1 = two
        .iter()
        .find(|(id, _)| id == "item:hardpoint_cooler_01")
        .unwrap()
        .1;
    assert!((c1 - 3.0).abs() < 1e-6, "cooler at 2 gave {c1}");

    // The settings replay on power-on, lowerings first: the weapons keep
    // the pips the radar gave up whatever order the two were set in (the
    // store moves a column it sets to the end, and touching the radar used
    // to take a pip back from the weapons).
    let radar_then_guns = levels(&PowerInput {
        levels: vec![
            ("item:hardpoint_radar".into(), 1),
            ("pool:WeaponGun".into(), 4),
        ],
        ..Default::default()
    });
    let guns_then_radar = levels(&PowerInput {
        levels: vec![
            ("pool:WeaponGun".into(), 4),
            ("item:hardpoint_radar".into(), 1),
        ],
        ..Default::default()
    });
    assert_eq!(
        radar_then_guns, guns_then_radar,
        "the order of the settings changed the split"
    );
    let guns = guns_then_radar
        .iter()
        .find(|(id, _)| id == "pool:WeaponGun")
        .unwrap()
        .1;
    assert!(
        (guns - 4.0).abs() < 1e-6,
        "the weapons got {guns} of 4 after the radar gave up its pips"
    );

    let v = view(&cooked, &input, &PowerInput::default()).unwrap();
    // erkul's default ("weapons first", the weapons pool on all 4 pips;
    // sc-vehicles' weapons_probe): each CF-557 sustains 341.7, each turret
    // CF-337 152.3 (the Mk V's capacitor buff). Power-on gives the pool 2,
    // and supply is all used, so 2 radar pips go to the weapons first.
    let erkul = PowerInput {
        levels: vec![
            ("item:hardpoint_radar".into(), 4),
            ("pool:WeaponGun".into(), 4),
        ],
        ..Default::default()
    };
    let w = view(&cooked, &input, &erkul).unwrap();
    for g in &w.guns {
        eprintln!(
            "  gun {:<60} power {:.3} burst {:?} sustained {:?}",
            g.path, g.power, g.burst, g.sustained
        );
    }
    assert_eq!(w.guns.len(), 12, "pilot 4 + turrets 8");
    let pilot = w
        .guns
        .iter()
        .find(|g| g.path.starts_with("hardpoint_gun_s5"))
        .unwrap();
    assert!((pilot.sustained.unwrap() - 341.7).abs() < 0.5, "{pilot:?}");
    let turret = w
        .guns
        .iter()
        .find(|g| g.path.starts_with("hardpoint_turret"))
        .unwrap();
    assert!(
        (turret.sustained.unwrap() - 152.3).abs() < 0.5,
        "{turret:?}"
    );
    eprintln!(
        "\naim assist {:?}, cross-section {:?}",
        v.summary.aim_assist, v.summary.cross_section
    );
    let s = &v.summary;
    eprintln!(
        "armor {:?}\nshield {:?}\ninsurance {:?}\ndims {:?} hull mass {:?} fuse {} component {}",
        s.armor,
        s.shield,
        s.insurance,
        s.hull.dimensions,
        s.hull.hull_mass,
        s.hull.fuse_multiplier,
        s.hull.component_multiplier
    );
    for p in &s.hull.parts {
        eprintln!(
            "{}{} {} {}",
            "  ".repeat(p.depth as usize),
            p.name,
            p.hp,
            if p.breakable { "breaks" } else { "" }
        );
    }
    if let Some(p) = &v.power {
        eprintln!("sources {:?}", p.sources);
    }
    // spviewer: 25,740 HP, deflection 531 physical / 380 energy, the hull
    // takes 70% / 50%, the armour 81% / 121%; 170,500 HP over the parts,
    // the body 45,000; a quadrant shield; CS 11,320 front, 8,350 side,
    // 25,430 top.
    eprintln!(
        "crew {:?}\nmissiles {:?}\nshield pips {:?} ratio {}",
        s.crew,
        s.missiles,
        s.shield.as_ref().and_then(|x| x.pips),
        s.shield.as_ref().map_or(0.0, |x| x.ratio)
    );
    for d in &s.hull.doors {
        eprintln!("door {} x{} {}", d.name, d.count, d.hp);
    }
    for t in &s.hull.thrusters {
        eprintln!("thruster {} {} {}", t.kind, t.place, t.hp);
    }
    // spviewer: the body is the vital part; airlock 13,500, cargo lift
    // 8,000; 4 main thrusters of 20,000; missiles armed 4, 4 s apart.
    let vital: Vec<_> = s.hull.parts.iter().filter(|p| p.vital).collect();
    assert_eq!(vital.len(), 1, "{vital:?}");
    assert_eq!(vital[0].hp, 45_000.0);
    assert!(s.hull.doors.iter().any(|d| d.hp == 13_500.0));
    assert_eq!(
        s.hull.thrusters.iter().filter(|t| t.kind == "Main").count(),
        4
    );
    // Boost (sc-holotable's component_sheets_probe; spviewer: regen
    // 26.7 s at 6/6, deplete 20 s; pitch / yaw +50 %, roll +100 %).
    eprintln!("boost {:?}", s.boost);
    assert_eq!(s.boost.len(), 7);
    assert!(
        (s.boost[2].regen_s - 41.10).abs() < 0.05,
        "{:?}",
        s.boost[2]
    );
    assert!((s.boost[6].regen_s - 27.77).abs() < 0.05);
    assert_eq!(s.boost[6].deplete_s.map(f32::round), Some(20.0));
    assert!((s.boost[6].turn[0] - 1.5).abs() < 0.01 && (s.boost[6].turn[2] - 2.0).abs() < 0.01);
    let m = s.missiles.as_ref().expect("missiles");
    assert_eq!((m.max_armed, m.cooldown), (Some(4), Some(4.0)));
    assert!(s.crew.pilot == 1 && s.crew.crew >= 3, "{:?}", s.crew);
    let armor = s.armor.as_ref().expect("armour");
    assert_eq!(armor.health, Some(25_740.0));
    let defl = armor.deflection.unwrap();
    assert_eq!((defl.physical, defl.energy), (531.0, 380.0));
    let parts_hp: f32 = s.hull.parts.iter().map(|p| p.hp).sum();
    assert_eq!(parts_hp, s.hull_health);
    assert_eq!(s.hull.parts[0].hp, 45_000.0);
    assert_eq!(s.shield.as_ref().unwrap().face.as_deref(), Some("Quadrant"));
    assert_eq!(s.cross_section.map(|c| c[2]), Some(25_430.0));
    // Swapping the quantum drive keeps the jump module (carry-over).
    let qd = candidates(
        &cooked,
        &input,
        &PowerInput::default(),
        &[vec!["hardpoint_quantum_drive".into()]],
    )
    .unwrap();
    let other = qd.iter().find(|c| !c.fitted).expect("another S2 drive");
    let swapped = LoadoutInput {
        ship: input.ship.clone(),
        edits: vec![mod_ships::Edit {
            path: vec!["hardpoint_quantum_drive".into()],
            item: Some(other.item.clone()),
        }],
    };
    let sv = view(&cooked, &swapped, &PowerInput::default()).unwrap();
    let drive = sv
        .slots
        .iter()
        .find(|s| s.path == ["hardpoint_quantum_drive"])
        .unwrap();
    eprintln!(
        "{} ({:?}, {:?}) holds {:?}",
        other.name,
        other.class,
        other
            .stats
            .iter()
            .find(|s| s.key == "qspeed")
            .map(|s| s.value),
        drive
            .children
            .iter()
            .map(|c| c.name.clone())
            .collect::<Vec<_>>()
    );
    assert!(
        drive
            .children
            .iter()
            .any(|c| c.item_type.as_deref() == Some("JumpDrive"))
    );
    let life = tree
        .iter()
        .find(|s| s.path == ["hardpoint_life_support"])
        .unwrap();
    assert!(!life.editable, "life support is fixed (sc-holotable v0.29)");
    eprintln!("\nsummary: {:?}", v.summary.shield_health);
    eprintln!(
        "hull {} armour {:?}",
        v.summary.hull_health, v.summary.armor_health
    );
    if let Some(f) = &v.summary.flight {
        eprintln!(
            "flight: SCM {} boost {}/{} NAV {} G {:?}",
            f.scm_speed, f.boost_speed_forward, f.boost_speed_backward, f.max_speed, f.acceleration
        );
    }
    if let Some(q) = &v.summary.quantum {
        eprintln!(
            "quantum: {} m/s, {:.3} SCU/Gm, tank {:.1}, range {:?} Gm, 10 Gm {:.1} s",
            q.speed, q.fuel_per_gm, q.fuel_capacity, q.range_gm, q.times[20].1
        );
    }
    assert_eq!(v.summary.shield_health, Some(200_000.0));

    // The picker: S2 coolers for cooler 1, then S5 guns for one pilot gimbal.
    let t = Instant::now();
    let coolers = candidates(
        &cooked,
        &input,
        &PowerInput::default(),
        &[vec!["hardpoint_cooler_01".into()]],
    )
    .unwrap();
    eprintln!("\n{} coolers in {:?}", coolers.len(), t.elapsed());
    for c in &coolers {
        eprintln!(
            "  {:<28} S{} G{} {}{} cooling {:?}",
            c.name,
            c.size,
            c.grade,
            c.stats
                .iter()
                .map(|s| format!("{}={} ", s.key, s.value))
                .collect::<String>(),
            if c.fitted { " FITTED" } else { "" },
            c.impact.cooling.map(|x| (x * 100.0).round())
        );
    }
    assert!(coolers.iter().any(|c| c.fitted && c.name == "Snowfall"));
    let gun = vec![
        "hardpoint_gun_s5_front_left".to_string(),
        "hardpoint_class_2".to_string(),
    ];
    let guns = candidates(&cooked, &input, &PowerInput::default(), &[gun]).unwrap();
    eprintln!("\n{} S5 guns", guns.len());
    for c in guns.iter().take(12) {
        eprintln!(
            "  {:<34} S{} {}{}",
            c.name,
            c.size,
            c.stats
                .iter()
                .map(|s| format!("{}={:.1} ", s.key, s.value))
                .collect::<String>(),
            if c.fitted { " FITTED" } else { "" }
        );
    }
    assert!(guns.iter().any(|c| c.fitted && c.name.contains("CF-557")));

    // Switching cooler 2 off: half the coolant.
    let off = PowerInput {
        off: vec!["hardpoint_cooler_02".into()],
        ..Default::default()
    };
    let p = view(&cooked, &input, &off).unwrap().power.unwrap();
    eprintln!(
        "\ncooler 2 off: coolant {:.1}, cooling {:?}",
        p.coolant_made, p.cooling
    );
    assert!((p.coolant_made - 41.0).abs() < 1e-6);
    // Weapons to 4 pips.
    let more = PowerInput {
        levels: vec![("pool:WeaponGun".into(), 4)],
        ..Default::default()
    };
    let p = view(&cooked, &input, &more).unwrap().power.unwrap();
    let w = p.columns.iter().find(|c| c.id == "pool:WeaponGun").unwrap();
    eprintln!(
        "weapons set to 4: level {:.2}, used {:.2} of {}",
        w.level, p.used, p.supply
    );
    assert!(p.used <= p.supply as f64 + 1e-6);
}

/// How many listed hulls find a store picture by name. Needs the store list
/// svc-shipstore's live test leaves in the temp dir.
#[test]
#[ignore = "needs a real Data.p4k and svc-shipstore's live store list"]
fn store_names_join() {
    let path = std::env::temp_dir().join("starlume-shipstore-live.json");
    let raw: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).expect("run svc-shipstore's live test first"))
            .unwrap();
    let store: Vec<(u32, String)> = raw["ships"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            (
                s["id"].as_u64().unwrap() as u32,
                s["name"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    let refs: Vec<(u32, &str)> = store.iter().map(|(i, n)| (*i, n.as_str())).collect();
    let cooked = cooked();
    let rows = ships(&cooked);
    let matched = mod_ships::match_store(&rows, &refs);
    let bases: Vec<_> = rows
        .iter()
        .filter(|r| r.base.is_none() && !r.ground)
        .collect();
    let missing: Vec<_> = bases
        .iter()
        .filter(|r| !matched.contains_key(&r.guid))
        .collect();
    eprintln!(
        "{} of {} hulls matched ({} of {} base hulls)",
        matched.len(),
        rows.len(),
        bases.len() - missing.len(),
        bases.len()
    );
    for r in &missing {
        eprintln!("  no picture: {} ({})", r.name, r.record_name);
    }
    let used: std::collections::HashSet<u32> = matched.values().copied().collect();
    for (id, name) in &store {
        if !used.contains(id) {
            eprintln!("  store unused: {id} {name}");
        }
    }
}

/// The places quantum travel goes and the default routes, with the
/// Centaurus's stock drive (sc-holotable's waypoints_probe: Hurston –
/// Crusader 31.924 Gm, Crusader – microTech 57.481 Gm).
#[test]
#[ignore = "needs a real Data.p4k"]
fn quantum_routes() {
    let cooked = cooked();
    let p = mod_ships::places(&cooked);
    let mut by_kind = std::collections::BTreeMap::new();
    for x in &p.places {
        *by_kind
            .entry((x.system.clone(), x.kind.clone()))
            .or_insert(0) += 1;
    }
    eprintln!("{} places: {by_kind:?}", p.places.len());
    let find = |g: &str| p.places.iter().find(|x| x.guid == g).unwrap();
    let d = |a: &mod_ships::Place, b: &mod_ships::Place| {
        (0..3)
            .map(|i| (a.position[i] - b.position[i]).powi(2))
            .sum::<f64>()
            .sqrt()
    };
    for (a, b) in &p.routes {
        let (a, b) = (find(a), find(b));
        eprintln!(
            "{}: {} -> {} {:.3} Gm (travel {:.3})",
            a.system,
            a.name,
            b.name,
            d(a, b) / 1e9,
            (d(a, b) - b.arrival) / 1e9
        );
    }
    let named = |n: &str| {
        p.places
            .iter()
            .find(|x| x.name == n)
            .unwrap_or_else(|| panic!("{n}"))
    };
    assert!((d(named("Hurston"), named("Crusader")) / 1e9 - 31.924).abs() < 0.001);

    let (planet, legs) = mod_ships::routes::moon_tour(&p).expect("a planet with moons");
    eprintln!(
        "moon tour: {planet}, legs Gm {:?}",
        legs.iter()
            .map(|l| (l / 1e7).round() / 100.0)
            .collect::<Vec<_>>()
    );
    assert!(legs.len() >= 2);
}

/// The Nova tank's S5 "Slayer Cannon" requires the `TMBL_Nova` tag, which
/// only the Nova offers: sc-vehicles' fit rule keeps it off a Centaurus
/// pilot gimbal (sc-holotable, LIVE 4.10 DCB).
#[test]
#[ignore = "needs a real Data.p4k"]
fn slayer_not_a_centaurus_gun() {
    use svc_data::ships::fits;
    let cooked = cooked();
    let index = &cooked.ships;
    let items = cooked.holotable.items.as_ref().unwrap();
    let paths = cooked.holotable.paths.as_ref().unwrap();
    let ship: svc_data::Guid = ships(&cooked)
        .iter()
        .find(|r| r.record_name == "RSI_Constellation_Mk5_Centaurus")
        .expect("centaurus")
        .guid
        .parse()
        .unwrap();
    let slayer = paths
        .iter()
        .find(|p| {
            p.name
                .to_ascii_lowercase()
                .contains("hrst_nova_ballisticcannon_s5")
        })
        .map(|p| p.guid)
        .expect("slayer record");
    let item = items.get(&slayer).expect("slayer item");
    eprintln!(
        "slayer tags {:?} required {:?}",
        item.tags, item.required_tags
    );
    let loadout = Loadout::stock(&index.vehicles, &ship).expect("stock");
    let port = ["hardpoint_gun_s5_body_right", "hardpoint_class_2"];
    let (chain, holders) = loadout
        .chain(&port, &index.vehicles, &index.containers)
        .expect("chain");
    let vehicle = index.vehicles.get(&ship).unwrap();
    let offered = vehicle.offered_tags(items, &chain, &holders);
    eprintln!("offered {offered:?}");
    eprintln!("fits {:?}", fits(chain.last().unwrap(), item, &offered));
    let options = loadout.candidates(&port, items, &index.vehicles, &index.containers);
    assert!(
        !options.contains(&slayer),
        "the Slayer Cannon is offered for the Centaurus S5 gun"
    );
}

/// How UEX's ships join ours: every UEX vehicle that is sold or rented
/// should be reached by some ship row, by GUID or by one of its names
/// (`ShipRow::names`). Reads the dev app's UEX cache (`starlume-dev/uex`),
/// so the app must have fetched it once. 2026-10-23: 170 of 174; the
/// misses are the ATLS suits (not ships), "Nova Tank" (ours: "Tumbril
/// Nova") and the CSV-SM, whose GUID UEX gives another record.
#[test]
#[ignore = "needs a real Data.p4k and the dev UEX cache"]
fn uex_ships_join() {
    let cooked = cooked();
    let dir = svc_uex::dir_in(&app_kit::app_data_root());
    let snap = svc_uex::UexService::new(dir).snapshot();
    assert!(
        !snap.vehicles.is_empty(),
        "no UEX cache; run the app online once"
    );
    let ix = svc_uex::Index::new(&snap);
    let mut reached = std::collections::HashSet::new();
    for r in ships(&cooked) {
        let names = r.names();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        if let Some(id) = ix.vehicle_id(&r.guid, &names) {
            reached.insert(id);
        }
    }
    let sold: std::collections::BTreeSet<u32> = snap
        .purchases
        .iter()
        .chain(&snap.rentals)
        .map(|p| p.id_vehicle)
        .collect();
    let missed: Vec<String> = sold
        .iter()
        .filter(|id| !reached.contains(id))
        .filter_map(|id| snap.vehicles.iter().find(|v| v.id == *id))
        .map(|v| {
            format!(
                "{} ({})",
                v.name_full,
                v.uuid.as_deref().unwrap_or("no uuid")
            )
        })
        .collect();
    eprintln!(
        "UEX ships sold or rented: {}, reached: {}",
        sold.len(),
        sold.len() - missed.len()
    );
    for m in &missed {
        eprintln!("  missed: {m}");
    }
    assert!(
        missed.len() * 20 <= sold.len(),
        "more than 5 % of UEX's ships unmatched"
    );
}

/// Your ships' insured and live loadouts from the dev app's inventory
/// cache, as edits on stock: they must apply (the view builds) and give
/// back the loadout the inventory describes. 2026-10-23: 35 owned ships,
/// 25 with a saved (insured) loadout, 11 with a live instance.
#[test]
#[ignore = "needs a real Data.p4k and the dev inventory cache"]
fn owned_loadouts_apply() {
    let cooked = cooked();
    let path = app_kit::app_data_root()
        .join("dossier")
        .join("inventory.json");
    let inv: svc_dossier::Inventory = app_kit::load_json(&path);
    assert!(
        !inv.entitlements.is_empty(),
        "no inventory cache; refresh My Items once"
    );
    let power = PowerInput {
        mode: Mode::Scm,
        off: vec![],
        levels: vec![],
    };
    let (mut ships_seen, mut insured, mut live) = (0, 0, 0);
    for row in ships(&cooked) {
        let owned = mod_ships::owned_loadouts(&cooked, &inv, &row.guid).unwrap();
        for o in &owned {
            ships_seen += 1;
            if !o.insured.is_empty() {
                insured += 1;
            }
            let lv = o.live.as_ref();
            if lv.is_some() {
                live += 1;
            }
            for edits in [Some(&o.insured), lv].into_iter().flatten() {
                let input = LoadoutInput {
                    ship: row.guid.clone(),
                    edits: edits.clone(),
                };
                view(&cooked, &input, &power).unwrap_or_else(|e| panic!("{}: {e:?}", row.name));
            }
            eprintln!(
                "{:40} insured {:>3} edits | live {}",
                row.name,
                o.insured.len(),
                lv.map_or("-".into(), |l| format!("{} edits", l.len()))
            );
            if row.name.contains("Super Hornet") {
                for e in lv.into_iter().flatten().take(8) {
                    let items = cooked.holotable.items.as_ref().unwrap();
                    let name = e
                        .item
                        .as_ref()
                        .and_then(|g| items.get(&g.parse().unwrap())?.display_name(&cooked.locale));
                    eprintln!("    live {:?} -> {:?}", e.path, name);
                }
            }
        }
    }
    eprintln!("owned {ships_seen}, insured changed {insured}, live {live}");
    assert!(ships_seen > 0);
}
