//! Ships — the shell side of the module: the loadout manager's IPC surface.
//!
//! What a ship carries and how its power network shares out is `mod-ships`;
//! the reference data is svc-data's cooked ship index. Nothing here touches
//! the network, so no online gate applies.

use mod_ships::{
    Candidate, Edit, LoadoutInput, Mode, Power, PowerInput, ShipRow, ShipView, Slot, Stat, System,
    Typed,
};
use std::collections::HashMap;
use svc_data::Guid;
use svc_data::ships::{Accelerations, Loadout};
use tauri::AppHandle;

use crate::data::cooked_for;
use crate::error::AppError;

/// Module id (Rust registry, settings, frontend descriptor).
pub(crate) const MODULE_ID: &str = "ships";

// ── Views ───────────────────────────────────────────────────────────────────

/// One ship in the picker.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ShipRowView {
    pub guid: String,
    pub record_name: String,
    pub name: String,
    pub manufacturer: Option<String>,
    pub manufacturer_code: Option<String>,
    pub career: Option<String>,
    pub role: Option<String>,
    /// CIG's crew figure (`crewSize`).
    pub crew: i32,
    /// The stock loadout's seats by what they control.
    pub seats: SeatsView,
    /// The base hull this is an edition of, when it is one.
    pub base: Option<String>,
    pub ground: bool,
    pub reward: bool,
    /// Cargo grids of the stock loadout, SCU.
    pub cargo: Option<f64>,
    /// The stock armour's deflection: a hit at or below it does nothing.
    pub deflection: Option<TypedView>,
}

impl From<ShipRow> for ShipRowView {
    fn from(r: ShipRow) -> Self {
        Self {
            guid: r.guid,
            record_name: r.record_name,
            name: r.name,
            manufacturer: r.manufacturer,
            manufacturer_code: r.manufacturer_code,
            career: r.career,
            role: r.role,
            crew: r.crew,
            seats: r.seats.into(),
            base: r.base,
            ground: r.ground,
            reward: r.reward,
            cargo: r.cargo,
            deflection: r.deflection.map(Into::into),
        }
    }
}

/// Seats by what their occupant controls.
#[derive(Debug, Clone, Copy, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SeatsView {
    pub pilot: u32,
    pub copilot: u32,
    pub manned_turrets: u32,
    pub remote_turrets: u32,
    pub mining: u32,
    pub salvage: u32,
    /// Seats with any role.
    pub crew: u32,
    pub passengers: u32,
}

impl From<mod_ships::Seats> for SeatsView {
    fn from(s: mod_ships::Seats) -> Self {
        let n = |v: usize| v as u32;
        Self {
            pilot: n(s.pilot),
            copilot: n(s.copilot),
            manned_turrets: n(s.manned_turrets),
            remote_turrets: n(s.remote_turrets),
            mining: n(s.mining),
            salvage: n(s.salvage),
            crew: n(s.crew),
            passengers: n(s.passengers),
        }
    }
}

/// Pitch, roll, yaw (sc-vehicles' order) as pitch, yaw, roll.
fn pyr(v: [f32; 3]) -> Vec<f32> {
    vec![v[0], v[2], v[1]]
}

/// The boost capacitor at one assignment level.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BoostView {
    pub regen_s: f32,
    pub deplete_s: Option<f32>,
    /// Boosted turn over normal: pitch, yaw, roll.
    pub turn: Vec<f32>,
}

/// The missiles and what fires them.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MissilesView {
    pub count: u32,
    pub max_armed: Option<i32>,
    pub cooldown: Option<f32>,
    pub controllers: u32,
}

/// One thruster.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ThrusterView {
    /// `Main`, `Retro`, `VTOL` or `Maneuvering`.
    pub kind: String,
    pub place: String,
    pub hp: f32,
}

/// Doors of one kind.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DoorView {
    pub name: String,
    pub count: u32,
    pub hp: f32,
    /// Its record name says `Breachable`.
    pub breachable: bool,
}

/// What a port's contents belong to.
#[derive(Debug, Clone, Copy, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum SystemView {
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
    Docked,
    Other,
}

impl From<System> for SystemView {
    fn from(s: System) -> Self {
        match s {
            System::Weapons => Self::Weapons,
            System::Turrets => Self::Turrets,
            System::Missiles => Self::Missiles,
            System::Countermeasures => Self::Countermeasures,
            System::Shields => Self::Shields,
            System::PowerPlants => Self::PowerPlants,
            System::Coolers => Self::Coolers,
            System::Quantum => Self::Quantum,
            System::Radar => Self::Radar,
            System::LifeSupport => Self::LifeSupport,
            System::Fuel => Self::Fuel,
            System::Thrusters => Self::Thrusters,
            System::Docked => Self::Docked,
            System::Other => Self::Other,
        }
    }
}

/// One port of a loadout and what it holds.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SlotView {
    pub path: Vec<String>,
    pub label: Option<String>,
    pub min_size: i32,
    pub max_size: i32,
    pub editable: bool,
    pub item: Option<String>,
    pub name: Option<String>,
    pub item_type: Option<String>,
    pub sub_type: Option<String>,
    pub size: i32,
    pub grade: i32,
    pub class: Option<String>,
    pub system: SystemView,
    pub stats: Vec<StatView>,
    pub children: Vec<SlotView>,
}

impl From<Slot> for SlotView {
    fn from(s: Slot) -> Self {
        Self {
            path: s.path,
            label: s.label,
            min_size: s.min_size,
            max_size: s.max_size,
            editable: s.editable,
            item: s.item,
            name: s.name,
            item_type: s.item_type,
            sub_type: s.sub_type,
            size: s.size,
            grade: s.grade,
            class: s.class,
            system: s.system.into(),
            stats: s.stats.into_iter().map(Into::into).collect(),
            children: s.children.into_iter().map(Into::into).collect(),
        }
    }
}

/// One swap on top of the stock loadout.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EditView {
    pub path: Vec<String>,
    pub item: Option<String>,
}

/// A ship and how it differs from stock.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ShipLoadoutView {
    pub ship: String,
    pub edits: Vec<EditView>,
}

impl From<ShipLoadoutView> for LoadoutInput {
    fn from(v: ShipLoadoutView) -> Self {
        Self {
            ship: v.ship,
            edits: v
                .edits
                .into_iter()
                .map(|e| Edit {
                    path: e.path,
                    item: e.item,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum ModeView {
    Scm,
    Nav,
}

/// The player's hand on the power network.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PowerSettingsView {
    pub mode: ModeView,
    /// Port paths (`/`-joined) of items switched off.
    pub off: Vec<String>,
    /// Columns set by hand, in the order they were set.
    pub levels: Vec<LevelView>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LevelView {
    pub column: String,
    pub pips: i32,
}

impl From<PowerSettingsView> for PowerInput {
    fn from(v: PowerSettingsView) -> Self {
        Self {
            mode: match v.mode {
                ModeView::Scm => Mode::Scm,
                ModeView::Nav => Mode::Nav,
            },
            off: v.off,
            levels: v
                .levels
                .into_iter()
                .map(|l| (l.column, i64::from(l.pips)))
                .collect(),
        }
    }
}

/// One column of the power screen.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ColumnView {
    pub id: String,
    pub item_type: String,
    pub items: Vec<String>,
    pub max: i32,
    pub minimums: Vec<i32>,
    pub level: f64,
    pub off: bool,
    pub excluded: bool,
}

/// The power network after the player's settings.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PowerView {
    pub mode: ModeView,
    pub supply: i32,
    pub used: f64,
    pub columns: Vec<ColumnView>,
    pub cooling: Option<f64>,
    pub coolant_made: f64,
    pub coolant_asked: f64,
    pub em: f64,
    pub ir: f64,
    pub shield_regen: f64,
    /// Power plants by port path, and whether each is on.
    pub plants: Vec<PlantView>,
    /// EM and IR by item type, largest first (IR: what the coolers give
    /// off for that type's heat).
    pub sources: Vec<SourceView>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SourceView {
    pub item_type: String,
    pub em: f64,
    pub ir: f64,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlantView {
    pub path: String,
    pub on: bool,
}

impl From<Power> for PowerView {
    fn from(p: Power) -> Self {
        Self {
            mode: match p.mode {
                Mode::Scm => ModeView::Scm,
                Mode::Nav => ModeView::Nav,
            },
            supply: p.supply as i32,
            used: p.used,
            columns: p
                .columns
                .into_iter()
                .map(|c| ColumnView {
                    id: c.id,
                    item_type: c.item_type,
                    items: c.items,
                    max: c.max as i32,
                    minimums: c.minimums.into_iter().map(|m| m as i32).collect(),
                    level: c.level,
                    off: c.off,
                    excluded: c.excluded,
                })
                .collect(),
            cooling: p.cooling,
            coolant_made: p.coolant_made,
            coolant_asked: p.coolant_asked,
            em: p.em,
            ir: p.ir,
            shield_regen: p.shield_regen,
            plants: p
                .plants
                .into_iter()
                .map(|(path, on)| PlantView { path, on })
                .collect(),
            sources: p
                .sources
                .into_iter()
                .map(|s| SourceView {
                    item_type: s.item_type,
                    em: s.em,
                    ir: s.ir,
                })
                .collect(),
        }
    }
}

/// G per direction.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AccelView {
    pub forward: f64,
    pub backward: f64,
    pub left: f64,
    pub right: f64,
    pub up: f64,
    pub down: f64,
}

impl From<&Accelerations> for AccelView {
    fn from(a: &Accelerations) -> Self {
        Self {
            forward: a.forward,
            backward: a.backward,
            left: a.left,
            right: a.right,
            up: a.up,
            down: a.down,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FlightView {
    pub mass: f64,
    pub scm_speed: f32,
    pub boost_forward: f32,
    pub boost_backward: f32,
    pub nav_speed: f32,
    /// deg/s: pitch, yaw, roll.
    pub rotation: Vec<f32>,
    /// deg/s at full boost (capacitor level 6): pitch, yaw, roll.
    pub rotation_boosted: Vec<f32>,
    pub acceleration: AccelView,
    pub acceleration_boosted: AccelView,
    /// SCU per minute at full thrust: main, retro, manoeuvring.
    pub hydrogen_burn: Vec<f64>,
    pub hydrogen_capacity: f64,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct QuantumView {
    pub drive: String,
    pub speed: f32,
    pub spool_up: f32,
    /// s after a jump that reached full speed; shorter jumps wait less.
    pub cooldown: f32,
    pub fuel_per_gm: f64,
    pub fuel_capacity: f64,
    pub range_gm: Option<f64>,
    /// [distance Gm, seconds] pairs, 0.5 Gm apart up to 60 Gm: spool-up
    /// and travel.
    pub times: Vec<(f64, f64)>,
    /// The same, 5 Gm apart up to 300 Gm (routes past the chart).
    pub far: Vec<(f64, f64)>,
    /// A planet's moons hopped in a row, cooldowns included.
    pub tour: Option<TourView>,
}

/// Several jumps in a row: spool-up and travel per leg, and the cooldown
/// before each next one.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TourView {
    pub planet: String,
    pub legs: u32,
    pub gm: f64,
    pub seconds: f64,
    /// Of `seconds`, waiting on the cooldown.
    pub cooling: f64,
}

/// A place quantum travel can go.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct QtPlaceView {
    pub guid: String,
    pub name: String,
    /// `Planet`, `Moon`, `Lagrange point`, `Jump point`, `Station`, `Rest
    /// stop`, `City` or `Outpost`.
    pub kind: String,
    pub system: String,
    /// System-global, m.
    pub position: (f64, f64, f64),
    /// Where travel drops out, m from the centre.
    pub arrival: f64,
}

/// The places and the routes shown by default (GUID pairs).
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct QtPlacesView {
    pub places: Vec<QtPlaceView>,
    pub routes: Vec<(String, String)>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SummaryView {
    pub hull_health: f32,
    pub armor_health: Option<f32>,
    pub shield_health: Option<f32>,
    pub shield_regen_max: Option<f32>,
    pub flight: Option<FlightView>,
    pub quantum: Option<QuantumView>,
    /// The radar's aim assist, m: [now, at full power].
    pub aim_assist: Option<(f64, f64)>,
    /// The radar cross-section seen from the side, the front and above.
    pub cross_section: Option<(f32, f32, f32)>,
    pub storage: Option<StorageView>,
    pub armor: Option<ArmorView>,
    pub shield: Option<ShieldView>,
    pub hull: HullView,
    pub insurance: Option<InsuranceView>,
    /// The loadout's seats by what they control.
    pub crew: SeatsView,
    pub missiles: Option<MissilesView>,
    /// Boost at thruster capacitor levels 0..=6; empty without one.
    pub boost: Vec<BoostView>,
    /// The client's default capacitor level.
    pub boost_default: u32,
}

/// Physical, energy, distortion.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TypedView {
    pub physical: f32,
    pub energy: f32,
    pub distortion: f32,
}

impl From<Typed<f32>> for TypedView {
    fn from(t: Typed<f32>) -> Self {
        Self {
            physical: t.physical,
            energy: t.energy,
            distortion: t.distortion,
        }
    }
}

/// Physical, energy, distortion: (min, max).
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TypedRangeView {
    pub physical: (f32, f32),
    pub energy: (f32, f32),
    pub distortion: (f32, f32),
}

impl From<Typed<[f32; 2]>> for TypedRangeView {
    fn from(t: Typed<[f32; 2]>) -> Self {
        let pair = |v: [f32; 2]| (v[0], v[1]);
        Self {
            physical: pair(t.physical),
            energy: pair(t.energy),
            distortion: pair(t.distortion),
        }
    }
}

/// The hull armour.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArmorView {
    pub health: Option<f32>,
    /// A hit at or below this does nothing.
    pub deflection: Option<TypedView>,
    /// Damage the hull takes per point through.
    pub hull_damage: Option<TypedView>,
    /// Damage the armour takes per point.
    pub durability: Option<TypedView>,
    /// IR, EM, cross-section multipliers.
    pub signals: (f32, f32, f32),
    pub penetration_reduction: f32,
}

/// The shield generators as one.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ShieldView {
    pub face: Option<String>,
    pub generators: u32,
    pub max_reallocation: Option<f32>,
    pub damaged_delay: f32,
    pub downed_delay: f32,
    pub resistance: Option<TypedRangeView>,
    pub absorption: Option<TypedRangeView>,
    /// The shields' pips over their most (resistance's lerp factor;
    /// inferred).
    pub ratio: f32,
    /// The shield column's pips now and its most.
    pub pips: Option<(f64, i32)>,
}

/// The hull's structure.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HullView {
    /// Width, length, height, m.
    pub dimensions: Option<(f32, f32, f32)>,
    pub hull_mass: Option<f32>,
    pub parts: Vec<HullPartView>,
    pub fuse_multiplier: f32,
    pub component_multiplier: f32,
    pub thrusters: Vec<ThrusterView>,
    pub doors: Vec<DoorView>,
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct HullPartView {
    pub name: String,
    pub depth: u8,
    pub hp: f32,
    pub breakable: bool,
    /// Losing it destroys the ship.
    pub vital: bool,
}

/// What an insurance claim costs.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InsuranceView {
    pub wait_minutes: f32,
    pub mandatory_minutes: f32,
    pub expedite_fee: u32,
}

/// What the ship stores.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct StorageView {
    pub cargo: f64,
    pub lockers: f64,
    pub inventory: Option<f64>,
    pub suit_lockers: u32,
    pub weapon_racks: u32,
    pub rack_slots: u32,
}

/// What the page shows for one loadout.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ShipPageView {
    pub ship: String,
    pub slots: Vec<SlotView>,
    pub power: Option<PowerView>,
    pub guns: Vec<GunView>,
    pub summary: SummaryView,
}

/// One gun's damage under the current power.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GunView {
    /// Port path, `/`-joined.
    pub path: String,
    pub burst: Option<f32>,
    pub sustained: Option<f32>,
    pub power: f32,
    /// Seconds of continuous fire at this power (capacitor or heat).
    pub fires: Option<f32>,
    /// Shots the capacitor holds at this power; null for a gun with rounds.
    pub loaded: Option<f32>,
    /// DPS over a 30 s engagement from a cold, full gun.
    pub window: Option<f32>,
}

impl From<ShipView> for ShipPageView {
    fn from(v: ShipView) -> Self {
        let s = v.summary;
        Self {
            ship: v.ship,
            slots: v.slots.into_iter().map(Into::into).collect(),
            power: v.power.map(Into::into),
            guns: v
                .guns
                .into_iter()
                .map(|g| GunView {
                    path: g.path,
                    burst: g.burst,
                    sustained: g.sustained,
                    power: g.power,
                    fires: g.fires,
                    loaded: g.loaded,
                    window: g.window,
                })
                .collect(),
            summary: SummaryView {
                hull_health: s.hull_health,
                armor_health: s.armor_health,
                shield_health: s.shield_health,
                shield_regen_max: s.shield_regen_max,
                aim_assist: s.aim_assist.map(|a| (a.distance, a.max)),
                cross_section: s.cross_section.map(|c| (c[0], c[1], c[2])),
                armor: s.armor.map(|a| ArmorView {
                    health: a.health,
                    deflection: a.deflection.map(Into::into),
                    hull_damage: a.hull_damage.map(Into::into),
                    durability: a.durability.map(Into::into),
                    signals: (a.signals[0], a.signals[1], a.signals[2]),
                    penetration_reduction: a.penetration_reduction,
                }),
                shield: s.shield.map(|h| ShieldView {
                    face: h.face,
                    generators: h.generators as u32,
                    max_reallocation: h.max_reallocation,
                    damaged_delay: h.damaged_delay,
                    downed_delay: h.downed_delay,
                    resistance: h.resistance.map(Into::into),
                    absorption: h.absorption.map(Into::into),
                    ratio: h.ratio,
                    pips: h.pips.map(|(l, m)| (l, m as i32)),
                }),
                hull: HullView {
                    dimensions: s.hull.dimensions.map(|d| (d[0], d[1], d[2])),
                    hull_mass: s.hull.hull_mass,
                    parts: s
                        .hull
                        .parts
                        .into_iter()
                        .map(|p| HullPartView {
                            name: p.name,
                            depth: p.depth,
                            hp: p.hp,
                            breakable: p.breakable,
                            vital: p.vital,
                        })
                        .collect(),
                    fuse_multiplier: s.hull.fuse_multiplier,
                    component_multiplier: s.hull.component_multiplier,
                    thrusters: s
                        .hull
                        .thrusters
                        .into_iter()
                        .map(|t| ThrusterView {
                            kind: t.kind,
                            place: t.place,
                            hp: t.hp,
                        })
                        .collect(),
                    doors: s
                        .hull
                        .doors
                        .into_iter()
                        .map(|d| DoorView {
                            name: d.name,
                            count: d.count as u32,
                            hp: d.hp,
                            breachable: d.breachable,
                        })
                        .collect(),
                },
                crew: s.crew.into(),
                boost: s
                    .boost
                    .into_iter()
                    .map(|b| BoostView {
                        regen_s: b.regen_s,
                        deplete_s: b.deplete_s,
                        turn: b.turn.to_vec(),
                    })
                    .collect(),
                boost_default: svc_data::ships::DEFAULT_CAPACITOR_LEVEL as u32,
                missiles: s.missiles.map(|m| MissilesView {
                    count: m.count as u32,
                    max_armed: m.max_armed,
                    cooldown: m.cooldown,
                    controllers: m.controllers as u32,
                }),
                insurance: s.insurance.map(|i| InsuranceView {
                    wait_minutes: i.wait_minutes,
                    mandatory_minutes: i.mandatory_minutes,
                    expedite_fee: i.expedite_fee,
                }),
                storage: s.storage.map(|t| StorageView {
                    cargo: t.cargo,
                    lockers: t.lockers,
                    inventory: t.inventory,
                    suit_lockers: t.suit_lockers as u32,
                    weapon_racks: t.weapon_racks as u32,
                    rack_slots: t.rack_slots as u32,
                }),
                flight: s.flight.map(|f| FlightView {
                    mass: f.mass,
                    scm_speed: f.scm_speed,
                    boost_forward: f.boost_speed_forward,
                    boost_backward: f.boost_speed_backward,
                    nav_speed: f.max_speed,
                    // sc-vehicles' axes are pitch, roll, yaw.
                    rotation: pyr(f.angular_velocity),
                    rotation_boosted: pyr(f.angular_velocity_boosted),
                    acceleration: (&f.acceleration).into(),
                    acceleration_boosted: (&f.acceleration_boosted).into(),
                    hydrogen_burn: vec![
                        f.hydrogen_burn.main,
                        f.hydrogen_burn.retro,
                        f.hydrogen_burn.maneuver,
                    ],
                    hydrogen_capacity: f.hydrogen_capacity,
                }),
                quantum: s.quantum.map(|q| QuantumView {
                    drive: q.drive,
                    speed: q.speed,
                    spool_up: q.spool_up,
                    cooldown: q.cooldown,
                    fuel_per_gm: q.fuel_per_gm,
                    fuel_capacity: q.fuel_capacity,
                    range_gm: q.range_gm,
                    times: q.times,
                    far: q.far,
                    tour: q.tour.map(|t| TourView {
                        planet: t.planet,
                        legs: t.legs,
                        gm: t.gm,
                        seconds: t.seconds,
                        cooling: t.cooling,
                    }),
                }),
            },
        }
    }
}

/// One figure of a part.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct StatView {
    pub key: String,
    pub label: String,
    pub value: f64,
    pub unit: String,
    pub better: i32,
}

impl From<Stat> for StatView {
    fn from(s: Stat) -> Self {
        Self {
            key: s.key,
            label: s.label,
            value: s.value,
            unit: s.unit,
            better: i32::from(s.better),
        }
    }
}

/// The ship with a part fitted.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ImpactView {
    pub supply: Option<i32>,
    pub free: Option<f64>,
    pub cooling: Option<f64>,
    pub em: Option<f64>,
    pub ir: Option<f64>,
    pub shield_health: Option<f32>,
    // Named by hand: serde's camelCase keeps `quantum32gm` whole while
    // specta's binding spells it `quantum32Gm`, and the page read nothing.
    #[serde(rename = "quantum32Gm")]
    pub quantum32gm: Option<f64>,
    /// Seconds for a 60 Gm jump.
    #[serde(rename = "quantum60Gm")]
    pub quantum60gm: Option<f64>,
    /// Seconds for the moon tour, cooldowns included.
    pub quantum_tour: Option<f64>,
    /// How far the ship's quantum fuel reaches with this drive, Gm.
    pub quantum_range_gm: Option<f64>,
    /// Every gun's sustained DPS together with the part fitted.
    pub gun_dps: Option<f64>,
    /// The share of their power the guns get, 0 to 1.
    pub gun_power: Option<f64>,
}

/// Owned copies of a part that are free to fit: in a place's storage, not
/// on a port, not inside a ship or container, owned (not rented, not
/// bricked). The inventory has no insurance or claim state.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OwnedPartView {
    pub place: String,
    pub count: u32,
}

/// One option for a port.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CandidateView {
    pub item: String,
    pub name: String,
    pub record_name: String,
    pub size: i32,
    pub grade: i32,
    pub class: Option<String>,
    pub item_type: String,
    pub sub_type: String,
    pub stats: Vec<StatView>,
    pub fitted: bool,
    pub impact: ImpactView,
    /// Free owned copies by place, most first; null without an inventory
    /// snapshot (My Items never refreshed).
    pub owned: Option<Vec<OwnedPartView>>,
    /// The shops that sell it, cheapest first (UEX's community prices);
    /// empty when none are known or the prices were never fetched.
    pub shops: Vec<OfferView>,
    /// What players ask and offer for it on UEX's marketplace, per side and
    /// quality tier.
    pub market: Vec<MarketView>,
}

/// UEX's marketplace average for an item, one side, one quality tier.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MarketView {
    /// "sell": players selling it (what you'd pay); "buy": players wanting it.
    pub side: String,
    /// A crafted item's quality tier; 0: none.
    pub tier: i32,
    /// Open listings now.
    pub listings: i32,
    /// aUEC: now (0 when nothing is listed), this week, this month.
    pub now: f64,
    pub week: f64,
    pub month: f64,
    /// Unix seconds of the last change.
    pub updated: f64,
}

impl From<&svc_uex::MarketAverage> for MarketView {
    fn from(m: &svc_uex::MarketAverage) -> Self {
        Self {
            side: m.operation.clone(),
            tier: m.quality_tier as i32,
            listings: m.listings_count as i32,
            now: m.price_avg,
            week: m.price_avg_week,
            month: m.price_avg_month,
            updated: m.date_modified as f64,
        }
    }
}

/// A place that sells (or rents) something, from UEX's community prices.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OfferView {
    /// The terminal as UEX names it ("Dumper's Area 18").
    pub terminal: String,
    /// Where it is, smallest first ("Area 18 · ArcCorp · Stanton").
    pub place: String,
    /// aUEC.
    pub price: f64,
    /// Unix seconds of the last player report.
    pub updated: f64,
}

impl From<svc_uex::Offer> for OfferView {
    fn from(o: svc_uex::Offer) -> Self {
        Self {
            terminal: o.terminal,
            place: o.place,
            price: o.price,
            updated: o.updated as f64,
        }
    }
}

impl From<Candidate> for CandidateView {
    fn from(c: Candidate) -> Self {
        Self {
            item: c.item,
            name: c.name,
            record_name: c.record_name,
            size: c.size,
            grade: c.grade,
            class: c.class,
            item_type: c.item_type,
            sub_type: c.sub_type,
            stats: c.stats.into_iter().map(Into::into).collect(),
            fitted: c.fitted,
            impact: ImpactView {
                supply: c.impact.supply.map(|s| s as i32),
                free: c.impact.free,
                cooling: c.impact.cooling,
                em: c.impact.em,
                ir: c.impact.ir,
                shield_health: c.impact.shield_health,
                quantum32gm: c.impact.quantum_32gm,
                quantum60gm: c.impact.quantum_60gm,
                quantum_tour: c.impact.quantum_tour,
                quantum_range_gm: c.impact.quantum_range_gm,
                gun_dps: c.impact.gun_dps,
                gun_power: c.impact.gun_power,
            },
            owned: None,
            shops: Vec::new(),
            market: Vec::new(),
        }
    }
}

/// Free owned copies per part GUID (see [`OwnedPartView`]), from the cached
/// inventory: no network.
fn owned_free(finder: &mod_items::Finder) -> HashMap<String, Vec<OwnedPartView>> {
    let mut by: HashMap<String, HashMap<String, u32>> = HashMap::new();
    for row in &finder.items {
        if row.fitted
            || !row.holders.is_empty()
            || row.ownership != mod_items::OwnershipState::Owned
        {
            continue;
        }
        *by.entry(row.kind.guid.clone())
            .or_default()
            .entry(row.place.name.clone())
            .or_default() += row.stack.max(1);
    }
    by.into_iter()
        .map(|(guid, places)| {
            let mut v: Vec<OwnedPartView> = places
                .into_iter()
                .map(|(place, count)| OwnedPartView { place, count })
                .collect();
            v.sort_by(|a, b| b.count.cmp(&a.count).then(a.place.cmp(&b.place)));
            (guid, v)
        })
        .collect()
}

// ── Commands ────────────────────────────────────────────────────────────────

/// What else fits the port at `targets[0]`, each option with its figures
/// and the ship with it fitted on every target port.
#[tauri::command]
#[specta::specta]
pub(crate) async fn ships_candidates(
    app: AppHandle,
    channel: String,
    loadout: ShipLoadoutView,
    power: PowerSettingsView,
    targets: Vec<Vec<String>>,
) -> Result<Vec<CandidateView>, AppError> {
    use tauri::Manager;
    let cooked = cooked_for(&app, &channel).await?;
    let inventory = app.state::<crate::AppState>().dossier.cached_inventory();
    let uex = app.state::<crate::AppState>().uex.clone();
    let input: LoadoutInput = loadout.into();
    let settings: PowerInput = power.into();
    tauri::async_runtime::spawn_blocking(move || {
        let list = mod_ships::candidates(&cooked, &input, &settings, &targets)?;
        let owned = inventory.map(|inv| owned_free(&mod_items::finder(&inv, &cooked)));
        // UEX's cached prices (no network), loaded for this list and dropped.
        let prices = uex.snapshot();
        let prices = svc_uex::Index::new(&prices);
        Ok::<_, mod_ships::ViewError>(
            list.into_iter()
                .map(|c| {
                    let guid = c.item.clone();
                    let shops = prices.item(&guid, &c.name).buy;
                    let market: Vec<MarketView> = prices
                        .market(&guid, &c.name)
                        .into_iter()
                        .map(Into::into)
                        .collect();
                    let mut v: CandidateView = c.into();
                    v.owned = owned
                        .as_ref()
                        .map(|o| o.get(&guid).cloned().unwrap_or_default());
                    v.shops = shops.into_iter().map(Into::into).collect();
                    v.market = market;
                    v
                })
                .collect::<Vec<_>>(),
        )
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
    .map_err(|e| AppError::Config(e.to_string()))
}

/// Where a ship is sold and rented (UEX's community prices).
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ShipPricesView {
    /// Cheapest first.
    pub buy: Vec<OfferView>,
    /// Per rental (UEX: one day), cheapest first.
    pub rent: Vec<OfferView>,
    /// When UEX's lists were last fetched, Unix seconds; null: never.
    pub fetched_at: Option<f64>,
}

/// Where `ship` is sold and rented, from the cached UEX lists (no
/// network). Matched by the record GUID, else by name: the plain name,
/// with its maker, and without a leading maker code.
#[tauri::command]
#[specta::specta]
pub(crate) async fn ships_prices(
    app: AppHandle,
    channel: String,
    ship: String,
) -> Result<ShipPricesView, AppError> {
    use tauri::Manager;
    let cooked = cooked_for(&app, &channel).await?;
    let uex = app.state::<crate::AppState>().uex.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let names = mod_ships::ships(&cooked)
            .into_iter()
            .find(|r| r.guid.eq_ignore_ascii_case(&ship))
            .map(|r| r.names())
            .unwrap_or_default();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let snap = uex.snapshot();
        let offers = svc_uex::Index::new(&snap).vehicle(&ship, &names);
        ShipPricesView {
            buy: offers.buy.into_iter().map(Into::into).collect(),
            rent: offers.rent.into_iter().map(Into::into).collect(),
            fetched_at: uex.fetched_at().map(|t| t as f64),
        }
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))
}

/// A ship with a loadout under the player's power settings: slots, power
/// network, and the figures the page leads with.
#[tauri::command]
#[specta::specta]
pub(crate) async fn ships_view(
    app: AppHandle,
    channel: String,
    loadout: ShipLoadoutView,
    power: PowerSettingsView,
) -> Result<ShipPageView, AppError> {
    let cooked = cooked_for(&app, &channel).await?;
    let input: LoadoutInput = loadout.into();
    let settings: PowerInput = power.into();
    tauri::async_runtime::spawn_blocking(move || mod_ships::view(&cooked, &input, &settings))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .map(Into::into)
        .map_err(|e| AppError::Config(e.to_string()))
}

/// Where quantum travel goes: the build's waypoints with game labels, and
/// the default routes. Reads the cooked data only.
#[tauri::command]
#[specta::specta]
pub(crate) async fn ships_places(
    app: AppHandle,
    channel: String,
) -> Result<QtPlacesView, AppError> {
    let cooked = cooked_for(&app, &channel).await?;
    let p = mod_ships::places(&cooked);
    Ok(QtPlacesView {
        places: p
            .places
            .into_iter()
            .map(|x| QtPlaceView {
                guid: x.guid,
                name: x.name,
                kind: x.kind,
                system: x.system,
                position: (x.position[0], x.position[1], x.position[2]),
                arrival: x.arrival,
            })
            .collect(),
        routes: p.routes,
    })
}

/// Fired after the store pictures were refreshed. The store reloads.
pub const SHIP_IMAGES_EVENT: &str = "ships:images";
/// Public pages, honest user agent.
const USER_AGENT: &str = concat!("starlume/", env!("CARGO_PKG_VERSION"));

/// A hull's store picture on disk.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ShipImageView {
    pub guid: String,
    /// Local path, for `convertFileSrc`.
    pub path: String,
}

/// Let the webview read the downloaded pictures (the asset protocol's
/// scope is empty in tauri.conf.json and gets exactly the image
/// directories).
pub fn allow_images(app: &AppHandle) {
    use tauri::Manager;
    let images = app.state::<crate::AppState>().shipstore.images_dir();
    let _ = std::fs::create_dir_all(&images);
    if let Err(e) = app.asset_protocol_scope().allow_directory(&images, false) {
        tracing::warn!("ship pictures not readable by the page: {e}");
    }
}

/// Each hull's store picture that is on disk (editions the store does not
/// list share their base hull's). **No network, no gate.**
#[tauri::command]
#[specta::specta]
pub(crate) async fn ships_images(
    app: AppHandle,
    channel: String,
) -> Result<Vec<ShipImageView>, AppError> {
    use tauri::Manager;
    let cooked = cooked_for(&app, &channel).await?;
    let service = app.state::<crate::AppState>().shipstore.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let Some(cache) = service.cached() else {
            return Ok(Vec::new());
        };
        let rows = mod_ships::ships(&cooked);
        let store: Vec<(u32, &str)> = cache
            .ships
            .iter()
            .map(|s| (s.id, s.name.as_str()))
            .collect();
        let by_id: std::collections::HashMap<u32, &svc_shipstore::StoreShip> =
            cache.ships.iter().map(|s| (s.id, s)).collect();
        Ok(mod_ships::match_store(&rows, &store)
            .into_iter()
            .filter_map(|(guid, id)| {
                let path = service.image_path(by_id.get(&id)?)?;
                Some(ShipImageView {
                    guid,
                    path: path.to_string_lossy().into_owned(),
                })
            })
            .collect())
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

/// Refresh the store's ship list and pictures when the cache is older than
/// a week (or `force`). Returns whether it read the store. **Gated** by
/// `require_online()`: a public website read, like the comm-links.
#[tauri::command]
#[specta::specta]
pub(crate) async fn ships_images_refresh(app: AppHandle, force: bool) -> Result<bool, AppError> {
    use tauri::{Emitter, Manager};
    let service = {
        let state = app.state::<crate::AppState>();
        state.require_online()?;
        state.shipstore.clone()
    };
    let cached = service.cached();
    let stale = cached
        .as_ref()
        .is_none_or(|c| c.stale(chrono::Utc::now().timestamp()));
    if !force && !stale {
        return Ok(false);
    }
    let result = {
        let _activity = cached.is_none().then(|| {
            crate::activity::Activity::start(
                &app,
                crate::activity::ActivityKind::ShipStore,
                None,
                "Ship pictures",
            )
            .stage("Reading RSI's ship store…")
        });
        service.refresh(USER_AGENT).await
    };
    let _ = app.emit(SHIP_IMAGES_EVENT, ());
    result
        .map(|_| true)
        .map_err(|e| AppError::Internal(format!("couldn't read RSI's ship store: {e}")))
}

/// Every ship a player can own, sorted by name.
#[tauri::command]
#[specta::specta]
pub(crate) async fn ships_list(
    app: AppHandle,
    channel: String,
) -> Result<Vec<ShipRowView>, AppError> {
    let cooked = cooked_for(&app, &channel).await?;
    Ok(mod_ships::ships(&cooked)
        .into_iter()
        .map(Into::into)
        .collect())
}

/// A ship's stock loadout as slots.
#[tauri::command]
#[specta::specta]
pub(crate) async fn ships_stock(
    app: AppHandle,
    channel: String,
    ship: String,
) -> Result<Vec<SlotView>, AppError> {
    let cooked = cooked_for(&app, &channel).await?;
    let guid: Guid = ship
        .parse()
        .map_err(|_| AppError::Config(format!("not a ship id: {ship}")))?;
    let loadout = Loadout::stock(&cooked.ships.vehicles, &guid)
        .ok_or_else(|| AppError::Config("this build's game data has no such ship".into()))?;
    Ok(mod_ships::slots(&cooked, &loadout)
        .into_iter()
        .map(Into::into)
        .collect())
}

// ── Home and module state ───────────────────────────────────────────────────

/// Serializes state read-modify-writes across commands.
static STATE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn state_path() -> std::path::PathBuf {
    app_kit::app_data_root().join("ships").join("state.json")
}

/// One owned ship class.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OwnedView {
    pub ship: String,
    pub count: u32,
    pub rented: bool,
}

/// A ship that arrived after Starlume started watching.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ArrivalView {
    pub ship: String,
    /// The build version it first appeared in.
    pub since: String,
}

/// The Ships page with no ship open.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ShipsHomeView {
    /// `None` until an inventory has been read (My Items); an empty list
    /// when it holds no ships.
    pub owned: Option<Vec<OwnedView>>,
    /// When the inventory was read, Unix seconds.
    pub owned_at: Option<f64>,
    pub arrivals: Vec<ArrivalView>,
    /// The build "new" counts from (the first one Starlume saw).
    pub watching_since: Option<String>,
    pub recent: Vec<String>,
}

/// Your ships (from the inventory snapshot already on disk: **no
/// network, no gate**), the ships new since the first build Starlume saw,
/// and the ones opened lately. Records this build's ships as seen.
#[tauri::command]
#[specta::specta]
pub(crate) async fn ships_home(app: AppHandle, channel: String) -> Result<ShipsHomeView, AppError> {
    use tauri::Manager;
    let cooked = cooked_for(&app, &channel).await?;
    let install = crate::data::install_for(&app, &channel).await?;
    let inventory = app.state::<crate::AppState>().dossier.cached_inventory();
    tauri::async_runtime::spawn_blocking(move || {
        let rows = mod_ships::ships(&cooked);
        let state = {
            let _lock = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let mut state: mod_ships::ShipsState = app_kit::load_json(&state_path());
            if state.sight(&install.version, rows.iter().map(|r| r.guid.as_str())) {
                app_kit::save_json(&state_path(), &state)?;
            }
            state
        };
        use svc_dossier::inventory::{
            EntitlementSourceRecord as Src, EntitlementStatusRecord as St,
        };
        let owned: Option<Vec<mod_ships::Entitlement>> = inventory.as_ref().map(|inv| {
            inv.entitlements
                .iter()
                .filter(|e| !matches!(e.status, St::Revoked | St::Failed))
                .filter(|e| !matches!(e.source, Src::IngameNonpersistent))
                .map(|e| mod_ships::Entitlement {
                    class: e.class_guid.clone(),
                    rental: matches!(e.source, Src::Rental) || e.rental_expiry.is_some(),
                })
                .collect()
        });
        let home = mod_ships::home(&rows, owned.as_deref(), &state);
        Ok::<_, AppError>(ShipsHomeView {
            owned: inventory.as_ref().map(|_| {
                home.owned
                    .into_iter()
                    .map(|o| OwnedView {
                        ship: o.ship,
                        count: o.count,
                        rented: o.rented,
                    })
                    .collect()
            }),
            owned_at: inventory
                .as_ref()
                .map(|i| i.fetched_at)
                .filter(|&t| t > 0)
                .map(|t| t as f64),
            arrivals: home
                .arrivals
                .into_iter()
                .map(|(ship, since)| ArrivalView { ship, since })
                .collect(),
            watching_since: state.baseline.clone(),
            recent: home.recent,
        })
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

/// Note that `ship` was opened (the home page's "Recently opened").
#[tauri::command]
#[specta::specta]
pub(crate) fn ships_opened(ship: String) -> Result<(), AppError> {
    let _lock = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut state: mod_ships::ShipsState = app_kit::load_json(&state_path());
    state.opened(&ship.to_ascii_lowercase());
    Ok(app_kit::save_json(&state_path(), &state)?)
}

// ── Loadouts ────────────────────────────────────────────────────────────
// The saved loadouts and each ship's working draft: one file of the
// player's own work, apart from the bookkeeping in state.json, and loaded
// so that a file that no longer parses is moved aside, never overwritten.

static LOADOUTS_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn loadouts_path() -> std::path::PathBuf {
    app_kit::app_data_root().join("ships").join("loadouts.json")
}

/// Read the loadouts file under its lock.
fn read_loadouts<R>(f: impl FnOnce(&mod_ships::Loadouts) -> R) -> R {
    let _lock = LOADOUTS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    f(&app_kit::load_json_or_set_aside(&loadouts_path()))
}

/// Read, change and write the loadouts file under its lock; returns what
/// `f` returns.
fn with_loadouts<R>(f: impl FnOnce(&mut mod_ships::Loadouts) -> R) -> Result<R, AppError> {
    let _lock = LOADOUTS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut l: mod_ships::Loadouts = app_kit::load_json_or_set_aside(&loadouts_path());
    let out = f(&mut l);
    app_kit::save_json(&loadouts_path(), &l)?;
    Ok(out)
}

/// A loadout as the Build tab edits it.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ShipDraftView {
    /// The saved loadout it started from; null: stock.
    pub based_on: Option<String>,
    pub edits: Vec<EditView>,
    pub off: Vec<String>,
    pub levels: Vec<LevelView>,
}

impl From<ShipDraftView> for mod_ships::Draft {
    fn from(v: ShipDraftView) -> Self {
        Self {
            based_on: v.based_on,
            edits: v
                .edits
                .into_iter()
                .map(|e| Edit {
                    path: e.path,
                    item: e.item,
                })
                .collect(),
            off: v.off,
            levels: v
                .levels
                .into_iter()
                .map(|l| (l.column, i64::from(l.pips)))
                .collect(),
        }
    }
}

fn edit_views(edits: &[Edit]) -> Vec<EditView> {
    edits
        .iter()
        .map(|e| EditView {
            path: e.path.clone(),
            item: e.item.clone(),
        })
        .collect()
}

fn level_views(levels: &[(String, i64)]) -> Vec<LevelView> {
    levels
        .iter()
        .map(|(column, pips)| LevelView {
            column: column.clone(),
            pips: *pips as i32,
        })
        .collect()
}

/// One saved loadout.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ShipSavedLoadoutView {
    pub id: String,
    pub name: String,
    pub edits: Vec<EditView>,
    pub off: Vec<String>,
    pub levels: Vec<LevelView>,
    /// The game build it was last saved in.
    pub build: String,
    /// Unix seconds.
    pub saved_at: f64,
}

/// A ship's saved loadouts, by name, and its working draft.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ShipLoadoutsView {
    pub saved: Vec<ShipSavedLoadoutView>,
    /// Null: stock, nothing changed.
    pub draft: Option<ShipDraftView>,
}

fn loadouts_view(l: &mod_ships::Loadouts, ship: &str) -> ShipLoadoutsView {
    ShipLoadoutsView {
        saved: l
            .of(ship)
            .into_iter()
            .map(|s| ShipSavedLoadoutView {
                id: s.id.clone(),
                name: s.name.clone(),
                edits: edit_views(&s.edits),
                off: s.off.clone(),
                levels: level_views(&s.levels),
                build: s.build.clone(),
                saved_at: s.saved_at as f64,
            })
            .collect(),
        draft: l.drafts.get(ship).map(|d| ShipDraftView {
            based_on: d.based_on.clone(),
            edits: edit_views(&d.edits),
            off: d.off.clone(),
            levels: level_views(&d.levels),
        }),
    }
}

/// `ship`'s saved loadouts and the draft the Build tab had open.
#[tauri::command]
#[specta::specta]
pub(crate) fn ships_loadouts(ship: String) -> Result<ShipLoadoutsView, AppError> {
    Ok(read_loadouts(|l| loadouts_view(l, &ship)))
}

/// Keep what the Build tab has open for `ship` (every edit, so a restart
/// loses nothing).
#[tauri::command]
#[specta::specta]
pub(crate) fn ships_set_draft(ship: String, draft: ShipDraftView) -> Result<(), AppError> {
    with_loadouts(|l| l.set_draft(&ship, draft.into()))
}

/// Save `draft` as `name` (into `id`, or by name, or as a new one); the
/// ship's draft is then based on it. Stamped with the channel's build.
#[tauri::command]
#[specta::specta]
pub(crate) async fn ships_save_loadout(
    app: AppHandle,
    channel: String,
    ship: String,
    id: Option<String>,
    name: String,
    draft: ShipDraftView,
) -> Result<ShipLoadoutsView, AppError> {
    if name.trim().is_empty() {
        return Err(AppError::Internal("a loadout needs a name".into()));
    }
    let build = crate::data::install_for(&app, &channel).await?.version;
    let now = chrono::Utc::now().timestamp();
    with_loadouts(|l| {
        l.save(
            &ship,
            id.as_deref(),
            &name,
            draft.into(),
            &build,
            now,
            || uuid::Uuid::now_v7().to_string(),
        );
        loadouts_view(l, &ship)
    })
}

/// Open a saved loadout (`null`: stock) on `ship`, replacing its draft.
#[tauri::command]
#[specta::specta]
pub(crate) fn ships_open_loadout(
    ship: String,
    id: Option<String>,
) -> Result<ShipLoadoutsView, AppError> {
    with_loadouts(|l| {
        l.open(&ship, id.as_deref());
        loadouts_view(l, &ship)
    })
}

/// Forget a saved loadout; the open draft keeps its edits.
#[tauri::command]
#[specta::specta]
pub(crate) fn ships_delete_loadout(ship: String, id: String) -> Result<ShipLoadoutsView, AppError> {
    with_loadouts(|l| {
        l.delete(&id);
        loadouts_view(l, &ship)
    })
}
