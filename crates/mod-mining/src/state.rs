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
    pub saved: Vec<SavedLoadout>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedLoadout {
    pub id: String,
    pub name: String,
    pub loadout: LoadoutInput,
}

impl MiningState {
    /// Save `loadout` under `name`, replacing a saved one of the same name
    /// (case-insensitive); `new_id` names a new entry. Returns its id.
    pub fn save(
        &mut self,
        name: &str,
        loadout: LoadoutInput,
        new_id: impl FnOnce() -> String,
    ) -> String {
        let name = name.trim();
        if let Some(s) = self
            .saved
            .iter_mut()
            .find(|s| s.name.eq_ignore_ascii_case(name))
        {
            s.name = name.to_string();
            s.loadout = loadout;
            return s.id.clone();
        }
        let id = new_id();
        self.saved.push(SavedLoadout {
            id: id.clone(),
            name: name.to_string(),
            loadout,
        });
        id
    }

    pub fn delete(&mut self, id: &str) -> bool {
        let before = self.saved.len();
        self.saved.retain(|s| s.id != id);
        self.saved.len() != before
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
            }],
            gadget: None,
        }
    }

    #[test]
    fn save_replaces_by_name() {
        let mut s = MiningState::default();
        let a = s.save("Golem", golem(), || "1".into());
        let b = s.save(" golem ", LoadoutInput::default(), || "2".into());
        assert_eq!(a, b);
        assert_eq!(s.saved.len(), 1);
        assert_eq!(s.saved[0].loadout, LoadoutInput::default());
        assert!(s.delete("1"));
        assert!(s.saved.is_empty());
    }

    #[test]
    fn old_state_files_load() {
        let s: MiningState = serde_json::from_str("{}").unwrap();
        assert_eq!(s, MiningState::default());
    }
}
