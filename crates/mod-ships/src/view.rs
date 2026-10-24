//! A ship with a loadout: its slots, its power network under the player's
//! settings, and the figures the page leads with (defence, flight,
//! quantum).
//!
//! A loadout travels as the ship's stock loadout plus edits (a port path
//! and what it now holds), applied in order the way a player swaps parts:
//! the new item brings its own default loadout. Saved loadouts are stored
//! the same way.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use svc_data::ships::{
    Allocation, Defense, EItemType, EMasterMode, Flight, Loadout, LoadoutContext, MinMax, Mounted,
    PerDamageType, PowerNetwork, Quantum, RecordCollection, ShipWeapon, Storage, SustainKind,
    Vehicle, network_items, weapon_power,
};
use svc_data::{CookedData, Guid};

use crate::power::{Power, PowerInput, network, power_of};
use crate::tree::{Slot, slots};

/// One swap: the port at `path` now holds `item` (`None` empties it).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edit {
    pub path: Vec<String>,
    pub item: Option<String>,
}

/// A ship and how it differs from stock.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadoutInput {
    /// The vehicle's `EntityClassDefinition` GUID.
    pub ship: String,
    pub edits: Vec<Edit>,
}

/// Why a loadout cannot be shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewError {
    /// Not a ship of this build.
    UnknownShip(String),
    /// The build's data has no ship index (cooked before ships existed).
    NotCooked,
}

impl std::fmt::Display for ViewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownShip(s) => write!(f, "this build's game data has no ship {s}"),
            Self::NotCooked => {
                write!(
                    f,
                    "this build's game data has no ships — reload it in Settings → Game data"
                )
            }
        }
    }
}

/// The figures the page leads with.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    pub hull_health: f32,
    pub armor_health: Option<f32>,
    pub shield_health: Option<f32>,
    /// HP/s at full power; [`Power::shield_regen`] is what the pips give.
    pub shield_regen_max: Option<f32>,
    pub flight: Option<Flight>,
    pub quantum: Option<QuantumSummary>,
    /// The radar's aim assist under the current power.
    pub aim_assist: Option<AimAssist>,
    /// The radar cross-section seen from the side, the front and above
    /// (the data's x / y / z) times the armour's signal multiplier, as the
    /// HUD shows it; `None` when it comes from the geometry.
    pub cross_section: Option<[f32; 3]>,
    pub storage: Option<StorageSummary>,
    pub armor: Option<ArmorSummary>,
    pub shield: Option<ShieldSummary>,
    pub hull: HullSummary,
    pub insurance: Option<InsuranceSummary>,
    /// The loadout's seats by what they control.
    pub crew: crate::Seats,
    pub missiles: Option<MissileSummary>,
    /// Boost at each thruster capacitor level 0..=6 (the client's default
    /// is [`svc_data::ships::DEFAULT_CAPACITOR_LEVEL`]); empty without an afterburner.
    pub boost: Vec<BoostLevel>,
}

/// The boost capacitor at one assignment level.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoostLevel {
    /// Empty to full, s, with the delay after use.
    pub regen_s: f32,
    /// Full to empty at rest, s; `None` when nothing drains at rest.
    pub deplete_s: Option<f32>,
    /// Boosted turn rate over normal: pitch, yaw, roll.
    pub turn: [f32; 3],
}

/// The missiles and what fires them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissileSummary {
    /// Missiles fitted.
    pub count: usize,
    /// The missile controller's `maxArmedMissiles` (the largest, when a
    /// ship has one per seat) and `launchCooldownTime`, s.
    pub max_armed: Option<i32>,
    pub cooldown: Option<f32>,
    /// Missile controllers (some ships have one per seat).
    pub controllers: usize,
}

/// Physical, energy and distortion: the damage types ship weapons deal.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Typed<T> {
    pub physical: T,
    pub energy: T,
    pub distortion: T,
}

impl<T: Copy> Typed<T> {
    pub(crate) fn of(v: &PerDamageType<T>) -> Self {
        Self {
            physical: v.physical,
            energy: v.energy,
            distortion: v.distortion,
        }
    }
}

/// The hull armour: what a hit must beat, and what gets through.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArmorSummary {
    /// The armour item's own HP.
    pub health: Option<f32>,
    /// A hit at or below this (scaled by the armour's health) does nothing.
    pub deflection: Option<Typed<f32>>,
    /// Damage the hull takes per point that gets through.
    pub hull_damage: Option<Typed<f32>>,
    /// Damage the armour itself takes per point (its durability).
    pub durability: Option<Typed<f32>>,
    /// Multipliers on the ship's IR, EM and cross-section.
    pub signals: [f32; 3],
    /// `basePenetrationReduction`.
    pub penetration_reduction: f32,
}

/// The shield generators as one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShieldSummary {
    /// `Bubble`, `Quadrant`, `FrontBack` or `Box` (the emitter's face
    /// layout); `None` without an emitter.
    pub face: Option<String>,
    pub generators: usize,
    /// The share of HP the crew can move between faces.
    pub max_reallocation: Option<f32>,
    /// Regen pauses after a hit / after a face drops, s.
    pub damaged_delay: f32,
    pub downed_delay: f32,
    /// The share of a hit the shield cancels, between min and max with
    /// power.
    pub resistance: Option<Typed<[f32; 2]>>,
    /// The share of the rest the face takes, between empty and full face.
    pub absorption: Option<Typed<[f32; 2]>>,
    /// The shields' pips now over their most: resistance is min + (max −
    /// min) × this (client 4.10, read by sc-holotable; that the ratio is
    /// pips over most pips is inferred).
    pub ratio: f32,
    /// The shield column's pips now and its most; `None` without a power
    /// network.
    pub pips: Option<(f64, i64)>,
}

/// The hull's structure.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HullSummary {
    /// Bounding box, m: width, length, height.
    pub dimensions: Option<[f32; 3]>,
    /// The hull alone, kg.
    pub hull_mass: Option<f32>,
    /// Parts with HP, depth-first; the first is the vital body.
    pub parts: Vec<HullPart>,
    /// On a penetrating hit: damage to the first live fuse of a relay, and
    /// to every other component, per point.
    pub fuse_multiplier: f32,
    pub component_multiplier: f32,
    /// Thrusters, by kind: they carry their own HP, outside the hull's.
    pub thrusters: Vec<ThrusterPart>,
    /// Doors with HP (interior and fitted), by name.
    pub doors: Vec<DoorSummary>,
}

/// One thruster.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThrusterPart {
    /// `Main`, `Retro`, `VTOL` or `Maneuvering`.
    pub kind: String,
    /// Where it sits (`Main Bottom Left`).
    pub place: String,
    pub hp: f32,
}

/// Doors of one kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DoorSummary {
    pub name: String,
    pub count: usize,
    pub hp: f32,
    /// Its record name says `Breachable` (the only place the data says
    /// it).
    pub breachable: bool,
}

/// One part of the hull.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HullPart {
    pub name: String,
    /// 0 for the body, 1 for what hangs off it, ...
    pub depth: u8,
    pub hp: f32,
    /// It breaks off (`detachRatio`).
    pub breakable: bool,
    /// Losing it destroys the ship (its damage behaviour fires `Destroy`).
    #[serde(default)]
    pub vital: bool,
}

/// What an insurance claim costs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InsuranceSummary {
    pub wait_minutes: f32,
    pub mandatory_minutes: f32,
    /// aUEC.
    pub expedite_fee: u32,
}

/// What the ship stores (sc-vehicles' `Storage`: matches erkul's cargo on
/// the Centaurus, Freelancer and Cutlass and the published totals).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StorageSummary {
    /// Cargo grids, SCU.
    pub cargo: f64,
    /// Lockers, personal and seat storage, SCU.
    pub lockers: f64,
    /// The ship's own inventory, SCU.
    pub inventory: Option<f64>,
    pub suit_lockers: usize,
    /// Weapon racks (fire-extinguisher cabinets left out) and their slots
    /// for primaries, secondaries and launchers.
    pub weapon_racks: usize,
    pub rack_slots: usize,
}

/// The stock-or-edited loadout's storage.
pub(crate) fn storage(
    cooked: &CookedData,
    vehicle: &svc_data::ships::Vehicle,
    guid: &Guid,
    mounted: &[Mounted],
) -> Option<StorageSummary> {
    let index = &cooked.ships;
    let items = cooked.holotable.items.as_ref()?;
    let entities = index.interiors.get(guid).map_or(&[][..], Vec::as_slice);
    let s = Storage::compute(
        vehicle,
        mounted,
        entities,
        &index.vehicles,
        items,
        &index.containers,
    );
    let weapon = |r: &svc_data::ships::Rack| {
        r.slots
            .iter()
            .filter(|s| {
                s.accepts
                    .iter()
                    .any(|t| t.item_type == EItemType::WeaponPersonal)
            })
            .count()
    };
    let racks: Vec<_> = s.weapon_racks.iter().filter(|r| weapon(r) > 0).collect();
    Some(StorageSummary {
        cargo: s.cargo_scu(),
        lockers: s.locker_scu(),
        inventory: s.ship_inventory,
        suit_lockers: s.suit_lockers.len(),
        weapon_racks: racks.len(),
        rack_slots: racks.iter().map(|r| weapon(r)).sum(),
    })
}

/// How far the radar's aim assist reaches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AimAssist {
    /// m, at the radar's current power.
    pub distance: f64,
    /// m, at full power.
    pub max: f64,
}

/// The quantum drive's travel, sampled for the travel-time chart.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuantumSummary {
    pub drive: String,
    /// m/s.
    pub speed: f32,
    pub spool_up: f32,
    /// s after a jump before the next spool, for a jump long enough to
    /// reach full speed; a shorter jump waits `cooldown × peak / speed`
    /// (client code, LIVE 4.10, sc-holotable's flight-model note).
    #[serde(default)]
    pub cooldown: f32,
    /// SCU per Gm.
    pub fuel_per_gm: f64,
    pub fuel_capacity: f64,
    /// Gm on a full tank; `None` when it burns nothing.
    pub range_gm: Option<f64>,
    /// (distance Gm, seconds), 0.5 Gm apart up to 60 Gm: the spool-up,
    /// then the travel (both acceleration stages, cruise, braking).
    pub times: Vec<(f64, f64)>,
    /// The same, 5 Gm apart up to 300 Gm, for routes past the chart
    /// (beyond cruise the time grows linearly, so between points it is
    /// exact to interpolate).
    pub far: Vec<(f64, f64)>,
    /// A planet's moons hopped in a row ([`crate::routes::moon_tour`]);
    /// `None` without waypoints.
    #[serde(default)]
    pub tour: Option<Tour>,
}

/// Several jumps in a row with one drive: each leg's spool-up and travel,
/// and before each next leg the cooldown the last one left (sized by its
/// peak speed: a short hop cools less). On one long jump the cooldown runs
/// after arrival and costs nothing; on hops it is the drive's real cost.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tour {
    /// The planet whose moons it visits.
    pub planet: String,
    pub legs: u32,
    /// Gm, all legs.
    pub gm: f64,
    pub seconds: f64,
    /// Of `seconds`, waiting on the cooldown.
    pub cooling: f64,
}

/// What the page shows for one loadout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShipView {
    pub ship: String,
    pub slots: Vec<Slot>,
    /// `None` when the build has no network rules.
    pub power: Option<Power>,
    /// Each gun on the ship's own network, under the power its pool gives
    /// it.
    pub guns: Vec<Gun>,
    pub summary: Summary,
}

/// One gun's damage under the current power.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Gun {
    /// Port path, `/`-joined (the network's spelling).
    pub path: String,
    /// Damage per second while firing, before heat or the capacitor stop
    /// it.
    pub burst: Option<f32>,
    /// Damage per second over a long fight: the capacitor's regen (energy)
    /// or the heat cycle (ballistic), at this power.
    pub sustained: Option<f32>,
    /// The gun's power ratio (granted over requested).
    pub power: f32,
    /// Seconds of continuous fire before the capacitor runs dry (energy,
    /// shrinking with the gun's power: fewer pips load fewer shots) or the
    /// gun overheats (ballistic).
    #[serde(default)]
    pub fires: Option<f32>,
    /// Shots the capacitor holds at this power; `None` for a gun with
    /// rounds.
    #[serde(default)]
    pub loaded: Option<f32>,
    /// Damage per second over a [`WINDOW`]-second engagement from a cold,
    /// full gun (spviewer's "continuous fire" DPS): burst while it lasts,
    /// then what regen or the heat cycle allows.
    #[serde(default)]
    pub window: Option<f32>,
}

/// The engagement [`Gun::window`] is figured over, s (spviewer's default).
pub const WINDOW: f32 = 30.0;

/// Every gun's damage: sc-vehicles gives each gun its power (pool share and
/// the ship's capacitor modifiers, e.g. the Mk V's buff), sc-weapons turns
/// the sheet at that power into DPS.
fn guns(
    cooked: &CookedData,
    loadout: &Loadout,
    net: &PowerNetwork,
    a: &Allocation,
    mode: &EMasterMode,
) -> Vec<Gun> {
    let index = &cooked.ships;
    let sheets: HashMap<&Guid, &ShipWeapon> = index.guns.iter().map(|w| (&w.guid, w)).collect();
    weapon_power(
        net,
        a,
        mode,
        &loadout.mounted,
        &index.vehicles,
        &index.modifiers,
    )
    .into_iter()
    .filter_map(|(i, p)| {
        let c = &net.consumers[i];
        let sheet = sheets.get(&c.item)?;
        let powered = sheet.powered(&p);
        let draws = c.draws(mode);
        let loaded = match &powered.sustain {
            SustainKind::Energy(e) => Some(e.max_ammo_load),
            _ => None,
        };
        Some(Gun {
            path: c.path.clone(),
            burst: powered.burst_dps(),
            sustained: if draws {
                powered.sustained_dps()
            } else {
                Some(0.0)
            },
            power: p.ratio,
            fires: powered.burst_seconds(),
            loaded,
            // `effective_dps` powers the sheet itself.
            window: if draws {
                sheet.effective_dps(&LoadoutContext {
                    window_seconds: WINDOW,
                    power: Some(p),
                })
            } else {
                Some(0.0)
            },
        })
    })
    .collect()
}

/// The loadout `input` describes: stock, then each edit in order. An edit
/// whose path names no port is skipped.
pub fn loadout(cooked: &CookedData, input: &LoadoutInput) -> Result<Loadout, ViewError> {
    let index = &cooked.ships;
    if index.is_empty() {
        return Err(ViewError::NotCooked);
    }
    let guid: Guid = input
        .ship
        .parse()
        .map_err(|_| ViewError::UnknownShip(input.ship.clone()))?;
    let mut loadout = Loadout::stock(&index.vehicles, &guid)
        .ok_or_else(|| ViewError::UnknownShip(input.ship.clone()))?;
    for e in &input.edits {
        let path: Vec<&str> = e.path.iter().map(String::as_str).collect();
        let item = e.item.as_ref().and_then(|i| i.parse().ok());
        let before = loadout
            .at(&path)
            .map(|m| m.children.clone())
            .unwrap_or_default();
        loadout.equip(&path, item, &index.vehicles, &index.containers);
        if item.is_some() {
            carry(cooked, &mut loadout, &e.path, &before);
        }
    }
    Ok(loadout)
}

/// What the replaced part held stays where the new part has the same port
/// and it fits there: a quantum drive swap keeps the jump module fitted,
/// as a player swapping only the drive expects. Below that, as far as it
/// fits, the same again.
fn carry(cooked: &CookedData, loadout: &mut Loadout, at: &[String], before: &[Mounted]) {
    let index = &cooked.ships;
    let Some(items) = cooked.holotable.items.as_ref() else {
        return;
    };
    for old in before {
        let Some(g) = old.item else {
            continue;
        };
        let mut path = at.to_vec();
        path.push(old.port.clone());
        let refs: Vec<&str> = path.iter().map(String::as_str).collect();
        let Some(item) = items.get(&g) else {
            continue;
        };
        let fits = loadout
            .check(&refs, item, items, &index.vehicles, &index.containers)
            .is_ok();
        if fits && loadout.at(&refs).and_then(|m| m.item) != Some(g) {
            let below = old.children.clone();
            loadout.equip(&refs, Some(g), &index.vehicles, &index.containers);
            carry(cooked, loadout, &path, &below);
        }
    }
}

/// The flight controller's boost at each capacitor level.
fn boost(cooked: &CookedData, mounted: &[Mounted]) -> Vec<BoostLevel> {
    let index = &cooked.ships;
    let Some(ab) = network_items(mounted, &index.vehicles)
        .iter()
        .find_map(|(_, g)| {
            index
                .components
                .get(g)?
                .flight
                .as_ref()?
                .afterburner
                .clone()
        })
    else {
        return Vec::new();
    };
    (0..=6)
        .map(|level| {
            let b = ab.at(level);
            // sc-items-ship-components' axes are pitch, roll, yaw.
            let [pitch, roll, yaw] = b.angular_velocity_multiplier;
            BoostLevel {
                regen_s: b.regen_s,
                deplete_s: b.deplete_s,
                turn: [pitch, yaw, roll],
            }
        })
        .collect()
}

/// The missiles fitted and the controller that arms them.
fn missiles(cooked: &CookedData, mounted: &[Mounted]) -> Option<MissileSummary> {
    let index = &cooked.ships;
    let items = cooked.holotable.items.as_ref()?;
    let all = network_items(mounted, &index.vehicles);
    let count = all
        .iter()
        .filter(|(_, g)| {
            items
                .get(g)
                .is_some_and(|i| i.item_type == EItemType::Missile)
        })
        .count();
    let controllers: Vec<_> = all
        .iter()
        .filter_map(|(_, g)| index.components.get(g)?.missiles.as_ref())
        .collect();
    if count == 0 && controllers.is_empty() {
        return None;
    }
    let top = controllers.iter().max_by_key(|c| c.max_armed);
    Some(MissileSummary {
        count,
        max_armed: top.map(|c| c.max_armed),
        cooldown: top.map(|c| c.launch_cooldown),
        controllers: controllers.len(),
    })
}

/// The armour sheet with the armour item's own durability.
fn armor(cooked: &CookedData, defense: &Defense, mounted: &[Mounted]) -> Option<ArmorSummary> {
    let index = &cooked.ships;
    let a = defense.armor.as_ref()?;
    let durability = network_items(mounted, &index.vehicles)
        .iter()
        .filter_map(|(_, g)| index.components.get(g))
        .find(|s| s.armor.is_some())
        .and_then(|s| s.health.as_ref()?.damage_multiplier.as_ref())
        .map(Typed::of);
    Some(ArmorSummary {
        health: defense.armor_health,
        deflection: a.deflection.as_ref().map(Typed::of),
        hull_damage: a.damage_multiplier.as_ref().map(Typed::of),
        durability,
        signals: [
            a.signal_infrared,
            a.signal_electromagnetic,
            a.signal_cross_section,
        ],
        penetration_reduction: a.penetration_reduction,
    })
}

/// The hull's parts with HP, as a tree flattened depth-first. Item ports
/// (thrusters, weapons) are items, not hull, and stay out.
fn hull(cooked: &CookedData, vehicle: &Vehicle, guid: &Guid, mounted: &[Mounted]) -> HullSummary {
    let parts = &vehicle.hull;
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); parts.len()];
    let mut roots = Vec::new();
    for (i, p) in parts.iter().enumerate() {
        match p.parent {
            Some(up) if up < parts.len() => children[up].push(i),
            _ => roots.push(i),
        }
    }
    let hp = |i: usize| {
        let p = &parts[i];
        if p.class == "ItemPort" {
            0.0
        } else {
            p.damage_max
        }
    };
    // A part is listed when it or something below it has HP.
    let mut listed = vec![false; parts.len()];
    for i in (0..parts.len()).rev() {
        listed[i] = hp(i) > 0.0 || children[i].iter().any(|&c| listed[c]);
    }
    // The vehicle's own root part holds no HP: its body is the top.
    while let [r] = roots[..]
        && hp(r) <= 0.0
        && !children[r].is_empty()
    {
        roots = children[r].clone();
    }
    let mut out = Vec::new();
    let mut stack: Vec<(usize, u8)> = roots.iter().rev().map(|&r| (r, 0)).collect();
    while let Some((i, depth)) = stack.pop() {
        if !listed[i] {
            continue;
        }
        let p = &parts[i];
        out.push(HullPart {
            name: part_name(&p.name),
            depth,
            hp: hp(i),
            breakable: !p.destroys_vehicle && p.detach_ratio.is_some_and(|r| r > 0.0),
            vital: p.destroys_vehicle,
        });
        stack.extend(
            children[i]
                .iter()
                .rev()
                .map(|&c| (c, depth.saturating_add(1))),
        );
    }
    HullSummary {
        dimensions: vehicle.bounding_box,
        hull_mass: vehicle.hull_mass(),
        parts: out,
        fuse_multiplier: vehicle.penetration.fuse_multiplier,
        component_multiplier: vehicle.penetration.component_multiplier,
        thrusters: thrusters(cooked, mounted),
        doors: doors(cooked, guid, mounted),
    }
}

/// The loadout's thrusters, grouped as the game's thruster types (VTOL
/// apart), with their own HP.
fn thrusters(cooked: &CookedData, mounted: &[Mounted]) -> Vec<ThrusterPart> {
    let index = &cooked.ships;
    let mut out: Vec<ThrusterPart> = network_items(mounted, &index.vehicles)
        .iter()
        .filter_map(|(path, g)| {
            let sheet = index.components.get(g)?;
            let t = sheet.thruster.as_ref()?;
            let kind = if t.only_active_in_vtol {
                "VTOL".to_string()
            } else {
                match format!("{:?}", t.thruster_type).as_str() {
                    "Maneuver" => "Maneuvering".to_string(),
                    other => other.to_string(),
                }
            };
            let port = path.rsplit('/').next().unwrap_or(path);
            let place = part_name(
                port.trim_start_matches("hardpoint_")
                    .trim_start_matches("thruster_"),
            );
            Some(ThrusterPart {
                kind,
                place,
                hp: sheet.health.as_ref().map_or(0.0, |h| h.health),
            })
        })
        .collect();
    let rank = |k: &str| {
        ["Main", "Retro", "VTOL", "Maneuvering"]
            .iter()
            .position(|x| *x == k)
            .unwrap_or(4)
    };
    out.sort_by(|a, b| {
        rank(&a.kind)
            .cmp(&rank(&b.kind))
            .then(a.place.cmp(&b.place))
    });
    out
}

/// The doors with HP the interior places and the loadout fits, by name.
fn doors(cooked: &CookedData, guid: &Guid, mounted: &[Mounted]) -> Vec<DoorSummary> {
    let index = &cooked.ships;
    let items = cooked.holotable.items.as_ref();
    let placed = index
        .interiors
        .get(guid)
        .into_iter()
        .flatten()
        .map(|e| (e.class, Some(e.name.as_str())));
    let fitted = network_items(mounted, &index.vehicles)
        .into_iter()
        .map(|(_, g)| (g, None));
    // The ship's own words (`Constellation`, `Mk5`) say nothing in a door's
    // name aboard it.
    let ship: Vec<String> = index
        .listed
        .iter()
        .find(|e| e.guid == *guid)
        .map(|e| e.record_name.split('_').map(str::to_lowercase).collect())
        .unwrap_or_default();
    let mut out: Vec<DoorSummary> = Vec::new();
    for (g, instance) in placed.chain(fitted) {
        let Some(&hp) = index.door_health.get(&g) else {
            continue;
        };
        // Most doors' display name is just "Door": the record name says
        // which one it is.
        let display = items
            .and_then(|i| i.get(&g))
            .and_then(|i| i.display_name(&cooked.locale))
            .filter(|n| !n.eq_ignore_ascii_case("door"));
        let raw = crate::tree::name_of(cooked, &g, display);
        let breachable = raw.split('_').any(|w| w.eq_ignore_ascii_case("breachable"));
        // An interior door's instance name often says where it leads
        // (`Door_Bridge_to_Armory`).
        let name = instance
            .and_then(room_door)
            .unwrap_or_else(|| door_name(&raw, &ship));
        match out.iter_mut().find(|d| d.name == name && d.hp == hp) {
            Some(d) => d.count += 1,
            None => out.push(DoorSummary {
                name,
                count: 1,
                hp,
                breachable,
            }),
        }
    }
    out.sort_by(|a, b| b.hp.total_cmp(&a.hp).then(a.name.cmp(&b.name)));
    out
}

/// `Door_Bridge_to_Armory-001` -> `Bridge to Armory`; `None` for an
/// instance name that only repeats the record's (`Door_RN_…`).
fn room_door(instance: &str) -> Option<String> {
    let rest = instance.strip_prefix("Door_")?;
    let rest = rest.split('-').next().unwrap_or(rest);
    if rest.starts_with("RN_") || !rest.contains("_to_") && !rest.contains('_') {
        return None;
    }
    let words: Vec<&str> = rest.split('_').filter(|w| !w.is_empty()).collect();
    Some(
        words
            .iter()
            .map(|w| {
                if *w == "to" {
                    "to".to_string()
                } else {
                    let mut c = w.chars();
                    c.next()
                        .map(|f| f.to_uppercase().chain(c).collect())
                        .unwrap_or_default()
                }
            })
            .collect::<Vec<String>>()
            .join(" "),
    )
}

/// A door record name as words: the door's own part, without the
/// technical ones and the ship's (`Door_RN_RoomConnector_Breachable_
/// OpenClose_Constellation_Mk5_Airlock` -> `Airlock`). A door that says
/// nothing else is named by kind. A display name stays as it is.
fn door_name(name: &str, ship: &[String]) -> String {
    if !name.contains('_') {
        return name.to_string();
    }
    const TECHNICAL: &[&str] = &[
        "door",
        "doors",
        "rn",
        "ip",
        "roomconnector",
        "noroomconnector",
        "breachable",
        "openclose",
        "openreverse",
        "component",
    ];
    let lower: Vec<String> = name.split('_').map(str::to_lowercase).collect();
    let words: Vec<&str> = name
        .split('_')
        .zip(&lower)
        .filter(|(w, l)| !w.is_empty() && !TECHNICAL.contains(&l.as_str()) && !ship.contains(l))
        .map(|(w, _)| w)
        .collect();
    if !words.is_empty() {
        return part_name(&words.join("_"));
    }
    if lower.iter().any(|l| l == "component") {
        "Component hatch".into()
    } else if lower.iter().any(|l| l == "roomconnector") {
        "Interior door".into()
    } else {
        "Door".into()
    }
}

/// `Dbr_Nacelle_Bottom_Left` -> `Nacelle Bottom Left`: the part's name
/// without the geometry / debris prefix.
fn part_name(raw: &str) -> String {
    let words: Vec<&str> = raw.split(['_', ' ']).filter(|w| !w.is_empty()).collect();
    let skip = usize::from(
        words.len() > 1
            && ["geo", "dbr", "part"]
                .iter()
                .any(|p| words[0].eq_ignore_ascii_case(p)),
    );
    words[skip..]
        .iter()
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_uppercase().chain(c).collect())
                .unwrap_or_default()
        })
        .collect::<Vec<String>>()
        .join(" ")
}

/// The page for `input` under `settings`.
pub fn view(
    cooked: &CookedData,
    input: &LoadoutInput,
    settings: &PowerInput,
) -> Result<ShipView, ViewError> {
    let index = &cooked.ships;
    let loadout = loadout(cooked, input)?;
    let vehicle = index
        .vehicles
        .get(&loadout.vehicle)
        .ok_or_else(|| ViewError::UnknownShip(input.ship.clone()))?;
    let empty = Default::default();
    let items = cooked.holotable.items.as_ref().unwrap_or(&empty);
    let net = index.network.as_ref().map(|params| {
        network(
            &loadout.mounted,
            index.containers.get(&loadout.vehicle),
            &index.vehicles,
            items,
            &index.profiles,
            params,
            settings,
        )
    });
    let defense = Defense::compute(
        vehicle,
        &loadout.mounted,
        &index.vehicles,
        &index.components,
    );
    let power = net
        .as_ref()
        .map(|(n, a, mode)| power_of(n, a, mode, settings.mode, vehicle, defense.armor.as_ref()));
    let guns = net
        .as_ref()
        .map(|(n, a, mode)| guns(cooked, &loadout, n, a, mode))
        .unwrap_or_default();
    // Aim assist under the radar's power (sc-items-ship-components, read
    // from the client: continuous in the radar's functionality ratio,
    // which is its power ratio while its coolant is met). With several
    // radars the farthest wins; 0 when the radar is off.
    let aim_assist = net.as_ref().and_then(|(n, a, mode)| {
        let ratios = n.ratios(a, mode);
        n.consumers
            .iter()
            .zip(ratios)
            .filter_map(|(c, r)| {
                let radar = index.components.get(&c.item)?.radar.as_ref()?;
                Some(AimAssist {
                    distance: f64::from(radar.aim_assist(r).unwrap_or(0.0)),
                    max: f64::from(radar.aim_assist_max),
                })
            })
            .max_by(|x, y| x.distance.total_cmp(&y.distance))
    });
    // The shield column's pips over its most: resistance follows it.
    let pips = power.as_ref().and_then(|p| {
        let c = p.columns.iter().find(|c| c.id == "pool:Shield")?;
        Some((if c.excluded { 0.0 } else { c.level }, c.max))
    });
    let ratio = pips.map_or(1.0, |(level, max)| {
        if max > 0 {
            (level / max as f64).clamp(0.0, 1.0) as f32
        } else {
            0.0
        }
    });
    let range = |v: &PerDamageType<MinMax>| Typed {
        physical: [v.physical.min, v.physical.max],
        energy: [v.energy.min, v.energy.max],
        distortion: [v.distortion.min, v.distortion.max],
    };
    let shield = defense.shield.as_ref().map(|s| ShieldSummary {
        face: defense
            .emitter
            .as_ref()
            .map(|e| format!("{:?}", e.face_type)),
        generators: s.generators,
        max_reallocation: defense.emitter.as_ref().map(|e| e.max_reallocation),
        damaged_delay: s.damaged_regen_delay,
        downed_delay: s.downed_regen_delay,
        resistance: s.resistance.as_ref().map(range),
        absorption: s.absorption.as_ref().map(range),
        ratio,
        pips,
    });
    let no_axes = Default::default();
    let flight = Flight::compute(
        vehicle,
        &loadout.mounted,
        &index.vehicles,
        items,
        &index.components,
        index.axes.get(&loadout.vehicle).unwrap_or(&no_axes),
    );
    let quantum = Quantum::compute(
        &loadout.mounted,
        &index.vehicles,
        &index.components,
        &index.profiles,
    )
    .map(|q| {
        // A jump as the player waits for it: spool-up, then the travel
        // (sc-vehicles' `time` is the travel alone). Nothing at 0 Gm.
        let spool = f64::from(q.travel.spool_up_time);
        let jump = |gm: f64| {
            if gm > 0.0 {
                spool + q.time(gm * 1e9)
            } else {
                0.0
            }
        };
        QuantumSummary {
            drive: q.drive.to_string(),
            speed: q.travel.drive_speed,
            spool_up: q.travel.spool_up_time,
            cooldown: q.travel.cooldown_time,
            fuel_per_gm: q.fuel_per_mm * 1000.0,
            fuel_capacity: q.fuel_capacity,
            range_gm: q.range().is_finite().then(|| q.range() / 1e9),
            times: (0..=120)
                .map(|i| {
                    let gm = f64::from(i) * 0.5;
                    (gm, jump(gm))
                })
                .collect(),
            far: (0..=60)
                .map(|i| {
                    let gm = f64::from(i) * 5.0;
                    (gm, jump(gm))
                })
                .collect(),
            tour: crate::routes::moon_tour(&crate::routes::places(cooked)).map(|(planet, legs)| {
                let trips: Vec<_> = legs.iter().map(|&d| q.trip(d)).collect();
                let moving: f64 = trips.iter().map(|t| spool + t.travel).sum();
                // No wait after the last leg: the trip is over.
                let cooling: f64 = trips.iter().rev().skip(1).map(|t| t.cooldown).sum();
                Tour {
                    planet,
                    legs: legs.len() as u32,
                    gm: legs.iter().sum::<f64>() / 1e9,
                    seconds: moving + cooling,
                    cooling,
                }
            }),
        }
    });
    Ok(ShipView {
        ship: input.ship.clone(),
        slots: slots(cooked, &loadout),
        power,
        guns,
        summary: Summary {
            hull_health: defense.hull_health,
            armor_health: defense.armor_health,
            shield_health: defense.shield.as_ref().map(|s| s.max_health),
            shield_regen_max: defense.shield.as_ref().map(|s| s.max_regen),
            flight,
            quantum,
            aim_assist,
            // As ShipSignature scales it, which needs a network's readout
            // this doesn't: the hull's axes times the armour's multiplier.
            cross_section: vehicle.cross_section.map(|axes| {
                let x = defense
                    .armor
                    .as_ref()
                    .map_or(1.0, |a| a.signal_cross_section);
                axes.map(|v| v * x)
            }),
            storage: storage(cooked, vehicle, &loadout.vehicle, &loadout.mounted),
            boost: boost(cooked, &loadout.mounted),
            armor: armor(cooked, &defense, &loadout.mounted),
            shield,
            hull: hull(cooked, vehicle, &loadout.vehicle, &loadout.mounted),
            crew: crate::seats(cooked, vehicle, &loadout.mounted),
            missiles: missiles(cooked, &loadout.mounted),
            insurance: vehicle
                .insurance
                .as_ref()
                .or_else(|| {
                    let base = index.vehicles.get(vehicle.insured_as.as_ref()?)?;
                    base.insurance.as_ref()
                })
                .map(|i| InsuranceSummary {
                    wait_minutes: i.base_wait_minutes,
                    mandatory_minutes: i.mandatory_wait_minutes,
                    expedite_fee: i.expediting_fee,
                }),
        },
    })
}
