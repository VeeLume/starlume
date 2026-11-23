//! Your ships' loadouts in the game, from the inventory snapshot My Items
//! keeps (svc-dossier's cache; nothing is fetched here): per owned ship
//! the saved loadout (what a claim restores; stock until changed) and
//! what is fitted now (on the instance in a stash). Both come out as
//! [`Edit`]s on the stock loadout, the shape saved loadouts have, so the
//! Build tab opens them like any other.
//!
//! The pages say "Saved" and "Fitted now". The fields here keep their
//! first names, `insured` and `live`: the loadout ids built from them
//! (`insured:<urn>`, `live:<urn>`) are in drafts on disk.
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
    EntitlementRecord, ItemRecord, LoadoutItemRecord, LoadoutSlotRecord, OwnershipRecord,
    StashRecord,
};

use crate::view::{Edit, LoadoutInput, ViewError, loadout};

/// One ship you own (an entitlement) and its loadouts in the game.
#[derive(Debug, Clone, PartialEq)]
pub struct OwnedLoadouts {
    /// The entitlement's URN.
    pub urn: String,
    /// The saved loadout as edits on stock: empty when it was never
    /// changed (a claim restores stock).
    pub insured: Vec<Edit>,
    /// What is fitted now as edits on stock, or why the snapshot has no
    /// read of it.
    pub live: Result<Vec<Edit>, NoRead>,
}

/// Why a copy has nothing fitted to read, from the snapshot alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoRead {
    /// The entitlement has no instance: the ship was never spawned.
    NotSpawned,
    /// It has an instance, but no stash the snapshot read holds it. The
    /// snapshot can not tell why: out in the world, destroyed and not
    /// claimed, or (seen on rentals) delivered with an instance that was
    /// never put in a hangar.
    Away,
    /// The instance in the stash is bricked: the leftover of a claim
    /// elsewhere, not the ship as it flies.
    Bricked,
}

/// The instance of `e` whose fittings can be read, or why there is none.
fn instance<'i>(
    e: &EntitlementRecord,
    stashes: &'i [StashRecord],
) -> Result<&'i ItemRecord, NoRead> {
    let geid = e.geid.ok_or(NoRead::NotSpawned)?;
    let node = stashes
        .iter()
        .find_map(|s| find(&s.items, geid))
        .ok_or(NoRead::Away)?;
    if matches!(node.ownership, OwnershipRecord::Bricked { .. }) {
        return Err(NoRead::Bricked);
    }
    Ok(node)
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
        .filter(|e| e.held() && e.class_guid.parse::<Guid>().ok() == Some(class))
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
            let live = instance(e, &inventory.stashes).map(|node| {
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
                quality: None,
                qualities: Vec::new(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use svc_dossier::inventory::{
        ContextRecord, EntitlementSourceRecord, EntitlementStatusRecord, LocationRecord,
    };

    fn entitlement(geid: Option<u64>) -> EntitlementRecord {
        EntitlementRecord {
            urn: "urn:sc:global:entitlement:uuid:test".into(),
            source: EntitlementSourceRecord::Rental,
            status: EntitlementStatusRecord::Delivered,
            class_guid: String::new(),
            geid,
            stack_size: 1,
            real_money: false,
            rental_expiry: None,
        }
    }

    fn node(geid: u64, ownership: OwnershipRecord, contents: Vec<ItemRecord>) -> ItemRecord {
        ItemRecord {
            geid,
            class_crc: 1,
            item_type: 0,
            stack_size: 1,
            parent_urn: None,
            port_crc: None,
            ownership,
            resource: None,
            ammo: None,
            crafted: None,
            attached: Vec::new(),
            contents,
        }
    }

    #[test]
    fn a_copy_without_a_read_says_why() {
        let owned = OwnershipRecord::Imprinted { since: None };
        let bricked = OwnershipRecord::Bricked { since: None };
        // A ship (11) in the hangar carrying another (12); a wreck (13).
        let stashes = vec![StashRecord {
            location: LocationRecord {
                inventory_id: "1:Hangar:7".into(),
                context: ContextRecord::Hangar(7),
            },
            items: vec![
                node(11, owned, vec![node(12, owned, Vec::new())]),
                node(13, bricked, Vec::new()),
            ],
        }];
        let read = |geid| instance(&entitlement(geid), &stashes).map(|n| n.geid);
        assert_eq!(read(Some(11)), Ok(11));
        // Inside another ship is still a read.
        assert_eq!(read(Some(12)), Ok(12));
        assert_eq!(read(None), Err(NoRead::NotSpawned));
        assert_eq!(read(Some(99)), Err(NoRead::Away));
        assert_eq!(read(Some(13)), Err(NoRead::Bricked));
    }
}
