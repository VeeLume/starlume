//! The ships a player can pick: every vehicle a player can own (base hulls,
//! editions, reward ships), named from the locale.

use serde::{Deserialize, Serialize};
use svc_data::ships::{Defense, Listing, RecordCollection};
use svc_data::{CookedData, LocaleKey};

/// One ship in the picker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShipRow {
    /// `EntityClassDefinition` GUID.
    pub guid: String,
    pub record_name: String,
    /// The locale's `vehicleName`, else the record name.
    pub name: String,
    /// The maker's name and code (`RSI`); `None` when the data has none.
    pub manufacturer: Option<String>,
    pub manufacturer_code: Option<String>,
    /// `vehicleCareer` / `vehicleRole`, resolved (`Transporter`,
    /// `Medium Freight / Gun Ship`).
    pub career: Option<String>,
    pub role: Option<String>,
    /// `crewSize`: CIG's crew figure.
    pub crew: i32,
    /// The stock loadout's seats by what they control.
    pub seats: crate::Seats,
    /// The base hull this is an edition or reward version of (the record
    /// its insurance returns), when it is one: the ship picker lists it
    /// under its base.
    pub base: Option<String>,
    /// A ground vehicle.
    pub ground: bool,
    /// Handed out by a contract or reward terminal.
    pub reward: bool,
    /// Cargo grids of the stock loadout, SCU (sc-vehicles' `Storage`).
    pub cargo: Option<f64>,
    /// The stock armour's deflection per damage type: a hit at or below it
    /// does nothing (which guns can hurt this ship at all).
    #[serde(default)]
    pub deflection: Option<crate::view::Typed<f32>>,
}

impl ShipRow {
    /// The ways other lists spell the ship, for joining one that has no
    /// GUID for it (UEX's prices): the name, with its maker in front, the
    /// maker in place of the name's first word ("Grey's Shiv", "Grey's
    /// Market Shiv"), and without a leading maker code ("RSI Constellation
    /// Mk V Centaurus", "Constellation Mk V Centaurus").
    pub fn names(&self) -> Vec<String> {
        let name = self.name.trim();
        let mut out = vec![name.to_string()];
        if let Some(m) = &self.manufacturer {
            out.push(format!("{m} {name}"));
            if let Some((_, rest)) = name.split_once(' ') {
                out.push(format!("{m} {rest}"));
            }
        }
        if let Some(code) = &self.manufacturer_code
            && let Some(rest) = name.strip_prefix(code.as_str())
        {
            out.push(rest.trim().to_string());
        }
        out
    }
}

/// Every ship a player can own, sorted by name.
pub fn ships(cooked: &CookedData) -> Vec<ShipRow> {
    let index = &cooked.ships;
    let text = |k: Option<&LocaleKey>| {
        k.and_then(|k| cooked.locale.resolve(k))
            .filter(|n| !n.is_empty())
            .map(str::to_string)
    };
    let makers = cooked.holotable.manufacturers.as_ref();
    let mut rows: Vec<ShipRow> = index
        .listed
        .iter()
        .filter_map(|e| {
            let v = index.vehicles.get(&e.guid)?;
            let maker = v
                .manufacturer
                .as_ref()
                .and_then(|g| makers.and_then(|m| m.get(g)));
            Some(ShipRow {
                guid: e.guid.to_string(),
                record_name: e.record_name.clone(),
                name: text(v.name_key.as_ref()).unwrap_or_else(|| e.record_name.clone()),
                manufacturer: maker.and_then(|m| text(m.name_key.as_ref())),
                manufacturer_code: maker.map(|m| m.code.clone()).filter(|c| !c.is_empty()),
                career: text(v.career_key.as_ref()),
                role: text(v.role_key.as_ref()),
                crew: v.crew_size,
                seats: crate::seats(cooked, v, &v.stock),
                base: v
                    .insured_as
                    .filter(|b| *b != e.guid && v.listing != Listing::Base)
                    .filter(|b| index.listed.iter().any(|l| l.guid == *b))
                    .map(|b| b.to_string()),
                ground: v.ground,
                reward: v.listing == Listing::Reward,
                cargo: crate::view::storage(cooked, v, &e.guid, &v.stock).map(|s| s.cargo),
                // The armour alone: no power network needed.
                deflection: Defense::compute(v, &v.stock, &index.vehicles, &index.components)
                    .armor
                    .as_ref()
                    .and_then(|a| a.deflection.as_ref())
                    .map(crate::view::Typed::of),
            })
        })
        .collect();
    rows.sort_by(|a, b| a.name.cmp(&b.name).then(a.record_name.cmp(&b.record_name)));
    group_editions(&mut rows);
    rows
}

/// Every edition under one root hull. Insurance names a base only one step
/// up (the F8C PYAM Exec's base is the F8C Executive Edition, itself a
/// base), so the chain is followed to its root; and a base whose name
/// extends another base's of the same maker (`Anvil F8C Lightning
/// Executive Edition`, `Anvil Terrapin Medic`) goes under the shorter one,
/// longest match first.
fn group_editions(rows: &mut [ShipRow]) {
    let names: Vec<(String, String, Option<String>)> = rows
        .iter()
        .filter(|r| r.base.is_none())
        .map(|r| (r.guid.clone(), r.name.clone(), r.manufacturer.clone()))
        .collect();
    for r in rows.iter_mut().filter(|r| r.base.is_none()) {
        // The same name twice (two records of one hull): the first is the
        // hull, the rest its editions. Else the longest name it extends.
        let twin = names
            .iter()
            .find(|(_, n, m)| *n == r.name && *m == r.manufacturer)
            .filter(|(g, _, _)| *g != r.guid);
        r.base = twin
            .or_else(|| {
                names
                    .iter()
                    .filter(|(g, n, m)| {
                        *g != r.guid && *m == r.manufacturer && r.name.starts_with(&format!("{n} "))
                    })
                    .max_by_key(|(_, n, _)| n.len())
            })
            .map(|(g, _, _)| g.clone());
    }
    let parent: std::collections::HashMap<String, Option<String>> = rows
        .iter()
        .map(|r| (r.guid.clone(), r.base.clone()))
        .collect();
    for r in rows.iter_mut() {
        let mut root = r.base.clone();
        let mut hops = 0;
        while let Some(up) = root.as_ref().and_then(|b| parent.get(b).cloned().flatten()) {
            root = Some(up);
            hops += 1;
            if hops > 8 {
                break;
            }
        }
        r.base = root;
    }
}
