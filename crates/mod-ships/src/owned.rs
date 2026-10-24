//! Your ships' loadouts in the game, from the inventory snapshot My Items
//! keeps (svc-dossier's cache; nothing is fetched here): per owned ship
//! the insured loadout (the saved one, what a claim restores; stock until
//! changed) and the live one (what is fitted on the instance in a stash
//! now). Both come out as [`Edit`]s on the stock loadout, the shape saved
//! loadouts have, so the Build tab opens them like any other.
//!
//! The saved loadout names its ports; the live tree gives each part's port
//! as a CRC of the lowercased name, read back through the stock tree's
//! port names. A part on a port the stock tree doesn't have (inside a part
//! swapped for one with other ports) is left out. A stock port the live
//! tree leaves empty is emptied (a rack whose missiles were fired).

use std::collections::{HashMap, HashSet};

use svc_data::ships::Mounted;
use svc_data::{CookedData, Guid, RecordCollection, class_crc};
use svc_dossier::Inventory;
use svc_dossier::inventory::{
    EntitlementRecord, EntitlementSourceRecord, EntitlementStatusRecord, ItemRecord,
    LoadoutItemRecord, LoadoutSlotRecord,
};

use crate::view::{Edit, LoadoutInput, ViewError, loadout};

/// One ship you own (an entitlement) and its loadouts in the game.
#[derive(Debug, Clone, PartialEq)]
pub struct OwnedLoadouts {
    /// The entitlement's URN.
    pub urn: String,
    /// The insured loadout as edits on stock: empty when it was never
    /// changed (a claim restores stock).
    pub insured: Vec<Edit>,
    /// The live loadout as edits on stock; `None` when the instance is in
    /// no stash the snapshot read (never spawned, or out of reach).
    pub live: Option<Vec<Edit>>,
}

/// Your ships of class `ship` (record GUID) in `inventory`.
pub fn owned_loadouts(
    cooked: &CookedData,
    inventory: &Inventory,
    ship: &str,
) -> Result<Vec<OwnedLoadouts>, ViewError> {
    let Ok(class) = ship.parse::<Guid>() else {
        return Err(ViewError::UnknownShip(ship.to_string()));
    };
    let stock = loadout(
        cooked,
        &LoadoutInput {
            ship: ship.to_string(),
            edits: Vec::new(),
        },
    )?;
    let entitlements: HashMap<&str, &EntitlementRecord> = inventory
        .entitlements
        .iter()
        .map(|e| (e.urn.as_str(), e))
        .collect();
    let class_of = |urn: &str| {
        entitlements
            .get(urn)
            .and_then(|e| e.class_guid.parse::<Guid>().ok())
    };
    let by_crc: HashMap<u32, Guid> = cooked
        .holotable
        .items
        .as_ref()
        .map(|items| items.iter().map(|(g, _)| (class_crc(g), *g)).collect())
        .unwrap_or_default();
    let mut ports = HashMap::new();
    port_names(&stock.mounted, &mut ports);

    Ok(inventory
        .entitlements
        .iter()
        .filter(|e| counts(e) && e.class_guid.parse::<Guid>().ok() == Some(class))
        .map(|e| {
            let saved = inventory
                .loadouts
                .iter()
                .flatten()
                .find(|l| l.root_urn == e.urn && l.slot == LoadoutSlotRecord::Saved);
            let insured = saved
                .map(|l| {
                    let wanted = l.entries.iter().filter_map(|en| {
                        let item = match &en.item {
                            LoadoutItemRecord::Entitlement(urn) => Some(class_of(urn)?),
                            LoadoutItemRecord::Class(g) => Some(g.parse().ok()?),
                            LoadoutItemRecord::Empty => None,
                            // Not a part we can name: leave the port as stock.
                            LoadoutItemRecord::Other(_) => return None,
                        };
                        Some((en.path.clone(), item))
                    });
                    edits(&stock, wanted.collect())
                })
                .unwrap_or_default();
            let live = e
                .geid
                .and_then(|geid| inventory.stashes.iter().find_map(|s| find(&s.items, geid)))
                .map(|node| {
                    let mut live = HashMap::new();
                    fitted(&node.attached, &[], &ports, &by_crc, &mut live);
                    let mut wanted: Vec<_> = live
                        .iter()
                        .filter_map(|(path, item)| Some((path.clone(), Some((*item)?))))
                        .collect();
                    emptied(&stock.mounted, &[], &live, &mut wanted);
                    edits(&stock, wanted)
                });
            OwnedLoadouts {
                urn: e.urn.clone(),
                insured,
                live,
            }
        })
        .collect())
}

/// An entitlement that is a ship you have: not revoked or failed, not a
/// transient grant (as My Items counts them).
fn counts(e: &EntitlementRecord) -> bool {
    !matches!(
        e.status,
        EntitlementStatusRecord::Revoked | EntitlementStatusRecord::Failed
    ) && e.source != EntitlementSourceRecord::IngameNonpersistent
}

/// Port CRC (of the lowercased name, as the inventory writes it) → the
/// port's name, at every depth of the stock tree.
fn port_names(mounted: &[Mounted], out: &mut HashMap<u32, String>) {
    for m in mounted {
        out.entry(svc_dossier::port_crc(&m.port.to_lowercase()))
            .or_insert_with(|| m.port.clone());
        port_names(&m.children, out);
    }
}

/// The instance `geid` anywhere in a stash: in a hangar, inside another
/// ship's inventory, docked on another ship.
fn find(nodes: &[ItemRecord], geid: u64) -> Option<&ItemRecord> {
    nodes.iter().find_map(|n| {
        if n.geid == geid {
            Some(n)
        } else {
            find(&n.attached, geid).or_else(|| find(&n.contents, geid))
        }
    })
}

/// What the live tree has fitted, by port path: every attached part whose
/// port is known, with its item when the class is known too (`None`: a
/// part is there, but not one we can name, so nothing below it is read).
fn fitted(
    nodes: &[ItemRecord],
    prefix: &[String],
    ports: &HashMap<u32, String>,
    by_crc: &HashMap<u32, Guid>,
    out: &mut HashMap<Vec<String>, Option<Guid>>,
) {
    for n in nodes {
        let Some(port) = n.port_crc.and_then(|p| ports.get(&p)) else {
            continue;
        };
        let mut path = prefix.to_vec();
        path.push(port.clone());
        let item = by_crc.get(&n.class_crc).copied();
        out.insert(path.clone(), item);
        if item.is_some() {
            fitted(&n.attached, &path, ports, by_crc, out);
        }
    }
}

/// The stock parts the live tree has taken off, as (port path, empty): a
/// stock port with a part that has nothing on it live (missiles fired,
/// a weapon removed), under parts that are as stock.
fn emptied(
    mounted: &[Mounted],
    prefix: &[String],
    live: &HashMap<Vec<String>, Option<Guid>>,
    out: &mut Vec<(Vec<String>, Option<Guid>)>,
) {
    for m in mounted {
        let mut path = prefix.to_vec();
        path.push(m.port.clone());
        match live.get(&path) {
            None if m.item.is_some() => out.push((path, None)),
            Some(Some(item)) if m.item == Some(*item) => emptied(&m.children, &path, live, out),
            _ => {}
        }
    }
}

/// The wanted (path, item) pairs as edits on `stock`, parents first: a
/// port whose part differs from stock, and every port under a changed
/// part (the new part brings its own fittings, so stock says nothing
/// there).
fn edits(
    stock: &svc_data::ships::Loadout,
    mut wanted: Vec<(Vec<String>, Option<Guid>)>,
) -> Vec<Edit> {
    wanted.sort_by(|a, b| a.0.len().cmp(&b.0.len()).then_with(|| a.0.cmp(&b.0)));
    let mut changed: HashSet<Vec<String>> = HashSet::new();
    let mut out = Vec::new();
    for (path, item) in wanted {
        let under_change = (1..path.len()).any(|n| changed.contains(&path[..n]));
        let p: Vec<&str> = path.iter().map(String::as_str).collect();
        let now = stock.at(&p).and_then(|m| m.item);
        if under_change || now != item {
            changed.insert(path.clone());
            out.push(Edit {
                path,
                item: item.map(|g| g.to_string()),
            });
        }
    }
    out
}
