//! A loadout as slots: each port the ship carries, what it holds, whether
//! the player can change it, and the system it belongs to.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::candidates::{Stat, class_of, stats, weapon_entries};
use svc_data::ships::{EItemSubType, EItemType, Loadout, Mounted, RecordCollection, is_editable};
use svc_data::{CookedData, Guid};

/// What a port's contents are part of, the way a pilot groups them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum System {
    Weapons,
    Turrets,
    Missiles,
    Countermeasures,
    Shields,
    PowerPlants,
    Coolers,
    Quantum,
    Radar,
    LifeSupport,
    Fuel,
    Thrusters,
    /// A docked vehicle (the Centaurus's Skylark): its own ship, on its own
    /// network.
    Docked,
    Other,
}

impl System {
    /// The system an item of `t` / `sub` belongs to on its own. A gimbal
    /// (`Turret` / `GunTurret`) is a mount, so it takes the system of what
    /// it holds; a crewed or remote turret is a turret.
    pub fn of(t: &EItemType, sub: &EItemSubType) -> Self {
        match t {
            EItemType::WeaponGun => Self::Weapons,
            EItemType::Turret if *sub == EItemSubType::GunTurret => Self::Other,
            EItemType::Turret | EItemType::TurretBase => Self::Turrets,
            EItemType::MissileLauncher | EItemType::Missile => Self::Missiles,
            EItemType::WeaponDefensive => Self::Countermeasures,
            EItemType::Shield => Self::Shields,
            EItemType::PowerPlant => Self::PowerPlants,
            EItemType::Cooler => Self::Coolers,
            EItemType::QuantumDrive | EItemType::JumpDrive => Self::Quantum,
            EItemType::Radar => Self::Radar,
            EItemType::LifeSupportGenerator => Self::LifeSupport,
            EItemType::FuelTank | EItemType::QuantumFuelTank | EItemType::FuelIntake => Self::Fuel,
            EItemType::MainThruster | EItemType::ManneuverThruster => Self::Thrusters,
            _ => Self::Other,
        }
    }
}

/// One port of a loadout and what it holds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Slot {
    /// Port names from the hull down.
    pub path: Vec<String>,
    /// The port's display name, resolved; `None` when it has none.
    pub label: Option<String>,
    pub min_size: i32,
    pub max_size: i32,
    /// The player may change what it holds.
    pub editable: bool,
    /// What it holds (`EntityClassDefinition` GUID); `None` when empty.
    pub item: Option<String>,
    /// The item's display name, else its record name.
    pub name: Option<String>,
    /// `AttachDef.Type` / `SubType` as the DCB spells them.
    pub item_type: Option<String>,
    pub sub_type: Option<String>,
    pub size: i32,
    pub grade: i32,
    /// Military, Civilian, Industrial, Stealth or Competition (components).
    pub class: Option<String>,
    /// The system this port's contents belong to: the item's own, or for a
    /// mount (gimbal, turret, rack) the first system found below it.
    pub system: System,
    /// The part's own figures (the picker's columns): a cooler's coolant,
    /// a launcher's ammunition, a tank's capacity. Empty for mounts and
    /// parts outside a system.
    pub stats: Vec<Stat>,
    pub children: Vec<Slot>,
}

/// `loadout` as slots, in loadout order. A port that holds nothing and
/// takes nothing the player can change is left out.
pub fn slots(cooked: &CookedData, loadout: &Loadout) -> Vec<Slot> {
    let mut path = Vec::new();
    let weapons = weapon_entries(cooked);
    walk(cooked, loadout, &loadout.mounted, &mut path, &weapons)
}

fn walk(
    cooked: &CookedData,
    loadout: &Loadout,
    level: &[Mounted],
    path: &mut Vec<String>,
    weapons: &HashMap<&str, &svc_data::ShipWeaponEntry>,
) -> Vec<Slot> {
    let index = &cooked.ships;
    let items = cooked.holotable.items.as_ref();
    let mut out = Vec::new();
    for m in level {
        path.push(m.port.clone());
        let refs: Vec<&str> = path.iter().map(String::as_str).collect();
        let chain = loadout.chain(&refs, &index.vehicles, &index.containers);
        let port = chain.as_ref().and_then(|(c, _)| c.last().copied());
        let editable = chain.as_ref().is_some_and(|(c, _)| is_editable(c));
        let item = m
            .item
            .as_ref()
            .and_then(|g| items.and_then(|i| i.get(g)).map(|i| (g, i)));
        // A docked vehicle is its own ship: one line, not its ports.
        let docked = m.item.is_some_and(|g| index.vehicles.get(&g).is_some());
        let children = if docked {
            Vec::new()
        } else {
            walk(cooked, loadout, &m.children, path, weapons)
        };
        let own = if docked {
            System::Docked
        } else {
            item.map_or(System::Other, |(_, i)| {
                System::of(&i.item_type, &i.item_sub_type)
            })
        };
        let system = if own == System::Other || own == System::Turrets {
            children
                .iter()
                .map(|c| c.system)
                .find(|s| *s != System::Other)
                .map_or(own, |below| {
                    if own == System::Turrets {
                        System::Turrets
                    } else {
                        below
                    }
                })
        } else {
            own
        };
        if m.item.is_some() || editable {
            out.push(Slot {
                path: path.clone(),
                label: port
                    .and_then(|p| p.display_name.as_ref())
                    .and_then(|k| cooked.locale.resolve(k))
                    .filter(|s| !s.is_empty())
                    .map(str::to_string),
                min_size: port.map_or(0, |p| p.min_size),
                max_size: port.map_or(0, |p| p.max_size),
                editable,
                item: m.item.as_ref().map(Guid::to_string),
                name: m.item.as_ref().map(|g| {
                    let display = if docked {
                        index
                            .vehicles
                            .get(g)
                            .and_then(|v| v.name_key.as_ref())
                            .and_then(|k| cooked.locale.resolve(k))
                    } else {
                        item.and_then(|(_, i)| i.display_name(&cooked.locale))
                    };
                    name_of(cooked, g, display)
                }),
                item_type: item.map(|(_, i)| i.item_type.as_dcb_str().to_string()),
                sub_type: item.map(|(_, i)| i.item_sub_type.as_dcb_str().to_string()),
                size: item.map_or(0, |(_, i)| i.size),
                grade: item.map_or(0, |(_, i)| i.grade),
                class: item.and_then(|(_, i)| class_of(cooked, i)),
                system,
                stats: match item {
                    Some((g, i)) if !docked && own != System::Other => {
                        stats(cooked, g, &i.item_type, weapons)
                    }
                    _ => Vec::new(),
                },
                children,
            });
        }
        path.pop();
    }
    out
}

/// An item's display name, else its record name, else its GUID. The
/// `<= PLACEHOLDER =>` name many fixed parts carry (thrusters, fuel intakes)
/// counts as none.
pub(crate) fn name_of(cooked: &CookedData, guid: &Guid, display: Option<&str>) -> String {
    if let Some(n) = display.filter(|n| !n.is_empty() && !n.contains("PLACEHOLDER")) {
        return n.to_string();
    }
    cooked
        .holotable
        .paths
        .as_ref()
        .and_then(|p| p.get(guid))
        .map(|p| readable(p.name.rsplit('.').next().unwrap_or(&p.name)))
        .unwrap_or_else(|| guid.to_string())
}

/// A component record name as a player would say it: the part after the
/// type, maker and size codes (`RADR_WLOP_S03_Lephari` → `Lephari`), else
/// the name as it is.
fn readable(record: &str) -> String {
    let parts: Vec<&str> = record.splitn(4, '_').collect();
    match parts.as_slice() {
        [kind, maker, size, rest]
            if kind.len() == 4
                && maker.len() == 4
                && size.starts_with('S')
                && size[1..].chars().all(|c| c.is_ascii_digit()) =>
        {
            rest.replace('_', " ")
        }
        _ => record.to_string(),
    }
}
