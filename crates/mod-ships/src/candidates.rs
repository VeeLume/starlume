//! What else fits a port, each option with its own figures and what the
//! ship looks like with it fitted — the part picker.
//!
//! The options are sc-vehicles' fit rule ([`Loadout::candidates`]) minus
//! records no player sees (no display name: templates and placeholders).
//! The ship figures are the page's ([`crate::view`]) with the option on
//! every target port, under the same power settings.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use svc_data::ships::{
    EItemType, Item, ItemClass, RecordCollection, ResourceNetworkResource, ResourceUnits,
};
use svc_data::{CookedData, Guid};

use crate::power::PowerInput;
use crate::view::{Edit, LoadoutInput, ViewError, loadout, view};

/// One figure of an option.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stat {
    /// Stable key, the same for every option of a port (a column).
    pub key: String,
    pub label: String,
    pub value: f64,
    pub unit: String,
    /// 1 when higher is better, -1 when lower is, 0 when neither.
    pub better: i8,
}

/// The ship with an option fitted.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Impact {
    pub supply: Option<i64>,
    pub free: Option<f64>,
    /// Coolant asked over made.
    pub cooling: Option<f64>,
    pub em: Option<f64>,
    pub ir: Option<f64>,
    pub shield_health: Option<f32>,
    /// Seconds for a 32 Gm jump.
    pub quantum_32gm: Option<f64>,
    /// Seconds for a long (60 Gm) jump: spool-up and the acceleration
    /// curve make a drive's speed a poor guide to it.
    #[serde(default)]
    pub quantum_60gm: Option<f64>,
    /// Seconds for the moon tour ([`crate::Tour`]), cooldowns included:
    /// short hops, where the cooldown is the drive's real cost.
    #[serde(default)]
    pub quantum_tour: Option<f64>,
    /// How far the ship's quantum fuel takes it with this drive, Gm.
    #[serde(default)]
    pub quantum_range_gm: Option<f64>,
    /// Every gun's sustained damage per second together (the weapons
    /// panel's figure), with the option fitted.
    #[serde(default)]
    pub gun_dps: Option<f64>,
    /// The guns' power: the lowest share of what they ask that they get
    /// (the weapons pool feeds them all alike), 0 to 1.
    #[serde(default)]
    pub gun_power: Option<f64>,
}

/// One option for a port.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub item: String,
    pub name: String,
    pub record_name: String,
    pub size: i32,
    pub grade: i32,
    /// [`class_of`]: Military, Civilian, Industrial, Stealth, Competition.
    pub class: Option<String>,
    pub item_type: String,
    pub sub_type: String,
    pub stats: Vec<Stat>,
    /// It is what every target holds now (swapping several ports that hold
    /// different parts, nothing is).
    pub fitted: bool,
    pub impact: Impact,
}

/// Every option for the port at `targets[0]` (the others take the same
/// item: a turret's other guns), best name first.
pub fn candidates(
    cooked: &CookedData,
    input: &LoadoutInput,
    settings: &PowerInput,
    targets: &[Vec<String>],
) -> Result<Vec<Candidate>, ViewError> {
    let Some(first) = targets.first() else {
        return Ok(Vec::new());
    };
    let index = &cooked.ships;
    let current = loadout(cooked, input)?;
    let empty = Default::default();
    let items = cooked.holotable.items.as_ref().unwrap_or(&empty);
    let path: Vec<&str> = first.iter().map(String::as_str).collect();
    let held = |t: &Vec<String>| {
        let p: Vec<&str> = t.iter().map(String::as_str).collect();
        current.at(&p).and_then(|m| m.item)
    };
    let fitted = held(first).filter(|g| targets.iter().all(|t| held(t) == Some(*g)));
    let weapons = weapon_entries(cooked);

    let mut out: Vec<Candidate> = current
        .candidates(&path, items, &index.vehicles, &index.containers)
        .into_iter()
        .filter_map(|g| {
            let item = items.get(&g)?;
            let name = item.display_name(&cooked.locale)?;
            if name.contains("PLACEHOLDER") {
                return None;
            }
            let mut with = input.clone();
            with.edits.extend(targets.iter().map(|t| Edit {
                path: t.clone(),
                item: Some(g.to_string()),
            }));
            let impact = view(cooked, &with, settings)
                .ok()
                .map_or_else(Impact::default, |v| Impact {
                    supply: v.power.as_ref().map(|p| p.supply),
                    free: v.power.as_ref().map(|p| p.supply as f64 - p.used),
                    cooling: v.power.as_ref().and_then(|p| p.cooling),
                    em: v.power.as_ref().map(|p| p.em),
                    ir: v.power.as_ref().map(|p| p.ir),
                    shield_health: v.summary.shield_health,
                    quantum_32gm: v.summary.quantum.as_ref().map(|q| q.times[64].1),
                    quantum_60gm: v.summary.quantum.as_ref().map(|q| q.times[120].1),
                    quantum_tour: v
                        .summary
                        .quantum
                        .as_ref()
                        .and_then(|q| q.tour.as_ref())
                        .map(|t| t.seconds),
                    quantum_range_gm: v.summary.quantum.as_ref().and_then(|q| q.range_gm),
                    gun_dps: (!v.guns.is_empty()).then(|| {
                        v.guns
                            .iter()
                            .map(|g| f64::from(g.sustained.unwrap_or(0.0)))
                            .sum()
                    }),
                    gun_power: v.guns.iter().map(|g| f64::from(g.power)).reduce(f64::min),
                });
            Some(Candidate {
                item: g.to_string(),
                name: name.to_string(),
                record_name: crate::tree::name_of(cooked, &g, None),
                size: item.size,
                grade: item.grade,
                class: class_of(cooked, item),
                item_type: item.item_type.as_dcb_str().to_string(),
                sub_type: item.item_sub_type.as_dcb_str().to_string(),
                stats: stats(cooked, &g, &item.item_type, &weapons),
                fitted: Some(g) == fitted,
                impact,
            })
        })
        .collect();
    // Records a player can't tell apart (same name, same figures: the
    // CF-557 has three) are one option; the fitted one is kept.
    out.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then(b.fitted.cmp(&a.fitted))
            .then(a.record_name.cmp(&b.record_name))
    });
    out.dedup_by(|b, a| a.name == b.name && a.stats == b.stats);
    Ok(out)
}

/// A component's class from its description's `Class:` line (sc-items
/// reads the shipped `global.ini`, which the cook keeps). Only the five
/// component classes: on weapons the same line names a weapon class.
pub(crate) fn class_of(cooked: &CookedData, item: &Item) -> Option<String> {
    match item.class(&cooked.locale)? {
        ItemClass::Other(_) => None,
        c => Some(format!("{c:?}")),
    }
}

/// The weapons catalog by GUID string, for [`stats`].
pub(crate) fn weapon_entries(cooked: &CookedData) -> HashMap<&str, &svc_data::ShipWeaponEntry> {
    cooked
        .weapons
        .ship_weapons
        .iter()
        .map(|w| (w.guid.as_str(), w))
        .collect()
}

fn stat(key: &str, label: &str, value: Option<f64>, unit: &str, better: i8) -> Option<Stat> {
    value.filter(|v| v.is_finite()).map(|value| Stat {
        key: key.into(),
        label: label.into(),
        value,
        unit: unit.into(),
        better,
    })
}

/// A part's own figures: what the picker's columns and the cards show.
pub(crate) fn stats(
    cooked: &CookedData,
    guid: &Guid,
    item_type: &EItemType,
    weapons: &HashMap<&str, &svc_data::ShipWeaponEntry>,
) -> Vec<Stat> {
    let index = &cooked.ships;
    let sheet = index.components.get(guid);
    let online = index.profiles.get(guid).and_then(|p| p.online());
    let segments = |u: &ResourceUnits| match u {
        ResourceUnits::Segments(n) => f64::from(*n),
        other => other.standard(),
    };
    let power_delta = online.and_then(|s| s.consumption(&ResourceNetworkResource::Power));
    let draw = power_delta
        .and_then(|d| d.consumes.as_ref())
        .map(|a| segments(&a.per_second));
    // The pips below which it does not run (the power screen's numbered
    // block): the request's minimum share, rounded up to whole pips as
    // sc-vehicles grants it.
    let least = power_delta
        .filter(|d| d.minimum_fraction > 0.0)
        .zip(draw)
        .map(|(d, n)| (n * f64::from(d.minimum_fraction)).ceil());
    let made = |r: ResourceNetworkResource| {
        online
            .and_then(|s| s.generation(&r))
            .and_then(|d| d.generates.as_ref())
            .map(|a| segments(&a.per_second))
    };
    let em = online
        .and_then(|s| s.em.as_ref())
        .map(|e| f64::from(e.nominal));
    let ir = online
        .and_then(|s| s.ir.as_ref())
        .map(|e| f64::from(e.nominal));
    let f = |v: Option<f32>| v.map(f64::from);

    let mut out: Vec<Option<Stat>> = Vec::new();
    match item_type {
        EItemType::WeaponGun => {
            if let Some(w) = weapons.get(guid.to_string().as_str()) {
                out.push(stat("dps", "DPS", f(w.burst_dps), "", 1));
                out.push(stat(
                    "alpha",
                    "Alpha",
                    w.damage.map(|d| f64::from(d.total())),
                    "",
                    1,
                ));
                out.push(stat("rpm", "Fire rate", f(w.rpm), "rpm", 1));
                // The magazine: rounds over the fire rate.
                out.push(stat(
                    "magazine",
                    "Magazine",
                    w.total_ammo
                        .filter(|&n| n > 0)
                        .zip(w.rpm.filter(|&r| r > 0.0))
                        .map(|(n, r)| f64::from(n) * 60.0 / f64::from(r)),
                    "s",
                    1,
                ));
                out.push(stat("burst", "Fires for", f(w.burst_seconds), "s", 1));
                // The damage split per shot, for the gun's type (ballistic,
                // energy, distortion); not a column of its own.
                if let Some(d) = w.damage {
                    out.push(stat(
                        "dmgPhysical",
                        "Physical",
                        Some(f64::from(d.physical)),
                        "",
                        0,
                    ));
                    out.push(stat(
                        "dmgEnergy",
                        "Energy",
                        Some(f64::from(d.energy)),
                        "",
                        0,
                    ));
                    out.push(stat(
                        "dmgDistortion",
                        "Distortion",
                        Some(f64::from(d.distortion)),
                        "",
                        0,
                    ));
                }
                out.push(stat("speed", "Speed", f(w.ammo_speed), "m/s", 1));
                out.push(stat(
                    "range",
                    "Range",
                    w.ammo_speed
                        .zip(w.ammo_lifetime)
                        .map(|(s, l)| f64::from(s * l)),
                    "m",
                    1,
                ));
                out.push(stat("pen", "Penetration", f(w.penetration_m), "m", 1));
                // Energy guns carry 0 here: no rounds, a capacitor.
                out.push(stat(
                    "ammo",
                    "Ammo",
                    w.total_ammo.filter(|&n| n > 0).map(f64::from),
                    "",
                    1,
                ));
            }
            out.push(stat("power", "Power", draw, "pips", -1));
            out.push(stat("em", "EM", em, "", -1));
        }
        EItemType::PowerPlant => {
            out.push(stat(
                "output",
                "Output",
                made(ResourceNetworkResource::Power),
                "pips",
                1,
            ));
            out.push(stat("em", "EM", em, "", -1));
        }
        EItemType::Cooler => {
            out.push(stat(
                "coolant",
                "Coolant",
                made(ResourceNetworkResource::Coolant),
                "/s",
                1,
            ));
            out.push(stat("power", "Power", draw, "pips", -1));
            out.push(stat("em", "EM", em, "", -1));
            out.push(stat("ir", "IR", ir, "", -1));
        }
        EItemType::Shield => {
            out.push(stat(
                "hp",
                "Shield HP",
                f(sheet.and_then(|s| s.shield_max_health)),
                "",
                1,
            ));
            out.push(stat(
                "regen",
                "Regen",
                f(sheet.and_then(|s| s.shield_regen)),
                "/s",
                1,
            ));
            let generator = sheet.and_then(|s| s.shield.as_ref());
            out.push(stat(
                "delay",
                "Back up after hit",
                f(generator.map(|g| g.damaged_regen_delay)),
                "s",
                -1,
            ));
            out.push(stat(
                "downDelay",
                "Back up when down",
                f(generator.map(|g| g.downed_regen_delay)),
                "s",
                -1,
            ));
            out.push(stat("power", "Power", draw, "pips", -1));
            out.push(stat("minPower", "Min power", least, "pips", -1));
            out.push(stat("em", "EM", em, "", -1));
        }
        EItemType::QuantumDrive => {
            out.push(stat(
                "qspeed",
                "Speed",
                sheet
                    .and_then(|s| s.quantum.as_ref())
                    .map(|q| f64::from(q.travel.drive_speed) / 1000.0),
                "km/s",
                1,
            ));
            out.push(stat(
                "spool",
                "Spool",
                f(sheet
                    .and_then(|s| s.quantum.as_ref())
                    .map(|q| q.travel.spool_up_time)),
                "s",
                -1,
            ));
            // After a jump long enough to reach full speed; shorter jumps
            // wait less (spviewer's "Max Cooldown").
            out.push(stat(
                "cooldown",
                "Max cooldown",
                f(sheet
                    .and_then(|s| s.quantum.as_ref())
                    .map(|q| q.travel.cooldown_time)),
                "s",
                -1,
            ));
            out.push(stat("power", "Power", draw, "pips", -1));
            out.push(stat("em", "EM", em, "", -1));
        }
        EItemType::Missile => {
            let id = guid.to_string();
            if let Some(m) = cooked.weapons.missiles.iter().find(|m| m.guid == id) {
                // A cluster missile's warhead plus every submunition, all
                // hitting (the Rattler II's 3,470, not its body's 1,370).
                out.push(stat(
                    "damage",
                    "Damage",
                    m.total_damage.or(m.damage).map(|d| f64::from(d.total())),
                    "",
                    1,
                ));
                out.push(stat(
                    "cluster",
                    "Cluster",
                    (m.submunitions > 0).then(|| f64::from(m.submunitions)),
                    "",
                    0,
                ));
                // What it homes on, as an index: 0 IR, 1 EM, 2 CS (the
                // part line and the picker name it).
                out.push(stat(
                    "tracking",
                    "Tracking",
                    m.tracking.as_ref().and_then(|t| {
                        ["Infrared", "Electromagnetic", "CrossSection"]
                            .iter()
                            .position(|s| *s == t.signal)
                            .map(|i| i as f64)
                    }),
                    "",
                    0,
                ));
                out.push(stat(
                    "lockRange",
                    "Lock range",
                    m.tracking.as_ref().map(|t| f64::from(t.lock_range_max_m)),
                    "m",
                    1,
                ));
                out.push(stat(
                    "lockMin",
                    "Lock min",
                    m.tracking.as_ref().map(|t| f64::from(t.lock_range_min_m)),
                    "m",
                    -1,
                ));
                out.push(stat(
                    "lockTime",
                    "Lock time",
                    m.tracking.as_ref().map(|t| f64::from(t.lock_time)),
                    "s",
                    -1,
                ));
                out.push(stat(
                    "lockAngle",
                    "Lock angle",
                    m.tracking.as_ref().map(|t| f64::from(t.lock_angle_deg)),
                    "°",
                    1,
                ));
                out.push(stat(
                    "arm",
                    "Arms after",
                    Some(f64::from(m.arm_time)),
                    "s",
                    -1,
                ));
                out.push(stat("speed", "Speed", f(m.speed), "m/s", 1));
            }
        }
        EItemType::Radar => {
            out.push(stat(
                "aim",
                "Aim assist",
                f(sheet.and_then(|s| s.radar_aim_assist_max)),
                "m",
                1,
            ));
            out.push(stat("power", "Power", draw, "pips", -1));
            out.push(stat("em", "EM", em, "", -1));
        }
        EItemType::WeaponDefensive => {
            out.push(stat(
                "ammo",
                "Ammo",
                sheet
                    .and_then(|s| s.ammo.as_ref())
                    .map(|a| f64::from(a.max)),
                "",
                1,
            ));
        }
        EItemType::FuelTank | EItemType::QuantumFuelTank => {
            out.push(stat(
                "capacity",
                "Capacity",
                sheet
                    .and_then(|s| s.capacity.as_ref())
                    .and_then(|c| c.to_scu())
                    .map(f64::from),
                "SCU",
                1,
            ));
        }
        _ => {
            out.push(stat("power", "Power", draw, "pips", -1));
            out.push(stat("em", "EM", em, "", -1));
        }
    }
    // Distortion: how much a part soaks before it shuts down, and how long
    // a full pool takes to drain again (the delay, then the decay).
    let distortion = sheet.and_then(|s| s.distortion.as_ref());
    out.push(stat(
        "distortion",
        "Distortion pool",
        distortion.map(|d| f64::from(d.maximum)),
        "",
        1,
    ));
    out.push(stat(
        "distRecover",
        "Recovers in",
        distortion
            .filter(|d| d.decay_rate > 0.0)
            .map(|d| f64::from(d.decay_delay + d.maximum / d.decay_rate)),
        "s",
        -1,
    ));
    out.push(stat(
        "health",
        "HP",
        f(sheet.and_then(|s| s.integrity_hp)),
        "",
        1,
    ));
    out.into_iter().flatten().collect()
}
