//! The UEX lists Starlume reads, as rows trimmed to what the pages use.
//! Field names are UEX's own, so a row reads like its documentation.
//!
//! UEX writes some numbers as strings (`"price_avg": "90000"`) and leaves
//! some empty as `null`; the lenient readers below take both, so one odd
//! row never sinks a whole list.

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// One list, its endpoint and its file in the cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Feed {
    /// Every item at every terminal: what a shop sells it for and buys it at.
    ItemPrices,
    /// Every ship at every terminal that sells it.
    VehiclePurchases,
    /// Every ship at every terminal that rents it.
    VehicleRentals,
    /// UEX's ship list: maps its vehicle ids to the game's GUIDs.
    Vehicles,
    /// Where each terminal is.
    Terminals,
    /// Player marketplace averages per item, quality tier and side.
    Marketplace,
}

impl Feed {
    pub const ALL: [Feed; 6] = [
        Feed::Terminals,
        Feed::Vehicles,
        Feed::ItemPrices,
        Feed::VehiclePurchases,
        Feed::VehicleRentals,
        Feed::Marketplace,
    ];

    pub fn endpoint(self) -> &'static str {
        match self {
            Feed::ItemPrices => "items_prices_all",
            Feed::VehiclePurchases => "vehicles_purchases_prices_all",
            Feed::VehicleRentals => "vehicles_rentals_prices_all",
            Feed::Vehicles => "vehicles",
            Feed::Terminals => "terminals",
            Feed::Marketplace => "marketplace_prices_averages_all",
        }
    }

    /// How long a fill stays fresh when the answer names no `max-age`
    /// (what UEX sent on 2026-10-23: 12 h, the marketplace 1 h).
    pub fn default_max_age(self) -> i64 {
        match self {
            Feed::Marketplace => 60 * 60,
            _ => 12 * 60 * 60,
        }
    }
}

/// One item's price at one terminal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemPrice {
    #[serde(deserialize_with = "id")]
    pub id_item: u32,
    #[serde(deserialize_with = "id")]
    pub id_terminal: u32,
    #[serde(default, deserialize_with = "id")]
    pub id_category: u32,
    /// What the player pays; 0: not sold there.
    #[serde(default, deserialize_with = "num")]
    pub price_buy: f64,
    /// What the terminal pays the player; 0: not bought there.
    #[serde(default, deserialize_with = "num")]
    pub price_sell: f64,
    /// The game's record GUID; `None` for items UEX has not matched yet.
    #[serde(default, deserialize_with = "text")]
    pub item_uuid: Option<String>,
    #[serde(default)]
    pub item_name: String,
    #[serde(default)]
    pub terminal_name: String,
    /// Unix seconds of the last report.
    #[serde(default, deserialize_with = "int")]
    pub date_modified: i64,
}

/// One ship's price at one terminal: to buy, or to rent for a day.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VehiclePrice {
    #[serde(deserialize_with = "id")]
    pub id_vehicle: u32,
    #[serde(deserialize_with = "id")]
    pub id_terminal: u32,
    #[serde(alias = "price_buy", alias = "price_rent", deserialize_with = "num")]
    pub price: f64,
    #[serde(default)]
    pub vehicle_name: String,
    #[serde(default)]
    pub terminal_name: String,
    #[serde(default, deserialize_with = "int")]
    pub date_modified: i64,
}

/// A ship in UEX's list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Vehicle {
    #[serde(deserialize_with = "id")]
    pub id: u32,
    #[serde(default, deserialize_with = "text")]
    pub uuid: Option<String>,
    /// "Mercury Star Runner".
    #[serde(default)]
    pub name: String,
    /// "Crusader Mercury Star Runner".
    #[serde(default)]
    pub name_full: String,
    /// The landing pad it needs: "XS", "S", "M", "L", "XL"; `None` for
    /// ground vehicles and ships UEX hasn't sized.
    #[serde(default, deserialize_with = "text")]
    pub pad_type: Option<String>,
}

/// Where a terminal is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Terminal {
    #[serde(deserialize_with = "id")]
    pub id: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default, deserialize_with = "text")]
    pub star_system_name: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub planet_name: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub moon_name: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub city_name: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub space_station_name: Option<String>,
    #[serde(default, deserialize_with = "text")]
    pub outpost_name: Option<String>,
    /// 0: not on the live servers (a PTU-only or retired terminal).
    #[serde(default = "one", deserialize_with = "int")]
    pub is_available_live: i64,
}

/// What players ask or offer for an item on UEX's marketplace, averaged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketAverage {
    #[serde(deserialize_with = "id")]
    pub id_item: u32,
    /// A crafted item's quality tier; 0: none.
    #[serde(default, deserialize_with = "int")]
    pub quality_tier: i64,
    /// "sell" (players selling) or "buy" (players wanting).
    #[serde(default)]
    pub operation: String,
    #[serde(default, deserialize_with = "int")]
    pub listings_count: i64,
    #[serde(default, deserialize_with = "num")]
    pub price_avg: f64,
    #[serde(default, deserialize_with = "num")]
    pub price_avg_week: f64,
    #[serde(default, deserialize_with = "num")]
    pub price_avg_month: f64,
    #[serde(default, deserialize_with = "text")]
    pub game_version: Option<String>,
    #[serde(default, deserialize_with = "int")]
    pub date_modified: i64,
    #[serde(default)]
    pub item_name: String,
    #[serde(default, deserialize_with = "text")]
    pub item_uuid: Option<String>,
}

fn one() -> i64 {
    1
}

fn number(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        Value::Bool(b) => Some(f64::from(u8::from(*b))),
        _ => None,
    }
}

/// A number, or a number written as a string; anything else is 0.
fn num<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    Ok(number(&Value::deserialize(d)?).unwrap_or(0.0))
}

fn int<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    Ok(number(&Value::deserialize(d)?).map_or(0, |n| n as i64))
}

fn id<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    Ok(number(&Value::deserialize(d)?).map_or(0, |n| n as u32))
}

/// A string that says something; `null` and `""` are `None`.
fn text<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::String(s) if !s.trim().is_empty() => Some(s),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lenient_rows() {
        let m: MarketAverage = serde_json::from_str(
            r#"{"id_item":4,"quality_tier":0,"operation":"sell","listings_count":2,
                "price_avg":"90000","price_avg_week":"90000","price_avg_month":null,
                "game_version":"4.10.2","date_modified":1792768178,
                "item_name":"Omnisky XII Cannon","item_uuid":""}"#,
        )
        .unwrap();
        assert_eq!(
            (m.price_avg, m.price_avg_month, m.item_uuid),
            (90000.0, 0.0, None)
        );
        let r: VehiclePrice = serde_json::from_str(
            r#"{"id":3,"id_vehicle":118,"id_terminal":147,"price_rent":37485,
                "vehicle_name":"M50","terminal_name":"Regal NBIS"}"#,
        )
        .unwrap();
        assert_eq!(r.price, 37485.0);
        // The cache writes `price` and reads it back.
        let back: VehiclePrice = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
        assert_eq!(back, r);
    }
}
