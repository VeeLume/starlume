//! Armour sets from the game data's entity names.
//!
//! Display names don't group: one set's pieces read "Tailwind Flight Helmet
//! Kruger" and "Tailwind Flight Suit Kruger", or "Sol-III Core Aviator" and
//! "Sol-III Flight Suit Aviator". The entity records agree where the names
//! don't: `vgl_flightsuit_helmet_05_01_01` and `vgl_flightsuit_05_01_01`,
//! `nvy_flightsuit_light_core_01_01_01` and `nvy_flightsuit_01_01_01`. A
//! set is the entity name without the piece's own slot word; the weight word
//! stays apart, since it tells models apart (`kap_combat_light_*` is Geist,
//! `kap_combat_heavy_*` Monolith). A piece without a weight word (a flight
//! suit's helmet) joins the one weighted set of its name, when there is
//! exactly one and it lacks that piece.
//!
//! A part from another line under the same set name (a backpack) joins it
//! when the two share no piece.
//!
//! Checked on 4.10.2 live: 2,481 named armour pieces, 941 sets, none with
//! two pieces in one slot; 32 names still stand for two sets, so the key,
//! not the name, tells sets apart. sc-holotable's `ItemCatalog` groups by display
//! name and so splits these sets, which is why this doesn't use it.

use std::collections::{BTreeMap, HashMap};

use serde::Serialize;
use svc_data::{CookedData, Guid, RecordCollection};

/// The set an armour piece belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArmourSet {
    /// Stable within a build; groups the pieces.
    pub key: String,
    /// "Sol-III Aviator": the set's main piece without its slot words.
    pub name: String,
    /// The pieces the game data has for the set, in [`SLOTS`] order —
    /// what "full" means for this set (a flight suit: helmet and suit).
    pub pieces: Vec<String>,
}

type Slot = (&'static str, &'static str, &'static [&'static str]);

/// Slot label, item type, and the entity-name words for the slot.
const SLOTS: [Slot; 6] = [
    ("Helmet", "Char_Armor_Helmet", &["helmet"]),
    ("Core", "Char_Armor_Torso", &["core"]),
    ("Arms", "Char_Armor_Arms", &["arms"]),
    ("Legs", "Char_Armor_Legs", &["legs"]),
    // `rsi_utility_heavy_suit_*` is the Zeus suit.
    ("Undersuit", "Char_Armor_Undersuit", &["undersuit", "suit"]),
    ("Backpack", "Char_Armor_Backpack", &["backpack"]),
];

const WEIGHTS: [&str; 4] = ["light", "medium", "heavy", "superheavy"];

/// Which piece names the set: the body first, the helmet last.
const NAMING: [&str; 6] = ["Core", "Legs", "Arms", "Undersuit", "Helmet", "Backpack"];

/// One armour record, as the index needs it.
pub struct Piece<'a> {
    pub guid: Guid,
    /// The entity record's name (`vgl_flightsuit_helmet_05_01_01`).
    pub entity: &'a str,
    pub item_type: &'a str,
    pub name: &'a str,
}

#[derive(Debug, Default)]
pub struct ArmourSets {
    by_guid: HashMap<Guid, usize>,
    sets: Vec<ArmourSet>,
}

impl ArmourSets {
    pub fn from_cooked(cooked: &CookedData) -> Self {
        let (Some(items), Some(paths)) = (
            cooked.holotable.items.as_ref(),
            cooked.holotable.paths.as_ref(),
        ) else {
            return Self::default();
        };
        let pieces: Vec<Piece<'_>> = items
            .iter()
            .filter_map(|(guid, item)| {
                let item_type = item.item_type.as_dcb_str();
                slot(item_type)?;
                let name = item.display_name(&cooked.locale)?;
                if name.starts_with(['<', '@']) || name.contains("PLACEHOLDER") {
                    return None;
                }
                let path = &paths.get(guid)?.path;
                let file = path.rsplit('/').next()?;
                let entity = file.strip_suffix(".xml").unwrap_or(file);
                Some(Piece {
                    guid: *guid,
                    entity,
                    item_type,
                    name,
                })
            })
            .collect();
        Self::build(&pieces)
    }

    pub fn build(pieces: &[Piece<'_>]) -> Self {
        let split: Vec<(String, Option<&str>)> = pieces.iter().map(split).collect();
        // Base → weight → the slots its weighted pieces fill.
        let mut weighted: HashMap<&str, BTreeMap<&str, Vec<&str>>> = HashMap::new();
        for (p, (base, weight)) in pieces.iter().zip(&split) {
            if let Some(w) = weight {
                weighted
                    .entry(base.as_str())
                    .or_default()
                    .entry(w)
                    .or_default()
                    .push(p.item_type);
            }
        }
        let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (i, (p, (base, weight))) in pieces.iter().zip(&split).enumerate() {
            let weight = weight.or_else(|| {
                let ws = weighted.get(base.as_str())?;
                let (w, types) = ws.iter().next().filter(|_| ws.len() == 1)?;
                (!types.contains(&p.item_type)).then_some(*w)
            });
            groups
                .entry(format!("{base}:{}", weight.unwrap_or("-")))
                .or_default()
                .push(i);
        }
        let slot_of = |i: &usize| slot(pieces[*i].item_type).map_or("", |s| s.0);
        let filled = |members: &[usize]| -> Vec<&str> { members.iter().map(slot_of).collect() };
        // A part from another line under the same name (Geist Rogue's
        // backpack is `cds_combat_*`, its armour `kap_combat_*`) joins the
        // set of that name, unless the two share a piece: CIG names some
        // distinct sets alike (TrueDef-Pro `_11` and `_15`, "Black/Silver").
        let mut named: Vec<(String, String, Vec<usize>)> = Vec::new();
        for (key, members) in groups {
            let lead = NAMING
                .iter()
                .find_map(|s| members.iter().find(|i| slot_of(i) == *s))
                .unwrap_or(&members[0]);
            let name = set_name(pieces[*lead].name);
            let have = filled(&members);
            match named
                .iter_mut()
                .find(|(_, n, m)| *n == name && !filled(m).iter().any(|s| have.contains(s)))
            {
                Some((_, _, m)) => m.extend(members),
                None => named.push((key, name, members)),
            }
        }
        let mut out = Self::default();
        for (key, name, members) in named {
            let have = filled(&members);
            let id = out.sets.len();
            out.sets.push(ArmourSet {
                key,
                name,
                pieces: SLOTS
                    .iter()
                    .filter(|s| have.contains(&s.0))
                    .map(|s| s.0.to_string())
                    .collect(),
            });
            for i in members {
                out.by_guid.insert(pieces[i].guid, id);
            }
        }
        out
    }

    pub fn of(&self, guid: &Guid) -> Option<&ArmourSet> {
        self.by_guid.get(guid).map(|&i| &self.sets[i])
    }
}

fn slot(item_type: &str) -> Option<&'static Slot> {
    SLOTS.iter().find(|s| s.1 == item_type)
}

/// The entity name without its own slot word and its weight word, and the
/// weight. Only the piece's own slot word goes: `srvl_undersuit_helmet` is
/// the helmet of the `srvl_undersuit` line.
fn split<'a>(p: &Piece<'_>) -> (String, Option<&'a str>) {
    let own: &[&str] = slot(p.item_type).map_or(&[], |s| s.2);
    let lower = p.entity.to_ascii_lowercase();
    let mut weight = None;
    let mut base = Vec::new();
    for t in lower.split('_') {
        if let Some(w) = WEIGHTS.iter().find(|w| **w == t) {
            weight.get_or_insert(*w);
        } else if !own.contains(&t) {
            base.push(t);
        }
    }
    (base.join("_"), weight)
}

/// "Geist Armor Core Golden Blossom" → "Geist Golden Blossom".
fn set_name(name: &str) -> String {
    name.split_whitespace()
        .filter(|w| {
            !matches!(
                *w,
                "Helmet" | "Core" | "Arms" | "Legs" | "Backpack" | "Undersuit" | "Suit" | "Armor"
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sets(rows: &[(&'static str, &'static str, &'static str)]) -> (ArmourSets, Vec<Guid>) {
        let guids: Vec<Guid> = (0..rows.len())
            .map(|i| format!("00000000-0000-0000-0000-{i:012}").parse().unwrap())
            .collect();
        let pieces: Vec<Piece<'_>> = rows
            .iter()
            .zip(&guids)
            .map(|((entity, item_type, name), guid)| Piece {
                guid: *guid,
                entity,
                item_type,
                name,
            })
            .collect();
        (ArmourSets::build(&pieces), guids)
    }

    #[test]
    fn a_flight_suit_and_its_helmet_are_one_set() {
        let (s, g) = sets(&[
            (
                "vgl_flightsuit_helmet_05_01_01",
                "Char_Armor_Helmet",
                "Tailwind Flight Helmet Kruger",
            ),
            (
                "vgl_flightsuit_05_01_01",
                "Char_Armor_Undersuit",
                "Tailwind Flight Suit Kruger",
            ),
            (
                "vgl_flightsuit_helmet_01_03_01",
                "Char_Armor_Helmet",
                "Tailwind Flight Helmet Big Bite",
            ),
        ]);
        let kruger = s.of(&g[0]).unwrap();
        assert_eq!(s.of(&g[1]), Some(kruger));
        assert_eq!(kruger.name, "Tailwind Flight Kruger");
        assert_eq!(kruger.pieces, ["Helmet", "Undersuit"]);
        assert_ne!(s.of(&g[2]), Some(kruger));
    }

    #[test]
    fn unweighted_pieces_join_the_one_weighted_set() {
        let (s, g) = sets(&[
            (
                "nvy_flightsuit_light_core_01_01_01",
                "Char_Armor_Torso",
                "Sol-III Core Aviator",
            ),
            (
                "nvy_flightsuit_helmet_01_01_01",
                "Char_Armor_Helmet",
                "Sol-III Flight Helmet Aviator",
            ),
            (
                "nvy_flightsuit_01_01_01",
                "Char_Armor_Undersuit",
                "Sol-III Flight Suit Aviator",
            ),
        ]);
        let set = s.of(&g[0]).unwrap();
        assert_eq!(s.of(&g[1]), Some(set));
        assert_eq!(s.of(&g[2]), Some(set));
        assert_eq!(set.name, "Sol-III Aviator");
        assert_eq!(set.pieces, ["Helmet", "Core", "Undersuit"]);
    }

    #[test]
    fn weights_tell_models_apart() {
        let (s, g) = sets(&[
            (
                "kap_combat_light_core_01_01_01",
                "Char_Armor_Torso",
                "Geist Armor Core",
            ),
            (
                "kap_combat_heavy_core_01_01_01",
                "Char_Armor_Torso",
                "Monolith Armor Core",
            ),
            (
                "kap_combat_light_helmet_01_01_01",
                "Char_Armor_Helmet",
                "Geist Armor Helmet",
            ),
        ]);
        assert_ne!(s.of(&g[0]), s.of(&g[1]));
        assert_eq!(s.of(&g[0]), s.of(&g[2]));
        assert_eq!(s.of(&g[0]).unwrap().name, "Geist");
    }

    #[test]
    fn a_backpack_from_another_line_joins_the_set_of_its_name() {
        let (s, g) = sets(&[
            (
                "kap_combat_light_core_01_01_02",
                "Char_Armor_Torso",
                "Geist Armor Core Rogue",
            ),
            (
                "cds_combat_light_backpack_01_02_02",
                "Char_Armor_Backpack",
                "Geist Backpack Rogue",
            ),
            (
                "vgl_armor_light_core_01_01_11",
                "Char_Armor_Torso",
                "TrueDef-Pro Core Black/Silver",
            ),
            (
                "vgl_armor_light_core_01_01_15",
                "Char_Armor_Torso",
                "TrueDef-Pro Core Black/Silver",
            ),
        ]);
        assert_eq!(s.of(&g[0]), s.of(&g[1]));
        assert_eq!(s.of(&g[0]).unwrap().pieces, ["Core", "Backpack"]);
        // Alike names, both a core: two sets, told apart by key.
        assert_ne!(s.of(&g[2]).unwrap().key, s.of(&g[3]).unwrap().key);
    }

    #[test]
    fn a_suit_word_marks_the_undersuit() {
        let (s, g) = sets(&[
            (
                "rsi_utility_heavy_helmet_01_01_01",
                "Char_Armor_Helmet",
                "Zeus Exploration Helmet",
            ),
            (
                "rsi_utility_heavy_suit_01_01_01",
                "Char_Armor_Undersuit",
                "Zeus Exploration Suit",
            ),
        ]);
        assert_eq!(s.of(&g[0]), s.of(&g[1]));
    }

    #[test]
    fn an_unweighted_piece_never_doubles_a_slot() {
        // Ixonia's helmet shares Aztalan's numbers but not its weight.
        let (s, g) = sets(&[
            (
                "thp_light_helmet_01_01_01",
                "Char_Armor_Helmet",
                "Aztalan Helmet",
            ),
            (
                "thp_light_core_01_01_01",
                "Char_Armor_Torso",
                "Aztalan Core",
            ),
            (
                "thp_helmet_01_01_01",
                "Char_Armor_Helmet",
                "Ixonia Helmet Desert Camo",
            ),
        ]);
        assert_eq!(s.of(&g[0]), s.of(&g[1]));
        assert_ne!(s.of(&g[0]), s.of(&g[2]));
    }
}
