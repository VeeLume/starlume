//! Where quantum travel goes, as a player names the places: the cooked
//! waypoints (sc-locations) with game labels for the star map's kinds, and
//! the routes worth showing by default: each planet to the next one out,
//! and each jump point from the planet nearest to it.
//!
//! The star map's kinds are not the game's words (LIVE 4.10): Lagrange
//! points are `Asteroid`, the Stanton–Pyro and Pyro–Nyx jump points
//! `Anomaly`, gateway stations and comm arrays `Manmade`, Lagrange rest
//! stops `Manmade_VisibleOnInteraction`; `JumpPoint` is Magnus and Terra,
//! not released, and left out.

use serde::{Deserialize, Serialize};
use svc_data::CookedData;
use svc_data::LocationKind;
use svc_data::ships::{RecordCollection, Waypoint};

/// A place quantum travel can go.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Place {
    /// `StarMapObject` GUID.
    pub guid: String,
    pub name: String,
    /// `Planet`, `Moon`, `Lagrange point`, `Jump point`, `Station`, `Rest
    /// stop`, `City` or `Outpost`.
    pub kind: String,
    /// The system's name (`Stanton`).
    pub system: String,
    /// System-global position, m (systems have separate frames).
    pub position: [f64; 3],
    /// Where travel drops out, m from the centre.
    pub arrival: f64,
}

/// The places and the default routes between them (GUID pairs).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Places {
    pub places: Vec<Place>,
    pub routes: Vec<(String, String)>,
}

/// The game's word for a place: its star-map kind, and for an `Anomaly` its
/// name (the jump points, but also the wreck site beside one).
fn label(kind: &LocationKind, name: &str) -> Option<&'static str> {
    Some(match kind {
        LocationKind::Anomaly if !name.to_lowercase().trim_end().ends_with("jump point") => {
            "Point of interest"
        }
        LocationKind::Planet => "Planet",
        LocationKind::Moon => "Moon",
        LocationKind::Asteroid | LocationKind::AsteroidValidQt => "Lagrange point",
        LocationKind::Anomaly | LocationKind::ManmadeJumpPoint => "Jump point",
        LocationKind::Manmade => "Station",
        LocationKind::ManmadeVisibleOnInteraction => "Rest stop",
        LocationKind::LandingZone => "City",
        LocationKind::Outpost => "Outpost",
        _ => return None,
    })
}

/// Every place with a name, by system, then kind, then name.
pub fn places(cooked: &CookedData) -> Places {
    let star_map = cooked.holotable.locations.as_ref();
    let system_name = |w: &Waypoint| {
        let g = w.system?;
        let l = star_map?.get(&g)?;
        l.display_name(&cooked.locale).map(str::to_string)
    };
    let mut places: Vec<Place> = cooked
        .ships
        .waypoints
        .iter()
        .filter_map(|(_, w)| {
            let name = w
                .display_name(&cooked.locale)
                .filter(|n| !n.contains("UNINITIALIZED") && !n.contains("PLACEHOLDER"))?;
            let kind = label(&w.kind, name)?;
            Some(Place {
                guid: w.guid.to_string(),
                name: name.to_string(),
                kind: kind.to_string(),
                system: system_name(w)?,
                position: w.position,
                arrival: f64::from(w.quantum.arrival_radius),
            })
        })
        .collect();
    let rank = |k: &str| {
        [
            "Planet",
            "Moon",
            "Jump point",
            "Station",
            "City",
            "Lagrange point",
            "Rest stop",
            "Outpost",
            "Point of interest",
        ]
        .iter()
        .position(|x| *x == k)
        .unwrap_or(9)
    };
    places.sort_by(|a, b| {
        a.system
            .cmp(&b.system)
            .then(rank(&a.kind).cmp(&rank(&b.kind)))
            .then(a.name.cmp(&b.name))
    });
    let routes = default_routes(&places);
    Places { places, routes }
}

fn dist(a: &Place, b: &Place) -> f64 {
    (0..3)
        .map(|i| (a.position[i] - b.position[i]).powi(2))
        .sum::<f64>()
        .sqrt()
}

/// One jump, m: centre to centre less where travel drops out at the
/// destination (the Ship tab's routes measure the same).
fn leg(a: &Place, b: &Place) -> f64 {
    (dist(a, b) - b.arrival).max(0.0)
}

/// The system every player starts in: its planets set the moon tour (by
/// moon count alone Pyro V won, six moons 0.05–0.17 Gm apart, LIVE 4.10:
/// tighter hops than most players fly).
const HOME: &str = "Stanton";

/// Short hops in a row, the case a drive's top speed says least about:
/// the planet with the most moons, in [`HOME`] when it has any with two
/// or more (a moon belongs to its nearest planet; ties go to the first
/// name), then the nearest moon not yet visited until every one is. The
/// planet's name and the legs, m.
pub fn moon_tour(places: &Places) -> Option<(String, Vec<f64>)> {
    let planets: Vec<&Place> = places
        .places
        .iter()
        .filter(|p| p.kind == "Planet")
        .collect();
    let parent = |m: &Place| {
        planets
            .iter()
            .filter(|p| p.system == m.system)
            .min_by(|a, b| dist(a, m).total_cmp(&dist(b, m)))
            .map(|p| p.guid.as_str())
    };
    let moons_of = |p: &Place| -> Vec<&Place> {
        places
            .places
            .iter()
            .filter(|m| m.kind == "Moon" && parent(m) == Some(p.guid.as_str()))
            .collect()
    };
    let (planet, mut left) = planets
        .iter()
        .map(|p| (*p, moons_of(p)))
        .filter(|(_, ms)| ms.len() > 1)
        .max_by(|(a, x), (b, y)| {
            (a.system == HOME)
                .cmp(&(b.system == HOME))
                .then(x.len().cmp(&y.len()))
                .then(b.name.cmp(&a.name))
        })?;
    let mut at = planet;
    let mut legs = Vec::new();
    while !left.is_empty() {
        let i = (0..left.len()).min_by(|&i, &j| dist(at, left[i]).total_cmp(&dist(at, left[j])))?;
        let next = left.swap_remove(i);
        legs.push(leg(at, next));
        at = next;
    }
    Some((planet.name.clone(), legs))
}

/// Planets closer than this count as one stop (LIVE 4.10: Pyro IV sits
/// 0.38 Gm from Pyro V).
const SAME_STOP: f64 = 2e9;

/// Per system: each planet to the next one out from the star (the star is
/// the frame's origin), then each jump point from its nearest planet.
fn default_routes(places: &[Place]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut systems: Vec<&str> = places.iter().map(|p| p.system.as_str()).collect();
    systems.dedup();
    for system in systems {
        let of = |kind: &str| -> Vec<&Place> {
            places
                .iter()
                .filter(|p| p.system == system && p.kind == kind)
                .collect()
        };
        let mut planets = of("Planet");
        let r = |p: &Place| p.position.iter().map(|v| v * v).sum::<f64>();
        planets.sort_by(|a, b| r(a).total_cmp(&r(b)));
        let mut stops: Vec<&Place> = Vec::new();
        for p in &planets {
            if stops.last().is_none_or(|l| dist(l, p) >= SAME_STOP) {
                stops.push(p);
            }
        }
        for w in stops.windows(2) {
            out.push((w[0].guid.clone(), w[1].guid.clone()));
        }
        for jp in of("Jump point") {
            if let Some(near) = planets
                .iter()
                .min_by(|a, b| dist(a, jp).total_cmp(&dist(b, jp)))
            {
                out.push((near.guid.clone(), jp.guid.clone()));
            }
        }
    }
    out
}
