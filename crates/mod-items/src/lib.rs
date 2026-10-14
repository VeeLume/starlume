//! My Items — the finder over the player's items and ships.
//!
//! Joins svc-dossier's inventory tier (wire-native ids: class CRCs, place
//! CRCs, geids, URNs) with svc-data's cooked game data (names, types,
//! places). Pure: the shell fetches (gated) and loads; this crate only
//! joins. Two views come out of one walk:
//!
//! - **items** — every owned item a player would look for, with where it
//!   is: the place (with its parents), and what holds it (a ship, a turret,
//!   a backpack). A ship's structure (thrusters, displays, doors,
//!   controllers) is left out; the swappable parts (weapons, components,
//!   armour) stay, so "where is my FR-76" finds the one fitted to a ship.
//! - **ships** — every owned vehicle, materialised or not, with its fitted
//!   parts beside its stock and saved loadouts.
//!
//! What the real data showed (one account, 4.10.2 live): every place CRC
//! resolves; the only unresolved item CRCs are `UNDEFINED` structure; a
//! rented ship's nodes say Imprinted, so "rented" comes from the
//! entitlement's source; a claim elsewhere leaves the old instance Bricked.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::str::FromStr;

use serde::Serialize;
use svc_data::{CookedData, Guid, LocaleMap, RecordCollection, class_crc};
use svc_dossier::Inventory;
use svc_dossier::inventory::{
    ContextRecord, EntitlementRecord, EntitlementSourceRecord, EntitlementStatusRecord, ItemRecord,
    LoadoutItemRecord, LoadoutRecord, LoadoutSlotRecord, OwnershipRecord,
};

/// The finder's two views over one inventory fetch.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Finder {
    pub items: Vec<ItemRow>,
    pub ships: Vec<ShipRow>,
    /// Item instances the game data couldn't name (not structure) — a
    /// stale build shows here first.
    pub unnamed: u32,
}

/// Where an inventory is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Place {
    pub name: String,
    /// Parents, nearest first (`["microTech", "Stanton"]`).
    pub parents: Vec<String>,
    pub kind: PlaceKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PlaceKind {
    /// A place's item storage.
    Storage,
    /// A hangar at a place.
    Hangar,
    /// On the character.
    Carried,
    Other,
}

/// Something an item sits on or in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Holder {
    pub geid: u64,
    pub name: String,
    pub item_type: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OwnershipState {
    Owned,
    Rented,
    /// Unusable — the old instance after a claim elsewhere.
    Bricked,
    /// The backend reports no ownership (loose loot, commodities).
    Unowned,
}

/// What a game-data record is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ItemKind {
    pub guid: String,
    pub name: String,
    pub item_type: String,
    pub sub_type: String,
    pub size: i32,
    pub grade: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ItemRow {
    pub geid: u64,
    pub kind: ItemKind,
    pub place: Place,
    /// What holds it, outermost first (`[ship, turret]`); empty when it is
    /// stowed straight in the place.
    pub holders: Vec<Holder>,
    /// Fitted to a port (vs stowed in an inventory).
    pub fitted: bool,
    pub ownership: OwnershipState,
    pub stack: u32,
    pub ammo: Option<u32>,
    pub resource: Option<ResourceStack>,
    pub crafted: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceStack {
    pub name: String,
    /// `0..=1000`.
    pub quality: u16,
    pub scu: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ShipRow {
    /// Stable key: the entitlement URN, or `geid:<n>` for an instance
    /// without one (a bricked leftover).
    pub key: String,
    pub kind: ItemKind,
    /// The world instance, when materialised and found in a stash.
    pub geid: Option<u64>,
    /// Where it is; `None` when not materialised or not in any stash.
    pub place: Option<Place>,
    /// What carries it (a Skylark inside a Constellation).
    pub holders: Vec<Holder>,
    pub ownership: OwnershipState,
    pub source: Source,
    /// Rental expiry, Unix seconds.
    pub rental_expiry: Option<i64>,
    /// Its swappable parts as fitted now.
    pub fitted: Vec<FittedPart>,
    pub stock: Option<Vec<LoadoutSlot>>,
    pub saved: Option<Vec<LoadoutSlot>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// Bought on the website.
    Pledge,
    /// Bought or earned in game.
    InGame,
    Rental,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
pub struct FittedPart {
    pub geid: u64,
    /// The port name, when the ship's loadouts name it.
    pub port: Option<String>,
    /// How deep below the hull: 0 = on the ship, 1 = on a part of it (a
    /// gun on a turret).
    pub depth: u32,
    pub kind: ItemKind,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoadoutSlot {
    /// Port names from the ship down.
    pub path: Vec<String>,
    /// `None` = emptied port.
    pub item: Option<ItemKind>,
}

/// Item types that are a vehicle's or a hangar's structure, not something a
/// player owns separately or looks for. Hidden when attached; their
/// attached children are still walked.
const STRUCTURE: &[&str] = &[
    "UNDEFINED",
    "AIModule",
    "AirTrafficController",
    "Armor",
    "AttachedPart",
    "CapacitorAssignmentController",
    "CargoGrid",
    "CommsController",
    "ControlPanel",
    "CoolerController",
    "Display",
    "DockingAnimator",
    "DockingCollar",
    "Door",
    "DoorController",
    "EnergyController",
    "FlightController",
    "FuelController",
    "FuelIntake",
    "FuelTank",
    "LandingSystem",
    "LifeSupportVent",
    "Light",
    "LightController",
    "MainThruster",
    "ManneuverThruster",
    "MiningController",
    "MissileController",
    "QuantumFuelTank",
    "Relay",
    "Room",
    "SalvageController",
    "Seat",
    "SeatAccess",
    "SeatDashboard",
    "SelfDestruct",
    "Sensor",
    "ShieldController",
    "TurretBase",
    "Usable",
    "Visor",
    "WeaponController",
    "WeaponDefensive",
];

/// Never an item row, wherever it sits: the character's own body parts,
/// and structure the game data has no type for.
fn is_body(item_type: &str) -> bool {
    item_type == "UNDEFINED"
        || item_type == "NOITEM_Player"
        || item_type == "Char_Body"
        || item_type == "Char_Hair_Color"
        || item_type.starts_with("Char_Head")
        || item_type.starts_with("Char_Accessory")
}

const VEHICLE: &str = "NOITEM_Vehicle";

struct Names<'a> {
    cooked: &'a CookedData,
    /// Resource CRC → display name.
    resources: HashMap<u32, String>,
}

impl<'a> Names<'a> {
    fn new(cooked: &'a CookedData) -> Self {
        let resources = cooked
            .holotable
            .resources
            .as_ref()
            .map(|rs| {
                rs.values()
                    .map(|r| {
                        let name = resolve(&cooked.locale, r.name_key.as_str());
                        (class_crc(&r.guid), name)
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self { cooked, resources }
    }

    fn by_crc(&self, crc: u32) -> Option<ItemKind> {
        let items = self.cooked.holotable.items.as_ref()?;
        let guid = items.guid_by_crc(crc)?;
        self.by_guid(&guid)
    }

    /// The class exists in the game data, named or not.
    fn known(&self, crc: u32) -> bool {
        self.cooked
            .holotable
            .items
            .as_ref()
            .is_some_and(|items| items.guid_by_crc(crc).is_some())
    }

    fn by_guid_str(&self, guid: &str) -> Option<ItemKind> {
        self.by_guid(&Guid::from_str(guid).ok()?)
    }

    fn by_guid(&self, guid: &Guid) -> Option<ItemKind> {
        let item = self.cooked.holotable.items.as_ref()?.get(guid)?;
        // A record without a real name is an internal part (a hull
        // container, a door panel) — nothing a player looks for.
        let name = item
            .display_name(&self.cooked.locale)
            .filter(|n| !n.contains("PLACEHOLDER"))?;
        Some(ItemKind {
            guid: guid.to_string(),
            name: name.to_string(),
            item_type: item.item_type.as_dcb_str().to_string(),
            sub_type: item.item_sub_type.as_dcb_str().to_string(),
            size: item.size,
            grade: item.grade,
        })
    }

    fn place(&self, context: &ContextRecord) -> Place {
        let (crc, kind) = match context {
            ContextRecord::Location(crc) => (*crc, PlaceKind::Storage),
            ContextRecord::Hangar(crc) => (*crc, PlaceKind::Hangar),
            ContextRecord::Player => {
                return Place {
                    name: "On you".into(),
                    parents: Vec::new(),
                    kind: PlaceKind::Carried,
                };
            }
            other => {
                return Place {
                    name: format!("{other:?}"),
                    parents: Vec::new(),
                    kind: PlaceKind::Other,
                };
            }
        };
        let locale = &self.cooked.locale;
        let Some(locations) = self.cooked.holotable.locations.as_ref() else {
            return unknown_place(crc, kind);
        };
        let Some(location) = locations.by_crc(crc) else {
            return unknown_place(crc, kind);
        };
        Place {
            name: location
                .display_name(locale)
                .unwrap_or("Unknown place")
                .to_string(),
            parents: locations
                .ancestors(&location.guid)
                .filter_map(|a| a.display_name(locale).map(str::to_string))
                .collect(),
            kind,
        }
    }
}

fn unknown_place(crc: u32, kind: PlaceKind) -> Place {
    Place {
        name: format!("Unknown place ({crc})"),
        parents: Vec::new(),
        kind,
    }
}

fn resolve(locale: &LocaleMap, key: &str) -> String {
    let key = key.strip_prefix('@').unwrap_or(key);
    locale.resolve(key).unwrap_or(key).to_string()
}

/// Entitlements that are the player's to keep (or rent): not transient
/// grants, not revoked or failed deliveries.
fn counts(e: &EntitlementRecord) -> bool {
    !matches!(
        e.source,
        EntitlementSourceRecord::IngameNonpersistent | EntitlementSourceRecord::ArenaCommander
    ) && !matches!(
        e.status,
        EntitlementStatusRecord::Revoked | EntitlementStatusRecord::Failed
    )
}

/// Where an instance came from: its entitlement's source, else the URN's
/// namespace — the entitlement list misses some pledged items (a Perseus,
/// a Prospector) whose instances carry a `urn:sc:platform:` URN.
fn source_of(urn: Option<&str>, entitlement: Option<&EntitlementRecord>) -> Source {
    match (entitlement, urn) {
        (Some(e), _) => source(e),
        (None, Some(u)) if u.starts_with("urn:sc:platform:") => Source::Pledge,
        _ => Source::Unknown,
    }
}

fn source(e: &EntitlementRecord) -> Source {
    match e.source {
        EntitlementSourceRecord::Platform => Source::Pledge,
        EntitlementSourceRecord::IngamePersistent => Source::InGame,
        EntitlementSourceRecord::Rental => Source::Rental,
        _ => Source::Unknown,
    }
}

fn ownership(record: &OwnershipRecord, entitlement: Option<&EntitlementRecord>) -> OwnershipState {
    match record {
        OwnershipRecord::Bricked { .. } => OwnershipState::Bricked,
        // A rental's nodes report Imprinted; the entitlement knows better.
        _ if entitlement.is_some_and(|e| e.source == EntitlementSourceRecord::Rental) => {
            OwnershipState::Rented
        }
        OwnershipRecord::Rented { .. } => OwnershipState::Rented,
        OwnershipRecord::Imprinted { .. } => OwnershipState::Owned,
        OwnershipRecord::None | OwnershipRecord::Other { .. } => OwnershipState::Unowned,
    }
}

/// Build both views.
pub fn finder(inventory: &Inventory, cooked: &CookedData) -> Finder {
    let names = Names::new(cooked);
    let entitlements: HashMap<&str, &EntitlementRecord> = inventory
        .entitlements
        .iter()
        .map(|e| (e.urn.as_str(), e))
        .collect();

    let mut out = Finder::default();
    // Vehicle instances found in stashes, with where they are.
    let mut placed_ships: Vec<(&ItemRecord, Place, Vec<Holder>)> = Vec::new();

    // 'i: the inventory, borrowed for the whole join; 'm: this walk's
    // borrows of the output.
    struct Walk<'i, 'm, 'a> {
        names: &'m Names<'a>,
        entitlements: &'m HashMap<&'i str, &'i EntitlementRecord>,
        out: &'m mut Finder,
        ships: &'m mut Vec<(&'i ItemRecord, Place, Vec<Holder>)>,
    }

    impl<'i> Walk<'i, '_, '_> {
        fn nodes(
            &mut self,
            nodes: &'i [ItemRecord],
            place: &Place,
            holders: &[Holder],
            attached: bool,
        ) {
            for node in nodes {
                let kind = self.names.by_crc(node.class_crc);
                let mut inner = holders.to_vec();
                if let Some(kind) = &kind {
                    let ty = kind.item_type.as_str();
                    if ty == VEHICLE {
                        self.ships.push((node, place.clone(), holders.to_vec()));
                    } else if !is_body(ty) && !(attached && STRUCTURE.contains(&ty)) {
                        let entitlement = node
                            .parent_urn
                            .as_deref()
                            .and_then(|u| self.entitlements.get(u).copied());
                        self.out.items.push(ItemRow {
                            geid: node.geid,
                            kind: kind.clone(),
                            place: place.clone(),
                            holders: holders.to_vec(),
                            fitted: attached,
                            ownership: ownership(&node.ownership, entitlement),
                            stack: node.stack_size,
                            ammo: node.ammo,
                            resource: node.resource.map(|r| ResourceStack {
                                name: self
                                    .names
                                    .resources
                                    .get(&r.resource_id)
                                    .cloned()
                                    .unwrap_or_else(|| "Unknown resource".into()),
                                quality: r.quality,
                                scu: r.scu,
                            }),
                            crafted: node.crafted.is_some(),
                        });
                    }
                    // Structure and the body hold things without being a
                    // place to look; everything else names the way down.
                    if !is_body(ty) && !STRUCTURE.contains(&ty) {
                        inner.push(Holder {
                            geid: node.geid,
                            name: kind.name.clone(),
                            item_type: kind.item_type.clone(),
                        });
                    }
                } else if node.item_type != UNDEFINED_WIRE && !self.names.known(node.class_crc) {
                    self.out.unnamed += 1;
                }
                self.nodes(&node.attached, place, &inner, true);
                self.nodes(&node.contents, place, &inner, false);
            }
        }
    }

    {
        let mut walk = Walk {
            names: &names,
            entitlements: &entitlements,
            out: &mut out,
            ships: &mut placed_ships,
        };
        for stash in &inventory.stashes {
            let place = names.place(&stash.location.context);
            walk.nodes(&stash.items, &place, &[], false);
        }
    }

    out.ships = ships(inventory, &names, &entitlements, &placed_ships);
    out.items.sort_by(|a, b| {
        a.kind
            .name
            .cmp(&b.kind.name)
            .then_with(|| a.place.name.cmp(&b.place.name))
    });
    out
}

/// The wire `EItemType` index of `UNDEFINED` — structure the game data has
/// no record for, so it is not "unnamed".
const UNDEFINED_WIRE: i32 = 193;

fn ships(
    inventory: &Inventory,
    names: &Names<'_>,
    entitlements: &HashMap<&str, &EntitlementRecord>,
    placed: &[(&ItemRecord, Place, Vec<Holder>)],
) -> Vec<ShipRow> {
    let loadouts: HashMap<(&str, LoadoutSlotRecord), &LoadoutRecord> = inventory
        .loadouts
        .iter()
        .flatten()
        .map(|l| ((l.root_urn.as_str(), l.slot), l))
        .collect();
    let have_loadouts = inventory.loadouts.is_some();
    let loadout = |urn: Option<&str>, slot| -> Option<Vec<LoadoutSlot>> {
        if !have_loadouts {
            return None;
        }
        let record = loadouts.get(&(urn?, slot))?;
        Some(
            record
                .entries
                .iter()
                .map(|e| LoadoutSlot {
                    path: e.path.clone(),
                    item: match &e.item {
                        LoadoutItemRecord::Entitlement(urn) => entitlements
                            .get(urn.as_str())
                            .and_then(|e| names.by_guid_str(&e.class_guid)),
                        LoadoutItemRecord::Class(guid) => names.by_guid_str(guid),
                        LoadoutItemRecord::Empty | LoadoutItemRecord::Other(_) => None,
                    },
                })
                .collect(),
        )
    };

    let mut rows = Vec::new();
    let mut seen_urns = HashSet::new();
    for (node, place, holders) in placed {
        let Some(kind) = names.by_crc(node.class_crc) else {
            continue;
        };
        let urn = node.parent_urn.as_deref();
        let entitlement = urn.and_then(|u| entitlements.get(u).copied());
        if let Some(urn) = urn {
            seen_urns.insert(urn);
        }
        let stock = loadout(urn, LoadoutSlotRecord::Stock);
        let saved = loadout(urn, LoadoutSlotRecord::Saved);
        let ports = port_names(stock.iter().chain(saved.iter()).flatten());
        rows.push(ShipRow {
            key: urn.map_or_else(|| format!("geid:{}", node.geid), str::to_string),
            kind,
            geid: Some(node.geid),
            place: Some(place.clone()),
            holders: holders.clone(),
            ownership: ownership(&node.ownership, entitlement),
            source: source_of(urn, entitlement),
            rental_expiry: entitlement.and_then(|e| e.rental_expiry),
            fitted: fitted(node, names, &ports),
            stock,
            saved,
        });
    }
    // Owned vehicles with no instance in any stash: never materialised, or
    // somewhere the stash walk doesn't reach.
    for e in &inventory.entitlements {
        if !counts(e) || seen_urns.contains(e.urn.as_str()) {
            continue;
        }
        let Some(kind) = names.by_guid_str(&e.class_guid) else {
            continue;
        };
        if kind.item_type != VEHICLE {
            continue;
        }
        rows.push(ShipRow {
            key: e.urn.clone(),
            kind,
            geid: e.geid,
            place: None,
            holders: Vec::new(),
            ownership: if e.source == EntitlementSourceRecord::Rental {
                OwnershipState::Rented
            } else {
                OwnershipState::Owned
            },
            source: source(e),
            rental_expiry: e.rental_expiry,
            fitted: Vec::new(),
            stock: loadout(Some(e.urn.as_str()), LoadoutSlotRecord::Stock),
            saved: loadout(Some(e.urn.as_str()), LoadoutSlotRecord::Saved),
        });
    }
    rows.sort_by(|a, b| {
        a.kind
            .name
            .cmp(&b.kind.name)
            .then_with(|| a.key.cmp(&b.key))
    });
    rows
}

/// Port CRC → name, from the port names a ship's loadouts spell out.
fn port_names<'l>(slots: impl Iterator<Item = &'l LoadoutSlot>) -> BTreeMap<u32, String> {
    let mut map = BTreeMap::new();
    for slot in slots {
        for port in &slot.path {
            map.entry(svc_dossier::port_crc(port))
                .or_insert_with(|| port.clone());
        }
    }
    map
}

/// A ship's swappable parts as fitted: attached nodes that aren't
/// structure, at any depth (a gun on a turret on the hull).
fn fitted(ship: &ItemRecord, names: &Names<'_>, ports: &BTreeMap<u32, String>) -> Vec<FittedPart> {
    fn walk(
        nodes: &[ItemRecord],
        depth: u32,
        names: &Names<'_>,
        ports: &BTreeMap<u32, String>,
        out: &mut Vec<FittedPart>,
    ) {
        for node in nodes {
            if let Some(kind) = names.by_crc(node.class_crc)
                && !STRUCTURE.contains(&kind.item_type.as_str())
                && !is_body(&kind.item_type)
            {
                out.push(FittedPart {
                    geid: node.geid,
                    port: node.port_crc.and_then(|crc| ports.get(&crc).cloned()),
                    depth,
                    kind,
                });
            }
            walk(&node.attached, depth + 1, names, ports, out);
        }
    }
    let mut out = Vec::new();
    walk(&ship.attached, 0, names, ports, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entitlement(source: EntitlementSourceRecord) -> EntitlementRecord {
        EntitlementRecord {
            urn: "urn:sc:global:entitlement:uuid:x".into(),
            source,
            status: EntitlementStatusRecord::Delivered,
            class_guid: String::new(),
            geid: None,
            stack_size: 1,
            real_money: false,
            rental_expiry: None,
        }
    }

    #[test]
    fn a_rental_reads_rented_though_its_nodes_say_imprinted() {
        let imprinted = OwnershipRecord::Imprinted { since: Some(1) };
        let rental = entitlement(EntitlementSourceRecord::Rental);
        assert_eq!(ownership(&imprinted, Some(&rental)), OwnershipState::Rented);
        assert_eq!(ownership(&imprinted, None), OwnershipState::Owned);
        // Bricked wins over everything: the instance is unusable.
        let bricked = OwnershipRecord::Bricked { since: None };
        assert_eq!(ownership(&bricked, Some(&rental)), OwnershipState::Bricked);
        assert_eq!(
            ownership(&OwnershipRecord::None, None),
            OwnershipState::Unowned
        );
    }

    #[test]
    fn a_missing_entitlement_takes_its_source_from_the_urn_namespace() {
        let pledge = entitlement(EntitlementSourceRecord::Platform);
        assert_eq!(source_of(Some("urn:x"), Some(&pledge)), Source::Pledge);
        assert_eq!(
            source_of(Some("urn:sc:platform:entitlement:uuid:y"), None),
            Source::Pledge
        );
        assert_eq!(
            source_of(Some("urn:sc:global:entitlement:uuid:z"), None),
            Source::Unknown
        );
        assert_eq!(source_of(None, None), Source::Unknown);
    }

    #[test]
    fn transient_and_failed_grants_dont_count() {
        assert!(counts(&entitlement(EntitlementSourceRecord::Platform)));
        assert!(!counts(&entitlement(
            EntitlementSourceRecord::IngameNonpersistent
        )));
        let mut failed = entitlement(EntitlementSourceRecord::Platform);
        failed.status = EntitlementStatusRecord::Failed;
        assert!(!counts(&failed));
    }
}
