//! Read the scan panel on every screenshot in a folder with the app's
//! reader and write the readings as JSON (for the annotation sheets).
//!
//! `STARLUME_SCAN_BIN` is the reader's learned data (not in the repo:
//! `tools/hud-ocr/genbin.py` writes it to the dev data dir).
//!
//! ```bash
//! STARLUME_REAL_P4K="C:\Games\StarCitizen\LIVE\Data.p4k" \
//! STARLUME_SCAN_BIN="$APPDATA/starlume-dev/mining/scan.bin" \
//!   cargo run -p mod-mining --release --example scan_read -- "D:\Recordings\Gaming" readings.json
//! ```

use std::collections::BTreeMap;
use std::path::Path;

use mod_mining::{Catalog, LaserInput, LoadoutInput, NameSet, read_scan};
use svc_capture::Frame;
use svc_data::{DataService, InstallRef};

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args.next().expect("screenshot folder");
    let out = args.next().unwrap_or_else(|| "readings.json".into());
    let bin = std::env::var("STARLUME_SCAN_BIN").expect("STARLUME_SCAN_BIN");
    assert_eq!(
        mod_mining::load_scan_data(Path::new(&bin)),
        Ok(true),
        "no scan.bin at {bin}"
    );
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
    // The names the game shows, rendered in its HUD font from the install.
    let p4k = std::path::PathBuf::from(std::env::var("STARLUME_REAL_P4K").unwrap());
    let movie =
        svc_data::read_p4k_file(&p4k, "ui/fonts/shared/fonts_en.gfx").expect("fonts_en.gfx");
    let font = svc_capture::gfxfont::find_font(&movie, "Blender Pro Bold").expect("font");
    let (locale, source) = svc_data::active_locale(&p4k).expect("locale");
    eprintln!("localisation: {source:?}");
    let names = NameSet::build(&font, &catalog, &locale);
    eprintln!("{} names rendered", names.len());
    let name_of = |id: &str| {
        catalog
            .element_names
            .iter()
            .find(|e| e.id == id)
            .map(|e| e.name.clone())
    };
    // The captures' ship: a Golem (Pitman, Focus III + Rieger-C3).
    let find = |items: &[mod_mining::Named<mod_mining::LaserItem>], n: &str| {
        items
            .iter()
            .find(|i| i.name.contains(n))
            .map(|i| i.id.clone())
    };
    let module = |n: &str| {
        catalog
            .modules
            .iter()
            .find(|m| m.name.contains(n))
            .map(|m| m.id.clone())
    };
    let loadout = LoadoutInput {
        lasers: vec![LaserInput {
            laser: find(&catalog.lasers, "Pitman").expect("Pitman"),
            modules: vec![
                module("Focus III").expect("Focus III"),
                module("Rieger-C3").expect("Rieger-C3"),
            ],
            owner: 0,
            who: None,
            crew: None,
            distance_m: None,
        }],
        gadget: None,
        lead: None,
    };

    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect("folder")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("png")))
        .collect();
    files.sort();
    let mut all = BTreeMap::new();
    for path in files {
        let frame = Frame::open(&path).expect("image");
        let t = std::time::Instant::now();
        let r = read_scan(&frame, &catalog, &names, Some(&loadout));
        let key = path.file_stem().unwrap().to_string_lossy().to_string();
        eprintln!("{key}: {:?} in {:?}", r.panel.map(|p| p.score), t.elapsed());
        let mut v = serde_json::to_value(&r).expect("json");
        v["ms"] = serde_json::json!(t.elapsed().as_millis());
        if let Some(rows) = v["rows"].as_array_mut() {
            for row in rows {
                let id = row["element"].as_str().map(str::to_string);
                row["element_name"] = serde_json::json!(id.as_deref().and_then(name_of));
            }
        }
        if let Some(spot) = r.panel {
            // the panel crop as read (tilted if the reader tilted the frame)
            let crop = mod_mining::panel_crop(&frame, &spot);
            let png = Path::new(&out).with_file_name(format!("{key}.panel.png"));
            std::fs::write(&png, crop.to_png()).expect("crop");
            v["panel_png"] = serde_json::json!(png.display().to_string());
        }
        all.insert(key, v);
    }
    std::fs::write(Path::new(&out), serde_json::to_string_pretty(&all).unwrap()).expect("write");
    eprintln!("wrote {out}");
}
