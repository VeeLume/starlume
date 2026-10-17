//! The rock generator: rocks that really spawn, from the game's own spawn
//! rules — where a rock type spawns (the providers and, once known, their
//! places), how often, and real rolls of it (scale, mesh, composition,
//! quality) through sc-mining's `Spawner`.
//!
//! An area is one harvestable provider: a planet or moon, a Lagrange field,
//! a belt. It is named after its places (cooked with the game data), else
//! after its record name. Rolls are a deterministic
//! sample ([`SAMPLE`] draws per rock type and area), so the same pick gives
//! the same rock; quality rolls at the area's place (its first, unless one
//! is asked for), falling back to the resource's default distribution
//! without one.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use svc_data::mining::{
    Composition, Elements, Entry, ModifiedRock, Player, QualityRoll, Rock, RockType, Ruleset,
    Setup, Shape, SimConfig, Spawn, Spawner, difficulty,
};
use svc_data::{
    GatheringMode, Guid, LocaleMap, LocationKind, Locations, MiningIndex, ProviderLocations,
    Providers, RecordCollection, Resources, Tags,
};

use crate::catalog::Catalog;
use crate::solve::{EntryInput, LoadoutInput, RockInput, SolveError, setup_of};

/// Rolls per rock type and area: shares to about a percent, and the median,
/// smallest and largest of a real spread.
pub const SAMPLE: u32 = 2000;
/// Composition rolls per rock type for its ore odds.
const ODDS_ROLLS: u64 = 2000;
/// Rolls per rock type and area for the class table (the finder's lists):
/// shares to a few percent, over every area at once.
const TABLE_SAMPLE: u64 = 400;
/// Player-model tries per graded mass in a band or a list: enough to tell
/// easy, manageable and hard apart, cheap enough to run many times.
pub const GRADE_RUNS: u32 = 24;
/// Masses graded per rock type (quantiles of its rolls): the finder's lists
/// and a pick's histogram classify every roll by the nearest one.
const SPREAD_GRADES: usize = 10;
/// Bars in a rock type's mass distribution.
const MASS_BUCKETS: usize = 36;
/// The seed the spawner derives a composition roll from (sc-mining's
/// `Spawner::spawn`); rolling it again with quality gives the same entries.
fn composition_seed(seed: u64) -> u64 {
    seed.wrapping_mul(0x9E37_79B9).wrapping_add(1)
}

/// Fields with no named place in the data but a name in the locale (keys
/// nothing references): the Nyx belts, whose segments hang off the star.
/// Valerie, 2026-10-16: show them by these names.
const NAMED_FIELDS: [(&str, &str); 2] = [
    ("e9aa8f98-4c87-468f-ae03-10a96d9497e5", "@Nyx_AsteroidBelt1"), // HPP_Nyx_GlaciemRing
    ("230aecd3-2e5b-4d58-a884-bbba59a40cf7", "@Nyx_AsteroidBelt2"), // HPP_Nyx_KeegerBelt
];

/// Providers left over from development, placed nowhere in today's game
/// (Valerie, 2026-10-16: hide them). Akiro Cluster still has a named star
/// map object and a line in the mining compendium.
const LEFTOVERS: [&str; 6] = [
    "f64ae7c9-932c-46c7-9b72-de48139030aa", // HPP_Lagrange_G
    "48ef0bf3-9d42-48ba-8798-198852090ef2", // HPP_Lagrange_Occupied
    "a90785e4-5d6c-46f1-9d52-b92d6b65988a", // HPP_Pyro_AkiroCluster
    "e9d9e8f5-12ef-4749-b365-f6d9bf8c3101", // HPP_ResourceRush_Gold
    "da2c9d82-3f29-4779-a4b2-68ffd1c80d16", // HPP_ResourceRush_Gold_HighDensity
    "088cd938-b161-4dd3-a5be-616d7b16b28e", // HPP_SpaceDerelict_General
];

/// Everything the generator reads, all from the cooked data.
pub struct Sources<'a> {
    pub index: &'a MiningIndex,
    pub providers: &'a Providers,
    pub tags: &'a Tags,
    pub resources: &'a Resources,
    pub locations: &'a Locations,
    pub locale: &'a LocaleMap,
    /// `None` when the cook could not read the places.
    pub places: Option<&'a ProviderLocations>,
}

/// One mining area (a harvestable provider).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Area {
    /// Provider GUID.
    pub id: String,
    pub name: String,
    /// The name is the provider's record name, not a place name.
    pub from_record: bool,
    pub places: Vec<Place>,
    /// Ship-mineable rock types, most common first.
    pub rocks: Vec<AreaRock>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Place {
    /// `StarMapObject` GUID.
    pub id: String,
    pub name: String,
}

/// A rock type as it spawns in an area.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AreaRock {
    /// `HarvestablePreset` GUID.
    pub rock_type: String,
    pub name: String,
    /// Share of the area's ship-mineable rocks.
    pub share: f32,
    /// `false` when no mesh is known for it (the cook had no p4k).
    pub rollable: bool,
    /// The ores its rocks can hold, most likely first.
    pub ores: Vec<OreOdds>,
}

/// One ore in a rock type: how often a rock holds it and how much.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OreOdds {
    /// Element GUID.
    pub element: String,
    /// Share of rocks holding it.
    pub chance: f32,
    /// Its share of the rock when present (parts of one element summed).
    pub mean_share: f32,
    pub max_share: f32,
}

/// Which rock of the sample to hand out.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Pick {
    /// The median rock by mass.
    Average,
    /// The most frequent ore mix (which ores a rock holds), and within it
    /// the roll closest to the mix's median mass and median ore shares.
    MostCommon,
    Smallest,
    Largest,
    /// The largest share of this element (its parts summed).
    MostOre {
        element: String,
    },
    /// The best quality of this element.
    BestQuality {
        element: String,
    },
    /// A typical rock (the median by mass) of this class for the loadout.
    Class {
        class: RockClass,
    },
    /// The roll closest to this mass (tonnes).
    Mass {
        mass: f32,
    },
}

/// How a rock goes for a loadout, in the calculator's words: can't break,
/// too much power (the charge climbs even at the lasers' minimum), else our
/// player model's clean cracks — easy (80 % or more), manageable (50 % or
/// more), hard. The same thresholds as the calculator's difficulty word, so
/// a band, a list and the outcome tell one story.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RockClass {
    Easy,
    Manageable,
    Hard,
    TooMuchPower,
    Impossible,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerateRequest {
    /// Area (provider) GUID.
    pub area: String,
    /// Rock type GUID.
    pub rock_type: String,
    pub pick: Pick,
    /// The place quality rolls at; the area's first place when `None`.
    #[serde(default)]
    pub place: Option<String>,
}

/// A generated rock, ready for the calculator.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Generated {
    pub rock: RockInput,
    pub scu: f32,
    /// The draw it came from (the same request gives it again).
    pub seed: u64,
    /// The place its quality rolled at, when one is known.
    pub place: Option<Place>,
    /// Rocks in the sample.
    pub rolled: u32,
    /// With a loadout: the sample's share of each class.
    pub classes: Option<ClassShares>,
    /// For a most-common pick: the share of rolls holding its ore mix.
    pub pick_share: Option<f32>,
    /// The sample's masses as bars, lightest first, up to the 98th
    /// percentile (the rest fall in the last bar).
    pub masses: Vec<MassBucket>,
}

/// One bar of a rock type's mass distribution.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct MassBucket {
    pub from: f32,
    pub to: f32,
    pub count: u32,
    /// With a loadout: the class most of its rolls have.
    pub class: Option<RockClass>,
}

/// A rock type at an area, with its class shares for the loadout.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RockClasses {
    pub area: String,
    pub rock_type: String,
    pub classes: ClassShares,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct ClassShares {
    pub easy: f32,
    pub manageable: f32,
    pub hard: f32,
    pub too_much_power: f32,
    pub impossible: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GenerateError {
    UnknownArea(String),
    UnknownRockType(String),
    /// No mesh or composition: nothing to roll (the cook had no p4k).
    NotRollable,
    /// No rolled rock holds this element.
    NoRockWith(String),
    /// No rolled rock is of this class for the loadout.
    NoRockOfClass(RockClass),
    /// A class pick without a laser.
    NeedsLoadout,
    Loadout(SolveError),
}

impl std::fmt::Display for GenerateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownArea(id) => write!(f, "unknown mining area {id}"),
            Self::UnknownRockType(id) => write!(f, "that rock type does not spawn there ({id})"),
            Self::NotRollable => write!(
                f,
                "no rock meshes for it — reload the game data with the game files present"
            ),
            Self::NoRockWith(_) => write!(f, "none of its rocks holds that ore"),
            Self::NoRockOfClass(_) => {
                write!(f, "none of its rocks is of that kind for your loadout")
            }
            Self::NeedsLoadout => write!(f, "add a laser first"),
            Self::Loadout(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for GenerateError {}

/// `HPP_Nyx_KeegerBelt` → `Nyx Keeger Belt`: a readable label from a record
/// name, for providers no place is known for.
fn record_label(record: &str) -> String {
    let base = record.strip_prefix("HPP_").unwrap_or(record);
    let mut out = String::new();
    let mut prev: Option<char> = None;
    for c in base.chars() {
        if c == '_' {
            out.push(' ');
        } else {
            if c.is_ascii_uppercase() && prev.is_some_and(|p| p.is_ascii_lowercase()) {
                out.push(' ');
            }
            out.push(c);
        }
        prev = Some(c);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

impl Sources<'_> {
    fn place_name(&self, guid: &Guid) -> Option<String> {
        self.locations
            .get(guid)
            .and_then(|l| l.display_name(self.locale))
            .map(str::to_string)
    }

    fn rock_name(&self, rock: &RockType) -> String {
        rock.composition
            .and_then(|c| self.index.compositions.get(&c))
            .and_then(|c| self.locale.resolve_name(c.deposit_name.as_str()))
            .map(str::to_string)
            .unwrap_or_else(|| {
                self.index
                    .rock_type_names
                    .iter()
                    .find(|r| r.guid == rock.guid)
                    .map_or_else(|| rock.guid.to_string(), |r| r.record_name.clone())
            })
    }

    /// The area's name and places.
    fn area_places(&self, provider: &Guid) -> (String, bool, Vec<Place>) {
        let record = self
            .index
            .provider_names
            .iter()
            .find(|p| p.guid == *provider)
            .map_or_else(|| provider.to_string(), |p| record_label(&p.record_name));
        let mut places: Vec<Place> = self
            .places
            .map(|p| p.locations_of(provider))
            .unwrap_or_default()
            .iter()
            .filter_map(|g| {
                Some(Place {
                    id: g.to_string(),
                    name: self.place_name(g)?,
                })
            })
            .collect();
        places.sort_by(|a, b| a.name.cmp(&b.name));
        // A named field: its locale name, with the places it hangs off.
        let id = provider.to_string();
        if let Some(name) = NAMED_FIELDS
            .iter()
            .find(|(guid, _)| *guid == id)
            .and_then(|(_, key)| self.locale.resolve_name(key))
        {
            // The bodies it hangs off (Nyx), not every station whose
            // segments include it (the QV Breaker Stations).
            let mut bodies: Vec<&str> = places
                .iter()
                .filter(|p| {
                    p.id.parse::<Guid>()
                        .ok()
                        .and_then(|g| self.locations.get(&g))
                        .is_some_and(|l| {
                            matches!(
                                l.kind,
                                LocationKind::Star | LocationKind::Planet | LocationKind::Moon
                            )
                        })
                })
                .map(|p| p.name.as_str())
                .collect();
            bodies.dedup();
            let name = match (bodies.as_slice(), places.as_slice()) {
                (_, []) => name.to_string(),
                ([], [one]) => format!("{name} ({})", one.name),
                ([], [first, rest @ ..]) => format!("{name} ({} +{})", first.name, rest.len()),
                (some, _) => format!("{name} ({})", some.join(", ")),
            };
            return (name, false, places);
        }
        let name = match places.as_slice() {
            [] => return (record, true, places),
            [one] => one.name.clone(),
            _ => {
                // Many places: the planets and moons among them, else the
                // record label (mining bases, claims, Lagrange fields).
                let mut bodies: Vec<String> = self
                    .places
                    .map(|p| p.locations_of(provider))
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|g| self.locations.get(g))
                    .filter(|l| matches!(l.kind, LocationKind::Planet | LocationKind::Moon))
                    .filter_map(|l| l.display_name(self.locale).map(str::to_string))
                    .collect();
                bodies.sort();
                // Several bodies (microTech's provider also covers "Green"):
                // all of them, nothing typed tells the main one apart.
                match bodies.len() {
                    1..=3 => bodies.join(" / "),
                    _ => record,
                }
            }
        };
        (name, false, places)
    }

    /// The meshes a provider entry spawns this rock type with.
    fn shapes<'s>(&'s self, rock: &RockType, geometry_tags: &[Guid]) -> Vec<&'s Shape> {
        rock.geometries_for(self.tags, geometry_tags)
            .iter()
            .filter_map(|g| self.index.shapes.get(&g.path))
            .collect()
    }
}

/// Ore odds of a composition over [`ODDS_ROLLS`] rolls, parts of one
/// element summed per rock.
fn ore_odds(composition: &Composition, catalog: &Catalog) -> Vec<OreOdds> {
    let mut seen: HashMap<Guid, Vec<f32>> = HashMap::new();
    for seed in 0..ODDS_ROLLS {
        let mut rock: HashMap<Guid, f32> = HashMap::new();
        for e in composition.roll(seed) {
            *rock.entry(e.element).or_default() += e.fraction;
        }
        for (element, share) in rock {
            seen.entry(element).or_default().push(share);
        }
    }
    let mut out: Vec<OreOdds> = seen
        .into_iter()
        .filter(|(g, _)| catalog.element_names.iter().any(|e| e.value == *g))
        .map(|(g, shares)| OreOdds {
            element: g.to_string(),
            chance: shares.len() as f32 / ODDS_ROLLS as f32,
            mean_share: shares.iter().sum::<f32>() / shares.len() as f32,
            max_share: shares.iter().copied().fold(0.0, f32::max),
        })
        .collect();
    out.sort_by(|a, b| {
        b.chance
            .total_cmp(&a.chance)
            .then(b.mean_share.total_cmp(&a.mean_share))
    });
    out
}

/// One ship-mineable entry of a provider: its rock type, share and meshes.
struct Entry_<'a> {
    rock: &'a RockType,
    share: f32,
    geometry_tags: &'a [Guid],
}

/// The provider's ship-mineable entries on the calculator's ruleset, with
/// their share of the area's ship rocks.
fn ship_entries<'a>(src: &'a Sources<'_>, catalog: &Catalog, provider: &Guid) -> Vec<Entry_<'a>> {
    let Some(p) = src.providers.get(provider) else {
        return vec![];
    };
    // The rock entries of each ship group. The weight counts only groups
    // that hold ship rocks: sc-gathering also files the salvage
    // "BrokenShips" groups under ship mode, and their weight would swamp the
    // mineables.
    let rocks_of = |g: &'a svc_data::gathering::ProviderGroup| {
        g.elements
            .iter()
            .filter_map(|e| {
                let rock = e.harvestable.and_then(|h| src.index.rock_types.get(&h))?;
                (rock.ruleset == Some(catalog.ruleset.guid)).then_some((rock, e))
            })
            .collect::<Vec<_>>()
    };
    let groups: Vec<_> = p
        .groups
        .iter()
        .filter(|g| g.mode == GatheringMode::Ship)
        .map(|g| (g, rocks_of(g)))
        .filter(|(_, rocks)| !rocks.is_empty())
        .collect();
    let weight: f32 = groups.iter().map(|(g, _)| g.group_probability).sum();
    let mut out: Vec<Entry_<'a>> = Vec::new();
    for (g, rocks) in groups {
        for (rock, e) in rocks {
            let share = if weight > 0.0 {
                g.group_probability / weight * e.share
            } else {
                0.0
            };
            // A rock type listed twice (two groups): one row, shares summed.
            match out.iter_mut().find(|x| std::ptr::eq(x.rock, rock)) {
                Some(x) => x.share += share,
                None => out.push(Entry_ {
                    rock,
                    share,
                    geometry_tags: &e.geometry_tags,
                }),
            }
        }
    }
    out
}

/// A provider with more places than this is one counted node in the place
/// tree (mining bases, claims, deep-space clusters), not one node a place.
const FIELD_SPLIT: usize = 12;

/// One system in the place tree.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlaceSystem {
    pub name: String,
    /// Planets (with their moons) first, then the other places and fields,
    /// each by name.
    pub nodes: Vec<PlaceNode>,
}

/// A place in the tree, or a field standing for many places.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlaceNode {
    /// `StarMapObject` GUID; the provider's GUID for a field node.
    pub id: String,
    pub name: String,
    /// `"body"` (planet or moon), `"place"` (any other place), `"field"`
    /// (a provider's many places, counted).
    pub kind: String,
    /// The areas (provider GUIDs) with ship rocks here; empty for a planet
    /// that only parents its moons.
    pub areas: Vec<String>,
    /// For a field node, how many places it stands for.
    pub count: u32,
    pub children: Vec<PlaceNode>,
}

struct Building {
    name: String,
    kind: &'static str,
    areas: Vec<String>,
    count: u32,
    parent: Option<Guid>,
    system: String,
}

/// The place tree for the generator: systems, their planets with moons
/// nested by the game's parent links, the other places and fields beside
/// them. Lagrange points hang off the star in the data (their planet is
/// only in the name, `HUR L1`), so they stay at the system's level, named
/// as in game.
pub fn place_tree(src: &Sources<'_>, catalog: &Catalog) -> Vec<PlaceSystem> {
    let body = |g: &Guid| {
        src.locations
            .get(g)
            .is_some_and(|l| matches!(l.kind, LocationKind::Planet | LocationKind::Moon))
    };
    let star_of = |g: &Guid| -> Option<String> {
        std::iter::once(src.locations.get(g)?)
            .chain(src.locations.ancestors(g))
            .find(|l| matches!(l.kind, LocationKind::Star | LocationKind::SolarSystem))
            .and_then(|l| l.display_name(src.locale))
            .map(str::to_string)
    };
    let mut nodes: HashMap<String, Building> = HashMap::new();
    let add_place = |nodes: &mut HashMap<String, Building>, g: &Guid, area: Option<&str>| {
        let key = g.to_string();
        if let Some(n) = nodes.get_mut(&key) {
            if let Some(a) = area
                && !n.areas.iter().any(|x| x == a)
            {
                n.areas.push(a.to_string());
            }
            return;
        }
        let Some(l) = src.locations.get(g) else {
            return;
        };
        let parent = l.parent.filter(|p| body(p));
        nodes.insert(
            key,
            Building {
                name: l.display_name(src.locale).unwrap_or("?").to_string(),
                kind: if body(g) { "body" } else { "place" },
                areas: area.map(|a| vec![a.to_string()]).unwrap_or_default(),
                count: 0,
                parent,
                system: star_of(g).unwrap_or_else(|| "Other".into()),
            },
        );
    };

    for (guid, _) in src.providers.iter() {
        if LEFTOVERS.contains(&guid.to_string().as_str())
            || ship_entries(src, catalog, guid).is_empty()
        {
            continue;
        }
        let area = guid.to_string();
        let places: Vec<Guid> = src
            .places
            .map(|p| p.locations_of(guid).to_vec())
            .unwrap_or_default();
        let on_star = places.iter().any(|g| {
            src.locations
                .get(g)
                .is_some_and(|l| matches!(l.kind, LocationKind::Star | LocationKind::SolarSystem))
        });
        if places.is_empty() || places.len() > FIELD_SPLIT || on_star {
            let system = places
                .iter()
                .find_map(star_of)
                .unwrap_or_else(|| "Other".into());
            // Under its system the field needs no "(Nyx)".
            let (name, _, _) = src.area_places(guid);
            let name = name
                .strip_suffix(&format!(" ({system})"))
                .map(str::to_string)
                .unwrap_or(name);
            nodes.insert(
                area.clone(),
                Building {
                    name,
                    kind: "field",
                    areas: vec![area],
                    count: places.len() as u32,
                    parent: None,
                    system,
                },
            );
            continue;
        }
        for g in &places {
            add_place(&mut nodes, g, Some(&area));
            // The planet a moon orbits shows even without rocks of its own.
            let mut up = src.locations.get(g).and_then(|l| l.parent);
            while let Some(pg) = up.filter(|pg| body(pg)) {
                add_place(&mut nodes, &pg, None);
                up = src.locations.get(&pg).and_then(|l| l.parent);
            }
        }
    }

    // Children under their planet, the rest at their system's level.
    fn build(
        key: &str,
        nodes: &HashMap<String, Building>,
        kids: &HashMap<String, Vec<String>>,
    ) -> PlaceNode {
        let b = &nodes[key];
        let mut children: Vec<PlaceNode> = kids
            .get(key)
            .map(|k| k.iter().map(|c| build(c, nodes, kids)).collect())
            .unwrap_or_default();
        children.sort_by(|a, b| a.name.cmp(&b.name));
        PlaceNode {
            id: key.to_string(),
            name: b.name.clone(),
            kind: b.kind.to_string(),
            areas: b.areas.clone(),
            count: b.count,
            children,
        }
    }
    let mut kids: HashMap<String, Vec<String>> = HashMap::new();
    let mut roots: Vec<String> = Vec::new();
    for (key, b) in &nodes {
        match b
            .parent
            .map(|p| p.to_string())
            .filter(|p| nodes.contains_key(p))
        {
            Some(p) => kids.entry(p).or_default().push(key.clone()),
            None => roots.push(key.clone()),
        }
    }
    let mut systems: Vec<PlaceSystem> = Vec::new();
    for key in roots {
        let node = build(&key, &nodes, &kids);
        let system = nodes[&key].system.clone();
        match systems.iter_mut().find(|s| s.name == system) {
            Some(s) => s.nodes.push(node),
            None => systems.push(PlaceSystem {
                name: system,
                nodes: vec![node],
            }),
        }
    }
    for s in &mut systems {
        s.nodes.sort_by(|a, b| {
            (a.kind != "body")
                .cmp(&(b.kind != "body"))
                .then(a.name.cmp(&b.name))
        });
    }
    // Stanton, Pyro, Nyx as the game lists them; unknown ones after.
    let rank = |n: &str| {
        ["Stanton", "Pyro", "Nyx"]
            .iter()
            .position(|x| *x == n)
            .unwrap_or(9)
    };
    systems.sort_by(|a, b| rank(&a.name).cmp(&rank(&b.name)).then(a.name.cmp(&b.name)));
    systems
}

/// Every area with ship-mineable rocks, by name.
pub fn areas(src: &Sources<'_>, catalog: &Catalog) -> Vec<Area> {
    let mut odds_cache: HashMap<Guid, Vec<OreOdds>> = HashMap::new();
    let mut out: Vec<Area> = src
        .providers
        .iter()
        .filter(|(guid, _)| !LEFTOVERS.contains(&guid.to_string().as_str()))
        .filter_map(|(guid, _)| {
            let entries = ship_entries(src, catalog, guid);
            if entries.is_empty() {
                return None;
            }
            let mut rocks: Vec<AreaRock> = entries
                .iter()
                .map(|e| {
                    let composition = e
                        .rock
                        .composition
                        .and_then(|c| src.index.compositions.get(&c));
                    let ores = match composition {
                        Some(c) => odds_cache
                            .entry(c.guid)
                            .or_insert_with(|| ore_odds(c, catalog))
                            .clone(),
                        None => vec![],
                    };
                    AreaRock {
                        rock_type: e.rock.guid.to_string(),
                        name: src.rock_name(e.rock),
                        share: e.share,
                        rollable: composition.is_some()
                            && !src.shapes(e.rock, e.geometry_tags).is_empty(),
                        ores,
                    }
                })
                .collect();
            rocks.sort_by(|a, b| b.share.total_cmp(&a.share));
            let (name, from_record, places) = src.area_places(guid);
            Some(Area {
                id: guid.to_string(),
                name,
                from_record,
                places,
                rocks,
            })
        })
        .collect();
    out.sort_by(|a, b| a.from_record.cmp(&b.from_record).then(a.name.cmp(&b.name)));
    out
}

/// One rolled rock with its quality, and what the picks rank by.
struct Rolled {
    seed: u64,
    spawn: Spawn,
    entries: Vec<Entry>,
}

impl Rolled {
    fn share_of(&self, element: &Guid) -> f32 {
        self.entries
            .iter()
            .filter(|e| e.element == *element)
            .map(|e| e.fraction)
            .sum()
    }

    fn quality_of(&self, element: &Guid) -> Option<u16> {
        self.entries
            .iter()
            .filter(|e| e.element == *element)
            .filter_map(|e| e.quality)
            .max()
    }
}

/// The ores a rock holds, as a sorted key.
fn mix(r: &Rolled) -> Vec<String> {
    let mut ores: Vec<String> = r.entries.iter().map(|e| e.element.to_string()).collect();
    ores.sort();
    ores.dedup();
    ores
}

fn median(mut v: Vec<f32>) -> f32 {
    v.sort_by(f32::total_cmp);
    v.get(v.len() / 2).copied().unwrap_or(0.0)
}

/// The most frequent ore mix and its most typical roll (closest to the
/// mix's median mass, on a log scale, and to its median share of each ore),
/// with the mix's share of the rolls. `rolled` is not empty.
fn most_common(rolled: &[Rolled]) -> (&Rolled, f32) {
    let mut groups: HashMap<Vec<String>, Vec<&Rolled>> = HashMap::new();
    for r in rolled {
        groups.entry(mix(r)).or_default().push(r);
    }
    // Ties go to the mix that sorts first, so a pick is stable.
    let (key, group) = groups
        .into_iter()
        .max_by(|(ka, a), (kb, b)| a.len().cmp(&b.len()).then(kb.cmp(ka)))
        .expect("rolled is not empty");
    let share = group.len() as f32 / rolled.len() as f32;
    let mass = median(group.iter().map(|r| r.spawn.rock.mass).collect());
    let ores: Vec<(Guid, f32)> = key
        .iter()
        .filter_map(|id| id.parse::<Guid>().ok())
        .map(|g| (g, median(group.iter().map(|r| r.share_of(&g)).collect())))
        .collect();
    let distance = |r: &Rolled| {
        (r.spawn.rock.mass / mass.max(1e-3)).ln().abs()
            + ores
                .iter()
                .map(|(g, m)| (r.share_of(g) - m).abs() / m.max(0.01))
                .sum::<f32>()
    };
    let rock = group
        .into_iter()
        .min_by(|a, b| distance(a).total_cmp(&distance(b)))
        .expect("a mix has a roll");
    (rock, share)
}

/// Lightest first.
fn by_mass(mut pool: Vec<&Rolled>) -> Vec<&Rolled> {
    pool.sort_by(|a, b| a.spawn.rock.mass.total_cmp(&b.spawn.rock.mass));
    pool
}

fn parse_guid(s: &str) -> Option<Guid> {
    s.parse().ok()
}

/// The class where breakability or power decide it without a simulation.
fn decided(ruleset: &Ruleset, m: &ModifiedRock) -> Option<RockClass> {
    if !m.is_breakable(ruleset) {
        Some(RockClass::Impossible)
    } else if m.overcharges_at_minimum(ruleset) {
        Some(RockClass::TooMuchPower)
    } else {
        None
    }
}

/// A clean-crack share in the calculator's words.
pub fn by_share(controlled: f32) -> RockClass {
    if controlled >= 0.8 {
        RockClass::Easy
    } else if controlled >= 0.5 {
        RockClass::Manageable
    } else {
        RockClass::Hard
    }
}

/// How a rock goes for a setup: [`RockClass`], our player model over
/// `runs` tries where breakability and power do not decide it.
pub fn grade(ruleset: &Ruleset, m: &ModifiedRock, runs: u32) -> RockClass {
    decided(ruleset, m).unwrap_or_else(|| {
        let d = difficulty(m, ruleset, &Player::default(), &SimConfig::default(), runs);
        by_share(d.controlled)
    })
}

/// Every rock's class: exact where breakability or power decide it, else
/// the class our player model gives the nearest of a few graded masses
/// (quantiles of the rocks, the median rock's composition) — a simulation
/// per rock would be thousands of them.
fn classes_of(
    ruleset: &Ruleset,
    elements: &Elements,
    rocks: &[&Rock],
    setup: &Setup,
) -> Vec<RockClass> {
    let mut by_mass: Vec<&Rock> = rocks.to_vec();
    by_mass.sort_by(|a, b| a.mass.total_cmp(&b.mass));
    let Some(median) = by_mass.get(by_mass.len() / 2) else {
        return vec![];
    };
    let n = SPREAD_GRADES.min(by_mass.len());
    let graded: Vec<(f32, RockClass)> = (0..n)
        .filter_map(|k| {
            let mass = by_mass[((k as f32 + 0.5) / n as f32 * by_mass.len() as f32) as usize].mass;
            let rock = Rock::from_mass(ruleset, elements, &median.entries, mass)?;
            let m = ModifiedRock::new(ruleset, &rock, setup);
            // Only simulated grades stand in for other rocks.
            decided(ruleset, &m)
                .is_none()
                .then(|| (mass, grade(ruleset, &m, GRADE_RUNS)))
        })
        .collect();
    rocks
        .iter()
        .map(|r| {
            let m = ModifiedRock::new(ruleset, r, setup);
            decided(ruleset, &m).unwrap_or_else(|| {
                graded
                    .iter()
                    .min_by(|a, b| {
                        (a.0 / r.mass)
                            .ln()
                            .abs()
                            .total_cmp(&(b.0 / r.mass).ln().abs())
                    })
                    .map(|g| g.1)
                    .unwrap_or_else(|| grade(ruleset, &m, GRADE_RUNS))
            })
        })
        .collect()
}

fn shares_of(classes: &[RockClass]) -> ClassShares {
    let n = classes.len().max(1) as f32;
    let share = |c: RockClass| classes.iter().filter(|x| **x == c).count() as f32 / n;
    ClassShares {
        easy: share(RockClass::Easy),
        manageable: share(RockClass::Manageable),
        hard: share(RockClass::Hard),
        too_much_power: share(RockClass::TooMuchPower),
        impossible: share(RockClass::Impossible),
    }
}

/// Every rollable rock type at every area with its class shares for
/// `loadout`, from a smaller sample than a pick's (spawns only: the class
/// does not depend on the composition roll).
pub fn class_table(
    src: &Sources<'_>,
    catalog: &Catalog,
    loadout: &LoadoutInput,
) -> Result<Vec<RockClasses>, SolveError> {
    let (lasers, gadgets) = setup_of(catalog, loadout)?;
    let setup = Setup { lasers, gadgets };
    let rs = &catalog.ruleset;
    let mut cache: HashMap<(Guid, Vec<Guid>), ClassShares> = HashMap::new();
    let mut out = Vec::new();
    for (guid, _) in src.providers.iter() {
        if LEFTOVERS.contains(&guid.to_string().as_str()) {
            continue;
        }
        for e in ship_entries(src, catalog, guid) {
            let key = (e.rock.guid, e.geometry_tags.to_vec());
            let shares = match cache.get(&key) {
                Some(s) => *s,
                None => {
                    let shapes = src.shapes(e.rock, e.geometry_tags);
                    if shapes.is_empty() || e.rock.composition.is_none() {
                        continue;
                    }
                    let spawner = Spawner {
                        rock_type: e.rock,
                        ruleset: rs,
                        elements: &catalog.elements,
                        compositions: &src.index.compositions,
                        shapes: &shapes,
                    };
                    let spawns: Vec<Spawn> = (0..TABLE_SAMPLE)
                        .filter_map(|seed| spawner.spawn(seed))
                        .collect();
                    let rocks: Vec<&Rock> = spawns.iter().map(|sp| &sp.rock).collect();
                    let classes = classes_of(rs, &catalog.elements, &rocks, &setup);
                    if classes.is_empty() {
                        continue;
                    }
                    let shares = shares_of(&classes);
                    cache.insert(key, shares);
                    shares
                }
            };
            out.push(RockClasses {
                area: guid.to_string(),
                rock_type: e.rock.guid.to_string(),
                classes: shares,
            });
        }
    }
    Ok(out)
}

/// The sample's masses as bars, each with the class most of its rolls have.
fn mass_buckets(rolled: &[Rolled], classes: Option<&Vec<RockClass>>) -> Vec<MassBucket> {
    let mut masses: Vec<f32> = rolled.iter().map(|r| r.spawn.rock.mass).collect();
    masses.sort_by(f32::total_cmp);
    let top = masses[((masses.len() as f32 * 0.98) as usize).min(masses.len() - 1)].max(1.0);
    let width = top / MASS_BUCKETS as f32;
    let mut counts = vec![[0u32; 6]; MASS_BUCKETS];
    let order = [
        RockClass::TooMuchPower,
        RockClass::Easy,
        RockClass::Manageable,
        RockClass::Hard,
        RockClass::Impossible,
    ];
    for (k, r) in rolled.iter().enumerate() {
        let b = ((r.spawn.rock.mass / width) as usize).min(MASS_BUCKETS - 1);
        counts[b][5] += 1;
        if let Some(cs) = classes {
            let c = order.iter().position(|x| *x == cs[k]).unwrap_or(4);
            counts[b][c] += 1;
        }
    }
    counts
        .iter()
        .enumerate()
        .map(|(b, c)| MassBucket {
            from: b as f32 * width,
            to: (b + 1) as f32 * width,
            count: c[5],
            class: classes.and_then(|_| {
                (0..5)
                    .filter(|&i| c[i] > 0)
                    .max_by_key(|&i| c[i])
                    .map(|i| order[i])
            }),
        })
        .collect()
}

/// Generate one rock for `request`; `loadout` (with a laser) adds the class
/// shares and is needed for a class pick.
pub fn generate(
    src: &Sources<'_>,
    catalog: &Catalog,
    request: &GenerateRequest,
    loadout: Option<&LoadoutInput>,
) -> Result<Generated, GenerateError> {
    let provider = parse_guid(&request.area)
        .ok_or_else(|| GenerateError::UnknownArea(request.area.clone()))?;
    if src.providers.get(&provider).is_none() {
        return Err(GenerateError::UnknownArea(request.area.clone()));
    }
    let entries = ship_entries(src, catalog, &provider);
    let entry = entries
        .iter()
        .find(|e| e.rock.guid.to_string() == request.rock_type)
        .ok_or_else(|| GenerateError::UnknownRockType(request.rock_type.clone()))?;
    let composition = entry
        .rock
        .composition
        .and_then(|c| src.index.compositions.get(&c))
        .ok_or(GenerateError::NotRollable)?;
    let shapes = src.shapes(entry.rock, entry.geometry_tags);
    if shapes.is_empty() {
        return Err(GenerateError::NotRollable);
    }
    let spawner = Spawner {
        rock_type: entry.rock,
        ruleset: &catalog.ruleset,
        elements: &catalog.elements,
        compositions: &src.index.compositions,
        shapes: &shapes,
    };

    // Quality rolls at the asked place, else the area's first; the client
    // walks the place and its parents for a location override.
    let (_, _, places) = src.area_places(&provider);
    let place = match &request.place {
        Some(id) => places.iter().find(|p| p.id == *id).cloned(),
        None => places.first().cloned(),
    };
    let location_ids: Vec<Guid> = place
        .as_ref()
        .and_then(|p| parse_guid(&p.id))
        .map(|g| {
            std::iter::once(g)
                .chain(src.locations.ancestors(&g).map(|l| l.guid))
                .collect()
        })
        .unwrap_or_default();
    let quality = QualityRoll {
        elements: &catalog.elements,
        resources: src.resources,
        catalog: &src.index.quality,
        locations: &location_ids,
    };

    let rolled: Vec<Rolled> = (0..u64::from(SAMPLE))
        .filter_map(|seed| {
            let spawn = spawner.spawn(seed)?;
            let entries = composition.roll_with_quality(composition_seed(seed), &quality);
            Some(Rolled {
                seed,
                spawn,
                entries,
            })
        })
        .collect();
    if rolled.is_empty() {
        return Err(GenerateError::NotRollable);
    }

    // Classes for the loadout, when it has a laser.
    let setup = match loadout.filter(|l| !l.lasers.is_empty()) {
        Some(l) => {
            let (lasers, gadgets) = setup_of(catalog, l).map_err(GenerateError::Loadout)?;
            Some(Setup { lasers, gadgets })
        }
        None => None,
    };
    let rs = &catalog.ruleset;
    let classes: Option<Vec<RockClass>> = setup.as_ref().map(|s| {
        let rocks: Vec<&Rock> = rolled.iter().map(|r| &r.spawn.rock).collect();
        classes_of(rs, &catalog.elements, &rocks, s)
    });
    let shares = classes.as_deref().map(shares_of);
    let masses = mass_buckets(&rolled, classes.as_ref());

    let all: Vec<&Rolled> = rolled.iter().collect();
    let mut pick_share = None;
    let chosen: &Rolled = match &request.pick {
        Pick::MostCommon => {
            let (rock, share) = most_common(&rolled);
            pick_share = Some(share);
            rock
        }
        Pick::Average => {
            let pool = by_mass(all);
            pool[pool.len() / 2]
        }
        Pick::Smallest => by_mass(all)[0],
        Pick::Largest => by_mass(all).last().unwrap(),
        Pick::MostOre { element } => {
            let g =
                parse_guid(element).ok_or_else(|| GenerateError::NoRockWith(element.clone()))?;
            all.into_iter()
                .filter(|r| r.share_of(&g) > 0.0)
                .max_by(|a, b| a.share_of(&g).total_cmp(&b.share_of(&g)))
                .ok_or_else(|| GenerateError::NoRockWith(element.clone()))?
        }
        Pick::BestQuality { element } => {
            let g =
                parse_guid(element).ok_or_else(|| GenerateError::NoRockWith(element.clone()))?;
            all.into_iter()
                .filter(|r| r.share_of(&g) > 0.0)
                .max_by(|a, b| {
                    a.quality_of(&g)
                        .cmp(&b.quality_of(&g))
                        .then(a.share_of(&g).total_cmp(&b.share_of(&g)))
                })
                .ok_or_else(|| GenerateError::NoRockWith(element.clone()))?
        }
        Pick::Class { class } => {
            let cs = classes.as_ref().ok_or(GenerateError::NeedsLoadout)?;
            let pool = by_mass(
                rolled
                    .iter()
                    .zip(cs)
                    .filter(|(_, c)| **c == *class)
                    .map(|(r, _)| r)
                    .collect(),
            );
            if pool.is_empty() {
                return Err(GenerateError::NoRockOfClass(*class));
            }
            pool[pool.len() / 2]
        }
        Pick::Mass { mass } => all
            .into_iter()
            .min_by(|a, b| {
                (a.spawn.rock.mass - mass)
                    .abs()
                    .total_cmp(&(b.spawn.rock.mass - mass).abs())
            })
            .expect("rolled is not empty"),
    };

    Ok(Generated {
        rock: RockInput {
            mass: chosen.spawn.rock.mass,
            entries: chosen
                .entries
                .iter()
                .map(|e| EntryInput {
                    element: e.element.to_string(),
                    percent: e.fraction * 100.0,
                    quality: e.quality,
                })
                .collect(),
        },
        scu: chosen.spawn.rock.scu,
        seed: chosen.seed,
        place,
        rolled: rolled.len() as u32,
        classes: shares,
        pick_share,
        masses,
    })
}

#[cfg(test)]
mod tests {
    use super::record_label;

    #[test]
    fn record_labels_read() {
        assert_eq!(record_label("HPP_Nyx_KeegerBelt"), "Nyx Keeger Belt");
        assert_eq!(record_label("HPP_Lagrange_A"), "Lagrange A");
        assert_eq!(record_label("HPP_Stanton2c_Belt"), "Stanton2c Belt");
    }
}
