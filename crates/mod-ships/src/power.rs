//! The power network as the ship's power screen shows it: one column per
//! pool (weapons, thrusters, shields) and per item outside a pool (quantum
//! drive, radar, life support, each cooler), the pips each holds, and what
//! the network gives off (cooling, EM, IR, shield regen).
//!
//! sc-vehicles models the client's network ([`PowerNetwork`]): its
//! power-on distribution per mode, how a pool shares its target out, and
//! the readout. This adds the player's hand on it: items switched off, a
//! column set to a level. A pool column sets the pool's target
//! ([`PowerNetwork::set_pool_target`]); an item column sets that item's
//! grant ([`PowerNetwork::set_grant`]). Both work in whole segments, as the
//! client does: a raise takes only the free supply, nothing snaps a level
//! to the members' minimums. Levels and free supply are read back from the
//! library ([`PowerNetwork::pool_level`], [`PowerNetwork::free_supply`]),
//! never re-summed from the members' fractional grants.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use svc_data::crafting::GameplayStat;
use svc_data::ships::Items;
use svc_data::ships::{
    Allocation, EItemType, EMasterMode, Mounted, NetworkParams, PoolSize, PortContainer,
    PowerNetwork, Readout, ResourceProfiles, ShipSignature, Vehicle, VehicleArmor, Vehicles,
};

/// Segments as the client counts them.
const SEGMENT: f64 = 1_000_000.0;

use crate::crafted::CraftedParts;

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
    /// Slots the screen draws; those above `max` are hollow, never filled:
    /// a fixed pool whose members ask for less than its size (ballistic
    /// guns), or a pool counting only some of its members (three shield
    /// generators asking 2 each in a pool counting 2 draw 6).
    pub slots: i64,
    /// The members' minimums, in pips: the blocks the screen draws at the
    /// bottom of the column (one per shield generator). Empty when the
    /// column has none worth drawing.
    pub minimums: Vec<i64>,
    /// Pips it holds now: a pool's target as the client charges it
    /// ([`PowerNetwork::pool_level`]), an item's grant; 0 when it takes no
    /// power in this mode.
    pub level: i64,
    /// Of `level`, the pips its items run on, whole. Below `level` when a
    /// pip is set but powers nothing: a pool's members get less than its
    /// target when a member's minimum does not fit (mining heads of
    /// minimum 2 at a target of 3 run at 2), yet the pool takes all of it.
    pub powered: i64,
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
    /// Pips taken in this mode: `supply` less the network's
    /// [`free_supply`](PowerNetwork::free_supply). Above `supply` when the
    /// settings ask for more than the plants make.
    pub used: i64,
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
    crafted: &CraftedParts,
) -> (PowerNetwork, Allocation, EMasterMode) {
    let mut net = PowerNetwork::build(loadout, container, vehicles, items, profiles, params);
    // Crafted plants and coolers ([`crate::crafted`]), before the power
    // settles: power-on deals the segments the plants make.
    for p in &mut net.plants {
        let c = crafted.change(&p.path, &GameplayStat::PowerGeneration);
        p.units = c.apply(f64::from(p.units)).round().max(0.0) as i32;
    }
    for c in &mut net.consumers {
        c.coolant = crafted
            .change(&c.path, &GameplayStat::CoolantGeneration)
            .apply(c.coolant);
    }
    let (a, mode) = settle(&mut net, input);
    (net, a, mode)
}

/// `net` under `input`: items switched off, the power-on distribution in
/// the mode, then the hand-set columns.
fn settle(net: &mut PowerNetwork, input: &PowerInput) -> (Allocation, EMasterMode) {
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
    apply_levels(net, &mut a, &mode, &input.levels);
    (a, mode)
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
    let used = a.supply - net.free_supply(a, mode);
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
}

/// The minimum blocks at the bottom of a pool's column, in pips: one per
/// member the pool counts that is on, in the client's order. The power
/// screen draws each as one block and sets it whole (a MOLE's three mining
/// heads of minimum 2: three blocks, 2 pips a click; a shield generator of
/// minimum 2 asking 4: one block, then single pips). A fixed pool splits
/// its target by ratio and has none.
fn pool_minimums(net: &PowerNetwork, pool: usize) -> Vec<i64> {
    let counted = match net.pools[pool].size {
        PoolSize::Dynamic { max_items: Some(n) } => usize::try_from(n).unwrap_or(0),
        PoolSize::Dynamic { max_items: None } => usize::MAX,
        _ => return Vec::new(),
    };
    net.pools[pool]
        .members
        .iter()
        .map(|&c| &net.consumers[c])
        .filter(|c| c.on)
        .take(counted)
        .map(|c| (c.minimum as f64 / SEGMENT).round() as i64)
        .filter(|&m| m > 0)
        .collect()
}

/// A loose item's minimum block, in pips; none when it has no minimum.
fn item_minimum(c: &svc_data::ships::Consumer) -> Option<i64> {
    let request = (c.request as f64 / SEGMENT).floor() as i64;
    Some((c.minimum as f64 / SEGMENT).ceil() as i64)
        .filter(|&m| m > 0)
        .map(|m| m.min(request))
}

/// `level` as the screen can hold it: a level inside a minimum block falls
/// to the block's foot, since a block is set whole or not at all.
fn whole_blocks(level: i64, minimums: &[i64]) -> i64 {
    let mut foot = 0;
    for m in minimums {
        if level >= foot + m {
            foot += m;
        } else {
            return foot;
        }
    }
    level
}

/// Set one column as the power screen does; with `lowering`, only when
/// that lowers it, else only when it raises it. The library clamps the ask
/// (a pool to its size, an item to its request) and caps a raise by the
/// free supply, as the server does; the screen only ever asks for whole
/// minimum blocks, so a level the supply leaves inside one falls back to
/// the block's foot.
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
        if (want < a.pool_targets[p]) == lowering {
            let minimums = pool_minimums(net, p);
            let got = net.set_pool_target(a, p, whole_blocks(want, &minimums), mode);
            let held = whole_blocks(got, &minimums);
            if held != got {
                net.set_pool_target(a, p, held, mode);
            }
        }
    } else if let Some(path) = id.strip_prefix("item:") {
        let Some(i) = net
            .consumers
            .iter()
            .position(|c| c.path == path && c.pool.is_none())
        else {
            return;
        };
        // The client truncates the grant to whole segments.
        if (want < a.granted[i] as i64) == lowering {
            let minimum: Vec<i64> = item_minimum(&net.consumers[i]).into_iter().collect();
            let got = net.set_grant(a, i, whole_blocks(want, &minimum), mode);
            if got.is_some_and(|got| whole_blocks(got, &minimum) != got) {
                net.set_grant(a, i, 0, mode);
            }
        }
    }
}

/// The pips a pool's members run on, whole: what they were granted,
/// rounded up to the segment (a fixed pool's members can take a fraction
/// of its last pip), at most the pool's `level`. The grants are summed in
/// micro-units first, so float noise (seven guns at 2/7 each) is no pip.
fn pool_powered(
    net: &PowerNetwork,
    a: &Allocation,
    pool: usize,
    mode: &EMasterMode,
    level: i64,
) -> i64 {
    let granted: f64 = net.pools[pool]
        .members
        .iter()
        .filter(|&&c| net.consumers[c].draws(mode))
        .map(|&c| a.granted[c])
        .sum();
    let micro = (granted * SEGMENT).round().max(0.0) as u64;
    (micro.div_ceil(SEGMENT as u64) as i64).min(level)
}

fn columns(net: &PowerNetwork, a: &Allocation, mode: &EMasterMode) -> Vec<Column> {
    let mut out = Vec::new();
    for (p, pool) in net.pools.iter().enumerate() {
        if pool.members.is_empty() {
            continue;
        }
        let members: Vec<_> = pool.members.iter().map(|&c| &net.consumers[c]).collect();
        let on: Vec<_> = members.iter().filter(|c| c.on).collect();
        // A member past those the pool counts only widens it (a third
        // shield generator: its slots hollow, no minimum block).
        let minimums = pool_minimums(net, p);
        let max = net.pool_size(p);
        // The hollow slots, as the game draws them:
        // - A fixed pool draws its size. Members asking less leave the rest
        //   hollow (ballistic guns). Members asking more get no more (12
        //   guns asking 12.2 in the Centaurus's 4).
        // - A pool sized by its members that counts only so many of them
        //   draws what all of them ask for, the uncounted ones hollow (the
        //   Super Hornet Mk II's shields: 2 of its 3 generators).
        let slots = match pool.size {
            PoolSize::Fixed(size) => max.max(i64::from(size)),
            PoolSize::Dynamic { .. } => {
                let asked = on.iter().map(|c| c.request as f64).sum::<f64>() / SEGMENT;
                max.max(asked.ceil() as i64)
            }
            PoolSize::Plain => max,
        };
        let level = net.pool_level(a, p, mode);
        out.push((
            rank(&pool.item_type),
            Column {
                id: format!("pool:{}", pool.item_type.as_dcb_str()),
                item_type: pool.item_type.as_dcb_str().to_string(),
                items: members.iter().map(|c| c.path.clone()).collect(),
                max,
                slots,
                minimums,
                level,
                powered: pool_powered(net, a, p, mode, level),
                off: on.is_empty(),
                excluded: !members.iter().any(|c| c.allowed(mode)),
            },
        ));
    }
    for (i, c) in net.consumers.iter().enumerate() {
        if c.pool.is_some() || c.request <= 0 {
            continue;
        }
        // A loose item's grant is whole segments; rounded as the library's
        // free supply counts it.
        let level = if c.draws(mode) {
            a.granted[i].round() as i64
        } else {
            0
        };
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
                minimums: item_minimum(c).into_iter().collect(),
                level,
                // The screen sets its minimum block whole, so a level it
                // holds is powered.
                powered: level,
                off: !c.on,
                excluded: !c.allowed(mode),
            },
        ));
    }
    out.sort_by_key(|(r, _)| *r);
    out.into_iter().map(|(_, c)| c).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use svc_data::Guid;
    use svc_data::ships::{Consumer, DistributionDefaults, Plant, Pool, Signals};

    const SEG: i64 = 1_000_000;
    /// The cooler every test network carries: power-on tops it up to its
    /// whole request, and nothing here sets it.
    const COOLER: i64 = 3;

    fn consumer(path: &str, item_type: EItemType, request: i64, minimum: i64) -> Consumer {
        Consumer {
            path: path.into(),
            item: Guid::default(),
            item_type,
            request,
            minimum,
            coolant: 0.0,
            shield: 0.0,
            pool: None,
            on: true,
            excluded_in: Vec::new(),
            signals: Signals::default(),
        }
    }

    /// One plant making `supply`, a cooler of [`COOLER`] outside any pool,
    /// then `members`, pooled by item type into `pools`.
    fn network(
        supply: i32,
        members: Vec<Consumer>,
        pools: &[(EItemType, PoolSize)],
    ) -> PowerNetwork {
        let mut cooler = consumer("cooler", EItemType::Cooler, COOLER * SEG, 0);
        cooler.coolant = 40.0;
        let mut consumers = vec![cooler];
        consumers.extend(members);
        let mut pools: Vec<Pool> = pools
            .iter()
            .map(|(t, size)| Pool {
                item_type: t.clone(),
                size: *size,
                members: Vec::new(),
            })
            .collect();
        for (i, c) in consumers.iter_mut().enumerate() {
            if let Some(p) = pools.iter().position(|p| p.item_type == c.item_type) {
                c.pool = Some(p);
                pools[p].members.push(i);
            }
        }
        PowerNetwork {
            plants: vec![Plant {
                path: "plant".into(),
                item: Guid::default(),
                units: supply,
                modifier: 0,
                on: true,
                signals: Signals::default(),
            }],
            consumers,
            pools,
            max_default_distribution: false,
            defaults: DistributionDefaults {
                pools_fill_min: true,
                cooler_min: 1.0,
                pools_proportion: 1.0,
                weapons: 0.5,
                engines: 0.5,
                shields: 0.5,
            },
        }
    }

    struct Seen {
        columns: Vec<Column>,
        free: i64,
        used: i64,
    }

    impl Seen {
        fn col(&self, id: &str) -> &Column {
            self.columns.iter().find(|c| c.id == id).unwrap()
        }
    }

    fn see(net: &PowerNetwork, input: PowerInput) -> Seen {
        let mut net = net.clone();
        let (a, mode) = settle(&mut net, &input);
        let free = net.free_supply(&a, &mode);
        Seen {
            columns: columns(&net, &a, &mode),
            free,
            used: a.supply - free,
        }
    }

    fn levels(levels: &[(&str, i64)]) -> PowerInput {
        PowerInput {
            levels: levels.iter().map(|(c, l)| (c.to_string(), *l)).collect(),
            ..Default::default()
        }
    }

    /// Guns asking 1.2 pips each in a fixed pool smaller than their total:
    /// each gets a fraction of a pip (3 guns t/3, 7 guns t/7, the Marauder's
    /// case). The pool is charged its whole target at every step, and the
    /// supply's last pip goes to it.
    #[test]
    fn fixed_pool_charges_its_whole_target() {
        for (guns, size) in [(3, 3), (7, 7)] {
            let members = (0..guns)
                .map(|i| consumer(&format!("gun_{i}"), EItemType::WeaponGun, 1_200_000, 0))
                .collect();
            let supply = COOLER + size;
            let net = network(
                supply as i32,
                members,
                &[(EItemType::WeaponGun, PoolSize::Fixed(size as i32))],
            );
            for t in 0..=size {
                let s = see(&net, levels(&[("pool:WeaponGun", t)]));
                let w = s.col("pool:WeaponGun");
                assert_eq!((w.level, w.powered), (t, t), "{guns} guns at {t}");
                assert_eq!(s.free, supply - COOLER - t, "{guns} guns at {t}");
                assert_eq!(s.used, COOLER + t, "{guns} guns at {t}");
            }
            // From power-on, one pip at a time up to the last.
            let on = see(&net, PowerInput::default());
            let mut level = on.col("pool:WeaponGun").level;
            assert!(level < size);
            while level < size {
                let s = see(&net, levels(&[("pool:WeaponGun", level + 1)]));
                assert_eq!(s.col("pool:WeaponGun").level, level + 1);
                level += 1;
                assert_eq!(s.free, supply - COOLER - level);
            }
            assert_eq!(see(&net, levels(&[("pool:WeaponGun", size)])).free, 0);
        }
    }

    fn heads() -> Vec<Consumer> {
        (1..=2)
            .map(|i| {
                consumer(
                    &format!("head_{i}"),
                    EItemType::WeaponMining,
                    4 * SEG,
                    2 * SEG,
                )
            })
            .collect()
    }

    const MINING: &str = "pool:WeaponMining";

    /// Two heads of minimum 2 asking 4 in a dynamic pool: each minimum is
    /// one block, set whole, then single pips (the game's power screen: a
    /// MOLE's mining column goes 23 -> 21 -> 19 -> 17 of 23, a column with
    /// one minimum block 23 -> 21 -> 20 -> 19).
    #[test]
    fn dynamic_pool_sets_minimum_blocks_whole() {
        let supply = COOLER + 8 + 2;
        let net = network(
            supply as i32,
            heads(),
            &[(
                EItemType::WeaponMining,
                PoolSize::Dynamic { max_items: None },
            )],
        );
        assert_eq!(
            see(&net, PowerInput::default()).col(MINING).minimums,
            [2, 2]
        );
        // An ask inside a block falls to the block's foot; above the
        // blocks every pip is its own.
        let held = [0, 0, 2, 2, 4, 5, 6, 7, 8];
        for (t, want) in held.into_iter().enumerate() {
            let s = see(&net, levels(&[(MINING, t as i64)]));
            let m = s.col(MINING);
            assert_eq!((m.level, m.powered), (want, want), "ask {t}");
            assert_eq!(s.free, supply - COOLER - want, "ask {t}");
        }
    }

    /// Three heads asking exactly their minimum of 2 (the MOLE): the column
    /// is three blocks and nothing else, 2 pips a click.
    #[test]
    fn heads_asking_their_minimum_are_all_blocks() {
        let members = (1..=3)
            .map(|i| {
                consumer(
                    &format!("head_{i}"),
                    EItemType::WeaponMining,
                    2 * SEG,
                    2 * SEG,
                )
            })
            .collect();
        let net = network(
            (COOLER + 6) as i32,
            members,
            &[(
                EItemType::WeaponMining,
                PoolSize::Dynamic { max_items: None },
            )],
        );
        for (ask, want) in [(1, 0), (2, 2), (3, 2), (4, 4), (5, 4), (6, 6)] {
            let s = see(&net, levels(&[(MINING, ask)]));
            let m = s.col(MINING);
            assert_eq!((m.minimums.as_slice(), m.max), ([2, 2, 2].as_slice(), 6));
            assert_eq!(
                (m.level, m.powered, s.free),
                (want, want, 6 - want),
                "ask {ask}"
            );
        }
    }

    /// The supply reaches only into a block: the block stays unset.
    #[test]
    fn a_block_the_supply_cannot_fill_stays_unset() {
        let net = network(
            (COOLER + 3) as i32,
            heads(),
            &[(
                EItemType::WeaponMining,
                PoolSize::Dynamic { max_items: None },
            )],
        );
        let s = see(&net, levels(&[(MINING, 4)]));
        assert_eq!((s.col(MINING).level, s.free), (2, 1));
    }

    /// A head switched off shrinks the pool to the one left: the pool is
    /// charged no more than that, whether power-on or the player set it.
    #[test]
    fn switched_off_members_shrink_the_charge() {
        let supply = COOLER + 8 + 2;
        let net = network(
            supply as i32,
            heads(),
            &[(
                EItemType::WeaponMining,
                PoolSize::Dynamic { max_items: None },
            )],
        );
        let on = see(&net, PowerInput::default());
        assert_eq!(on.col(MINING).level, 8);
        for set in [vec![], vec![(MINING.to_string(), 8)]] {
            let s = see(
                &net,
                PowerInput {
                    off: vec!["head_2".into()],
                    levels: set,
                    ..Default::default()
                },
            );
            let m = s.col(MINING);
            assert_eq!((m.level, m.powered, m.max), (4, 4, 4));
            assert_eq!(s.free, supply - COOLER - 4);
        }
        let s = see(
            &net,
            PowerInput {
                off: vec!["head_1".into(), "head_2".into()],
                ..Default::default()
            },
        );
        assert!(s.col(MINING).off);
        assert_eq!((s.col(MINING).level, s.free), (0, supply - COOLER));
    }

    /// Shields excluded in NAV: their pool keeps its target on record but
    /// is charged nothing, and setting it there changes nothing.
    #[test]
    fn excluded_pool_charges_nothing() {
        let supply = COOLER + 8;
        let shields = (1..=2)
            .map(|i| {
                let mut s = consumer(&format!("shield_{i}"), EItemType::Shield, 4 * SEG, 2 * SEG);
                s.excluded_in = vec![EMasterMode::Navigation];
                s
            })
            .collect();
        let net = network(
            supply as i32,
            shields,
            &[(EItemType::Shield, PoolSize::Dynamic { max_items: Some(2) })],
        );
        let scm = see(&net, PowerInput::default());
        assert!(scm.col("pool:Shield").level > 0);
        for set in [vec![], vec![("pool:Shield".to_string(), 8)]] {
            let s = see(
                &net,
                PowerInput {
                    mode: Mode::Nav,
                    levels: set,
                    ..Default::default()
                },
            );
            let c = s.col("pool:Shield");
            assert!(c.excluded);
            assert_eq!((c.level, c.powered), (0, 0));
            assert_eq!((s.free, s.used), (supply - COOLER, COOLER));
        }
    }

    /// An item outside a pool: its minimum is one block, set whole or not
    /// at all, and a raise takes only the free supply.
    #[test]
    fn item_minimum_is_one_block() {
        let mut net = network(COOLER as i32 + 1, Vec::new(), &[]);
        net.consumers[0].minimum = 2 * SEG;
        let s = see(&net, levels(&[("item:cooler", 1)]));
        assert_eq!((s.col("item:cooler").level, s.free), (0, COOLER + 1));
        let s = see(&net, levels(&[("item:cooler", 2)]));
        assert_eq!((s.col("item:cooler").level, s.free), (2, COOLER - 1));
        // Asked for more than it requests: its whole request, no more.
        let s = see(&net, levels(&[("item:cooler", 9)]));
        assert_eq!((s.col("item:cooler").level, s.free), (COOLER, 1));
    }

    /// A raise past the free supply goes as far as it reaches.
    #[test]
    fn raise_is_capped_by_free_supply() {
        let net = network(
            (COOLER + 4) as i32,
            heads(),
            &[(
                EItemType::WeaponMining,
                PoolSize::Dynamic { max_items: None },
            )],
        );
        let s = see(&net, levels(&[(MINING, 8)]));
        assert_eq!((s.col(MINING).level, s.free), (4, 0));
        // The cooler gives up a pip, the heads take it: lowerings go first.
        let s = see(&net, levels(&[(MINING, 8), ("item:cooler", 2)]));
        assert_eq!((s.col(MINING).level, s.free), (5, 0));
    }
}
