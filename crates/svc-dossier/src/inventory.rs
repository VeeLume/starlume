//! The inventory tier: where the player's items and ships are, what is
//! attached to what, and the saved loadouts — the durable snapshot the
//! My Items finder reads.
//!
//! The types mirror [`sc_dossier`]'s stash / entitlement / loadout trees as
//! serde data (the client's types aren't serializable, and its structs are
//! `non_exhaustive`). Ids stay wire-native — class CRCs, place CRCs, geids,
//! GUID strings, URNs; naming them is svc-data's job, joining them the
//! module's.

use serde::{Deserialize, Serialize};

/// One fetch of the inventory tier.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Inventory {
    /// When fetched, Unix seconds. `0` = never (reads as no cache).
    pub fetched_at: i64,
    /// One per inventory that loaded.
    pub stashes: Vec<StashRecord>,
    /// Inventories whose query failed (e.g. the server's AQL limit on a big
    /// ship); the rest still loaded.
    pub failed: Vec<FailedStash>,
    /// Every owned-item grant: the join from URNs to class GUIDs, and the
    /// only record of ships that were never materialised in the world.
    pub entitlements: Vec<EntitlementRecord>,
    /// Stock and saved loadouts. `None` when the loadouts feature was off
    /// for this fetch — distinct from "fetched, none saved".
    pub loadouts: Option<Vec<LoadoutRecord>>,
    /// The game build this was fetched under; `None` when no install was
    /// found (or a fetch from before builds were recorded).
    #[serde(default)]
    pub build: Option<BuildTag>,
}

/// The game build an inventory was fetched under — what the archive keys by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildTag {
    /// The version label (`4.10.2-live.12881860`).
    pub version: String,
    /// The build key (the changelist, `12881860`) — the archive's file name.
    pub key: String,
}

/// An inventory and the items stowed directly in it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StashRecord {
    pub location: LocationRecord,
    pub items: Vec<ItemRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedStash {
    pub location: LocationRecord,
    pub error: String,
}

/// Which inventory: its id plus what it belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocationRecord {
    /// The inventory's unique id (the entity-graph sharding key).
    pub inventory_id: String,
    pub context: ContextRecord,
}

/// What an inventory belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id")]
pub enum ContextRecord {
    /// The character's own inventory.
    Player,
    /// The entitlement holding area.
    Entitlement,
    /// A place's storage — a StarMapObject class CRC.
    Location(u32),
    /// A hangar — a StarMapObject class CRC.
    Hangar(u32),
    /// A ship's or container's inventory — its geid.
    Container(u64),
    Other(String),
}

/// One item instance and what hangs off it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemRecord {
    /// Instance id.
    pub geid: u64,
    /// Item-class GUID CRC (CRC32C of the GUID's storage bytes).
    pub class_crc: u32,
    /// `EItemType` index.
    pub item_type: i32,
    pub stack_size: u32,
    /// The entitlement this instance belongs to, if any.
    pub parent_urn: Option<String>,
    /// The port on the parent it is attached to (CRC32C of the lowercased
    /// port name); `None` when stowed.
    pub port_crc: Option<u32>,
    pub ownership: OwnershipRecord,
    pub resource: Option<ResourceRecord>,
    /// Rounds left, for magazines.
    pub ammo: Option<u32>,
    pub crafted: Option<CraftedRecord>,
    /// Items attached to this one's ports.
    pub attached: Vec<ItemRecord>,
    /// Items stowed in this one's own inventory.
    pub contents: Vec<ItemRecord>,
}

/// Ownership state; `since` is Unix seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state")]
pub enum OwnershipRecord {
    None,
    Imprinted {
        since: Option<i64>,
    },
    Rented {
        since: Option<i64>,
    },
    /// Unusable — e.g. the old instance after a claim elsewhere.
    Bricked {
        since: Option<i64>,
    },
    Other {
        code: i32,
    },
}

/// A cargo or resource stack's material.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ResourceRecord {
    /// `ResourceType` CRC.
    pub resource_id: u32,
    /// `0..=1000`.
    pub quality: u16,
    pub scu: f64,
}

/// A crafted item's recipe record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CraftedRecord {
    pub ingredients: Vec<IngredientRecord>,
    /// Crafted stat values: (`CraftingGameplayPropertyDef` CRC, value).
    pub properties: Vec<(u32, f32)>,
    /// Output entity-class CRC.
    pub output_class_crc: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngredientRecord {
    /// `"resource"`, `"item"`, or `"other"`.
    pub kind: IngredientKindRecord,
    /// `ResourceType` or entity-class CRC, by `kind`.
    pub class_crc: u32,
    /// `0..=1000`.
    pub quality: u16,
    /// µSCU for resources, count for items.
    pub amount: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IngredientKindRecord {
    Resource,
    Item,
    Other,
}

/// One owned-item grant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntitlementRecord {
    pub urn: String,
    pub source: EntitlementSourceRecord,
    pub status: EntitlementStatusRecord,
    /// Full item-class GUID string.
    pub class_guid: String,
    /// The world instance, once materialised.
    pub geid: Option<u64>,
    pub stack_size: i32,
    pub real_money: bool,
    /// Rental expiry, Unix seconds.
    pub rental_expiry: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntitlementSourceRecord {
    /// Webstore / web hangar.
    Platform,
    IngamePersistent,
    /// Transient grant — not owned.
    IngameNonpersistent,
    Rental,
    ArenaCommander,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntitlementStatusRecord {
    Packaged,
    Undelivered,
    Delivered,
    Revoked,
    Failed,
    Unknown,
}

/// One root item's loadout in one slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadoutRecord {
    /// Entitlement URN of the root item (ship, weapon, armour…).
    pub root_urn: String,
    pub slot: LoadoutSlotRecord,
    /// Attached items, sorted by port path.
    pub entries: Vec<LoadoutEntryRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LoadoutSlotRecord {
    /// The item's default loadout.
    Stock,
    /// The player's saved loadout — what a claim restores. Only present
    /// once changed.
    Saved,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadoutEntryRecord {
    /// Item-port names from the root down.
    pub path: Vec<String>,
    pub item: LoadoutItemRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id")]
pub enum LoadoutItemRecord {
    /// An owned item, by entitlement URN.
    Entitlement(String),
    /// A bare entity-class GUID (e.g. missiles).
    Class(String),
    /// An emptied port.
    Empty,
    Other(String),
}

// ── From sc-dossier ─────────────────────────────────────────────────────────

impl From<&sc_dossier::Location> for LocationRecord {
    fn from(l: &sc_dossier::Location) -> Self {
        use sc_dossier::Context;
        Self {
            inventory_id: l.inventory_id.clone(),
            context: match &l.context {
                Context::Player => ContextRecord::Player,
                Context::Entitlement => ContextRecord::Entitlement,
                Context::Location(crc) => ContextRecord::Location(*crc),
                Context::Hangar(crc) => ContextRecord::Hangar(*crc),
                Context::Container(geid) => ContextRecord::Container(*geid),
                Context::Other(s) => ContextRecord::Other(s.clone()),
                #[allow(unreachable_patterns)]
                other => ContextRecord::Other(format!("{other:?}")),
            },
        }
    }
}

impl From<&sc_dossier::ItemNode> for ItemRecord {
    fn from(n: &sc_dossier::ItemNode) -> Self {
        Self {
            geid: n.geid,
            class_crc: n.class_crc,
            item_type: n.item_type_enum,
            stack_size: n.stack_size,
            parent_urn: n.parent_urn.clone(),
            port_crc: n.port_crc,
            ownership: (&n.ownership).into(),
            resource: n.resource.as_ref().map(|r| ResourceRecord {
                resource_id: r.resource_id,
                quality: r.quality,
                scu: r.scu,
            }),
            ammo: n.ammo,
            crafted: n.crafted.as_ref().map(|c| CraftedRecord {
                ingredients: c
                    .ingredients
                    .iter()
                    .map(|i| IngredientRecord {
                        kind: match i.kind {
                            sc_dossier::IngredientKind::Resource => IngredientKindRecord::Resource,
                            sc_dossier::IngredientKind::Item => IngredientKindRecord::Item,
                            _ => IngredientKindRecord::Other,
                        },
                        class_crc: i.class_crc,
                        quality: i.quality,
                        amount: i.amount,
                    })
                    .collect(),
                properties: c.properties.clone(),
                output_class_crc: c.output_class_crc,
            }),
            attached: n.attached.iter().map(Into::into).collect(),
            contents: n.contents.iter().map(Into::into).collect(),
        }
    }
}

impl From<&sc_dossier::Ownership> for OwnershipRecord {
    fn from(o: &sc_dossier::Ownership) -> Self {
        use sc_dossier::Ownership;
        match *o {
            Ownership::None => Self::None,
            Ownership::Imprinted { since } => Self::Imprinted { since },
            Ownership::Rented { since } => Self::Rented { since },
            Ownership::Bricked { since } => Self::Bricked { since },
            Ownership::Other(code) => Self::Other { code },
            _ => Self::Other { code: -1 },
        }
    }
}

impl From<&sc_dossier::Entitlement> for EntitlementRecord {
    fn from(e: &sc_dossier::Entitlement) -> Self {
        use sc_dossier::{EntitlementSource as Src, EntitlementStatus as St};
        Self {
            urn: e.urn.clone(),
            source: match e.source {
                Src::Platform => EntitlementSourceRecord::Platform,
                Src::IngamePersistent => EntitlementSourceRecord::IngamePersistent,
                Src::IngameNonpersistent => EntitlementSourceRecord::IngameNonpersistent,
                Src::Rental => EntitlementSourceRecord::Rental,
                Src::ArenaCommander => EntitlementSourceRecord::ArenaCommander,
                Src::Unknown(_) => EntitlementSourceRecord::Unknown,
            },
            status: match e.status {
                St::Packaged => EntitlementStatusRecord::Packaged,
                St::Undelivered => EntitlementStatusRecord::Undelivered,
                St::Delivered => EntitlementStatusRecord::Delivered,
                St::Revoked => EntitlementStatusRecord::Revoked,
                St::Failed => EntitlementStatusRecord::Failed,
                St::Unknown(_) => EntitlementStatusRecord::Unknown,
            },
            class_guid: e.class_guid.clone(),
            geid: e.geid,
            stack_size: e.stack_size,
            real_money: e.real_money,
            rental_expiry: e.rental_expiry,
        }
    }
}

impl From<&sc_dossier::Loadout> for LoadoutRecord {
    fn from(l: &sc_dossier::Loadout) -> Self {
        use sc_dossier::{LoadoutItem, LoadoutSlot};
        Self {
            root_urn: l.root_urn.clone(),
            slot: match l.slot {
                LoadoutSlot::Stock => LoadoutSlotRecord::Stock,
                LoadoutSlot::Saved => LoadoutSlotRecord::Saved,
                _ => LoadoutSlotRecord::Other,
            },
            entries: l
                .entries
                .iter()
                .map(|e| LoadoutEntryRecord {
                    path: e.path.clone(),
                    item: match &e.item {
                        LoadoutItem::Entitlement(urn) => {
                            LoadoutItemRecord::Entitlement(urn.clone())
                        }
                        LoadoutItem::Class(guid) => LoadoutItemRecord::Class(guid.clone()),
                        LoadoutItem::Empty => LoadoutItemRecord::Empty,
                        LoadoutItem::Other(s) => LoadoutItemRecord::Other(s.clone()),
                        _ => LoadoutItemRecord::Other(String::new()),
                    },
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_roundtrip_through_json() {
        let inv = Inventory {
            fetched_at: 1_700_000_000,
            stashes: vec![StashRecord {
                location: LocationRecord {
                    inventory_id: "1:Location:3723364946".into(),
                    context: ContextRecord::Location(3_723_364_946),
                },
                items: vec![ItemRecord {
                    geid: u64::MAX - 1,
                    class_crc: 42,
                    item_type: 187,
                    stack_size: 1,
                    parent_urn: Some("urn:sc:global:entitlement:uuid:x".into()),
                    port_crc: None,
                    ownership: OwnershipRecord::Imprinted { since: Some(5) },
                    resource: None,
                    ammo: Some(30),
                    crafted: None,
                    attached: Vec::new(),
                    contents: Vec::new(),
                }],
            }],
            failed: Vec::new(),
            entitlements: Vec::new(),
            loadouts: Some(vec![LoadoutRecord {
                root_urn: "urn:x".into(),
                slot: LoadoutSlotRecord::Saved,
                entries: vec![LoadoutEntryRecord {
                    path: vec!["hardpoint_weapon_left".into()],
                    item: LoadoutItemRecord::Empty,
                }],
            }]),
            build: Some(BuildTag {
                version: "4.10.2-live.12881860".into(),
                key: "12881860".into(),
            }),
        };
        let json = serde_json::to_string(&inv).unwrap();
        let back: Inventory = serde_json::from_str(&json).unwrap();
        assert_eq!(back.stashes[0].items[0].geid, u64::MAX - 1);
        assert_eq!(back.stashes[0].location, inv.stashes[0].location);
        assert_eq!(back.loadouts, inv.loadouts);
        assert_eq!(back.build, inv.build);
    }

    #[test]
    fn a_fetch_from_before_builds_were_recorded_still_reads() {
        let json = r#"{"fetched_at":1,"stashes":[],"failed":[],"entitlements":[],"loadouts":null}"#;
        let inv: Inventory = serde_json::from_str(json).unwrap();
        assert_eq!(inv.fetched_at, 1);
        assert!(inv.build.is_none());
    }
}
