//! Ignored-by-default check against real game data: cook (or reuse) the
//! snapshot, build the catalog from it, and solve the Bexalite rock scanned
//! on 2026-10-16 from a Golem (Focus III + Rieger-C3, Sabir on). The scanner
//! read resistance 30 % and instability 440.59 with the beam on.
//!
//! Shares svc-data's real-data cache; a cook-rev bump recooks from the
//! extract snapshot (tier 2):
//! ```text
//! STARLUME_REAL_P4K="C:\Games\StarCitizen\LIVE\Data.p4k" \
//!   cargo test -p mod-mining --test real_solve -- --ignored --nocapture
//! ```

use std::time::Instant;

use mod_mining::{Catalog, EntryInput, LaserInput, LoadoutInput, Named, RockInput, solve};
use svc_data::{DataService, InstallRef};

fn id<T>(items: &[Named<T>], what: &str, name: &str) -> String {
    let hits: Vec<_> = items.iter().filter(|i| i.name.contains(name)).collect();
    match hits.as_slice() {
        [one] => one.id.clone(),
        _ => panic!(
            "{what} '{name}': {:?}",
            hits.iter().map(|h| &h.name).collect::<Vec<_>>()
        ),
    }
}

#[test]
#[ignore = "needs a real Data.p4k — set STARLUME_REAL_P4K"]
fn bexalite_scan_from_a_golem() {
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

    let catalog = Catalog::build(&cooked.mining, &cooked.locale).expect("ship ruleset");
    eprintln!(
        "catalog: {} lasers, {} modules, {} gadgets, {} elements",
        catalog.lasers.len(),
        catalog.modules.len(),
        catalog.gadgets.len(),
        catalog.element_names.len()
    );
    for l in &catalog.lasers {
        eprintln!("  laser {} (S{}) [{}]", l.name, l.value.size, l.record_name);
    }
    assert!(catalog.lasers.len() >= 10, "ship lasers missing");
    assert!(!catalog.modules.is_empty() && !catalog.gadgets.is_empty());

    let bexalite = id(&catalog.element_names, "element", "Bexalite");
    let loadout = LoadoutInput {
        lasers: vec![LaserInput {
            laser: id(&catalog.lasers, "laser", "Pitman"),
            modules: vec![
                id(&catalog.modules, "module", "Focus III"),
                id(&catalog.modules, "module", "Rieger-C3"),
            ],
            owner: 0,
            who: None,
        }],
        gadget: Some(id(&catalog.gadgets, "gadget", "Sabir")),
    };
    let entry = |element: String, percent, quality| EntryInput {
        element,
        percent,
        quality,
    };
    let rock = RockInput {
        mass: 20998.0,
        entries: vec![
            entry(bexalite.clone(), 6.22, Some(597)),
            entry(bexalite, 43.65, Some(302)),
            entry(id(&catalog.element_names, "element", "Gold"), 2.49, None),
            entry(id(&catalog.element_names, "element", "Borase"), 4.13, None),
        ],
    };

    let start = Instant::now();
    // A crew mate's Prospector (Arbor MH1, no modules) as support.
    let crew = [mod_mining::CrewShip {
        id: "juno".into(),
        owner: "Juno".into(),
        name: "Prospector".into(),
        loadout: LoadoutInput {
            lasers: vec![LaserInput {
                laser: id(&catalog.lasers, "laser", "Arbor MH1"),
                modules: vec![],
                owner: 0,
                who: None,
            }],
            gadget: None,
        },
    }];
    let report = solve(&catalog, &loadout, &rock, &crew).expect("solve");
    let juno = report.crew.first().expect("the crew ship is offered");
    eprintln!(
        "crew: {} {} margin {:.0} %",
        juno.owner,
        juno.name,
        juno.eval.margin * 100.0
    );
    assert!(juno.eval.margin > report.yours.as_ref().unwrap().eval.margin);
    eprintln!("solve: {} ms", start.elapsed().as_millis());

    let yours = report.yours.as_ref().expect("a laser was given");
    eprintln!(
        "rock {:.0} t {:.2} SCU | scanner R {} % I {:.2} | margin {:.0} % | {:?}",
        report.rock.mass,
        report.rock.scu,
        yours.scanner_resistance_pct,
        yours.scanner_instability,
        yours.eval.margin * 100.0,
        yours.eval.difficulty
    );
    for g in &report.gadgets {
        eprintln!(
            "  gadget {:?}: margin {:.0} %",
            g.gadget,
            g.eval.margin * 100.0
        );
    }
    for s in &report.support {
        eprintln!(
            "  support {} {:?}: margin {:.0} %",
            s.laser,
            s.modules,
            s.eval.margin * 100.0
        );
    }

    assert_eq!(yours.scanner_resistance_pct, 30.0);
    assert!(
        (yours.scanner_instability - 440.59).abs() < 0.5,
        "instability {}",
        yours.scanner_instability
    );

    check_guide_against_simulation(&catalog, &loadout, &rock);
    keep_on_versus_cut(&catalog, &loadout, &rock);

    // A small rock for a Golem: the laser is too strong to hold it even at
    // its minimum throttle (Valerie's 2,000 t test rock).
    let small = RockInput {
        mass: 2000.0,
        entries: vec![
            entry(
                id(&catalog.element_names, "element", "Laranite (Raw)"),
                51.07,
                None,
            ),
            entry(
                id(&catalog.element_names, "element", "Tungsten (Ore)"),
                31.09,
                None,
            ),
        ],
    };
    let golem = LoadoutInput {
        lasers: vec![LaserInput {
            modules: vec![
                id(&catalog.modules, "module", "Rieger-C3"),
                id(&catalog.modules, "module", "Focus III"),
            ],
            ..loadout.lasers[0].clone()
        }],
        gadget: loadout.gadget.clone(),
    };
    let g = solve(&catalog, &golem, &small, &[])
        .unwrap()
        .yours
        .unwrap()
        .guide;
    eprintln!("small rock: {g:#?}");
    assert!(
        g.too_much_power,
        "the Pitman should be too strong for 2,000 t"
    );
    assert_eq!(g.category, Some(mod_mining::Category::TooMuchPower));
    assert!(g.hold_throttle.is_none());
}

/// The throttle guide's peak jumps against the tick simulation: with a held
/// throttle, the most charge gained in any half of the longest wave period
/// (once the aim has settled) must stay under the predicted jump and come
/// close to it. On the crackable kit (Rieger-C3 ×2 + Sabir).
fn check_guide_against_simulation(catalog: &Catalog, loadout: &LoadoutInput, input: &RockInput) {
    use svc_data::mining::{Entry, Fixed, Laser, ModifiedRock, Rock, Setup, SimConfig, simulate};

    let rs = &catalog.ruleset;
    let mut laser = Laser::new(
        catalog
            .laser(&loadout.lasers[0].laser)
            .unwrap()
            .value
            .stats
            .clone(),
    );
    let rieger = catalog
        .module(&loadout.lasers[0].modules[1])
        .unwrap()
        .value
        .clone();
    laser.modules = vec![rieger.clone(), rieger];
    let gadget = catalog
        .gadget(loadout.gadget.as_ref().unwrap())
        .unwrap()
        .value
        .clone();
    let entries: Vec<Entry> = input
        .entries
        .iter()
        .map(|e| Entry {
            element: catalog.element(&e.element).unwrap().value,
            fraction: e.percent / 100.0,
            quality: e.quality,
        })
        .collect();
    // The scanned rock, and a lighter one of the same make with room
    // between the balance throttle and full.
    for mass in [input.mass, 12000.0] {
        let rock = Rock::from_mass(rs, &catalog.elements, &entries, mass).unwrap();
        let m = ModifiedRock::new(
            rs,
            &rock,
            &Setup {
                lasers: vec![laser.clone()],
                gadgets: vec![gadget.clone()],
            },
        );
        let off = ModifiedRock::new(
            rs,
            &rock,
            &Setup {
                lasers: vec![],
                gadgets: vec![gadget.clone()],
            },
        );
        // Even metering, as the simulation does: checks the wave arithmetic.
        let g = mod_mining::guide::throttle_guide_with(&m, &off, rs, 1.0);
        let real = mod_mining::throttle_guide(&m, &off, rs);
        eprintln!("{mass} t, with the hit queue: {real:#?}");
        if let (Some(d), Some(b)) = (real.decline, real.balance)
            && d > real.throttle_minimum
        {
            assert!(
                (d - b / mod_mining::guide::HIT_DRAIN).abs() < 0.01,
                "decline {d} vs balance {b}"
            );
        }
        eprintln!("{mass} t: {g:#?}");

        g.balance.expect("crackable");
        eprintln!(
            "ruleset: controlled fill {} / above {} / decay {}; danger fill {} exp {} decay {}; optimal {:?} catastrophic {:?}",
            rs.controlled_breaking_fill_rate,
            rs.controlled_breaking_fill_rate_danger,
            rs.controlled_breaking_decay_rate,
            rs.danger_breaking_fill_rate,
            rs.danger_breaking_fill_rate_exponent,
            rs.danger_breaking_decay_rate,
            m.optimal_rate,
            m.catastrophic_rate
        );
        let cases = [
            // Clamped at full throttle, the safe point is the full case.
            (
                "safe climb",
                g.safe_climb.filter(|t| *t < 1.0),
                Some(g.window_high - g.window_low),
            ),
            ("full", Some(1.0), Some(g.peak_jump_full)),
        ];
        let settle = rs.hit_consistency.history_window;
        let span = g.wave_period_max_s / 2.0;
        for (what, throttle, predicted) in cases {
            let (Some(throttle), Some(predicted)) = (throttle, predicted) else {
                continue;
            };
            if throttle > 1.0 {
                continue;
            }
            let mut observed = 0.0f32;
            for seed in 0..100 {
                let trace = simulate(
                    &m,
                    rs,
                    &mut Fixed(throttle),
                    &SimConfig {
                        seed,
                        ..SimConfig::default()
                    },
                );
                let s = &trace.samples;
                for i in 0..s.len() {
                    if s[i].time_s < settle {
                        continue;
                    }
                    for j in i + 1..s.len() {
                        if s[j].time_s - s[i].time_s > span || s[j].power >= 0.999 {
                            break;
                        }
                        observed = observed.max(s[j].power - s[i].power);
                    }
                }
            }
            eprintln!(
                "  {what} at {:.1} %: predicted {predicted:.4}, observed {observed:.4}",
                throttle * 100.0
            );
            assert!(
                observed <= predicted * 1.02,
                "{what}: the guide underestimates the wave"
            );
            if predicted < 0.9 {
                assert!(
                    observed >= predicted * 0.6,
                    "{what}: the guide overestimates the wave"
                );
            }
        }
    }
}

/// Where keeping the laser on beats cutting it, for every laser fitted with
/// the two modules that widen the window most (12,000 t of the scanned make).
fn keep_on_versus_cut(catalog: &Catalog, loadout: &LoadoutInput, input: &RockInput) {
    use svc_data::mining::{Entry, Laser, ModifiedRock, Rock, Setup};

    let rs = &catalog.ruleset;
    let entries: Vec<Entry> = input
        .entries
        .iter()
        .map(|e| Entry {
            element: catalog.element(&e.element).unwrap().value,
            fraction: e.percent / 100.0,
            quality: e.quality,
        })
        .collect();
    let rock = Rock::from_mass(rs, &catalog.elements, &entries, 12000.0).unwrap();
    let gadget = catalog
        .gadget(loadout.gadget.as_ref().unwrap())
        .unwrap()
        .value
        .clone();
    let window = |m: &svc_data::mining::MiningModifiers| {
        m.optimal_charge_window_size.map_or(0.0, |w| w.value)
    };
    let mut wide: Vec<_> = catalog
        .modules
        .iter()
        .filter(|m| !mod_mining::catalog::is_active(&m.value))
        .collect();
    wide.sort_by(|a, b| window(&b.value.modifiers).total_cmp(&window(&a.value.modifiers)));
    eprintln!(
        "widest passive modules: {}",
        wide.iter()
            .take(3)
            .map(|m| format!("{} {:+}", m.name, window(&m.value.modifiers)))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let off = ModifiedRock::new(
        rs,
        &rock,
        &Setup {
            lasers: vec![],
            gadgets: vec![gadget.clone()],
        },
    );
    for l in &catalog.lasers {
        let mut laser = Laser::new(l.value.stats.clone());
        laser.modules = wide
            .iter()
            .take(l.value.stats.module_slots as usize)
            .map(|m| m.value.clone())
            .collect();
        let m = ModifiedRock::new(
            rs,
            &rock,
            &Setup {
                lasers: vec![laser],
                gadgets: vec![gadget.clone()],
            },
        );
        let g = mod_mining::throttle_guide(&m, &off, rs);
        eprintln!(
            "  {:<32} laser window {:+.0} % | top {:.1} % on, {:.1} % off | keep on below {:?} | no return off {:?} min {:?}",
            l.name,
            window(&l.value.stats.modifiers),
            g.window_high * 100.0,
            g.window_high_off * 100.0,
            g.keep_on_below.map(|p| (p * 1000.0).round() / 10.0),
            g.no_return_off.map(|p| (p * 1000.0).round() / 10.0),
            g.no_return_min.map(|p| (p * 1000.0).round() / 10.0),
        );
    }
}
