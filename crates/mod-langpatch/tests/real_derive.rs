//! Ignored-by-default end-to-end test against real game data: derive every
//! built-in patcher from the cooked snapshot, run the full apply pipeline,
//! and write into a **temp fake install dir** (the real install is never
//! touched — that's the app's job, behind its write gates).
//!
//! Needs the svc-data real-data cache from `real_sc_data` (run that test
//! first) plus the p4k for the base INI:
//! ```text
//! STARLUME_REAL_P4K="C:\Games\StarCitizen\LIVE\Data.p4k" \
//!   cargo test -p mod-langpatch --test real_derive -- --ignored --nocapture
//! ```

use std::collections::HashMap;

use mod_langpatch::{LangpatchConfig, PatchOp, builtin_patchers, derive_ops, merge};
use svc_data::{DataService, InstallRef};

#[test]
#[ignore = "needs real game data — set STARLUME_REAL_P4K and run svc-data's real_sc_data first"]
fn derive_and_apply_from_real_data() {
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
        .get_or_reload_fast(&install)
        .expect("processed snapshot from real_sc_data run present");

    // ── Derive all built-ins ────────────────────────────────────────────
    let work_dir = tempfile::tempdir().unwrap();
    let config = LangpatchConfig::default();
    let patchers = builtin_patchers();
    let ops = derive_ops(
        work_dir.path(),
        "real-test",
        &install.build_id,
        Some(&cooked),
        &config,
        &patchers,
        None,
    )
    .expect("derive");

    for p in &ops {
        eprintln!(
            "patcher {}: {} renames, {} patches",
            p.id,
            p.ops.renames.len(),
            p.ops.patches.len()
        );
    }
    let count = |id: &str| {
        ops.iter()
            .find(|p| p.id == id)
            .map(|p| p.ops.patches.len())
            .unwrap_or(0)
    };
    assert!(
        count("component_grades") > 100,
        "component grades came up short"
    );
    assert!(count("illegal_goods") > 10, "illegal goods came up short");
    assert!(
        count("weapon_enhancer") > 100,
        "weapon enhancer came up short"
    );
    assert!(count("label_fixes") > 0, "label fixes empty");

    // ── Apply into a fake install dir ───────────────────────────────────
    let base = svc_data::read_base_global_ini(&install.p4k_path).expect("base global.ini");
    let mut lines = merge::decode_ini(&base).expect("decode base INI");
    let line_count_before = lines.len();

    let renames: Vec<_> = ops
        .iter()
        .flat_map(|p| p.ops.renames.iter().cloned())
        .collect();
    merge::apply_renames(&mut lines, &renames);
    let mut patch_map: HashMap<String, Vec<PatchOp>> = HashMap::new();
    for p in &ops {
        for (key, op) in &p.ops.patches {
            patch_map.entry(key.clone()).or_default().push(op.clone());
        }
    }
    let stats = merge::apply_patches(&mut lines, &patch_map);
    eprintln!(
        "apply: {} lines patched, {} placeholder-skipped, {} missing (of {} keys, {} base lines)",
        stats.patched_lines,
        stats.skipped_placeholders,
        stats.missing_keys,
        patch_map.len(),
        line_count_before
    );
    assert!(stats.patched_lines > 200, "suspiciously few lines patched");
    assert!(
        stats.missing_keys < patch_map.len() / 10,
        "more than 10% of patch keys missed the base INI"
    );

    let fake_install = tempfile::tempdir().unwrap();
    let bytes = merge::encode_utf8_bom(&lines);
    merge::write_patch(fake_install.path(), &bytes).expect("write patch");

    let written = std::fs::read(merge::override_path(fake_install.path())).unwrap();
    let reparsed = merge::parse_ini(&merge::decode_ini(&written).unwrap());
    // Spot-checks: an illegal good got its marker, some weapon description
    // got a stats block.
    assert!(
        reparsed.values().any(|v| v.contains("Illegal in:")),
        "no 'Illegal in:' block in output"
    );
    assert!(
        reparsed.values().any(|v| v.contains("Weapon Stats")),
        "no weapon stats block in output"
    );
    // Mission enhancer: info + region blocks on descriptions, at least one
    // pooled key that diverged into a variants section, and title tags.
    assert!(
        reparsed.values().any(|v| v.contains("Mission Info")),
        "no mission info block in output"
    );
    assert!(
        reparsed.values().any(|v| v.contains("Available at")),
        "no region block in output"
    );
    assert!(
        reparsed.values().any(|v| v.contains("Variants (")),
        "no variants section in output"
    );
    assert!(
        reparsed
            .values()
            .any(|v| v.ends_with("[Solo]") || v.contains("[Solo] [")),
        "no [Solo] title tag in output"
    );
    assert!(
        std::fs::read_to_string(fake_install.path().join("user.cfg"))
            .unwrap()
            .contains("g_language"),
        "user.cfg not upserted"
    );

    // Removal restores vanilla.
    assert!(merge::remove_patch(fake_install.path()).unwrap());
    assert!(!merge::override_path(fake_install.path()).exists());
}

/// Eyeball helper, not an assertion suite: prints a handful of the mission
/// enhancer's rendered patches (title tags, a variants section, an
/// encounter block) so a human can sanity-check the player-facing text.
/// Same setup as [`derive_and_apply_from_real_data`].
#[test]
#[ignore = "needs real game data — set STARLUME_REAL_P4K and run svc-data's real_sc_data first"]
fn dump_mission_enhancer_samples() {
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
        .get_or_reload_fast(&install)
        .expect("processed snapshot from real_sc_data run present");

    let mfrs = cooked.manufacturers();
    eprintln!("manufacturers: {} rows", mfrs.len());
    for m in mfrs.iter().take(8) {
        eprintln!("  {} → {:?}", m.code, m.name);
    }
    let with_enc = cooked.missions.iter().find(|m| !m.encounters.is_empty());
    if let Some(m) = with_enc {
        let slot = &m.encounters[0].waves[0].ships[0];
        eprintln!("sample raw slot ships: {:?}", slot.ships);
    }

    let work_dir = tempfile::tempdir().unwrap();
    let config = LangpatchConfig::default();
    let patchers: Vec<_> = builtin_patchers()
        .into_iter()
        .filter(|p| p.id() == "mission_enhancer")
        .collect();
    let ops = derive_ops(
        work_dir.path(),
        "real-test",
        &install.build_id,
        Some(&cooked),
        &config,
        &patchers,
        None,
    )
    .expect("derive");
    let patches = &ops[0].ops.patches;

    let dump = |label: &str, pred: &dyn Fn(&str, &str) -> bool, n: usize| {
        eprintln!("\n════ {label} ════");
        for (key, op) in patches
            .iter()
            .filter(|(k, op)| {
                let PatchOp::Suffix(v) = op else { return false };
                pred(k, v)
            })
            .take(n)
        {
            let PatchOp::Suffix(v) = op else { continue };
            eprintln!("--- {key}\n{}", v.replace("\\n", "\n"));
        }
    };

    dump(
        "title tags",
        &|k, _| k.contains("title") || !k.contains("desc"),
        12,
    );
    dump("variants sections", &|_, v| v.contains("Variants ("), 3);
    dump("encounter blocks", &|_, v| v.contains("Encounters"), 3);
    dump(
        "blueprint blocks",
        &|_, v| v.contains("Potential Blueprints"),
        2,
    );
}
