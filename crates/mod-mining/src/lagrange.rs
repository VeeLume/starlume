//! Lagrange points as the finder shows them (design pass 4): under their
//! planet, with the rest stop there and what hangs off them.
//!
//! The data parents a Lagrange point to the star; its planet is only in its
//! locale key (`@Stanton1_L1` → `@Stanton1` = Hurston, `@Pyro3_L1` → Bloom).
//! Rest stops hang off the star too and are keyed by the point
//! (`@RR_HUR_L1` for HUR L1, `@RR_P3_L1` for PYR3 L1). Gas pockets
//! (`HUR L1-A`, `@Stanton1_L1_01`) and mining claims (`@MiningClaim_…`) are
//! the point's children — a claim sometimes under a pocket (CRU L2-B). The
//! point is the place you quantum to; pockets are hidden and claims are not
//! searchable on the star map, so both are named by their point.

use std::collections::HashMap;

use svc_data::{Guid, LocaleMap, LocationKind, Locations, RecordCollection};

/// One Lagrange point.
pub(crate) struct Point {
    /// As the game names it: `HUR L1`, `PYR3 L1`.
    pub name: String,
    pub planet: Guid,
    pub rest_stop: Option<String>,
}

/// What a place is to its Lagrange point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Role {
    /// The point itself (Pyro's spawn tables sit on it).
    Point,
    /// A gas pocket at the point.
    Pocket,
    /// A mining claim at the point.
    Claim,
}

pub(crate) struct Points {
    by_guid: HashMap<Guid, Point>,
}

impl Points {
    pub fn build(locations: &Locations, locale: &LocaleMap) -> Self {
        let key = |l: &svc_data::Location| l.name_key.as_ref().map(|k| k.as_str().to_string());
        let mut planets: HashMap<String, Guid> = HashMap::new();
        let mut rest_stops: HashMap<String, String> = HashMap::new();
        for l in locations.values() {
            let Some(k) = key(l) else { continue };
            match l.kind {
                LocationKind::Planet => {
                    planets.insert(k, l.guid);
                }
                LocationKind::Manmade | LocationKind::ManmadeVisibleOnInteraction
                    if k.starts_with("@RR_") =>
                {
                    if let Some(name) = l.display_name(locale) {
                        rest_stops.insert(k, name.to_string());
                    }
                }
                _ => {}
            }
        }
        let mut by_guid = HashMap::new();
        for l in locations.values() {
            if !matches!(
                l.kind,
                LocationKind::Asteroid | LocationKind::AsteroidValidQt
            ) {
                continue;
            }
            let Some(k) = key(l) else { continue };
            let Some((base, n)) = k.rsplit_once("_L") else {
                continue;
            };
            if n.len() != 1 || !n.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let (Some(&planet), Some(name)) = (planets.get(base), l.display_name(locale)) else {
                continue;
            };
            // Stanton's rest stops are keyed by the point's name
            // (@RR_HUR_L1), Pyro's by the planet's number (@RR_P3_L1).
            let by_name = format!("@RR_{}", name.replace(' ', "_"));
            let by_number = base.strip_prefix("@Pyro").map(|p| format!("@RR_P{p}_L{n}"));
            let rest_stop = rest_stops
                .get(&by_name)
                .or_else(|| by_number.and_then(|k| rest_stops.get(&k)))
                .cloned();
            by_guid.insert(
                l.guid,
                Point {
                    name: name.to_string(),
                    planet,
                    rest_stop,
                },
            );
        }
        Self { by_guid }
    }

    pub fn get(&self, guid: &Guid) -> Option<&Point> {
        self.by_guid.get(guid)
    }

    /// The point a place belongs to, and what it is there.
    pub fn of(&self, locations: &Locations, guid: &Guid) -> Option<(Guid, Role)> {
        if self.by_guid.contains_key(guid) {
            return Some((*guid, Role::Point));
        }
        let claim = locations
            .get(guid)?
            .name_key
            .as_ref()
            .is_some_and(|k| k.as_str().starts_with("@MiningClaim"));
        locations
            .ancestors(guid)
            .find(|a| self.by_guid.contains_key(&a.guid))
            .map(|a| (a.guid, if claim { Role::Claim } else { Role::Pocket }))
    }
}

/// The title a claim table gets: the yield it adds, from its record name.
pub(crate) fn claim_label(record: &str) -> String {
    if record.contains("Medium_Yield") {
        "Medium-yield claims".into()
    } else if record.contains("Low_Yield") {
        "Low-yield claims".into()
    } else if record.contains("High_Yield") {
        "High-yield claims".into()
    } else {
        "Mining claims".into()
    }
}

/// `L1, L2, L3` → `L1–L3`; gaps stay listed.
pub(crate) fn ranges(mut ns: Vec<u8>) -> String {
    ns.sort_unstable();
    ns.dedup();
    let consecutive = ns.windows(2).all(|w| w[1] == w[0] + 1);
    match ns.as_slice() {
        [] => String::new(),
        [one] => format!("L{one}"),
        [first, .., last] if consecutive => format!("L{first}–L{last}"),
        _ => ns
            .iter()
            .map(|n| format!("L{n}"))
            .collect::<Vec<_>>()
            .join(", "),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_ranges_read() {
        assert_eq!(ranges(vec![3, 1, 2, 5, 4]), "L1–L5");
        assert_eq!(ranges(vec![2, 4]), "L2, L4");
        assert_eq!(ranges(vec![3]), "L3");
    }

    #[test]
    fn claim_labels_read() {
        assert_eq!(
            claim_label("AsteroidCluster_Medium_Yield"),
            "Medium-yield claims"
        );
        assert_eq!(claim_label("AsteroidCluster_Low_Yield"), "Low-yield claims");
        assert_eq!(claim_label("HPP_Lagrange_C"), "Mining claims");
    }
}
