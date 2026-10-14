//! Ignored-by-default check of the mission preview's showcase pick against
//! real cooked data: every option the picks show, how long the pick takes,
//! and which missions come out.
//!
//! Reads a build-keyed cache (`<root>/builds/<changelist>/`, e.g. a copy of
//! the dev app's `%APPDATA%\starlume-dev\cache`) — point it at a copy, since
//! loading stamps the build's `last_used`:
//! ```text
//! STARLUME_REAL_CACHE="C:\tmp\cache" STARLUME_REAL_VERSION="4.10.2-live.12913470" \
//!   cargo test -p mod-langpatch --test real_showcase -- --ignored --nocapture
//! ```

use std::time::Instant;

use mod_langpatch::builtin_patchers;
use mod_langpatch::preview::mission_showcases;
use svc_data::{DataService, InstallRef};

#[test]
#[ignore = "needs a real cooked build — set STARLUME_REAL_CACHE and STARLUME_REAL_VERSION"]
fn showcases_cover_every_mission_option() {
    let root = std::env::var("STARLUME_REAL_CACHE").expect("set STARLUME_REAL_CACHE");
    let version = std::env::var("STARLUME_REAL_VERSION").expect("set STARLUME_REAL_VERSION");
    let install = InstallRef {
        channel_key: "live".into(),
        p4k_path: Default::default(),
        build_id: version.clone(),
        version,
    };
    let cooked = DataService::new(root.into())
        .get_or_reload_fast(&install)
        .expect("a current cooked snapshot under STARLUME_REAL_CACHE");

    let patchers = builtin_patchers();
    let missions = patchers
        .iter()
        .find(|p| p.id() == "mission_enhancer")
        .expect("mission enhancer");

    let started = Instant::now();
    let picks = mission_showcases(&cooked, missions.as_ref(), None).expect("showcases");
    eprintln!("showcase pick: {:?}", started.elapsed());

    for pick in &picks {
        let title_key = pick.id.split('|').next().unwrap_or("");
        let title = cooked.locale.resolve(title_key).unwrap_or_default();
        eprintln!("{title:?}  [{}]  ({})", pick.options.join(", "), pick.id);
    }
    assert!(!picks.is_empty(), "no mission shows any option");

    // Without an owned set the owned-blueprint options show nowhere; every
    // other option must land in some pick.
    let shown: Vec<&str> = picks
        .iter()
        .flat_map(|p| p.options.iter().map(String::as_str))
        .collect();
    for option in missions.options() {
        if option.id.starts_with("owned_") {
            continue;
        }
        assert!(
            shown.contains(&option.id.as_str()),
            "no pick shows {}",
            option.id
        );
    }
}
