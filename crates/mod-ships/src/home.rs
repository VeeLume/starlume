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
    /// How many of those are rentals.
    pub rented: u32,
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
                o.rented += u32::from(e.rental);
            }
            None => owned.push(Owned {
                ship: key,
                count: 1,
                rented: u32::from(e.rental),
            }),
        }
    }
    // Classes you only rent come last.
    let only_rented = |o: &Owned| o.rented == o.count;
    owned.sort_by(|a, b| {
        only_rented(a).cmp(&only_rented(b)).then_with(|| {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn row(guid: &str, name: &str) -> ShipRow {
        ShipRow {
            guid: guid.into(),
            record_name: name.into(),
            name: name.into(),
            manufacturer: None,
            manufacturer_code: None,
            career: None,
            role: None,
            crew: 1,
            seats: Default::default(),
            base: None,
            ground: false,
            gravlev: false,
            reward: false,
            cargo: None,
            deflection: None,
        }
    }

    fn copy(class: &str, rental: bool) -> Entitlement {
        Entitlement {
            class: class.into(),
            rental,
        }
    }

    #[test]
    fn a_class_counts_its_copies_and_how_many_are_rented() {
        let ships = [
            row("aa", "Zephyr"),
            row("bb", "Albatross"),
            row("cc", "Mule"),
        ];
        let held = [
            // A pledged and a rented Zephyr; the class GUID in either case.
            copy("AA", false),
            copy("aa", true),
            copy("bb", true),
            copy("cc", false),
            // Not a ship of this build: a part, a paint.
            copy("dd", false),
        ];
        let home = home(&ships, Some(&held), &ShipsState::default());
        let got: Vec<(&str, u32, u32)> = home
            .owned
            .iter()
            .map(|o| (o.ship.as_str(), o.count, o.rented))
            .collect();
        // By name, the class that is only rented last.
        assert_eq!(got, [("cc", 1, 0), ("aa", 2, 1), ("bb", 1, 1)]);
    }
}
