//! The power network as the ship's power screen shows it: one column per
//! pool (weapons, thrusters, shields) and per item outside a pool (quantum
//! drive, radar, life support, each cooler), the pips each holds, and what
//! the network gives off (cooling, EM, IR, shield regen).
//!
//! sc-vehicles models the client's network ([`PowerNetwork`]): its
//! power-on distribution per mode, how a pool shares its target out, and
//! the readout. This adds the player's hand on it: items switched off, a
//! column set to a level. A pool column sets the pool's target (then the
//! pool shares it out, [`PowerNetwork::split_pools`]); an item column sets
//! that item's grant. Levels never exceed what the plants have left.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use svc_data::ships::Items;
use svc_data::ships::{
    Allocation, EItemType, EMasterMode, Mounted, NetworkParams, PortContainer, PowerNetwork,
    Readout, ResourceProfiles, ShipSignature, Vehicle, VehicleArmor, Vehicles,
};

/// Segments as the client counts them.
const SEGMENT: f64 = 1_000_000.0;

/// Which master mode the ship is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Mode {
    #[default]
    Scm,
    Nav,
}

impl Mode {
    fn master(self) -> EMasterMode {
        match self {
            Self::Scm => EMasterMode::SCM,
            Self::Nav => EMasterMode::Navigation,
        }
    }
}

/// The player's settings on top of power-on.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PowerInput {
    pub mode: Mode,
    /// Items switched off, by port path (`/`-joined, as [`Column::id`]s and
    /// the network's paths spell them); everything below one is off too.
    pub off: Vec<String>,
    /// Columns set by hand: (column id, segments).
    pub levels: Vec<(String, i64)>,
}

/// One column of the power screen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Column {
    /// `pool:<ItemType>` or `item:<path>`.
    pub id: String,
    /// `AttachDef.Type` of what it powers, as the DCB spells it.
    pub item_type: String,
    /// Port paths of the items it powers (a pool's members).
    pub items: Vec<String>,
    /// Pips it can take.
    pub max: i64,
    /// Slots the screen draws: `max`, or more when a pool's members ask
    /// for more than it holds (three shield generators asking 2 each in a
    /// pool of 4 draw 6, the top 2 hollow and never filled).
    pub slots: i64,
    /// The members' minimums, in pips: the blocks the screen draws at the
    /// bottom of the column (one per shield generator). Empty when the
    /// column has none worth drawing.
    pub minimums: Vec<i64>,
    /// Pips it holds now.
    pub level: f64,
    /// Every item it powers is switched off.
    pub off: bool,
    /// It takes no power in this mode (shields and weapons in NAV, the
    /// quantum drive in SCM).
    pub excluded: bool,
}

/// The network's state after the player's settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Power {
    pub mode: Mode,
    /// What the plants that are on make, pips.
    pub supply: i64,
    /// Pips assigned to columns that take power in this mode.
    pub used: f64,
    pub columns: Vec<Column>,
    /// Coolant asked over coolant made; `None` without coolers.
    pub cooling: Option<f64>,
    pub coolant_made: f64,
    pub coolant_asked: f64,
    /// The ship's EM and IR as the screen shows them: the items' sum times
    /// the armour's signal multipliers ([`ShipSignature`]).
    pub em: f64,
    pub ir: f64,
    /// Shield resource the generators make per second.
    pub shield_regen: f64,
    /// Plants: (path, on).
    pub plants: Vec<(String, bool)>,
    /// Where the EM and IR come from, per item type, largest first.
    pub sources: Vec<Source>,
}

/// One item type's share of the signatures, under the armour's
/// multipliers like the totals, so the shares add up to them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Source {
    /// `AttachDef.Type`, as the DCB spells it.
    pub item_type: String,
    pub em: f64,
    /// The IR the coolers give off for this type's heat: a cooler's IR
    /// follows the coolant drawn from it, so each heat source carries its
    /// share of the coolant asked for.
    pub ir: f64,
}

/// The order the power screen lists item types in; pools first.
fn rank(t: &EItemType) -> u8 {
    match t {
        EItemType::WeaponGun => 0,
        EItemType::FlightController => 1,
        EItemType::Shield => 2,
        EItemType::QuantumDrive => 3,
        EItemType::Radar => 4,
        EItemType::LifeSupportGenerator => 5,
        EItemType::Cooler => 7,
        _ => 6,
    }
}

/// The network of `loadout` under `input`: items switched off, the
/// power-on distribution in the mode, then the hand-set columns.
#[allow(clippy::too_many_arguments)]
pub(crate) fn network(
    loadout: &[Mounted],
    container: Option<&PortContainer>,
    vehicles: &Vehicles,
    items: &Items,
    profiles: &ResourceProfiles,
    params: &NetworkParams,
    input: &PowerInput,
) -> (PowerNetwork, Allocation, EMasterMode) {
    let mut net = PowerNetwork::build(loadout, container, vehicles, items, profiles, params);
    // Switching a mount off (a turret) switches off what it carries.
    let off = |path: &str| {
        input.off.iter().any(|p| {
            path == p
                || path
                    .strip_prefix(p.as_str())
                    .is_some_and(|r| r.starts_with('/'))
        })
    };
    let paths: Vec<String> = net
        .plants
        .iter()
        .map(|p| p.path.clone())
        .chain(net.consumers.iter().map(|c| c.path.clone()))
        .filter(|p| off(p))
        .collect();
    let mode = input.mode.master();
    // The power-on split of the whole ship, then the switched-off parts go
    // dark: switching a part off frees its pips and moves nothing else (a
    // power-on with it missing would deal its pips to whatever the player
    // has not set by hand). A part that is off keeps its grant on record
    // but draws nothing; a pool shares its target among the members on.
    let mut a = net.power_on(&mode);
    for p in &paths {
        net.set_on(p, false);
    }
    a.supply = net.supply();
    net.split_pools(&mut a);
    a.remaining = free_supply(&net, &a, &mode);
    apply_levels(&net, &mut a, &mode, &input.levels);
    (net, a, mode)
}

/// What the screen shows of `net` under `a`, on `vehicle` in `armor`.
pub(crate) fn power_of(
    net: &PowerNetwork,
    a: &Allocation,
    mode: &EMasterMode,
    m: Mode,
    vehicle: &Vehicle,
    armor: Option<&VehicleArmor>,
) -> Power {
    let readout = net.readout(a, mode);
    let ship = ShipSignature::compute(&readout, vehicle, armor);
    let columns = columns(net, a, mode);
    let used = columns
        .iter()
        .filter(|c| !c.excluded)
        .map(|c| c.level)
        .sum();
    Power {
        mode: m,
        supply: a.supply,
        used,
        columns,
        cooling: readout.cooling(),
        coolant_made: readout.coolant_made,
        coolant_asked: readout.coolant_asked,
        em: ship.em,
        ir: ship.ir,
        shield_regen: readout.shield_made,
        plants: net.plants.iter().map(|p| (p.path.clone(), p.on)).collect(),
        sources: sources(net, &readout, armor),
    }
}

/// The readout's EM and IR by item type, under `armor`'s multipliers.
fn sources(net: &PowerNetwork, readout: &Readout, armor: Option<&VehicleArmor>) -> Vec<Source> {
    let (em_x, ir_x) = armor.map_or((1.0, 1.0), |a| {
        (
            f64::from(a.signal_electromagnetic),
            f64::from(a.signal_infrared),
        )
    });
    let types: HashMap<&str, &EItemType> = net
        .consumers
        .iter()
        .map(|c| (c.path.as_str(), &c.item_type))
        .collect();
    let mut out: Vec<Source> = Vec::new();
    for i in &readout.items {
        let t = types
            .get(i.path.as_str())
            .copied()
            .unwrap_or(&EItemType::PowerPlant);
        let ir = if readout.coolant_asked > 0.0 {
            readout.ir * i.coolant_asked / readout.coolant_asked * ir_x
        } else {
            0.0
        };
        let em = f64::from(i.em) * em_x;
        let name = t.as_dcb_str();
        match out.iter_mut().find(|s| s.item_type == name) {
            Some(s) => {
                s.em += em;
                s.ir += ir;
            }
            None => out.push(Source {
                item_type: name.to_string(),
                em,
                ir,
            }),
        }
    }
    out.retain(|s| s.em > 0.0 || s.ir > 0.0);
    out.sort_by(|a, b| (b.em + b.ir).total_cmp(&(a.em + a.ir)));
    out
}

/// Set the hand-set columns, each capped by what the plants have left.
///
/// Every lowering goes first, then the raises in the order given: the
/// settings replay on a fresh power-on split, so a raise replayed before
/// the lowering that paid for it would find no free pips and stop short
/// (raising the radar would take a pip back from the weapons).
fn apply_levels(
    net: &PowerNetwork,
    a: &mut Allocation,
    mode: &EMasterMode,
    levels: &[(String, i64)],
) {
    for lowering in [true, false] {
        for (id, want) in levels {
            apply_level(net, a, mode, id, *want, lowering);
        }
    }
    a.remaining = free_supply(net, a, mode);
}

/// Set one column; with `lowering`, only when that lowers it.
fn apply_level(
    net: &PowerNetwork,
    a: &mut Allocation,
    mode: &EMasterMode,
    id: &str,
    want: i64,
    lowering: bool,
) {
    if let Some(t) = id.strip_prefix("pool:") {
        let Some(p) = net.pools.iter().position(|p| p.item_type.as_dcb_str() == t) else {
            return;
        };
        let size = net.pool_size(p);
        let min = net.pool_minimum(p);
        let now = a.pool_targets[p];
        let target = want.clamp(min.min(size), size);
        if lowering && target > now {
            return;
        }
        let free = free_supply(net, a, mode) + now;
        // More than the plants have left: as far as they reach, never
        // below where it was.
        a.pool_targets[p] = target.min(free.max(now));
        net.split_pools(a);
    } else if let Some(path) = id.strip_prefix("item:") {
        let Some(i) = net.consumers.iter().position(|c| c.path == path) else {
            return;
        };
        let c = &net.consumers[i];
        let full = (c.request as f64 / SEGMENT).floor();
        let min = (c.minimum as f64 / SEGMENT).ceil().min(full);
        let now = a.granted[i];
        // An item runs at its minimum or above (the power screen draws
        // the minimum as one block): asking for less than the minimum
        // gets the minimum, asking for nothing switches it to 0.
        let level = if want <= 0 {
            0.0
        } else {
            (want as f64).max(min).min(full)
        };
        if lowering && level > now {
            return;
        }
        let free = (free_supply(net, a, mode) as f64 + now).floor();
        // More than the plants have left: as far as they reach, never
        // below where it was.
        a.granted[i] = if level <= free {
            level
        } else if free >= min {
            free.max(now)
        } else {
            now
        };
    }
}

/// Supply not granted to anything that draws in `mode`.
fn free_supply(net: &PowerNetwork, a: &Allocation, mode: &EMasterMode) -> i64 {
    let drawn: f64 = net
        .consumers
        .iter()
        .zip(&a.granted)
        .filter(|(c, _)| c.draws(mode))
        .map(|(_, g)| g)
        .sum();
    a.supply - drawn.ceil() as i64
}

fn columns(net: &PowerNetwork, a: &Allocation, mode: &EMasterMode) -> Vec<Column> {
    let mut out = Vec::new();
    for (p, pool) in net.pools.iter().enumerate() {
        if pool.members.is_empty() {
            continue;
        }
        let members: Vec<_> = pool.members.iter().map(|&c| &net.consumers[c]).collect();
        let on: Vec<_> = members.iter().filter(|c| c.on).collect();
        let minimums: Vec<i64> = if pool.item_type == EItemType::Shield {
            on.iter()
                .map(|c| (c.minimum as f64 / SEGMENT).round() as i64)
                .filter(|&m| m > 0)
                .collect()
        } else {
            Vec::new()
        };
        let max = net.pool_size(p);
        // What the members on ask for, whole segments up (the client caps
        // a fixed pool at min(that, its size); the screen draws both).
        let asked = on.iter().map(|c| c.request as f64).sum::<f64>() / SEGMENT;
        out.push((
            rank(&pool.item_type),
            Column {
                id: format!("pool:{}", pool.item_type.as_dcb_str()),
                item_type: pool.item_type.as_dcb_str().to_string(),
                items: members.iter().map(|c| c.path.clone()).collect(),
                max,
                slots: max.max(asked.ceil() as i64),
                minimums,
                level: pool
                    .members
                    .iter()
                    .filter(|&&c| net.consumers[c].draws(mode))
                    .map(|&c| a.granted[c])
                    .sum(),
                off: on.is_empty(),
                excluded: !members.iter().any(|c| c.allowed(mode)),
            },
        ));
    }
    for (i, c) in net.consumers.iter().enumerate() {
        if c.pool.is_some() || c.request <= 0 {
            continue;
        }
        out.push((
            rank(&c.item_type),
            Column {
                id: format!("item:{}", c.path),
                item_type: c.item_type.as_dcb_str().to_string(),
                items: vec![c.path.clone()],
                max: (c.request as f64 / SEGMENT).floor() as i64,
                slots: (c.request as f64 / SEGMENT).floor() as i64,
                // Its minimum, the bottom block of its column, as the game
                // draws it (a cooler's or radar's floor).
                minimums: Some((c.minimum as f64 / SEGMENT).ceil() as i64)
                    .filter(|&m| m > 0)
                    .map(|m| m.min((c.request as f64 / SEGMENT).floor() as i64))
                    .into_iter()
                    .collect(),
                level: if c.draws(mode) { a.granted[i] } else { 0.0 },
                off: !c.on,
                excluded: !c.allowed(mode),
            },
        ));
    }
    out.sort_by_key(|(r, _)| *r);
    out.into_iter().map(|(_, c)| c).collect()
}
