//! Ignored-by-default probe of a real inventory snapshot against real cooked
//! data: how much of it names, what the ships look like, where things are.
//! Prints a report; asserts only that the files load.
//!
//! Point it at copies (loading stamps the build's `last_used`):
//! ```text
//! STARLUME_REAL_CACHE="C:\tmp\cache" STARLUME_REAL_VERSION="4.10.2-live.12913470" \
//! STARLUME_REAL_INVENTORY="C:\tmp\inventory.json" \
//!   cargo test -p mod-items --test real_inventory -- --ignored --nocapture
//! ```

use std::collections::BTreeMap;
use std::str::FromStr;

use svc_data::{CookedData, DataService, Guid, InstallRef, RecordCollection};
use svc_dossier::Inventory;
use svc_dossier::inventory::{ContextRecord, ItemRecord};

fn cooked() -> std::sync::Arc<CookedData> {
    let root = std::env::var("STARLUME_REAL_CACHE").expect("set STARLUME_REAL_CACHE");
    let version = std::env::var("STARLUME_REAL_VERSION").expect("set STARLUME_REAL_VERSION");
    let install = InstallRef {
        channel_key: "live".into(),
        p4k_path: Default::default(),
        build_id: version.clone(),
        version,
    };
    let data = DataService::new(root.into());
    // A cook-revision bump makes the cooked snapshot stale: re-cook from the
    // raw extract beside it (no p4k needed).
    data.get_or_reload_fast(&install)
        .or_else(|| data.load(&install, |_| {}).ok())
        .expect("a cooked snapshot or raw extract under STARLUME_REAL_CACHE")
}

fn walk<'a>(items: &'a [ItemRecord], depth: usize, out: &mut Vec<(usize, &'a ItemRecord)>) {
    for i in items {
        out.push((depth, i));
        walk(&i.attached, depth + 1, out);
        walk(&i.contents, depth + 1, out);
    }
}

#[test]
#[ignore = "needs real data — set STARLUME_REAL_CACHE, STARLUME_REAL_VERSION, STARLUME_REAL_INVENTORY"]
fn probe_real_inventory() {
    let path = std::env::var("STARLUME_REAL_INVENTORY").expect("set STARLUME_REAL_INVENTORY");
    let inv: Inventory = app_kit::load_json(std::path::Path::new(&path));
    assert!(inv.fetched_at != 0, "inventory.json missing or empty");
    let cooked = cooked();
    let items = cooked.holotable.items.as_ref().expect("items");
    let locations = cooked.holotable.locations.as_ref().expect("locations");
    let name = |crc: u32| {
        items.by_crc(crc).map(|i| {
            (
                i.display_name(&cooked.locale)
                    .unwrap_or("<no name>")
                    .to_string(),
                i.item_type.as_dcb_str().to_string(),
            )
        })
    };

    // ── Places ──────────────────────────────────────────────────────────
    eprintln!("== stashes");
    for s in &inv.stashes {
        let place = match s.location.context {
            ContextRecord::Location(crc) | ContextRecord::Hangar(crc) => {
                match locations.by_crc(crc) {
                    Some(l) => {
                        let mut chain: Vec<String> = locations
                            .ancestors(&l.guid)
                            .map(|a| a.display_name(&cooked.locale).unwrap_or("?").to_string())
                            .collect();
                        chain.reverse();
                        format!(
                            "{} [{}] < {}",
                            l.display_name(&cooked.locale).unwrap_or("<no name>"),
                            l.kind.as_dcb_str(),
                            chain.join(" < ")
                        )
                    }
                    None => format!("UNRESOLVED place crc {crc}"),
                }
            }
            ref other => format!("{other:?}"),
        };
        eprintln!("{:<45} {place}", s.location.inventory_id);
    }

    // ── Item coverage ───────────────────────────────────────────────────
    let mut nodes = Vec::new();
    for s in &inv.stashes {
        walk(&s.items, 0, &mut nodes);
    }
    let mut unresolved: BTreeMap<u32, (usize, i32)> = BTreeMap::new();
    let mut unnamed = 0;
    let mut wire_to_type: BTreeMap<i32, BTreeMap<String, usize>> = BTreeMap::new();
    for (_, n) in &nodes {
        match name(n.class_crc) {
            Some((nm, ty)) => {
                if nm == "<no name>" {
                    unnamed += 1;
                }
                *wire_to_type
                    .entry(n.item_type)
                    .or_default()
                    .entry(ty)
                    .or_default() += 1;
            }
            None => {
                let e = unresolved.entry(n.class_crc).or_insert((0, n.item_type));
                e.0 += 1;
            }
        }
    }
    let missing: usize = unresolved.values().map(|v| v.0).sum();
    eprintln!(
        "\n== items: {} nodes, {} unresolved ({} distinct crcs), {} without a display name",
        nodes.len(),
        missing,
        unresolved.len(),
        unnamed
    );
    for (crc, (count, wire)) in unresolved.iter().take(15) {
        eprintln!("  unresolved crc {crc} x{count} (wire type {wire})");
    }
    eprintln!("\n== wire item_type -> holotable type (first 40)");
    for (wire, types) in wire_to_type.iter().take(40) {
        eprintln!("  {wire:>4}: {types:?}");
    }

    // ── Ships ───────────────────────────────────────────────────────────
    eprintln!("\n== vehicles (NOITEM_Vehicle) in stashes");
    for s in &inv.stashes {
        let mut here = Vec::new();
        walk(&s.items, 0, &mut here);
        for (depth, n) in here {
            if let Some((nm, ty)) = name(n.class_crc)
                && ty == "NOITEM_Vehicle"
            {
                let mut sub = Vec::new();
                walk(&n.attached, 0, &mut sub);
                let mut stowed = Vec::new();
                walk(&n.contents, 0, &mut stowed);
                eprintln!(
                    "  {nm} geid {} depth {depth} in {} — {} attached, {} stowed, urn {}, {:?}",
                    n.geid,
                    s.location.inventory_id,
                    sub.len(),
                    stowed.len(),
                    n.parent_urn.is_some(),
                    n.ownership
                );
            }
        }
    }

    // ── Entitlements ───────────────────────────────────────────────────
    let mut by_type: BTreeMap<String, usize> = BTreeMap::new();
    let mut ent_unresolved = 0;
    let mut ships = Vec::new();
    for e in &inv.entitlements {
        let item = Guid::from_str(&e.class_guid)
            .ok()
            .and_then(|g| items.get(&g));
        match item {
            Some(i) => {
                let ty = i.item_type.as_dcb_str().to_string();
                if ty == "NOITEM_Vehicle" {
                    ships.push((
                        i.display_name(&cooked.locale)
                            .unwrap_or("<no name>")
                            .to_string(),
                        e.geid.is_some(),
                        format!("{:?}", e.source),
                    ));
                }
                *by_type.entry(ty).or_default() += 1;
            }
            None => ent_unresolved += 1,
        }
    }
    eprintln!(
        "\n== entitlements: {} total, {} unresolved by class GUID",
        inv.entitlements.len(),
        ent_unresolved
    );
    let mut types: Vec<_> = by_type.into_iter().collect();
    types.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
    eprintln!("  types: {:?}", &types[..types.len().min(20)]);
    eprintln!("  vehicle entitlements: {}", ships.len());
    for (nm, materialised, src) in ships.iter().take(60) {
        eprintln!("    {nm} — in world: {materialised}, {src}");
    }

    // ── Loadouts ───────────────────────────────────────────────────────
    if let Some(loadouts) = &inv.loadouts {
        let urn_class: BTreeMap<&str, &str> = inv
            .entitlements
            .iter()
            .map(|e| (e.urn.as_str(), e.class_guid.as_str()))
            .collect();
        eprintln!("\n== saved loadouts");
        for l in loadouts
            .iter()
            .filter(|l| l.slot == svc_dossier::inventory::LoadoutSlotRecord::Saved)
            .take(10)
        {
            let root = urn_class
                .get(l.root_urn.as_str())
                .and_then(|g| Guid::from_str(g).ok())
                .and_then(|g| items.get(&g))
                .map(|i| {
                    i.display_name(&cooked.locale)
                        .unwrap_or("<no name>")
                        .to_string()
                })
                .unwrap_or_else(|| "UNRESOLVED root".into());
            eprintln!("  {root}: {} entries", l.entries.len());
            for e in l.entries.iter().take(4) {
                eprintln!("    {} = {:?}", e.path.join("/"), e.item);
            }
        }
    }
}

#[test]
#[ignore = "needs real data — set STARLUME_REAL_CACHE, STARLUME_REAL_VERSION, STARLUME_REAL_INVENTORY"]
fn finder_on_real_data() {
    let path = std::env::var("STARLUME_REAL_INVENTORY").expect("set STARLUME_REAL_INVENTORY");
    let inv: Inventory = app_kit::load_json(std::path::Path::new(&path));
    let cooked = cooked();
    let started = std::time::Instant::now();
    let finder = mod_items::finder(&inv, &cooked);
    eprintln!(
        "finder: {} items, {} ships, {} unnamed, in {:?}",
        finder.items.len(),
        finder.ships.len(),
        finder.unnamed,
        started.elapsed()
    );

    let mut by_type: BTreeMap<&str, usize> = BTreeMap::new();
    for i in &finder.items {
        *by_type.entry(i.kind.item_type.as_str()).or_default() += 1;
    }
    let mut types: Vec<_> = by_type.into_iter().collect();
    types.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
    eprintln!("item types: {types:?}");

    eprintln!(
        "
== sample items"
    );
    for i in finder
        .items
        .iter()
        .step_by(finder.items.len().max(1) / 25 + 1)
    {
        let holders: Vec<&str> = i.holders.iter().map(|h| h.name.as_str()).collect();
        eprintln!(
            "  {} [{}] @ {} ({}) via {:?} fitted={} {:?} x{}{}",
            i.kind.name,
            i.kind.item_type,
            i.place.name,
            i.place.parents.join(", "),
            holders,
            i.fitted,
            i.ownership,
            i.stack,
            i.resource
                .as_ref()
                .map(|r| format!(" {} q{} {:.2}scu", r.name, r.quality, r.scu))
                .unwrap_or_default()
        );
    }

    eprintln!("\n== places");
    let mut places: BTreeMap<String, usize> = BTreeMap::new();
    for i in &finder.items {
        let p = &i.place;
        *places
            .entry(format!(
                "{:?} {} < {}",
                p.kind,
                p.name,
                p.parents.join(" < ")
            ))
            .or_default() += 1;
    }
    for (p, n) in &places {
        eprintln!("  {n:>5}  {p}");
    }

    let weapons: Vec<_> = finder
        .items
        .iter()
        .filter(|i| i.kind.item_type == "WeaponPersonal")
        .collect();
    let with_stock = weapons.iter().filter(|i| i.stock.is_some()).count();
    eprintln!(
        "\n== weapons: {} , {} with a stock loadout",
        weapons.len(),
        with_stock
    );
    for w in weapons.iter().filter(|i| i.stock.is_some()).take(5) {
        let stock: Vec<String> = w
            .stock
            .iter()
            .flatten()
            .map(|s| {
                format!(
                    "{}={}",
                    s.path.join("/"),
                    s.item.as_ref().map_or("—", |k| k.name.as_str())
                )
            })
            .collect();
        eprintln!("  {}: {}", w.kind.name, stock.join(", "));
    }

    eprintln!("\n== crafted");
    for i in finder.items.iter().filter(|i| i.crafted.is_some()).take(8) {
        let c = i.crafted.as_ref().unwrap();
        let ing: Vec<String> = c
            .ingredients
            .iter()
            .map(|g| {
                format!(
                    "{} Q{} x{}",
                    g.name.as_deref().unwrap_or("?"),
                    g.quality,
                    g.amount
                )
            })
            .collect();
        let props: Vec<String> = c
            .properties
            .iter()
            .map(|p| {
                format!(
                    "{}={:.3} (up better: {:?})",
                    p.name.as_deref().unwrap_or("?"),
                    p.factor,
                    p.factor_up_is_better
                )
            })
            .collect();
        eprintln!(
            "  {}: [{}] {}",
            i.kind.name,
            ing.join(", "),
            props.join(" ")
        );
    }

    eprintln!(
        "
== ships"
    );
    for s in &finder.ships {
        let named = s.fitted.iter().filter(|f| f.port.is_some()).count();
        eprintln!(
            "  {} {:?} {:?} @ {} — fitted {} ({} ports named), stock {:?}, saved {:?}",
            s.kind.name,
            s.ownership,
            s.source,
            s.place.as_ref().map_or("—".to_string(), |p| p.name.clone()),
            s.fitted.len(),
            named,
            s.stock.as_ref().map(Vec::len),
            s.saved.as_ref().map(Vec::len),
        );
    }
    if let Some(s) = finder.ships.iter().find(|s| s.fitted.len() > 20) {
        eprintln!(
            "
== fitted on {}",
            s.kind.name
        );
        for f in &s.fitted {
            eprintln!(
                "  {}{} [{} S{}] port {:?}",
                "  ".repeat(f.depth as usize),
                f.kind.name,
                f.kind.item_type,
                f.kind.size,
                f.port
            );
        }
    }
    assert!(!finder.items.is_empty());
}
