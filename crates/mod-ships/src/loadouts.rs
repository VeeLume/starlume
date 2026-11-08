//! The player's loadouts: the ones saved by name ("PVE Centaurus") and,
//! per ship, the working draft the Build tab has open, so an app restart
//! keeps the edits (frontend rule 6). A draft is a try-out against the
//! saved loadout it came from (or stock) until saved; mining's saved
//! loadouts work the same way. Persisted as one JSON file by the shell;
//! this crate does no I/O (module rule 1).
//!
//! Ids in the edits are record GUIDs and port names, which survive
//! patches; an edit a later build no longer fits is the view's to report,
//! not dropped here.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::view::Edit;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Loadouts {
    /// Ship GUID → what the Build tab has open for it.
    pub drafts: BTreeMap<String, Draft>,
    pub saved: Vec<SavedLoadout>,
}

/// A loadout as the Build tab edits it: swaps on top of the stock
/// loadout, in the order made, and the hand on the power network.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Draft {
    /// The saved loadout it started from; `None`: stock.
    pub based_on: Option<String>,
    pub edits: Vec<Edit>,
    /// Port paths (`/`-joined) switched off.
    pub off: Vec<String>,
    /// Power columns set by hand: (column id, pips), in the order set.
    pub levels: Vec<(String, i64)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedLoadout {
    pub id: String,
    /// The ship's GUID: an edition (Mk V Centaurus) is a ship of its own.
    pub ship: String,
    pub name: String,
    pub edits: Vec<Edit>,
    #[serde(default)]
    pub off: Vec<String>,
    #[serde(default)]
    pub levels: Vec<(String, i64)>,
    /// The game build it was last saved in (`4.10.2-live.12913470`).
    #[serde(default)]
    pub build: String,
    /// Unix seconds.
    #[serde(default)]
    pub saved_at: i64,
}

impl Loadouts {
    /// The saved loadouts of `ship`, by name.
    pub fn of(&self, ship: &str) -> Vec<&SavedLoadout> {
        let mut out: Vec<&SavedLoadout> = self.saved.iter().filter(|s| s.ship == ship).collect();
        out.sort_by_key(|s| s.name.to_lowercase());
        out
    }

    /// Save `draft` for `ship` as `name`. With `id` it updates that entry
    /// (renames included); otherwise it replaces the ship's saved loadout
    /// of the same name (case-insensitive) or adds one named by `new_id`.
    /// The ship's draft is then based on it, with nothing left unsaved.
    /// Returns the entry's id. The build, the time and the id maker come in
    /// from outside so the crate stays free of clocks and I/O.
    #[allow(clippy::too_many_arguments)]
    pub fn save(
        &mut self,
        ship: &str,
        id: Option<&str>,
        name: &str,
        draft: Draft,
        build: &str,
        now: i64,
        new_id: impl FnOnce() -> String,
    ) -> String {
        let name = name.trim();
        let hit = match id {
            Some(id) => self.saved.iter_mut().find(|s| s.id == id && s.ship == ship),
            None => self
                .saved
                .iter_mut()
                .find(|s| s.ship == ship && s.name.eq_ignore_ascii_case(name)),
        };
        let id = match hit {
            Some(s) => {
                s.name = name.to_string();
                s.edits = draft.edits.clone();
                s.off = draft.off.clone();
                s.levels = draft.levels.clone();
                s.build = build.to_string();
                s.saved_at = now;
                s.id.clone()
            }
            None => {
                let id = new_id();
                self.saved.push(SavedLoadout {
                    id: id.clone(),
                    ship: ship.to_string(),
                    name: name.to_string(),
                    edits: draft.edits.clone(),
                    off: draft.off.clone(),
                    levels: draft.levels.clone(),
                    build: build.to_string(),
                    saved_at: now,
                });
                id
            }
        };
        self.drafts.insert(
            ship.to_string(),
            Draft {
                based_on: Some(id.clone()),
                ..draft
            },
        );
        id
    }

    /// Open the saved loadout `id` (or stock, with `None`) on its ship,
    /// replacing the ship's draft. `false` when `id` is not one of the
    /// ship's.
    pub fn open(&mut self, ship: &str, id: Option<&str>) -> bool {
        let draft = match id {
            None => Draft::default(),
            Some(id) => match self.saved.iter().find(|s| s.id == id && s.ship == ship) {
                Some(s) => Draft {
                    based_on: Some(s.id.clone()),
                    edits: s.edits.clone(),
                    off: s.off.clone(),
                    levels: s.levels.clone(),
                },
                None => return false,
            },
        };
        self.drafts.insert(ship.to_string(), draft);
        true
    }

    /// Rename the saved loadout `id` of `ship`; its parts, its build stamp
    /// and any draft based on it stay. `false` when `id` is not the ship's.
    pub fn rename(&mut self, ship: &str, id: &str, name: &str) -> bool {
        match self.saved.iter_mut().find(|s| s.id == id && s.ship == ship) {
            Some(s) => {
                s.name = name.to_string();
                true
            }
            None => false,
        }
    }

    /// Forget a saved loadout; a draft based on it keeps its edits as a
    /// try-out against stock.
    pub fn delete(&mut self, id: &str) -> bool {
        let before = self.saved.len();
        self.saved.retain(|s| s.id != id);
        for d in self.drafts.values_mut() {
            if d.based_on.as_deref() == Some(id) {
                d.based_on = None;
            }
        }
        self.saved.len() != before
    }

    /// Keep `draft` as the ship's working loadout. A `based_on` that names
    /// no saved loadout of the ship (nor a loadout in the game) falls back
    /// to stock.
    pub fn set_draft(&mut self, ship: &str, mut draft: Draft) {
        if let Some(b) = &draft.based_on
            && !in_game(b)
            && !self.saved.iter().any(|s| &s.id == b && s.ship == ship)
        {
            draft.based_on = None;
        }
        if draft == Draft::default() {
            self.drafts.remove(ship);
        } else {
            self.drafts.insert(ship.to_string(), draft);
        }
    }
}

/// A draft's base that is one of your ships' loadouts in the game
/// (`insured:<urn>`, `live:<urn>`, see [`crate::owned`]), not a saved one:
/// read-only, so kept as written and never saved into.
pub fn in_game(based_on: &str) -> bool {
    based_on.starts_with("insured:") || based_on.starts_with("live:")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn swap(port: &str, item: &str) -> Edit {
        Edit {
            path: vec![port.to_string()],
            item: Some(item.to_string()),
        }
    }

    fn draft(edits: Vec<Edit>) -> Draft {
        Draft {
            edits,
            ..Draft::default()
        }
    }

    #[test]
    fn save_replaces_by_name_per_ship_and_bases_the_draft() {
        let mut l = Loadouts::default();
        let a = l.save(
            "ship",
            None,
            "PVE",
            draft(vec![swap("qd", "hemera")]),
            "4.10",
            1,
            || "1".into(),
        );
        assert_eq!(l.drafts["ship"].based_on.as_deref(), Some("1"));
        let b = l.save("ship", None, " pve ", draft(vec![]), "4.10", 2, || {
            "2".into()
        });
        assert_eq!(a, b);
        assert_eq!(l.saved.len(), 1);
        assert!(l.saved[0].edits.is_empty());
        // The same name on another ship is another loadout.
        assert_eq!(
            l.save("other", None, "PVE", draft(vec![]), "4.10", 3, || "3"
                .into()),
            "3"
        );
        assert_eq!(l.of("ship").len(), 1);
        // An id edits in place, renames included.
        l.save(
            "ship",
            Some("1"),
            "Bounty",
            draft(vec![]),
            "4.11",
            4,
            || "4".into(),
        );
        assert_eq!(
            (l.saved[0].name.as_str(), l.saved[0].build.as_str()),
            ("Bounty", "4.11")
        );
    }

    #[test]
    fn rename_keeps_the_parts_and_the_draft() {
        let mut l = Loadouts::default();
        l.save(
            "ship",
            None,
            "PVE",
            draft(vec![swap("qd", "hemera")]),
            "4.10",
            1,
            || "1".into(),
        );
        // An unsaved change on top of the saved loadout.
        let edited = Draft {
            based_on: Some("1".into()),
            ..draft(vec![swap("qd", "atlas")])
        };
        l.set_draft("ship", edited.clone());

        assert!(l.rename("ship", "1", "Bounty"));
        assert_eq!(l.saved[0].name, "Bounty");
        assert_eq!(l.saved[0].edits, vec![swap("qd", "hemera")]);
        assert_eq!(l.saved[0].saved_at, 1);
        assert_eq!(l.drafts["ship"], edited);

        assert!(!l.rename("other", "1", "Nope"));
        assert!(!l.rename("ship", "2", "Nope"));
        assert_eq!(l.saved[0].name, "Bounty");
    }

    #[test]
    fn open_delete_and_stale_bases() {
        let mut l = Loadouts::default();
        l.save(
            "ship",
            None,
            "PVE",
            draft(vec![swap("qd", "hemera")]),
            "4.10",
            1,
            || "1".into(),
        );
        l.set_draft("ship", draft(vec![]));
        assert!(!l.drafts.contains_key("ship"));
        assert!(l.open("ship", Some("1")));
        assert_eq!(l.drafts["ship"].edits, vec![swap("qd", "hemera")]);
        assert!(!l.open("other", Some("1")));
        assert!(l.delete("1"));
        // The draft keeps its edits, now against stock.
        assert_eq!(l.drafts["ship"].based_on, None);
        assert_eq!(l.drafts["ship"].edits.len(), 1);
        l.set_draft(
            "ship",
            Draft {
                based_on: Some("gone".into()),
                ..draft(vec![swap("qd", "xl1")])
            },
        );
        assert_eq!(l.drafts["ship"].based_on, None);
        // A loadout in the game is a base too.
        l.set_draft(
            "ship",
            Draft {
                based_on: Some("live:urn:x".into()),
                ..draft(vec![swap("qd", "xl1")])
            },
        );
        assert_eq!(l.drafts["ship"].based_on.as_deref(), Some("live:urn:x"));
    }

    #[test]
    fn old_files_load() {
        let l: Loadouts = serde_json::from_str("{}").unwrap();
        assert_eq!(l, Loadouts::default());
    }
}
