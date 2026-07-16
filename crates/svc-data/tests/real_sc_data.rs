//! Ignored-by-default smoke test against a real Star Citizen install.
//!
//! Verifies on live data what the unit tests can't: that the `jurisdiction`
//! pool is actually populated under svc-data's feature set (the legality
//! index depends on it), that the weapons cook materializes real entities,
//! and that mission loc keys / crimestat flags come out non-trivial.
//!
//! Run (dev machine only):
//! ```text
//! STARLUME_REAL_P4K="C:\Games\StarCitizen\LIVE\Data.p4k" \
//!   cargo test -p svc-data --test real_sc_data -- --ignored --nocapture
//! ```
//!
//! (Not named `real_install` — an exe with "install" in its name trips
//! Windows' installer-elevation heuristic, os error 740. Same bug that
//! renamed svc-install → svc-discovery.)
//!
//! Caches under `%TEMP%\starlume-svcdata-real-test` so re-runs skip the p4k
//! read (tier 2). The processed snapshot is deleted before each run so the
//! current cook code always executes (a cached `foundations.cook` from a
//! previous run of the *same* rev would otherwise short-circuit it).

use svc_data::{CrimestatRisk, DataService, InstallRef, LegalityKind};

#[test]
#[ignore = "needs a real Data.p4k — set STARLUME_REAL_P4K"]
fn cook_from_real_install() {
    let p4k =
        std::env::var("STARLUME_REAL_P4K").expect("set STARLUME_REAL_P4K to a real Data.p4k path");
    let cache_root = std::env::temp_dir().join("starlume-svcdata-real-test");
    let channel_key = "real-test";
    // Force the cook to run: drop the processed snapshot, keep the extract
    // snapshot (tier 2 — skips the p4k read on re-runs).
    let _ = std::fs::remove_file(cache_root.join(channel_key).join("foundations.cook"));

    let install = InstallRef {
        channel_key: channel_key.into(),
        p4k_path: p4k.into(),
        build_id: "real-test-build".into(),
        version: "real-test".into(),
    };

    let service = DataService::new(cache_root);
    let cooked = service
        .load(&install, |stage| eprintln!("stage: {stage:?}"))
        .expect("load real install");

    // ── Legality (the jurisdiction-pool spike) ─────────────────────────
    let drugs = cooked
        .legality
        .iter()
        .filter(|e| e.kind == LegalityKind::Drug)
        .count();
    let contraband = cooked.legality.len() - drugs;
    eprintln!(
        "legality: {} entries ({drugs} drugs, {contraband} contraband)",
        cooked.legality.len()
    );
    for e in cooked.legality.iter().take(5) {
        eprintln!(
            "  {:?} {} → {} (illegal in {} jurisdictions)",
            e.kind,
            e.record_name,
            e.name_key,
            e.jurisdictions.len()
        );
    }
    assert!(
        drugs >= 3,
        "expected several controlled substances (WiDoW, E'tam, …); jurisdiction pool empty?"
    );
    assert!(contraband >= 1, "expected at least one prohibited good");
    assert!(
        cooked
            .legality
            .iter()
            .all(|e| !e.name_key.is_empty() && !e.jurisdictions.is_empty())
    );

    // ── Weapons ────────────────────────────────────────────────────────
    let w = &cooked.weapons;
    let tracked = w.missiles.iter().filter(|m| m.tracking.is_some()).count();
    eprintln!(
        "weapons: {} ship weapons, {} missiles ({} with tracking)",
        w.ship_weapons.len(),
        w.missiles.len(),
        tracked
    );
    assert!(w.ship_weapons.len() > 100, "ship-weapon cook came up short");
    // 4.8 LIVE materializes 45 distinct missiles/torpedoes.
    assert!(w.missiles.len() > 20, "missile cook came up short");
    assert!(tracked > 0, "no guided missiles materialized");
    assert!(
        w.ship_weapons
            .iter()
            .any(|s| s.damage.is_some() && s.name_key.is_some()),
        "no ship weapon with damage + loc key"
    );

    // ── Mission loc keys + facts ───────────────────────────────────────
    let with_title_key = cooked
        .missions
        .iter()
        .filter(|m| m.title_key.is_some())
        .count();
    let risky = cooked
        .missions
        .iter()
        .filter(|m| m.facts.crimestat != CrimestatRisk::None)
        .count();
    eprintln!(
        "missions: {} entries, {} with title_key, {} with crimestat risk",
        cooked.missions.len(),
        with_title_key,
        risky
    );
    assert!(with_title_key > 0, "no mission retained its title key");
    assert!(
        risky > 0,
        "no mission classified as crimestat-risky (DontHarm walk broken?)"
    );
}
