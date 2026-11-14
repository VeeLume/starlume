//! Display names for what a session names internally: places, shops,
//! vehicles, items and resources.
//!
//! A session stores the game's own names (`Stanton4_NewBabbage`, a vehicle
//! class, an item GUID): they are what the log and the stream said, and
//! they stay true across patches. The page shows names; this command joins
//! the two against the reference catalogs of the session's channel
//! (`svc_data::names`), and falls back to a cleaned form of the internal
//! name (`mod_session::names`) for what no catalog knows. So the page never
//! shows a raw id, and never reads game data itself.
//!
//! **What resolves:** places (with the planet and system they lie in),
//! vehicles, items, resources. **What does not:** shops (no cooked index
//! maps a shop entity to the name on its sign) and contracts' internal
//! names; both come back cleaned, with `resolved: false`.
//!
//! **Memory** (CLAUDE.md "Memory"): the cooked data is reloaded from its
//! snapshot at most (never parsed) and only when the query names something
//! a catalog could know. The store asks once per name, not per render.
//!
//! **No network.** Local data only, so no online gate applies.

use std::collections::BTreeMap;

use mod_session::names::{clean_contract, clean_item, clean_place, clean_shop, clean_vehicle};
use tauri::{AppHandle, Manager};

use crate::AppState;

/// What the page wants named. Every list holds internal names as the
/// session stores them.
#[derive(Debug, Clone, Default, serde::Deserialize, specta::Type)]
pub struct SessionNameQuery {
    /// The session's install channel (`LIVE`): whose game data to ask.
    /// `None` (a telemetry-only session): the default channel.
    pub channel: Option<String>,
    /// `EntryKind::Location::location`, refinery locations.
    pub places: Vec<String>,
    /// `shop` of purchases and trades.
    pub shops: Vec<String>,
    /// Vehicle class names.
    pub vehicles: Vec<String>,
    pub items: Vec<ItemRef>,
    /// Resource GUIDs of commodity trades.
    pub resources: Vec<String>,
    /// Contracts' internal names (`MissionEnded::contract`).
    pub contracts: Vec<String>,
}

/// An item as a purchase names it.
#[derive(Debug, Clone, Default, serde::Deserialize, specta::Type)]
pub struct ItemRef {
    pub guid: String,
    /// The item's class: what is cleaned when the GUID is unknown.
    pub class_name: String,
}

/// One name for the page.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct DisplayName {
    pub name: String,
    /// For a place: what it lies in, nearest first (planet, system).
    pub parents: Vec<String>,
    /// From the game data. `false`: a cleaned internal name.
    pub resolved: bool,
}

impl DisplayName {
    fn cleaned(name: String) -> Self {
        Self {
            name,
            parents: Vec::new(),
            resolved: false,
        }
    }

    fn resolved(name: String) -> Self {
        Self {
            name,
            parents: Vec::new(),
            resolved: true,
        }
    }
}

/// The names, each keyed by the internal name (or GUID) it was asked by.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct SessionNames {
    /// The channel's game data was there to ask. `false`: everything is a
    /// cleaned name, and asking again after `data:changed` may do better.
    pub data_loaded: bool,
    pub places: BTreeMap<String, DisplayName>,
    pub shops: BTreeMap<String, DisplayName>,
    pub vehicles: BTreeMap<String, DisplayName>,
    /// By item GUID.
    pub items: BTreeMap<String, DisplayName>,
    /// By resource GUID.
    pub resources: BTreeMap<String, DisplayName>,
    pub contracts: BTreeMap<String, DisplayName>,
}

fn clean_place_name(internal: &str) -> DisplayName {
    let place = clean_place(internal);
    DisplayName {
        name: place.name,
        parents: place.system.into_iter().collect(),
        resolved: false,
    }
}

/// Answer `query` from `cooked` (or from the cleaners alone without it).
fn resolve(query: &SessionNameQuery, cooked: Option<&svc_data::CookedData>) -> SessionNames {
    let places = cooked.map(|c| c.place_names());
    SessionNames {
        data_loaded: cooked.is_some(),
        places: query
            .places
            .iter()
            .map(|internal| {
                let name = places
                    .as_ref()
                    .and_then(|p| p.get(internal))
                    .map(|p| DisplayName {
                        name: p.name,
                        parents: p.parents,
                        resolved: true,
                    })
                    .unwrap_or_else(|| clean_place_name(internal));
                (internal.clone(), name)
            })
            .collect(),
        shops: query
            .shops
            .iter()
            .map(|shop| {
                (
                    shop.clone(),
                    DisplayName::cleaned(clean_shop(shop, &query.places)),
                )
            })
            .collect(),
        vehicles: query
            .vehicles
            .iter()
            .map(|class| {
                let name = cooked
                    .and_then(|c| c.vehicle_name(class))
                    .map(DisplayName::resolved)
                    .unwrap_or_else(|| DisplayName::cleaned(clean_vehicle(class)));
                (class.clone(), name)
            })
            .collect(),
        items: query
            .items
            .iter()
            .map(|item| {
                let name = cooked
                    .and_then(|c| c.item_name(&item.guid))
                    .map(DisplayName::resolved)
                    .unwrap_or_else(|| DisplayName::cleaned(clean_item(&item.class_name)));
                (item.guid.clone(), name)
            })
            .collect(),
        resources: query
            .resources
            .iter()
            .map(|guid| {
                let name = cooked
                    .and_then(|c| c.resource_name(guid))
                    .map(DisplayName::resolved)
                    // A GUID cleans to nothing readable.
                    .unwrap_or_else(|| DisplayName::cleaned("Unknown commodity".into()));
                (guid.clone(), name)
            })
            .collect(),
        contracts: query
            .contracts
            .iter()
            .map(|c| (c.clone(), DisplayName::cleaned(clean_contract(c))))
            .collect(),
    }
}

/// Does the query name anything a catalog could know? Shops and contracts
/// are cleaned either way: a query of only those loads nothing.
fn needs_data(query: &SessionNameQuery) -> bool {
    !(query.places.is_empty()
        && query.vehicles.is_empty()
        && query.items.is_empty()
        && query.resources.is_empty())
}

/// The channel of the install the app treats as the user's default.
async fn default_channel(app: &AppHandle) -> Option<String> {
    let cached = app.state::<AppState>().installs.lock().unwrap().clone();
    let installs = if cached.is_empty() {
        crate::data::refresh_installs(app).await.ok()?
    } else {
        cached
    };
    crate::data::default_install(&installs).map(|i| i.channel_key.clone())
}

/// Display names for a session's internal names. Never fails: without game
/// data (not loaded, no install for the channel) every name is the cleaned
/// internal one and `data_loaded` says so.
#[tauri::command]
#[specta::specta]
pub(crate) async fn session_names(app: AppHandle, query: SessionNameQuery) -> SessionNames {
    let cooked = if needs_data(&query) {
        let channel = match &query.channel {
            Some(channel) => Some(channel.clone()),
            None => default_channel(&app).await,
        };
        match channel {
            Some(channel) => crate::data::cooked_for(&app, &channel).await.ok(),
            None => None,
        }
    } else {
        None
    };
    // The place walk and the vehicle scan are a few milliseconds of CPU on
    // a loaded bundle: off the async threads all the same.
    tokio::task::spawn_blocking(move || resolve(&query, cooked.as_deref()))
        .await
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Every name below is made up, in the shape the game writes them.

    fn query() -> SessionNameQuery {
        SessionNameQuery {
            channel: Some("LIVE".into()),
            places: vec!["Vega2_NewCorvo".into(), "RR_JP_VegaTarn".into()],
            shops: vec!["SCShop_TinkersBench_NewCorvo-002".into()],
            vehicles: vec!["VEGA_Courier_Mk2".into()],
            items: vec![ItemRef {
                guid: "5e6f7a8b-aaaa-4bbb-8ccc-ddddeeeeffff".into(),
                class_name: "tnkr_multitool_01_SCItem".into(),
            }],
            resources: vec!["0a1b2c3d-aaaa-4bbb-8ccc-ddddeeeeffff".into()],
            contracts: vec!["Vega_EscortHauler".into()],
        }
    }

    #[test]
    fn without_game_data_every_name_is_the_cleaned_internal_one() {
        let names = resolve(&query(), None);
        assert!(!names.data_loaded);
        let place = &names.places["Vega2_NewCorvo"];
        assert_eq!(place.name, "New Corvo");
        assert_eq!(place.parents, ["Vega"]);
        assert!(!place.resolved);
        assert_eq!(names.places["RR_JP_VegaTarn"].name, "Vega Tarn jump point");
        // The shop loses the place the session names beside it.
        assert_eq!(
            names.shops["SCShop_TinkersBench_NewCorvo-002"].name,
            "Tinkers Bench"
        );
        assert_eq!(names.vehicles["VEGA_Courier_Mk2"].name, "VEGA Courier Mk2");
        assert_eq!(
            names.items["5e6f7a8b-aaaa-4bbb-8ccc-ddddeeeeffff"].name,
            "tnkr multitool 01"
        );
        assert_eq!(
            names.resources["0a1b2c3d-aaaa-4bbb-8ccc-ddddeeeeffff"].name,
            "Unknown commodity"
        );
        assert_eq!(
            names.contracts["Vega_EscortHauler"].name,
            "Vega Escort Hauler"
        );
    }

    #[test]
    fn game_data_that_knows_none_of_it_still_answers_everything() {
        let empty = svc_data::CookedData::default();
        let names = resolve(&query(), Some(&empty));
        assert!(names.data_loaded);
        assert_eq!(names.places.len(), 2);
        assert!(names.places.values().all(|p| !p.resolved));
        assert!(!names.vehicles["VEGA_Courier_Mk2"].resolved);
    }

    #[test]
    fn only_catalog_names_need_the_game_data() {
        assert!(needs_data(&query()));
        let cleaned_only = SessionNameQuery {
            shops: vec!["SCShop_TinkersBench".into()],
            contracts: vec!["Vega_EscortHauler".into()],
            ..SessionNameQuery::default()
        };
        assert!(!needs_data(&cleaned_only));
    }
}
