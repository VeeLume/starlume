//! Ship reference data — sc-vehicles' vehicles (hull, ports, stock
//! loadouts), every entity's port container, the items' resource profiles,
//! the network rules, the ship component sheets and the item-port
//! modifiers, cooked into one serde bundle. Reference-catalog framework
//! data (CONTRIBUTING module rule 4): mod-ships builds loadouts, power
//! networks and flight figures against it, and never needs the raw
//! Datacore.
//!
//! The sc-vehicles and component types are serde already, so they cross
//! into the snapshot as they are. Two things are added beside them: each
//! listed vehicle's record name (search keys, a label when the locale has
//! none) and its hull's thruster axes ([`sc_vehicles::port_axes`] reads the
//! hull geometry, which needs the live p4k: the cook passes it when it can
//! open it and leaves [`ShipsIndex::axes`] empty when it cannot, as the
//! mining cook does for the rock meshes).
//!
//! The ship guns ride along as sc-weapons' own sheets (serde since
//! sc-holotable v0.28): the DPS maths needs the whole sheet, which the
//! flat [`crate::weapons`] catalog entries don't carry.
//!
//! sc-vehicles and the component sheets are re-exported here, so mod-ships
//! computes against the snapshot without its own sc-holotable pin (the
//! one-pin rule).

use std::collections::HashMap;

use sc_holotable::asset::{AssetSource, Datacore, Guid};
use serde::{Deserialize, Serialize};

pub use sc_holotable::asset::generated::{
    EItemSubType, EItemType, EMasterMode, ResourceNetworkResource,
};
pub use sc_holotable::items::{Item, ItemClass, Items};
pub use sc_holotable::locations::{QuantumTravel, Waypoint, Waypoints};
pub use sc_holotable::ship_components::*;
pub use sc_holotable::vehicles::*;
pub use sc_holotable::weapons::{LoadoutContext, ShipWeapon, SustainKind, WeaponPower};

/// The cooked ship bundle.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ShipsIndex {
    /// Every entity with `VehicleComponentParams`: player ships, their
    /// variants and AI hulls. All of them, not just the ones a player owns:
    /// sc-vehicles reads any item `Vehicles` knows as a docked vehicle (its
    /// items stay off the parent's network), so a docked or fitting
    /// vehicle missing here would join its parent's network. ~4.9 MB of
    /// the cook on LIVE 4.10; a lib-side narrowing is planned upstream.
    pub vehicles: Vehicles,
    /// Every entity's own ports and default loadout: what an item brings
    /// below it when it is equipped.
    pub containers: PortContainers,
    /// Every item's resource states (power, coolant, signatures).
    pub profiles: ResourceProfiles,
    /// `ItemResourceNetworkGlobal`: plant modifiers, mode exclusions and the
    /// power-on distribution defaults. `None` when the record is missing.
    pub network: Option<NetworkParams>,
    /// Ship component sheets (shields, coolers, plants, quantum drives,
    /// thrusters, fuel tanks …).
    pub components: ShipComponents,
    /// Item-port modifiers (weapon regen and the like).
    pub modifiers: ItemportModifiers,
    /// The vehicles a player can own (base hulls, editions, reward ships)
    /// by record name, sorted.
    pub listed: Vec<VehicleEntry>,
    /// Each of those vehicles' thruster axes by lower-case port name. Empty
    /// when the cook could not open the p4k.
    #[serde(default)]
    pub axes: HashMap<Guid, HashMap<String, [f64; 3]>>,
    /// Every ship gun's sheet (fire rate, damage, capacitor, heat):
    /// sc-weapons computes burst and sustained DPS from it under the power
    /// the network gives the gun ([`ShipWeapon::powered`]).
    #[serde(default)]
    pub guns: Vec<ShipWeapon>,
    /// What each player ship's interior places (lockers, weapon racks, suit
    /// lockers, the grids the stock tree doesn't carry), for
    /// [`Storage::compute`]. Empty when the cook could not open the p4k.
    #[serde(default)]
    pub interiors: HashMap<Guid, Vec<PlacedEntity>>,
    /// Every door's own HP (`SHealthComponentParams.Health`), by entity
    /// GUID: what a boarding party has to breach. Doors are `Door` items and
    /// the docking collars' `DockingAnimator` items that carry health (the
    /// Mk5's docking door, 13,500).
    #[serde(default)]
    pub door_health: HashMap<Guid, f32>,
    /// Where quantum travel can go: planets, moons, Lagrange and jump
    /// points, and the stations on them, with system-global positions
    /// (sc-locations, ~1 s from the system socpaks). Empty when the cook
    /// could not open the p4k.
    #[serde(default)]
    pub waypoints: Waypoints,
}

/// One listed vehicle: what a ship picker offers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VehicleEntry {
    pub guid: Guid,
    /// DCB record name (`RSI_Constellation_Mk5_Centaurus`).
    pub record_name: String,
}

/// What a player can own and fit: base hulls, their editions and reward
/// ships ([`Vehicles::listed`] leaves the reward ships out; a loadout
/// manager needs them).
fn offered(v: &Vehicle) -> bool {
    matches!(
        v.listing,
        Listing::Base | Listing::Edition | Listing::Reward
    )
}

/// Cook the ship index from one parsed `Datacore`; `items` comes from the
/// foundations build the caller already paid for, `assets` (the live p4k,
/// when the cook could open it) feeds the thruster axes.
pub(crate) fn build_ships(
    datacore: &Datacore,
    items: &Items,
    assets: Option<&AssetSource>,
) -> ShipsIndex {
    let db = datacore.db();
    let (containers, vehicles, axes, interiors) = match assets {
        Some(a) => {
            // Port containers read XML loadouts too (sc-holotable v0.30:
            // 1,060 entities, the Hammerhead's grid behind its cargo door).
            let containers = PortContainers::build(datacore, a);
            let vehicles = Vehicles::build(datacore, items, &containers, a);
            // ~100–160 ms per hull read, and editions share their base's
            // hull (147 files for 246 ships on LIVE 4.10): read each once.
            let mut by_hull: HashMap<&str, HashMap<String, [f64; 3]>> = HashMap::new();
            let mut axes = HashMap::new();
            for (g, v) in vehicles.iter().filter(|(_, v)| offered(v)) {
                let Some(hull) = v.geometry.as_deref() else {
                    continue;
                };
                let read = by_hull.entry(hull).or_insert_with(|| port_axes(a, v));
                if !read.is_empty() {
                    axes.insert(*g, read.clone());
                }
            }
            // What the interiors place (lockers, racks, cargo grids): the
            // socpaks are read once per set of object containers (~17 s for
            // all of them on LIVE 4.10; editions share their base's).
            let mut by_interior: HashMap<String, Vec<PlacedEntity>> = HashMap::new();
            let mut interiors = HashMap::new();
            for (g, v) in vehicles.iter().filter(|(_, v)| offered(v)) {
                if v.object_containers.is_empty() {
                    continue;
                }
                let key = format!("{:?}", v.object_containers);
                let read = by_interior
                    .entry(key)
                    .or_insert_with(|| Interior::read(a, v).entities);
                if !read.is_empty() {
                    interiors.insert(*g, read.clone());
                }
            }
            (containers, vehicles, axes, interiors)
        }
        None => {
            tracing::warn!("no p4k for the ship cook: no hulls, thruster axes or interiors");
            (
                PortContainers::new(),
                Vehicles::new(),
                HashMap::new(),
                HashMap::new(),
            )
        }
    };
    let mut listed: Vec<VehicleEntry> = vehicles
        .iter()
        .filter(|(_, v)| offered(v))
        .map(|(g, _)| VehicleEntry {
            guid: *g,
            record_name: db
                .record(g)
                .and_then(|r| {
                    r.name()
                        .map(|n| n.rsplit('.').next().unwrap_or(n).to_string())
                })
                .unwrap_or_default(),
        })
        .collect();
    listed.sort_by(|a, b| a.record_name.cmp(&b.record_name));
    let healths = ComponentHealths::build(datacore);
    let door_health = items
        .iter()
        .filter(|(_, i)| matches!(i.item_type, EItemType::Door | EItemType::DockingAnimator))
        .filter_map(|(g, _)| Some((*g, healths.get(g)?.health)))
        .filter(|(_, hp)| *hp > 0.0)
        .collect();
    let waypoints = assets
        .and_then(|a| {
            use sc_holotable::locations::{Locations, ObjectContainers, Universe};
            ObjectContainers::systems(a, 1)
                .inspect_err(|e| tracing::warn!("no system object containers: {e}"))
                .ok()
                .map(|oc| {
                    Waypoints::build(&Universe::join(Locations::build(datacore.records()), oc))
                })
        })
        .unwrap_or_default();
    ShipsIndex {
        door_health,
        waypoints,
        profiles: ResourceProfiles::build(datacore),
        network: NetworkParams::build(datacore),
        components: ShipComponents::build(datacore, items),
        modifiers: ItemportModifiers::build(datacore),
        guns: sc_holotable::weapons::iter_ship_weapons(datacore, items).collect(),
        vehicles,
        containers,
        listed,
        axes,
        interiors,
    }
}

impl ShipsIndex {
    /// Number of listed vehicles (a quick "is it cooked" check).
    pub fn len(&self) -> usize {
        self.listed.len()
    }

    pub fn is_empty(&self) -> bool {
        self.listed.is_empty()
    }

    /// The listed vehicle with this record name.
    pub fn by_record_name(&self, name: &str) -> Option<(&Guid, &Vehicle)> {
        let entry = self.listed.iter().find(|e| e.record_name == name)?;
        self.vehicles.get(&entry.guid).map(|v| (&entry.guid, v))
    }
}
