//! Ignored-by-default check against real game data and in-game captures:
//! the rocks scanned from a Golem (Pitman, Focus III + Rieger-C3, no gadget)
//! on 2026-10-17 at ArcCorp's moons, with what the scan panel read. The
//! solver's scanner prediction must match the panel, and the guide for
//! each rock is printed for comparison with the recordings.
//!
//! ```text
//! STARLUME_REAL_P4K="C:\Games\StarCitizen\LIVE\Data.p4k" \
//!   cargo test -p mod-mining --release --test real_captures -- --ignored --nocapture
//! ```

use mod_mining::{Catalog, EntryInput, LaserInput, LoadoutInput, Named, RockInput, solve};
use svc_data::{DataService, InstallRef};

/// The one item whose name is `name`, else the one containing it.
fn id<T>(items: &[Named<T>], what: &str, name: &str) -> String {
    if let Some(exact) = items.iter().find(|i| i.name == name) {
        return exact.id.clone();
    }
    let hits: Vec<_> = items.iter().filter(|i| i.name.contains(name)).collect();
    match hits.as_slice() {
        [one] => one.id.clone(),
        _ => panic!(
            "{what} '{name}': {:?}",
            hits.iter().map(|h| &h.name).collect::<Vec<_>>()
        ),
    }
}

/// A capture: mass, composition rows (element, %, quality) and the panel's
/// resistance (whole percent) and instability.
struct Capture {
    shot: &'static str,
    mass: f32,
    rows: &'static [(&'static str, f32, u16)],
    resist_pct: f32,
    instab: f32,
    /// The panel showed the beam-on values (tinted red); white is the bare rock.
    beam: bool,
    label: &'static str,
}

const CAPTURES: &[Capture] = &[
    Capture {
        shot: "03-50-54",
        mass: 44047.0,
        rows: &[
            ("Tin (Ore)", 7.96, 537),
            ("Copper (Ore)", 6.73, 593),
            ("Copper (Ore)", 52.68, 359),
        ],
        resist_pct: 0.0,
        instab: 56.44,
        beam: true,
        label: "IMPOSSIBLE",
    },
    Capture {
        shot: "03-51-51",
        mass: 13481.0,
        rows: &[
            ("Tin (Ore)", 8.92, 537),
            ("Copper (Ore)", 10.22, 593),
            ("Copper (Ore)", 57.39, 359),
        ],
        resist_pct: 0.0,
        instab: 17.64,
        beam: true,
        label: "EASY",
    },
    Capture {
        shot: "03-56-40",
        mass: 40742.0,
        rows: &[
            ("Tin (Ore)", 5.82, 340),
            ("Copper (Ore)", 7.19, 742),
            ("Copper (Ore)", 58.43, 359),
        ],
        resist_pct: 0.0,
        instab: 40.14,
        beam: false,
        label: "IMPOSSIBLE",
    },
    Capture {
        shot: "03-57-06",
        mass: 13176.0,
        rows: &[
            ("Tin (Ore)", 6.84, 537),
            ("Copper (Ore)", 11.50, 593),
            ("Copper (Ore)", 72.34, 359),
        ],
        resist_pct: 0.0,
        instab: 18.19,
        beam: true,
        label: "EASY",
    },
    Capture {
        shot: "03-59-01",
        mass: 3440.0,
        rows: &[("Copper (Ore)", 100.0, 359)],
        resist_pct: 0.0,
        instab: 5.05,
        beam: true,
        label: "EASY",
    },
    Capture {
        shot: "04-01-05",
        mass: 2147.0,
        rows: &[
            ("Tin (Ore)", 6.20, 537),
            ("Copper (Ore)", 8.11, 652),
            ("Copper (Ore)", 55.46, 359),
        ],
        resist_pct: 0.0,
        instab: 2.83,
        beam: true,
        label: "EASY",
    },
    Capture {
        shot: "04-04-03",
        mass: 6393.0,
        rows: &[
            ("Laranite (Raw)", 7.27, 510),
            ("Laranite (Raw)", 73.05, 298),
            ("Tungsten (Ore)", 7.06, 530),
        ],
        resist_pct: 54.0,
        instab: 50.47,
        beam: true,
        label: "EASY",
    },
];

#[test]
#[ignore = "needs a real Data.p4k — set STARLUME_REAL_P4K"]
fn captured_rocks_from_a_golem() {
    let p4k =
        std::env::var("STARLUME_REAL_P4K").expect("set STARLUME_REAL_P4K to a real Data.p4k path");
    let install = InstallRef {
        channel_key: "real-test".into(),
        p4k_path: p4k.into(),
        build_id: "real-test-build".into(),
        version: "real-test".into(),
    };
    let service = DataService::new(std::env::temp_dir().join("starlume-svcdata-real-test"));
    let cooked = service.load(&install, |_| {}).expect("load real install");
    let catalog = Catalog::build(&cooked.mining, &cooked.locale).expect("ship ruleset");

    let loadout = LoadoutInput {
        lasers: vec![LaserInput {
            laser: id(&catalog.lasers, "laser", "Pitman"),
            modules: vec![
                id(&catalog.modules, "module", "Focus III"),
                id(&catalog.modules, "module", "Rieger-C3"),
            ],
            owner: 0,
            who: None,
            crew: None,
            distance_m: None,
            off: false,
        }],
        gadget: None,
        lead: None,
    };

    let mut misses = Vec::new();
    for c in CAPTURES {
        let rock = RockInput {
            surface: None,
            mass: c.mass,
            entries: c
                .rows
                .iter()
                .map(|(name, percent, q)| EntryInput {
                    element: id(&catalog.element_names, "element", name),
                    percent: *percent,
                    quality: Some(*q),
                })
                .collect(),
        };
        let report = solve(&catalog, &loadout, &rock, &[]).expect("solve");
        let y = report.yours.as_ref().expect("a laser");
        let g = &y.guide;
        eprintln!(
            "{} {:>6.0} t | panel R {:>2.0} % I {:>6.2} {:<10} | bare R {:>4.1} % I {:>6.2} | beam on R {:>2.0} % I {:>6.2} | margin {:>4.0} % | {:?}",
            c.shot,
            c.mass,
            c.resist_pct,
            c.instab,
            c.label,
            report.rock.resistance * 100.0,
            report.rock.instability,
            y.scanner_resistance_pct,
            y.scanner_instability,
            y.eval.margin * 100.0,
            y.eval.difficulty.map(|d| (
                (d.controlled * 100.0).round(),
                (d.overcharged * 100.0).round(),
                d.median_time_s.map(f32::round)
            )),
        );
        eprintln!(
            "    guide: min {:.0} %, balance {:?}, hold {:?}, safe climb {:?}, full until {:?}, window {:.2}–{:.2}, too much power {}, category {:?}",
            g.throttle_minimum * 100.0,
            g.balance.map(|b| (b * 100.0).round()),
            g.hold_throttle.map(|t| (t * 100.0).round()),
            g.safe_climb.map(|t| (t * 100.0).round()),
            g.full_until.map(|t| (t * 100.0).round()),
            g.window_low,
            g.window_high,
            g.too_much_power,
            g.category,
        );
        // The linear part of the model (no wave): drain at the minimum is
        // decay − gain(minimum); balance = decay / gain per unit throttle.
        if let (Some(b), Some(drain)) = (g.balance, g.drain_at_minimum) {
            let k = drain / (b - g.throttle_minimum);
            eprintln!(
                "    model: gain {:.2} %/s per throttle, decay {:.2} %/s, time to window {:?} s",
                k * 100.0,
                k * b * 100.0,
                g.time_to_window_s.map(f32::round)
            );
        }
        eprintln!(
            "    climb: {:?}",
            g.climb
                .iter()
                .map(|s| (
                    (s.until_charge * 100.0).round(),
                    (s.throttle * 100.0).round()
                ))
                .collect::<Vec<_>>()
        );
        // Beam on, the panel reads the modified rock (tinted); bare, the
        // rock alone. Allow the panel's rounding.
        let (r, i) = if c.beam {
            (y.scanner_resistance_pct, y.scanner_instability)
        } else {
            (
                (report.rock.resistance * 100.0).floor(),
                report.rock.instability,
            )
        };
        if (r - c.resist_pct).abs() > 1.0 || (i - c.instab).abs() > 0.01 * c.instab.max(1.0) {
            misses.push(c.shot);
        }
    }
    assert!(misses.is_empty(), "scanner prediction off for {misses:?}");

    // The Laranite was worked from just past the Pitman's 40 m optimal
    // range: the recording's falling stretches put its balance near 82 %
    // (the model at full power says 61 %). At 41.3 m the falloff leaves
    // about 75 % of the power.
    let laranite = CAPTURES.iter().find(|c| c.shot == "04-04-03").unwrap();
    let rock = |c: &Capture| RockInput {
        surface: None,
        mass: c.mass,
        entries: c
            .rows
            .iter()
            .map(|(name, percent, q)| EntryInput {
                element: id(&catalog.element_names, "element", name),
                percent: *percent,
                quality: Some(*q),
            })
            .collect(),
    };
    let mut far = loadout.clone();
    far.lasers[0].distance_m = Some(41.3);
    let report = solve(&catalog, &far, &rock(laranite), &[]).expect("solve");
    let y = report.yours.as_ref().unwrap();
    let range = y.range.as_ref().expect("range");
    eprintln!(
        "Laranite at 41.3 m: balance {:?}, power share {:.2}, range {:.0}-{:.0} m",
        y.guide.balance, range.power_share, range.full_m, range.zero_m
    );
    let b = y.guide.balance.expect("balance");
    assert!((0.76..=0.88).contains(&b), "balance {b}");

    // The fracture blast (holotable's reference: 40.8 m and ~19.5 physical
    // on a clean crack for this rock; Valerie saw the HUD's stripes).
    let blast = y.blast;
    eprintln!("Laranite blast: {blast:?}");
    assert!(
        (blast.clean_max_m - 40.8).abs() < 0.5,
        "clean blast {}",
        blast.clean_max_m
    );

    // The small copper rock is too much power within optimal range; backing
    // off past it brings the balance back into the Pitman's range.
    // Copper/tin rocks make no blast on a clean crack (no stripes in game).
    let copper = CAPTURES.iter().find(|c| c.shot == "03-57-06").unwrap();
    let report = solve(&catalog, &loadout, &rock(copper), &[]).expect("solve");
    let cb = report.yours.as_ref().unwrap().blast;
    eprintln!("copper blast: {cb:?}");
    assert_eq!(cb.clean_max_m, 0.0, "copper clean blast");

    let small = CAPTURES.iter().find(|c| c.shot == "04-01-05").unwrap();
    let report = solve(&catalog, &loadout, &rock(small), &[]).expect("solve");
    let back = report
        .yours
        .as_ref()
        .unwrap()
        .range
        .as_ref()
        .unwrap()
        .back_off;
    eprintln!("small copper: back off {back:?}");
    let back = back.expect("a back-off distance");
    let mut there = loadout.clone();
    there.lasers[0].distance_m = Some(back.best_m);
    let g = solve(&catalog, &there, &rock(small), &[])
        .unwrap()
        .yours
        .unwrap()
        .guide;
    eprintln!(
        "  at {} m: balance {:?}, too much power {}",
        back.best_m, g.balance, g.too_much_power
    );
    assert!(!g.too_much_power);
}
