//! Print the calculator's element names (id and in-game name), one per
//! line, for the scan reader's name templates.
//!
//! ```bash
//! STARLUME_REAL_P4K="C:\Games\StarCitizen\LIVE\Data.p4k" \
//!   cargo run -p mod-mining --release --example catalog_names
//! ```

use mod_mining::Catalog;
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
    for e in &catalog.element_names {
        println!("{}\t{}", e.id, e.name);
    }
}
