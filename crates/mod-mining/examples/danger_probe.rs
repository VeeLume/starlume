//! How full the danger pool is when a rock cracks, and what that costs:
//! the fracture blast at a few danger levels, the danger at the cracks our
//! player model makes following the plan, and a sweep of the rise setting
//! (the window's lower half) against the guide's pick.
//!
//! ```bash
//! STARLUME_REAL_P4K="C:\Games\StarCitizen\LIVE\Data.p4k" \
//!   cargo run -p mod-mining --release --example danger_probe
//! ```

use mod_mining::player::{Environment, Human, Plan};
use mod_mining::{Catalog, throttle_guide};
use svc_data::mining::{
    Combined, Entry, Laser, ModifiedRock, Outcome, Pool, Rock, Ruleset, Setup, SimConfig, simulate,
};
use svc_data::{DataService, InstallRef};

const RUNS: u64 = 200;

struct Case {
    name: &'static str,
    mass: f32,
    entries: Vec<(&'static str, f32, Option<u16>)>,
    lasers: Vec<(&'static str, Vec<&'static str>, f32)>,
    gadget: Option<&'static str>,
    surface: bool,
}

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
    let rs = &catalog.ruleset;
    println!(
        "ruleset: dangerPoolFactor {} danger fill {} exp {} decay {} | controlled fill {} fill-above {} decay {} | defaultMass {} defaultVolume {}",
        rs.danger_pool_factor,
        rs.danger_breaking_fill_rate,
        rs.danger_breaking_fill_rate_exponent,
        rs.danger_breaking_decay_rate,
        rs.controlled_breaking_fill_rate,
        rs.controlled_breaking_fill_rate_danger,
        rs.controlled_breaking_decay_rate,
        rs.default_mass,
        rs.default_volume
    );

    let cases = [
        Case {
            name: "Laranite 6,393 t, Golem at 41 m (recording 04-04-06)",
            mass: 6393.0,
            entries: vec![
                ("Laranite", 7.27, Some(510)),
                ("Laranite", 73.05, Some(298)),
                ("Tungsten", 7.06, Some(530)),
            ],
            lasers: vec![("Pitman", vec!["Focus III", "Rieger-C3"], 41.0)],
            gadget: None,
            surface: true,
        },
        Case {
            name: "Laranite 1,500 t, Golem in range (too much power: pulsing)",
            mass: 1500.0,
            entries: vec![
                ("Laranite", 7.27, Some(510)),
                ("Laranite", 73.05, Some(298)),
                ("Tungsten", 7.06, Some(530)),
            ],
            lasers: vec![("Pitman", vec!["Focus III", "Rieger-C3"], 0.0)],
            gadget: None,
            surface: true,
        },
        Case {
            name: "Ouratite 2,745 t, Golem (pass-2 screenshot)",
            mass: 2745.0,
            entries: vec![
                ("Ouratite", 63.74, Some(310)),
                ("Ouratite", 3.53, Some(860)),
                ("Agricium", 7.85, Some(588)),
            ],
            lasers: vec![("Pitman", vec!["Focus III", "Focus III"], 0.0)],
            gadget: None,
            surface: true,
        },
        Case {
            name: "Quantainium 114,945 t, 3x Helix II (Rieger-C3 x3) + Sabir (rating note)",
            mass: 114_945.0,
            entries: vec![
                ("Quantainium", 7.67, None),
                ("Quantainium", 74.23, None),
                ("Beryl", 8.24, None),
            ],
            lasers: vec![
                ("Helix II", vec!["Rieger-C3"; 3], 0.0),
                ("Helix II", vec!["Rieger-C3"; 3], 0.0),
                ("Helix II", vec!["Rieger-C3"; 3], 0.0),
            ],
            gadget: Some("Sabir"),
            surface: false,
        },
    ];

    for case in &cases {
        println!("\n=== {}", case.name);
        let entries: Vec<Entry> = case
            .entries
            .iter()
            .map(|(n, pct, q)| Entry {
                element: catalog
                    .element_names
                    .iter()
                    .find(|e| e.name.contains(n))
                    .unwrap_or_else(|| panic!("element {n}"))
                    .value,
                fraction: pct / 100.0,
                quality: *q,
            })
            .collect();
        let rock = Rock::from_mass(rs, &catalog.elements, &entries, case.mass).expect("rock");
        let lasers: Vec<Laser> = case
            .lasers
            .iter()
            .map(|(name, mods, d)| {
                let item = catalog
                    .lasers
                    .iter()
                    .find(|l| l.name.contains(name))
                    .expect(name);
                let mut l = Laser::new(item.value.stats.clone());
                l.modules = mods
                    .iter()
                    .map(|m| {
                        catalog
                            .modules
                            .iter()
                            .find(|x| x.name.contains(m))
                            .expect(m)
                            .value
                            .clone()
                    })
                    .collect();
                l.distance_m = *d;
                l
            })
            .collect();
        let gadgets = case
            .gadget
            .map(|g| {
                vec![
                    catalog
                        .gadgets
                        .iter()
                        .find(|x| x.name.contains(g))
                        .expect(g)
                        .value
                        .clone(),
                ]
            })
            .unwrap_or_default();
        let setup = Setup {
            lasers,
            gadgets,
            activations: vec![],
        };
        let m = ModifiedRock::new(rs, &rock, &setup);
        let g = throttle_guide(&m, &mod_mining::player::beam_off(rs, &rock, &setup), rs);
        println!(
            "rock: V {:.0} m3, explosionMult {:.1}, window {:.3}..{:.3} ({:.1} %), I {:.2}, balance {:?}, breakable {}",
            rock.volume,
            rock.explosion_multiplier,
            m.window.low,
            m.window.high,
            m.window.size() * 100.0,
            m.instability,
            m.balance_throttle(rs),
            m.is_breakable(rs)
        );
        println!(
            "guide: keep low {:?} / high {:?}, margin {:?}, too much power {}, climb limit {:?}, window top beam off {:.3}, climb at minimum {:?}/s",
            g.keep_low,
            g.keep_high,
            g.keep_margin,
            g.too_much_power,
            g.safe_climb,
            g.window_high_off,
            g.climb_at_minimum
        );

        // The blast by danger at fracture.
        println!("  danger  max radius  full-damage radius  damage (phys)  vs clean");
        let clean = rock.fracture_blast(rs, m.shatter_damage, 0.0);
        for d in [0.0, 0.05, 0.10, 0.25, 0.5, 1.0] {
            let b = rock.fracture_blast(rs, m.shatter_damage, d);
            let rel = if clean.damage.physical > 0.0 {
                format!("x{:.2}", b.damage.physical / clean.damage.physical)
            } else {
                "from none".into()
            };
            println!(
                "  {:>5.2}  {:>8.1} m  {:>12.1} m  {:>14.1}  {}",
                d, b.radius_max, b.radius_min, b.damage.physical, rel
            );
        }
        let _ = Combined::NONE;

        if !m.is_breakable(rs) {
            println!("  (not breakable: no simulation)");
            continue;
        }

        // Danger at the crack, for our player model following the plan.
        let env = if case.surface {
            Environment::Surface
        } else {
            Environment::Space
        };
        let plan = Plan::from_guide(&g).in_environment(env);
        let mut buckets = [0u32; 6];
        let labels = ["0", "<0.05", "<0.10", "<0.25", "<1 (ctrl)", "overcharge"];
        let mut timed_out = 0;
        for seed in 0..RUNS {
            let config = SimConfig {
                seed,
                record: false,
                ..SimConfig::default()
            };
            let mut h = Human::following(seed, plan.clone());
            match simulate(&m, rs, &mut h, &config).outcome {
                Outcome::Fractured {
                    pool: Pool::Danger, ..
                } => buckets[5] += 1,
                Outcome::Fractured { danger, .. } => {
                    let k = if danger <= 0.0 {
                        0
                    } else if danger < 0.05 {
                        1
                    } else if danger < 0.10 {
                        2
                    } else if danger < 0.25 {
                        3
                    } else {
                        4
                    };
                    buckets[k] += 1;
                }
                Outcome::TimedOut => timed_out += 1,
            }
        }
        if std::env::var("TRACE").is_ok() && g.too_much_power {
            let seed = (0..RUNS)
                .find(|&sd| {
                    let mut h = Human::following(sd, plan.clone());
                    let c = SimConfig {
                        seed: sd,
                        record: false,
                        ..SimConfig::default()
                    };
                    matches!(
                        simulate(&m, rs, &mut h, &c).outcome,
                        Outcome::Fractured {
                            pool: Pool::Danger,
                            ..
                        }
                    )
                })
                .unwrap_or(0);
            let mut h = Human::following(seed, plan.clone());
            let tr = simulate(
                &m,
                rs,
                &mut h,
                &SimConfig {
                    seed,
                    ..SimConfig::default()
                },
            );
            println!(
                "  trace seed {seed}, pulse window {:?}: {:?}",
                plan.pulse_window, tr.outcome
            );
            let mut next = 0.0;
            for (s, l) in tr.samples.iter().zip(&tr.lasers) {
                if s.time_s >= next {
                    next += 0.25;
                    println!(
                        "    {:>6.2} s  set {:>4.2} thr {:>4.2} fire {:>5} contrib {:>5} queued {:>4.2}  power {:.3}  wave {:+.2}  ctrl {:.3}  danger {:.3}",
                        s.time_s,
                        l[0].setting,
                        l[0].throttle,
                        l[0].firing,
                        l[0].contributing,
                        l[0].queued_s,
                        s.power,
                        s.wave,
                        s.controlled,
                        s.danger
                    );
                }
            }
        }
        print!("  Human, danger at the crack ({RUNS} runs):");
        for (l, n) in labels.iter().zip(buckets) {
            print!("  {l}: {n}");
        }
        println!("  timed out: {timed_out}");

        // The rise setting: the guide's pick against the simulated player
        // flying each candidate (the fall setting as the guide has it).
        let Some(balance) = m.balance_throttle(rs) else {
            continue;
        };
        let step = mod_mining::player::STEP;
        let first = ((balance + step / 2.0) / step).ceil() * step;
        let gain = |t: f32| m.gain_rate(rs, t);
        let decay = gain(1.0) - m.net_rate(rs, 1.0);
        println!(
            "  rise-setting sweep (Human, fall at {:?}; room = half window - misjudgement = {:.3}):",
            plan.keep_high,
            m.window.size() / 2.0 - mod_mining::player::CHARGE_UNCERTAINTY
        );
        println!("    rise   no loss  d<.25  over  t/o  median s   big    usual  net*1s");
        for k in 0..8 {
            let t = first + k as f32 * step;
            if t > 1.0 + 1e-4 {
                break;
            }
            let p = Plan {
                keep_low: t,
                ..plan.clone()
            };
            let (mut clean, mut mild, mut over, mut to) = (0, 0, 0, 0);
            let mut times = vec![];
            for seed in 0..RUNS {
                let config = SimConfig {
                    seed,
                    record: false,
                    ..SimConfig::default()
                };
                let mut h = Human::following(seed, p.clone());
                match simulate(&m, rs, &mut h, &config).outcome {
                    Outcome::Fractured {
                        pool,
                        danger,
                        time_s,
                    } => {
                        times.push(time_s);
                        match pool {
                            Pool::Controlled if danger <= mod_mining::player::CLEAN_DANGER => {
                                clean += 1
                            }
                            Pool::Controlled if danger < 0.25 => mild += 1,
                            _ => over += 1,
                        }
                    }
                    Outcome::TimedOut => to += 1,
                }
            }
            times.sort_by(f32::total_cmp);
            let (big, usual) = mod_mining::guide::wave_lift(&m, rs, t);
            let pct = |n: i32| n as f32 * 100.0 / RUNS as f32;
            println!(
                "    {:>3.0} %{}  {:>5.0}  {:>5.0}  {:>4.0}  {:>3}  {:>8}  {:>6.3}  {:>6.3}  {:>6.4}",
                t * 100.0,
                if g.keep_low.is_some_and(|x| (x - t).abs() < 1e-3) {
                    "*"
                } else {
                    " "
                },
                pct(clean),
                pct(mild),
                pct(over),
                to,
                times
                    .get(times.len() / 2)
                    .map_or("-".into(), |v| format!("{v:.0}")),
                big,
                usual,
                (gain(t) - decay).max(0.0)
            );
        }
        let _: &Ruleset = rs;
    }
}
