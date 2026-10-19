//! Why the mass band flips between easy and manageable: along the mass
//! axis, the balance throttle against the 1 % setting grid (how far the
//! gentlest rise and fall sit from it) beside the simulated loss-free share,
//! at the band's 24 runs and at 400.
//!
//! ```bash
//! STARLUME_REAL_P4K="C:\Games\StarCitizen\LIVE\Data.p4k" \
//!   cargo run -p mod-mining --release --example band_probe
//! ```

use mod_mining::Catalog;
use mod_mining::player::{CLEAN_DANGER, Human, Plan};
use svc_data::mining::{
    Entry, Laser, ModifiedRock, Outcome, Pool, Rock, Setup, SimConfig, simulate,
};
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
    let rs = &catalog.ruleset;
    let el = |n: &str| {
        catalog
            .element_names
            .iter()
            .find(|e| e.name.contains(n))
            .expect(n)
            .value
    };
    let entries = vec![
        Entry {
            element: el("Laranite"),
            fraction: 0.0727,
            quality: Some(510),
        },
        Entry {
            element: el("Laranite"),
            fraction: 0.7305,
            quality: Some(298),
        },
        Entry {
            element: el("Tungsten"),
            fraction: 0.0706,
            quality: Some(530),
        },
    ];
    let pitman = catalog
        .lasers
        .iter()
        .find(|l| l.name.contains("Pitman"))
        .expect("Pitman");
    let module = |n: &str| {
        catalog
            .modules
            .iter()
            .find(|m| m.name.contains(n))
            .expect(n)
            .value
            .clone()
    };
    let mut laser = Laser::new(pitman.value.stats.clone());
    laser.modules = vec![module("Focus III"), module("Rieger-C3")];
    let setup = Setup {
        lasers: vec![laser],
        gadgets: vec![],
        activations: vec![],
    };

    let share = |m: &ModifiedRock, plan: &Plan, runs: u64| {
        let mut clean = 0;
        for seed in 0..runs {
            let config = SimConfig {
                seed,
                record: false,
                ..SimConfig::default()
            };
            let mut h = Human::following(seed, plan.clone());
            if let Outcome::Fractured {
                pool: Pool::Controlled,
                danger,
                ..
            } = simulate(m, rs, &mut h, &config).outcome
                && danger <= CLEAN_DANGER
            {
                clean += 1;
            }
        }
        clean as f32 * 100.0 / runs as f32
    };

    println!("  mass t   balance  rise-b  b-fall  margin   24 runs  400 runs");
    let mut mass = 4000.0;
    while mass <= 14000.0 {
        let rock = Rock::from_mass(rs, &catalog.elements, &entries, mass).expect("rock");
        let m = ModifiedRock::new(rs, &rock, &setup);
        let off = mod_mining::player::beam_off(rs, &rock, &setup);
        let g = mod_mining::throttle_guide(&m, &off, rs);
        if let (Some(b), false) = (g.balance, g.too_much_power) {
            let plan = Plan::for_rock(&m, &off, rs);
            println!(
                "  {:>6.0}   {:>6.2}  {:>6.2}  {:>6.2}  {:>6.3}   {:>6.0}  {:>8.1}",
                mass,
                b * 100.0,
                (plan.keep_low - b) * 100.0,
                (b - plan.keep_high) * 100.0,
                g.keep_margin.unwrap_or(f32::NAN),
                share(&m, &plan, 24),
                share(&m, &plan, 400)
            );
        }
        mass += 250.0;
    }
}
