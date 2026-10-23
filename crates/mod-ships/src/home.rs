//! The Ships page with no ship open: your ships, the ships new since
//! Starlume started watching, and the ones you opened lately.
//!
//! Your ships come from the inventory snapshot svc-dossier keeps (the
//! shell passes the owned classes in; reading it needs no network). A ship
//! is "new" when a build lists it that the first build Starlume saw did
//! not ([`ShipsState::sight`]).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::catalog::ShipRow;
use crate::state::ShipsState;

/// One owned ship class.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Owned {
    pub ship: String,
    /// How many of it the account holds.
    pub count: u32,
    /// All of them are rentals.
    pub rented: bool,
}

/// An entitlement as the shell reads it from the inventory snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct Entitlement {
    pub class: String,
    pub rental: bool,
}

/// The home page's data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Home {
    pub owned: Vec<Owned>,
    /// (ship, the build it arrived in), newest build first.
    pub arrivals: Vec<(String, String)>,
    pub recent: Vec<String>,
}

/// What the home page shows, given the build's `ships`, the owned
/// `entitlements` (None: no inventory read yet) and the module state.
pub fn home(ships: &[ShipRow], entitlements: Option<&[Entitlement]>, state: &ShipsState) -> Home {
    let known: HashMap<&str, &ShipRow> = ships.iter().map(|r| (r.guid.as_str(), r)).collect();
    let mut owned: Vec<Owned> = Vec::new();
    for e in entitlements.unwrap_or_default() {
        let key = e.class.to_ascii_lowercase();
        if !known.contains_key(key.as_str()) {
            continue;
        }
        match owned.iter_mut().find(|o| o.ship == key) {
            Some(o) => {
                o.count += 1;
                o.rented &= e.rental;
            }
            None => owned.push(Owned {
                ship: key,
                count: 1,
                rented: e.rental,
            }),
        }
    }
    owned.sort_by(|a, b| {
        a.rented.cmp(&b.rented).then_with(|| {
            known[a.ship.as_str()]
                .name
                .cmp(&known[b.ship.as_str()].name)
        })
    });
    let mut arrivals: Vec<(String, String)> = state
        .arrivals()
        .into_iter()
        .filter(|(s, _)| known.contains_key(s))
        .map(|(s, b)| (s.to_string(), b.to_string()))
        .collect();
    arrivals.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| known[a.0.as_str()].name.cmp(&known[b.0.as_str()].name))
    });
    Home {
        owned,
        arrivals,
        recent: state
            .recent
            .iter()
            .filter(|s| known.contains_key(s.as_str()))
            .cloned()
            .collect(),
    }
}
