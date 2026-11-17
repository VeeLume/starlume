//! Display names for the internal names the game writes into its log and
//! its telemetry stream: a place (`Stanton4_NewBabbage`), a vehicle class
//! (`RSI_Constellation_Mk5_Centaurus`), an item or resource GUID.
//!
//! Reference-catalog framework (CONTRIBUTING module rule 4): every lookup
//! here reads the cooked indices and the bundled locale, nothing else. A
//! name the cook does not know is `None`; the caller decides what to show
//! instead.
//!
//! **Places** are `StarMapObject` records, and the log names a place by its
//! record name. The cook's record paths give each location's record name,
//! so the join is exact (no guessing from the text).
//!
//! **Not here: shops.** The log names a shop by its entity in the level
//! (`SCShop_CousinCrows_Orison`); no cooked index maps that to the name on
//! the shop's sign. (The `Shop_*` manufacturer records look like the join
//! and are not: most carry a placeholder name.)

use std::collections::HashMap;
use std::str::FromStr;

use sc_holotable::asset::{Guid, RecordCollection};
use sc_holotable::locations::Location;

use crate::cooked::CookedData;

/// A place with the places it lies in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceName {
    pub name: String,
    /// What it lies in, nearest first (`microTech`, `Stanton`).
    pub parents: Vec<String>,
}

/// A name the locale really has: not empty, not one of the game's
/// placeholder texts.
fn real(name: &str) -> Option<&str> {
    let name = name.trim();
    (!name.is_empty() && !name.contains("PLACEHOLDER") && !name.contains("UNINITIALIZED"))
        .then_some(name)
}

/// A record name without its type prefix, lower-cased
/// (`StarMapObject.Stanton4_NewBabbage` → `stanton4_newbabbage`).
fn record_key(name: &str) -> String {
    name.rsplit('.').next().unwrap_or(name).to_ascii_lowercase()
}

/// The places of one cook by record name. Built per batch of lookups (one
/// walk over the locations), not kept: a session names a handful of places
/// a few times an hour.
pub struct PlaceNames<'a> {
    cooked: &'a CookedData,
    by_record: HashMap<String, &'a Location>,
}

impl<'a> PlaceNames<'a> {
    /// The place the game calls `internal`, if the cook has it and the
    /// locale names it.
    pub fn get(&self, internal: &str) -> Option<PlaceName> {
        let location = self.by_record.get(&internal.to_ascii_lowercase())?;
        let locale = &self.cooked.locale;
        let name = real(location.display_name(locale)?)?;
        let parents = self
            .cooked
            .holotable
            .locations
            .as_ref()?
            .ancestors(&location.guid)
            .filter_map(|a| a.display_name(locale).and_then(real))
            .map(str::to_owned)
            .collect();
        Some(PlaceName {
            name: name.to_owned(),
            parents,
        })
    }
}

impl CookedData {
    /// The lookup for places by the game's internal name.
    pub fn place_names(&self) -> PlaceNames<'_> {
        let by_record = match (&self.holotable.locations, &self.holotable.paths) {
            (Some(locations), Some(paths)) => locations
                .iter()
                .filter_map(|(guid, location)| Some((record_key(&paths.get(guid)?.name), location)))
                .collect(),
            _ => HashMap::new(),
        };
        PlaceNames {
            cooked: self,
            by_record,
        }
    }

    /// A vehicle's name by its class (its record name), as the stream
    /// reports it. Every vehicle the cook has, not only the ones a player
    /// can own: a session also sits in mission ships.
    pub fn vehicle_name(&self, class_name: &str) -> Option<String> {
        let ships = &self.ships;
        let vehicle = match ships.by_record_name(class_name) {
            Some((_, vehicle)) => vehicle,
            None => {
                let wanted = class_name.to_ascii_lowercase();
                let paths = self.holotable.paths.as_ref()?;
                ships
                    .vehicles
                    .iter()
                    .find(|(guid, _)| {
                        paths
                            .get(guid)
                            .is_some_and(|p| record_key(&p.name) == wanted)
                    })?
                    .1
            }
        };
        let name = self.locale.resolve(vehicle.name_key.as_ref()?)?;
        real(name).map(str::to_owned)
    }

    /// An item's name by its record GUID.
    pub fn item_name(&self, guid: &str) -> Option<String> {
        let guid = Guid::from_str(guid).ok()?;
        let item = self.holotable.items.as_ref()?.get(&guid)?;
        item.display_name(&self.locale)
            .and_then(real)
            .map(str::to_owned)
    }

    /// Items' names by class (record name), for the lines that name an item
    /// that way and carry no GUID (`arma_barrel_supp_s2`). One walk over
    /// the record names for the whole batch; a class the cook does not
    /// have, or the locale does not name, is left out.
    pub fn item_names_by_class(&self, classes: &[String]) -> HashMap<String, String> {
        let (Some(items), Some(paths)) = (&self.holotable.items, &self.holotable.paths) else {
            return HashMap::new();
        };
        if classes.is_empty() {
            return HashMap::new();
        }
        let mut names = HashMap::new();
        for record in paths.iter() {
            let key = record.name.rsplit('.').next().unwrap_or(&record.name);
            let Some(class) = classes.iter().find(|c| c.eq_ignore_ascii_case(key)) else {
                continue;
            };
            // Other record types can share the name: only an item counts.
            if let Some(name) = items
                .get(&record.guid)
                .and_then(|item| item.display_name(&self.locale))
                .and_then(real)
            {
                names.insert(class.clone(), name.to_owned());
            }
        }
        names
    }

    /// A resource's (commodity's) name by its record GUID.
    pub fn resource_name(&self, guid: &str) -> Option<String> {
        let guid = Guid::from_str(guid).ok()?;
        let resource = self.holotable.resources.as_ref()?.get(&guid)?;
        self.locale
            .resolve(&resource.name_key)
            .and_then(real)
            .map(str::to_owned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_holotable::HolotableSnapshot;
    use sc_holotable::asset::generated::{ERespawnLocationType, NavPointIconEnum};
    use sc_holotable::asset::{LocaleKey, LocaleMap, RecordPath, RecordPaths};
    use sc_holotable::locations::{LocationKind, Locations};

    fn guid(n: u8) -> Guid {
        Guid::from_bytes([n; 16])
    }

    fn location(n: u8, name_key: &str, parent: Option<u8>, kind: LocationKind) -> Location {
        Location {
            guid: guid(n),
            name_key: Some(LocaleKey::new(name_key)),
            desc_key: None,
            callouts: Vec::new(),
            kind,
            nav_icon: NavPointIconEnum::Default,
            respawn: ERespawnLocationType::None,
            parent: parent.map(guid),
            jurisdiction: None,
            quantum: None,
        }
    }

    fn path(n: u8, name: &str) -> RecordPath {
        RecordPath {
            guid: guid(n),
            name: name.to_owned(),
            struct_index: 0,
            is_main: true,
            path: format!("libs/foundry/records/starmap/{n}.xml"),
        }
    }

    /// Three made-up places: a landing zone on a planet in a system, and one
    /// the locale has only a placeholder for.
    fn cooked() -> CookedData {
        let locations = Locations::from_locations([
            location(1, "@sys_Vega", None, LocationKind::Star),
            location(2, "@planet_Aremis", Some(1), LocationKind::Planet),
            location(3, "@lz_NewCorvo", Some(2), LocationKind::LandingZone),
            location(4, "@lz_Unnamed", Some(2), LocationKind::LandingZone),
        ]);
        let mut paths = RecordPaths::new();
        paths.insert(path(1, "StarMapObject.Vega"));
        paths.insert(path(2, "StarMapObject.Vega2"));
        paths.insert(path(3, "StarMapObject.Vega2_NewCorvo"));
        paths.insert(path(4, "StarMapObject.Vega2_Unnamed"));
        let mut locale = LocaleMap::new();
        locale.set("sys_Vega", "Vega");
        locale.set("planet_Aremis", "Aremis");
        locale.set("lz_NewCorvo", "New Corvo");
        locale.set("lz_Unnamed", "<= PLACEHOLDER =>");
        CookedData {
            holotable: HolotableSnapshot {
                locations: Some(locations),
                paths: Some(paths),
                ..Default::default()
            },
            locale,
            ..Default::default()
        }
    }

    #[test]
    fn a_place_is_found_by_its_record_name_with_what_it_lies_in() {
        let cooked = cooked();
        let places = cooked.place_names();
        assert_eq!(
            places.get("Vega2_NewCorvo"),
            Some(PlaceName {
                name: "New Corvo".into(),
                parents: vec!["Aremis".into(), "Vega".into()],
            })
        );
        // The log's casing is not the record's.
        assert!(places.get("vega2_newcorvo").is_some());
    }

    #[test]
    fn an_unknown_or_unnamed_place_is_none() {
        let cooked = cooked();
        let places = cooked.place_names();
        assert_eq!(places.get("Vega3_Nowhere"), None);
        assert_eq!(
            places.get("Vega2_Unnamed"),
            None,
            "a placeholder is no name"
        );
        assert_eq!(
            CookedData::default().place_names().get("Vega2_NewCorvo"),
            None
        );
    }

    #[test]
    fn items_are_found_by_class_when_a_line_has_no_guid() {
        use sc_holotable::asset::generated::{EItemSubType, EItemType};
        use sc_holotable::items::{Item, Items};

        let item = |name_key: &str| Item {
            name_key: Some(LocaleKey::new(name_key)),
            short_name_key: None,
            desc_key: None,
            item_type: EItemType::from_dcb_str("WeaponAttachment"),
            item_sub_type: EItemSubType::from_dcb_str("UNDEFINED"),
            size: 2,
            grade: 1,
            tags: vec![],
            required_tags: vec![],
            mass: None,
            inventory: None,
            occupancy: Default::default(),
            resource_container: None,
        };
        let mut items = Items::new();
        items.insert(guid(1), item("@item_NameScope"));
        items.insert(guid(2), item("@item_NameUnnamed"));
        let mut paths = RecordPaths::new();
        paths.insert(path(1, "EntityClassDefinition.tnkr_optics_x4_s2"));
        paths.insert(path(2, "EntityClassDefinition.tnkr_barrel_supp_s2"));
        // A record of another type with an asked-for name is no item.
        paths.insert(path(3, "SomethingElse.tnkr_ubarrel_lasr_s1"));
        let mut locale = LocaleMap::new();
        locale.set("item_NameScope", "Tinker 4x Scope");
        locale.set("item_NameUnnamed", "<= PLACEHOLDER =>");
        let cooked = CookedData {
            holotable: HolotableSnapshot {
                items: Some(items),
                paths: Some(paths),
                ..Default::default()
            },
            locale,
            ..Default::default()
        };

        let asked = [
            // The log's casing is not the record's.
            "TNKR_optics_x4_s2".to_owned(),
            "tnkr_barrel_supp_s2".to_owned(),
            "tnkr_ubarrel_lasr_s1".to_owned(),
            "tnkr_never_cooked".to_owned(),
        ];
        let names = cooked.item_names_by_class(&asked);
        assert_eq!(names.len(), 1, "{names:?}");
        assert_eq!(names["TNKR_optics_x4_s2"], "Tinker 4x Scope");
        assert!(CookedData::default().item_names_by_class(&asked).is_empty());
    }

    #[test]
    fn nothing_cooked_names_nothing() {
        let empty = CookedData::default();
        assert_eq!(empty.vehicle_name("VEGA_Courier"), None);
        assert_eq!(
            empty.item_name("5e6f7a8b-aaaa-4bbb-8ccc-ddddeeeeffff"),
            None
        );
        assert_eq!(empty.resource_name("not a guid"), None);
    }
}
