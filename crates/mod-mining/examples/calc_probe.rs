//! Solve one rock against a loadout and print the guide, the crew plan and
//! both wave tables (everyone, and per operator), for checking the
//! calculator's arithmetic by hand.
//!
//! ```bash
//! STARLUME_REAL_P4K="C:\Games\StarCitizen\LIVE\Data.p4k" \
//!   cargo run -p mod-mining --release --example calc_probe
//! ```

use mod_mining::{Catalog, EntryInput, LaserInput, LoadoutInput, RockInput, solve};
use svc_data::{DataService, InstallRef};

fn main() {
    let install = InstallRef {
        channel_key: "real-test".into(),
        p4k_path: std::env::var("STARLUME_REAL_P4K")
            .expect("STARLUME_REAL_P4K")
            .into(),
        build_id: "real-test-build".into(),
        version: "real-test".into(),
    };
    let service = DataService::new(std::env::temp_dir().join("starlume-svcdata-real-test"));
    let cooked = service.load(&install, |_| {}).expect("load real install");
    let catalog = Catalog::build(&cooked.mining, &cooked.locale).expect("ship ruleset");
    let el = |n: &str| {
        catalog
            .element_names
            .iter()
            .find(|e| e.name == n)
            .expect(n)
            .id
            .clone()
    };
    let laser = catalog
        .lasers
        .iter()
        .find(|l| l.name.contains("Helix II"))
        .expect("Helix II")
        .id
        .clone();
    let rieger = catalog
        .modules
        .iter()
        .find(|m| m.name.contains("Rieger-C3"))
        .expect("Rieger-C3")
        .id
        .clone();
    let head = |owner: u32, who: Option<&str>| LaserInput {
        laser: laser.clone(),
        modules: vec![rieger.clone(); 3],
        owner,
        who: who.map(str::to_string),
        crew: None,
        distance_m: None,
        off: false,
    };
    // A MOLE-like three-head ship (one owner), as in the screenshot.
    let laranite = std::env::var("LARANITE").is_ok();
    let pitman = catalog
        .lasers
        .iter()
        .find(|l| l.name.contains("Pitman"))
        .expect("Pitman")
        .id
        .clone();
    let module = |n: &str| {
        catalog
            .modules
            .iter()
            .find(|m| m.name.contains(n))
            .expect(n)
            .id
            .clone()
    };
    let golem = LaserInput {
        laser: pitman,
        modules: vec![module("Focus III"), module("Rieger-C3")],
        owner: 0,
        who: None,
        crew: None,
        distance_m: None,
        off: false,
    };
    let loadout = if laranite {
        LoadoutInput {
            lasers: vec![golem],
            gadget: None,
            lead: None,
        }
    } else {
        LoadoutInput {
            lasers: vec![
                head(0, None),
                head(0, Some("Head 2")),
                head(0, Some("Head 3")),
            ],
            gadget: catalog
                .gadgets
                .iter()
                .find(|g| g.name.contains("Sabir"))
                .map(|g| g.id.clone()),
            lead: None,
        }
    };
    let rock = if laranite {
        RockInput {
            surface: None,
            mass: 6393.0,
            entries: vec![
                EntryInput {
                    element: el("Laranite (Raw)"),
                    percent: 7.27,
                    quality: Some(510),
                },
                EntryInput {
                    element: el("Laranite (Raw)"),
                    percent: 73.05,
                    quality: Some(298),
                },
                EntryInput {
                    element: el("Tungsten (Ore)"),
                    percent: 7.06,
                    quality: Some(530),
                },
            ],
        }
    } else {
        RockInput {
            surface: None,
            mass: 114_945.0,
            entries: vec![
                EntryInput {
                    element: el("Quantainium (Raw)"),
                    percent: 7.67,
                    quality: None,
                },
                EntryInput {
                    element: el("Quantainium (Raw)"),
                    percent: 74.23,
                    quality: None,
                },
                EntryInput {
                    element: el("Beryl (Raw)"),
                    percent: 8.24,
                    quality: None,
                },
            ],
        }
    };
    let r = solve(&catalog, &loadout, &rock, &[]).expect("solve");
    println!(
        "rock: R {:.3} I {:.2} window {:.3}..{:.3} scu {:.1}",
        r.rock.resistance, r.rock.instability, r.rock.window_low, r.rock.window_high, r.rock.scu
    );
    let y = r.yours.expect("lasers");
    println!(
        "eval: margin {:.3} power {:.0} need {:.0} R {:.3} I {:.2} balance {:?}",
        y.eval.margin,
        y.eval.power,
        y.eval.need,
        y.eval.resistance,
        y.eval.instability,
        y.eval.balance_throttle
    );
    println!(
        "lasers: {:?}",
        catalog
            .lasers
            .iter()
            .filter(|l| l.name.contains("Helix"))
            .map(|l| (&l.name, l.value.size))
            .collect::<Vec<_>>()
    );
    let g = &y.guide;
    println!(
        "window {:.3}..{:.3} ({:.1} %), balance {:?}, keep low {:?} high {:?} margin {:?}, unholdable {}",
        g.window_low,
        g.window_high,
        (g.window_high - g.window_low) * 100.0,
        g.balance,
        g.keep_low,
        g.keep_high,
        g.keep_margin,
        g.unholdable
    );
    if let Some(d) = &y.eval.difficulty {
        println!(
            "human player: {:.0} % clean, {:.0} % overcharged, {:.0} % timed out; narrow window {}",
            d.controlled * 100.0,
            d.overcharged * 100.0,
            d.timed_out * 100.0,
            y.eval.narrow_window
        );
    }
    // The human player under different hit metering.
    {
        use svc_data::mining::SimConfig;
        let rs = &catalog.ruleset;
        for interval in [1.0 / 30.0, 0.04, 2.0 / 30.0] {
            let cfg = SimConfig {
                hit_interval_s: interval,
                ..SimConfig::default()
            };
            if let Some((m, off)) = mod_mining::modified_for(&catalog, &loadout, &rock) {
                let d = mod_mining::player::difficulty(&m, &off, rs, &cfg, 60);
                println!(
                    "  hit every {:.3} s: {:.0} % clean, {:.0} % overcharged, mean danger {:.2}, in window {:.0} %",
                    interval,
                    d.controlled * 100.0,
                    d.overcharged * 100.0,
                    d.mean_danger,
                    d.in_window * 100.0
                );
            }
        }
    }
    if std::env::var("TRACE").is_ok()
        && let Some((m, off)) = mod_mining::modified_for(&catalog, &loadout, &rock)
    {
        use svc_data::mining::{SimConfig, simulate};
        // the first seed that ends overcharged (or not clean)
        let seed = (0u64..40).find(|&sd| {
            let mut h = mod_mining::player::Human::following(sd, mod_mining::player::Plan::for_rock(&m, &off, &catalog.ruleset));
            let tr = simulate(&m, &catalog.ruleset, &mut h, &SimConfig { seed: sd, record: false, ..SimConfig::default() });
            !matches!(tr.outcome, svc_data::mining::Outcome::Fractured { danger, .. } if danger <= mod_mining::player::CLEAN_DANGER)
        }).unwrap_or(3);
        let mut h = mod_mining::player::Human::following(
            seed,
            mod_mining::player::Plan::for_rock(&m, &off, &catalog.ruleset),
        );
        let tr = simulate(
            &m,
            &catalog.ruleset,
            &mut h,
            &SimConfig {
                seed,
                ..SimConfig::default()
            },
        );
        println!(
            "trace (window {:.2}..{:.2}, balance {:?}): outcome {:?}",
            m.window.low,
            m.window.high,
            m.balance_throttle(&catalog.ruleset),
            tr.outcome
        );
        for s in tr.samples.iter().step_by(15) {
            println!(
                "  t {:5.1} throttle {:.2} charge {:.3} danger {:.2} controlled {:.2}",
                s.time_s, s.throttle, s.power, s.danger, s.controlled
            );
        }
    }
    println!("lead guide jump rows:");
    for j in &g.jumps {
        println!(
            "  {:>5.2} {:<12} big {:.4} usual {:.4}",
            j.throttle,
            j.label.as_deref().unwrap_or(""),
            j.big,
            j.usual
        );
    }
    println!("everyone (total rows):");
    for t in &y.total {
        println!(
            "  {:<12} throttles {:?} big {:.4} usual {:.4} shares {:?}",
            t.label.as_deref().unwrap_or(""),
            t.throttles,
            t.big,
            t.usual,
            t.shares
        );
    }
    for o in &y.operators {
        println!("operator {} lead {} hold {:?}", o.index, o.lead, o.hold);
        for j in &o.jumps {
            println!(
                "    {:>5.2} {:<6} big {:.4} usual {:.4}",
                j.throttle,
                j.label.as_deref().unwrap_or(""),
                j.big,
                j.usual
            );
        }
    }
}
