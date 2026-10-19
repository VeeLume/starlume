//! The module's personal state: the loadout on the calculator and the ones
//! the player saved ("my Golem", "MOLE crew"). Ids are record GUIDs, which
//! survive patches; an id a later build no longer has is reported by the
//! solve, not dropped here.

use serde::{Deserialize, Serialize};

use crate::solve::LoadoutInput;

/// Persisted as one JSON file by the shell.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MiningState {
    /// The loadout the calculator opens with.
    pub current: LoadoutInput,
    /// The saved loadout (its id) your part of `current` came from: edits
    /// on the calculator are a try-out against it until saved.
    pub based_on: Option<String>,
    pub saved: Vec<SavedLoadout>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedLoadout {
    pub id: String,
    pub name: String,
    pub loadout: LoadoutInput,
    /// The crew mate it belongs to; `None` for your own.
    #[serde(default)]
    pub owner: Option<String>,
}

impl SavedLoadout {
    /// A crew loadout as support for the solver.
    pub fn as_crew(&self) -> Option<crate::solve::CrewShip> {
        Some(crate::solve::CrewShip {
            id: self.id.clone(),
            owner: self.owner.clone()?,
            name: self.name.clone(),
            loadout: self.loadout.clone(),
        })
    }
}

impl MiningState {
    /// Save `loadout` as `name` for `owner` (`None`: yours). With `id` it
    /// updates that entry; otherwise it replaces a saved one of the same
    /// name and owner (name case-insensitive), or adds one named by
    /// `new_id`. Returns the entry's id.
    pub fn save(
        &mut self,
        id: Option<&str>,
        name: &str,
        owner: Option<&str>,
        loadout: LoadoutInput,
        new_id: impl FnOnce() -> String,
    ) -> String {
        let name = name.trim();
        let owner = owner
            .map(str::trim)
            .filter(|o| !o.is_empty())
            .map(str::to_string);
        let hit = match id {
            Some(id) => self.saved.iter_mut().find(|s| s.id == id),
            None => self
                .saved
                .iter_mut()
                .find(|s| s.name.eq_ignore_ascii_case(name) && s.owner == owner),
        };
        if let Some(s) = hit {
            s.name = name.to_string();
            s.owner = owner;
            s.loadout = loadout;
            return s.id.clone();
        }
        let id = new_id();
        self.saved.push(SavedLoadout {
            id: id.clone(),
            name: name.to_string(),
            loadout,
            owner,
        });
        id
    }

    pub fn delete(&mut self, id: &str) -> bool {
        let before = self.saved.len();
        self.saved.retain(|s| s.id != id);
        if self.based_on.as_deref() == Some(id) {
            self.based_on = None;
        }
        self.saved.len() != before
    }

    /// The crew loadouts, as support for the solver.
    pub fn crew(&self) -> Vec<crate::solve::CrewShip> {
        self.saved
            .iter()
            .filter_map(SavedLoadout::as_crew)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solve::LaserInput;

    fn golem() -> LoadoutInput {
        LoadoutInput {
            lasers: vec![LaserInput {
                laser: "pitman".into(),
                modules: vec![],
                owner: 0,
                who: None,
                crew: None,
                distance_m: None,
            }],
            gadget: None,
            lead: None,
        }
    }

    #[test]
    fn save_replaces_by_name_and_owner() {
        let mut s = MiningState::default();
        let a = s.save(None, "Golem", None, golem(), || "1".into());
        let b = s.save(None, " golem ", None, LoadoutInput::default(), || {
            "2".into()
        });
        assert_eq!(a, b);
        assert_eq!(s.saved.len(), 1);
        assert_eq!(s.saved[0].loadout, LoadoutInput::default());
        // The same name for a crew mate is another loadout.
        let c = s.save(None, "Golem", Some("Juno"), golem(), || "3".into());
        assert_eq!(c, "3");
        assert_eq!(s.crew().len(), 1);
        // An id edits in place, renames included.
        s.save(Some("3"), "Prospector", Some("Juno"), golem(), || {
            "4".into()
        });
        assert_eq!(s.saved.len(), 2);
        assert_eq!(s.crew()[0].name, "Prospector");
        assert!(s.delete("1"));
        assert_eq!(s.saved.len(), 1);
    }

    #[test]
    fn old_state_files_load() {
        let s: MiningState = serde_json::from_str("{}").unwrap();
        assert_eq!(s, MiningState::default());
    }
}
