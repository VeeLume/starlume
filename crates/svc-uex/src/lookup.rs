//! Finding a part or a ship in the cached lists. The game's record GUID
//! first (UEX's `uuid`, the same GUID the cooked data uses: 24 of 26 S2
//! power plants on 2026-10-23); then the name, for the records UEX has not
//! matched yet (new items, 37 of the 174 ships on sale). One UEX id per
//! thing, so a matched item brings every terminal row it has.

use std::collections::{HashMap, HashSet};

use crate::feeds::{ItemPrice, MarketAverage, Terminal, Vehicle, VehiclePrice};

/// Everything cached, loaded for one question and dropped after.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub item_prices: Vec<ItemPrice>,
    pub purchases: Vec<VehiclePrice>,
    pub rentals: Vec<VehiclePrice>,
    pub vehicles: Vec<Vehicle>,
    pub terminals: Vec<Terminal>,
    pub market: Vec<MarketAverage>,
}

/// One place that sells (or rents, or buys) a thing.
#[derive(Debug, Clone, PartialEq)]
pub struct Offer {
    /// The terminal as UEX names it ("Dumper's Area 18").
    pub terminal: String,
    /// Where it is, smallest first ("Area 18 · ArcCorp · Stanton"); empty
    /// when the terminal is not in the list.
    pub place: String,
    pub price: f64,
    /// Unix seconds of the last report.
    pub updated: i64,
}

/// What the shops do with an item.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ItemOffers {
    /// Where to buy it, cheapest first.
    pub buy: Vec<Offer>,
    /// Where to sell it, best price first.
    pub sell: Vec<Offer>,
}

/// Where a ship is sold and rented, cheapest first.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VehicleOffers {
    pub buy: Vec<Offer>,
    /// Price per rental (UEX: one day).
    pub rent: Vec<Offer>,
    /// The landing pad it needs ("XS" to "XL"), as UEX lists it.
    pub pad: Option<String>,
}

/// Lowercase letters and digits only: "Helios IP-20" and "helios ip20"
/// are one name.
pub fn norm(s: &str) -> String {
    s.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// The questions one page asks, over one [`Snapshot`]: built once, so a
/// list of options asks in constant time each.
pub struct Index<'a> {
    snap: &'a Snapshot,
    item_by_uuid: HashMap<String, HashSet<u32>>,
    item_by_name: HashMap<String, HashSet<u32>>,
    prices_of: HashMap<u32, Vec<&'a ItemPrice>>,
    market_of: HashMap<u32, Vec<&'a MarketAverage>>,
    vehicle_by_uuid: HashMap<String, Vec<u32>>,
    vehicle_by_name: HashMap<String, Vec<u32>>,
    vehicle_by_letters: HashMap<String, Vec<u32>>,
    /// Vehicles with any buy or rent row.
    vehicle_offered: HashSet<u32>,
    terminals: HashMap<u32, &'a Terminal>,
}

/// A name's letters in order: the last resort for the same words in
/// another order ("Ares Star Fighter Inferno", "Ares Inferno Starfighter").
fn letters(s: &str) -> String {
    let mut c: Vec<char> = norm(s).chars().collect();
    c.sort_unstable();
    c.into_iter().collect()
}

impl<'a> Index<'a> {
    pub fn new(snap: &'a Snapshot) -> Self {
        let mut item_by_uuid: HashMap<String, HashSet<u32>> = HashMap::new();
        let mut item_by_name: HashMap<String, HashSet<u32>> = HashMap::new();
        let mut prices_of: HashMap<u32, Vec<&ItemPrice>> = HashMap::new();
        for p in &snap.item_prices {
            if let Some(u) = &p.item_uuid {
                item_by_uuid
                    .entry(u.to_ascii_lowercase())
                    .or_default()
                    .insert(p.id_item);
            }
            item_by_name
                .entry(norm(&p.item_name))
                .or_default()
                .insert(p.id_item);
            prices_of.entry(p.id_item).or_default().push(p);
        }
        let mut market_of: HashMap<u32, Vec<&MarketAverage>> = HashMap::new();
        for m in &snap.market {
            if let Some(u) = &m.item_uuid {
                item_by_uuid
                    .entry(u.to_ascii_lowercase())
                    .or_default()
                    .insert(m.id_item);
            }
            item_by_name
                .entry(norm(&m.item_name))
                .or_default()
                .insert(m.id_item);
            market_of.entry(m.id_item).or_default().push(m);
        }
        // UEX gives some records the same GUID or name (Prowler and Prowler
        // Utility shared one on 2026-10-23): every id is kept, and the
        // question picks among them.
        let mut vehicle_by_uuid: HashMap<String, Vec<u32>> = HashMap::new();
        let mut vehicle_by_name: HashMap<String, Vec<u32>> = HashMap::new();
        let mut vehicle_by_letters: HashMap<String, Vec<u32>> = HashMap::new();
        for v in &snap.vehicles {
            if let Some(u) = &v.uuid {
                vehicle_by_uuid
                    .entry(u.to_ascii_lowercase())
                    .or_default()
                    .push(v.id);
            }
            for n in [&v.name_full, &v.name] {
                vehicle_by_name.entry(norm(n)).or_default().push(v.id);
                vehicle_by_letters.entry(letters(n)).or_default().push(v.id);
            }
        }
        let vehicle_offered = snap
            .purchases
            .iter()
            .chain(&snap.rentals)
            .map(|p| p.id_vehicle)
            .collect();
        Self {
            snap,
            item_by_uuid,
            item_by_name,
            prices_of,
            market_of,
            vehicle_by_uuid,
            vehicle_by_name,
            vehicle_by_letters,
            vehicle_offered,
            terminals: snap.terminals.iter().map(|t| (t.id, t)).collect(),
        }
    }

    /// UEX's ids for the item with record GUID `guid`, else named `name`.
    fn items(&self, guid: &str, name: &str) -> HashSet<u32> {
        self.item_by_uuid
            .get(&guid.to_ascii_lowercase())
            .or_else(|| self.item_by_name.get(&norm(name)))
            .cloned()
            .unwrap_or_default()
    }

    /// The terminal's place, smallest first.
    fn place(&self, terminal: u32) -> String {
        let Some(t) = self.terminals.get(&terminal) else {
            return String::new();
        };
        let site = t
            .city_name
            .as_ref()
            .or(t.space_station_name.as_ref())
            .or(t.outpost_name.as_ref());
        let body = t.moon_name.as_ref().or(t.planet_name.as_ref());
        [site, body, t.star_system_name.as_ref()]
            .into_iter()
            .flatten()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(" · ")
    }

    /// A terminal the live servers have; one missing from the list counts.
    fn live(&self, terminal: u32) -> bool {
        self.terminals
            .get(&terminal)
            .is_none_or(|t| t.is_available_live != 0)
    }

    /// The shops for the item with record GUID `guid` (display name
    /// `name` when UEX has no GUID for it).
    pub fn item(&self, guid: &str, name: &str) -> ItemOffers {
        let ids = self.items(guid, name);
        let rows = ids
            .iter()
            .flat_map(|id| self.prices_of.get(id).into_iter().flatten())
            .filter(|p| self.live(p.id_terminal));
        let mut out = ItemOffers::default();
        for p in rows {
            let offer = |price| Offer {
                terminal: p.terminal_name.clone(),
                place: self.place(p.id_terminal),
                price,
                updated: p.date_modified,
            };
            if p.price_buy > 0.0 {
                out.buy.push(offer(p.price_buy));
            }
            if p.price_sell > 0.0 {
                out.sell.push(offer(p.price_sell));
            }
        }
        out.buy.sort_by(|a, b| {
            a.price
                .total_cmp(&b.price)
                .then(a.terminal.cmp(&b.terminal))
        });
        out.sell.sort_by(|a, b| {
            b.price
                .total_cmp(&a.price)
                .then(a.terminal.cmp(&b.terminal))
        });
        out
    }

    /// Marketplace averages for the item: per quality tier and side.
    pub fn market(&self, guid: &str, name: &str) -> Vec<&'a MarketAverage> {
        let ids = self.items(guid, name);
        let mut out: Vec<&MarketAverage> = ids
            .iter()
            .flat_map(|id| self.market_of.get(id).into_iter().flatten().copied())
            .collect();
        out.sort_by(|a, b| {
            a.operation
                .cmp(&b.operation)
                .then(a.quality_tier.cmp(&b.quality_tier))
        });
        out
    }

    /// UEX's id for the ship with record GUID `guid`, else the first of
    /// `names` it knows (with and without the maker: "Crusader Mercury Star
    /// Runner", "Mercury Star Runner"), else the same letters in another
    /// order.
    pub fn vehicle_id(&self, guid: &str, names: &[&str]) -> Option<u32> {
        let named: HashSet<String> = names.iter().map(|n| norm(n)).collect();
        if let Some(ids) = self.vehicle_by_uuid.get(&guid.to_ascii_lowercase()) {
            // Several records on one GUID: the one named like ours.
            let by_name = ids.iter().copied().find(|id| {
                self.snap.vehicles.iter().any(|v| {
                    v.id == *id
                        && (named.contains(&norm(&v.name_full)) || named.contains(&norm(&v.name)))
                })
            });
            return by_name.or_else(|| self.pick(ids));
        }
        names
            .iter()
            .find_map(|n| {
                self.vehicle_by_name
                    .get(&norm(n))
                    .and_then(|ids| self.pick(ids))
            })
            .or_else(|| {
                names.iter().find_map(|n| {
                    self.vehicle_by_letters
                        .get(&letters(n))
                        .and_then(|ids| self.pick(ids))
                })
            })
    }

    /// Of several ids for one ship, the one with prices.
    fn pick(&self, ids: &[u32]) -> Option<u32> {
        ids.iter()
            .copied()
            .find(|id| self.vehicle_offered.contains(id))
            .or_else(|| ids.first().copied())
    }

    /// Where the ship is sold and rented.
    pub fn vehicle(&self, guid: &str, names: &[&str]) -> VehicleOffers {
        let Some(id) = self.vehicle_id(guid, names) else {
            return VehicleOffers::default();
        };
        let offers = |rows: &[VehiclePrice]| {
            let mut out: Vec<Offer> = rows
                .iter()
                .filter(|r| r.id_vehicle == id && r.price > 0.0 && self.live(r.id_terminal))
                .map(|r| Offer {
                    terminal: r.terminal_name.clone(),
                    place: self.place(r.id_terminal),
                    price: r.price,
                    updated: r.date_modified,
                })
                .collect();
            out.sort_by(|a, b| {
                a.price
                    .total_cmp(&b.price)
                    .then(a.terminal.cmp(&b.terminal))
            });
            out
        };
        VehicleOffers {
            buy: offers(&self.snap.purchases),
            rent: offers(&self.snap.rentals),
            pad: self
                .snap
                .vehicles
                .iter()
                .find(|v| v.id == id)
                .and_then(|v| v.pad_type.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn price(item: u32, terminal: u32, buy: f64, uuid: Option<&str>, name: &str) -> ItemPrice {
        ItemPrice {
            id_item: item,
            id_terminal: terminal,
            id_category: 21,
            price_buy: buy,
            price_sell: 0.0,
            item_uuid: uuid.map(str::to_string),
            item_name: name.to_string(),
            terminal_name: format!("Shop {terminal}"),
            date_modified: 1,
        }
    }

    fn terminal(id: u32, live: i64) -> Terminal {
        Terminal {
            id,
            name: format!("T{id}"),
            star_system_name: Some("Stanton".into()),
            planet_name: Some("ArcCorp".into()),
            moon_name: None,
            city_name: Some("Area 18".into()),
            space_station_name: None,
            outpost_name: None,
            is_available_live: live,
        }
    }

    #[test]
    fn items_by_guid_then_name_cheapest_first_live_only() {
        let snap = Snapshot {
            item_prices: vec![
                price(
                    1,
                    10,
                    242_000.0,
                    Some("1FC13DB3-38BD-40B4-AADA-3D4D665D1679"),
                    "StarBurn",
                ),
                price(
                    1,
                    11,
                    240_000.0,
                    Some("1fc13db3-38bd-40b4-aada-3d4d665d1679"),
                    "StarBurn",
                ),
                price(
                    1,
                    12,
                    1.0,
                    Some("1fc13db3-38bd-40b4-aada-3d4d665d1679"),
                    "StarBurn",
                ),
                price(2, 10, 300_000.0, None, "Helios IP-20"),
            ],
            terminals: vec![terminal(10, 1), terminal(11, 1), terminal(12, 0)],
            ..Snapshot::default()
        };
        let ix = Index::new(&snap);
        let star = ix.item("1fc13db3-38bd-40b4-aada-3d4d665d1679", "whatever");
        assert_eq!(
            star.buy.iter().map(|o| o.price).collect::<Vec<_>>(),
            [240_000.0, 242_000.0]
        );
        assert_eq!(star.buy[0].place, "Area 18 · ArcCorp · Stanton");
        // No GUID at UEX: the name finds it, spelling aside.
        assert_eq!(
            ix.item("00000000-0000-0000-0000-000000000000", "Helios IP20")
                .buy
                .len(),
            1
        );
        assert!(
            ix.item("00000000-0000-0000-0000-000000000000", "Nothing")
                .buy
                .is_empty()
        );
    }

    #[test]
    fn vehicles_by_guid_or_either_name() {
        let snap = Snapshot {
            vehicles: vec![
                Vehicle {
                    id: 7,
                    uuid: None,
                    name: "Mercury Star Runner".into(),
                    name_full: "Crusader Mercury Star Runner".into(),
                    pad_type: Some("M".into()),
                },
                Vehicle {
                    id: 8,
                    uuid: Some("abc".into()),
                    name: "M50".into(),
                    name_full: "Origin M50".into(),
                    pad_type: None,
                },
            ],
            purchases: vec![VehiclePrice {
                id_vehicle: 7,
                id_terminal: 1,
                price: 5_000_000.0,
                vehicle_name: "Mercury Star Runner".into(),
                terminal_name: "New Deal".into(),
                date_modified: 1,
            }],
            rentals: vec![VehiclePrice {
                id_vehicle: 8,
                id_terminal: 1,
                price: 37_485.0,
                vehicle_name: "M50".into(),
                terminal_name: "Regal".into(),
                date_modified: 1,
            }],
            ..Snapshot::default()
        };
        let ix = Index::new(&snap);
        assert_eq!(
            ix.vehicle("nope", &["Crusader Mercury Star Runner"])
                .buy
                .len(),
            1
        );
        assert_eq!(
            ix.vehicle("nope", &["MSR", "Mercury Star Runner"])
                .buy
                .len(),
            1
        );
        assert_eq!(ix.vehicle("ABC", &[]).rent[0].price, 37_485.0);
        assert_eq!(
            ix.vehicle("nope", &["Mercury Star Runner"]).pad.as_deref(),
            Some("M")
        );
    }
}
