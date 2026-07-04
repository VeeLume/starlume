//! End-to-end svc-data verification against the real SC install on this
//! machine. `#[ignore]`d: needs a local Star Citizen installation and takes
//! ~30-60s cold. Run explicitly:
//!
//! ```text
//! cargo test -p starlume --test real_data_load -- --ignored --nocapture
//! ```
//!
//! Uses a temp cache dir — never touches the app's real cache.

use std::sync::Arc;

#[test]
#[ignore = "needs a real SC install; ~30-60s cold parse"]
fn full_waterfall_against_real_install() {
    let scan = svc_discovery::scan().expect("install scan");
    let Some(info) = scan.installs.first() else {
        eprintln!("no SC install found — skipping");
        return;
    };
    // Mirror the shell's staleness-key fallback (CIG ships "None" build ids
    // on current Live builds — see src/data.rs::staleness_key).
    let build_id = if info.build_id.is_empty() || info.build_id == "None" {
        info.version.clone()
    } else {
        info.build_id.clone()
    };
    let install = svc_data::InstallRef {
        channel_key: info.channel.to_ascii_lowercase(),
        p4k_path: std::path::PathBuf::from(&info.directory).join("Data.p4k"),
        build_id,
        version: info.version.clone(),
    };
    eprintln!(
        "install: {} {} (build {})",
        info.channel, info.version, info.build_id
    );

    let cache_root = tempfile::tempdir().expect("tempdir");
    let service = svc_data::DataService::new(cache_root.path().to_path_buf());

    // Cold load: full live parse; both snapshots get written.
    let t = std::time::Instant::now();
    let start = std::time::Instant::now();
    let cooked = service
        .load(&install, move |stage| {
            eprintln!("  [{:>6.1}s] {stage:?}", start.elapsed().as_secs_f32())
        })
        .expect("cold load");
    eprintln!(
        "cold load: {:.1}s — {} items, {} resources, {} missions",
        t.elapsed().as_secs_f32(),
        cooked.item_count(),
        cooked.resource_count(),
        cooked.mission_count()
    );
    assert!(cooked.item_count() > 1000, "expected a real item corpus");
    assert!(
        cooked.resource_count() > 50,
        "expected a real resource catalog"
    );

    // Mission catalog: pooled templates with resolved text + rewards.
    assert!(
        cooked.mission_count() > 100,
        "expected a real mission catalog"
    );
    let titled = cooked.missions.iter().filter(|m| m.title.is_some()).count();
    let with_payout = cooked
        .missions
        .iter()
        .filter(|m| m.payout.fixed.is_some() || m.payout.estimate.is_some())
        .count();
    let with_bp = cooked
        .missions
        .iter()
        .filter(|m| !m.blueprint_rewards.is_empty())
        .count();
    let with_loc = cooked
        .missions
        .iter()
        .filter(|m| !m.locations.is_empty())
        .count();
    eprintln!(
        "missions: {titled} titled, {with_payout} with payout, {with_bp} with BP pools, {with_loc} with locations"
    );
    assert!(titled > 50, "locale-resolved mission titles missing");
    assert!(with_payout > 50, "payout estimation produced nothing");
    assert!(with_bp > 0, "no blueprint-pool rewards resolved");
    assert!(with_loc > 50, "locality aggregation produced nothing");
    if let Some(m) = cooked
        .missions
        .iter()
        .find(|m| m.title.is_some() && !m.blueprint_rewards.is_empty())
    {
        eprintln!(
            "  sample: {} — {} pools, payout {:?}/{:?}",
            m.title.as_deref().unwrap_or(&m.debug_name),
            m.blueprint_rewards.len(),
            m.payout.fixed,
            m.payout.estimate
        );
    }

    // Locale must resolve: search something that exists in every SC build.
    let page = cooked.search_items(&svc_data::ItemQuery {
        text: "arrow".into(),
        limit: 5,
        ..Default::default()
    });
    eprintln!("search 'arrow': {} matches", page.total);
    assert!(page.total > 0, "locale-resolved search returned nothing");
    for row in &page.rows {
        eprintln!("  {} [{} / {}]", row.name, row.item_type, row.item_sub_type);
    }

    // Snapshot files exist and report their sizes.
    let dir = cache_root.path().join(&install.channel_key);
    for name in ["foundations.cook", "extract.snap"] {
        let size = std::fs::metadata(dir.join(name)).expect(name).len();
        eprintln!("{name}: {:.1} MB", size as f64 / 1e6);
    }

    // Evict, then tier-1 reload must be fast and never parse.
    service.evict();
    let t = std::time::Instant::now();
    let reloaded: Arc<_> = service
        .get_or_reload_fast(&install)
        .expect("tier-1 reload after evict");
    let reload_s = t.elapsed().as_secs_f32();
    eprintln!("tier-1 reload after evict: {reload_s:.2}s");
    assert_eq!(reloaded.item_count(), cooked.item_count());
    assert_eq!(reloaded.mission_count(), cooked.mission_count());
    // Measured 5.7s in a debug build (release is ~1s — msgpack decode + the
    // Items by_crc index rebuild dominate); this is a regression tripwire,
    // not a performance target.
    assert!(reload_s < 20.0, "processed-snapshot reload should be fast");
}
