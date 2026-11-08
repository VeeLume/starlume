//! The module's persisted state: the ships opened lately, and when each
//! ship was first seen (what "new in this build" reads). Persisted as one
//! JSON file by the shell; this crate does no I/O (module rule 1).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// How many ships "Recently opened" keeps.
pub const RECENT: usize = 8;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShipsState {
    /// Ship GUIDs, most recent first.
    pub recent: Vec<String>,
    /// Ship GUID → the build version it was first seen in.
    pub seen: BTreeMap<String, String>,
    /// The build the first sighting was taken in: every ship seen then is
    /// the baseline, not new.
    pub baseline: Option<String>,
}

impl ShipsState {
    /// Put `ship` at the front of the recent list.
    pub fn opened(&mut self, ship: &str) {
        self.recent.retain(|s| s != ship);
        self.recent.insert(0, ship.to_string());
        self.recent.truncate(RECENT);
    }

    /// Record the ships `build` lists; true when anything changed. The
    /// first build ever recorded is the baseline.
    pub fn sight<'a>(&mut self, build: &str, ships: impl IntoIterator<Item = &'a str>) -> bool {
        let first = self.seen.is_empty();
        let mut changed = false;
        for s in ships {
            if !self.seen.contains_key(s) {
                self.seen.insert(s.to_string(), build.to_string());
                changed = true;
            }
        }
        if first && changed {
            self.baseline = Some(build.to_string());
        }
        changed
    }

    /// Ships first seen after the baseline, with the build they arrived in.
    pub fn arrivals(&self) -> Vec<(&str, &str)> {
        self.seen
            .iter()
            .filter(|(_, b)| Some(b.as_str()) != self.baseline.as_deref())
            .map(|(s, b)| (s.as_str(), b.as_str()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_then_arrivals() {
        let mut s = ShipsState::default();
        assert!(s.sight("4.10.1", ["a", "b"]));
        assert!(s.arrivals().is_empty());
        assert!(!s.sight("4.10.1", ["a", "b"]));
        assert!(s.sight("4.10.2", ["a", "b", "c"]));
        assert_eq!(s.arrivals(), [("c", "4.10.2")]);
    }

    #[test]
    fn recent_moves_to_front_and_caps() {
        let mut s = ShipsState::default();
        for i in 0..10 {
            s.opened(&i.to_string());
        }
        s.opened("5");
        assert_eq!(s.recent.len(), RECENT);
        assert_eq!(s.recent[0], "5");
        assert_eq!(s.recent.iter().filter(|r| *r == "5").count(), 1);
    }
}
