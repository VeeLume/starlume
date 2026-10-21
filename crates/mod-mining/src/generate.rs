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
    Composition, Elements, Entry, ModifiedRock, QualityRoll, Rock, RockType, Ruleset, Setup, Shape,
    SimConfig, Spawn, Spawner,
};
use svc_data::{
    GatheringMode, Guid, LocaleMap, LocationKind, Locations, MiningIndex, ProviderLocations,
    Providers, RecordCollection, Resources, Tags,
};

use crate::catalog::Catalog;
use crate::lagrange::{self, Points, Role};
use crate::solve::{EntryInput, LoadoutInput, RockInput, SolveError, Superseded, setup_of};

/// Rolls per rock type and area: shares to about a percent, and the median,
/// smallest and largest of a real spread.
pub const SAMPLE: u32 = 2000;
/// Composition rolls per rock type for its ore odds.
const ODDS_ROLLS: u64 = 2000;
/// Rolls per rock type and area for the class table (the finder's lists):
/// shares to a few percent, over every area at once.
const TABLE_SAMPLE: u64 = 400;
/// Player-model tries per graded mass in a band or a list: enough to tell
/// easy, manageable and hard apart, cheap enough to run many times. At 24
/// the noise (±10 points) flipped neighbouring slices of the mass band
/// between classes (band_probe, 2026-10-18); every slice uses the same
/// seeds, so neighbours differ by the rock, not the draw.
pub const GRADE_RUNS: u32 = 64;
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
    /// Its distribution: the rock type with the meshes this area spawns it
    /// with. Rows with the same key roll the same rocks (masses, ores,
    /// classes), so the finder shows them once, listing their places.
    pub dist: String,
    /// Lightest, most common and heaviest of its rolls; `None` when it
    /// cannot be rolled.
    pub mass: Option<MassRange>,
}

/// A distribution's masses in tonnes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct MassRange {
    pub min: f32,
    /// The middle of the fullest of [`MASS_BUCKETS`] bars up to the 98th
    /// percentile.
    pub mode: f32,
    pub max: f32,
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

/// A distribution with a loadout (design pass 5): its class shares and its
/// masses as bars, each counted by class — what the comfort zone draws and
/// the two scores sum. One solve per loadout covers every rock that spawns;
/// the finder's class table reads it too.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Comfort {
    /// The distribution key ([`AreaRock::dist`]).
    pub dist: String,
    pub classes: ClassShares,
    pub bars: Vec<ComfortBar>,
}

/// One bar of a distribution's masses, its rolls counted by class.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ComfortBar {
    pub from: f32,
    pub to: f32,
    /// Rolls that are too much power, easy, manageable, hard, can't break —
    /// in power order, as a chart stacks them.
    pub counts: [u32; 5],
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
    /// A newer request replaced this one ([`crate::Superseded`]).
    Superseded,
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
            Self::Superseded => write!(f, "a newer request replaced this one"),
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
    /// For a field node, how many places it stands for; for a Lagrange
    /// point, its mining claims.
    pub count: u32,
    /// A Lagrange point's rest stop, as the game names it.
    pub rest_stop: Option<String>,
    /// A Lagrange point's spawn tables by what they are at the point: the
    /// table on the point itself (Pyro), its gas pockets, the table on every
    /// claim, the yield tables with the claims carrying them. Empty for any
    /// other place (its `areas` are its tables).
    pub groups: Vec<PlaceGroup>,
    pub children: Vec<PlaceNode>,
}

/// Spawn tables at a Lagrange point, by what they spawn at.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PlaceGroup {
    /// `None` for the point's own table.
    pub title: Option<String>,
    /// The pockets or claims, by name, or how many.
    pub detail: Option<String>,
    pub areas: Vec<String>,
}

struct Building {
    name: String,
    kind: &'static str,
    areas: Vec<String>,
    count: u32,
    parent: Option<Guid>,
    system: String,
    rest_stop: Option<String>,
    groups: Vec<PlaceGroup>,
}

/// What a provider spawns at one Lagrange point, by role.
#[derive(Default)]
struct AtPoint {
    direct: Vec<String>,
    /// Pocket → its providers.
    pockets: HashMap<Guid, Vec<String>>,
    /// Claim → its providers.
    claims: HashMap<Guid, Vec<String>>,
}

/// The place tree for the generator: systems, their planets with moons
/// nested by the game's parent links, the other places and fields beside
/// them. Lagrange points sit under their planet ([`lagrange`]: the data
/// parents them to the star) with their rest stop, and what hangs off a
/// point — gas pockets, mining claims — is the point's, grouped
/// ([`PlaceGroup`]) instead of fields of its own.
pub fn place_tree(src: &Sources<'_>, catalog: &Catalog) -> Vec<PlaceSystem> {
    let points = Points::build(src.locations, src.locale);
    let record_of = |g: &Guid| {
        src.index
            .provider_names
            .iter()
            .find(|p| p.guid == *g)
            .map(|p| p.record_name.clone())
            .unwrap_or_default()
    };
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
                rest_stop: None,
                groups: vec![],
            },
        );
    };
    let mut at_points: HashMap<Guid, AtPoint> = HashMap::new();

    for (guid, _) in src.providers.iter() {
        if LEFTOVERS.contains(&guid.to_string().as_str())
            || ship_entries(src, catalog, guid).is_empty()
        {
            continue;
        }
        let area = guid.to_string();
        let all: Vec<Guid> = src
            .places
            .map(|p| p.locations_of(guid).to_vec())
            .unwrap_or_default();
        // What spawns at a Lagrange point (on it, in a pocket, on a claim)
        // is the point's; only the rest is placed on its own.
        let mut places: Vec<Guid> = Vec::new();
        for g in &all {
            match points.of(src.locations, g) {
                Some((p, role)) => {
                    let at = at_points.entry(p).or_default();
                    let list = match role {
                        Role::Point => &mut at.direct,
                        Role::Pocket => at.pockets.entry(*g).or_default(),
                        Role::Claim => at.claims.entry(*g).or_default(),
                    };
                    if !list.contains(&area) {
                        list.push(area.clone());
                    }
                }
                None => places.push(*g),
            }
        }
        if places.is_empty() && !all.is_empty() {
            continue;
        }
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
                    rest_stop: None,
                    groups: vec![],
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

    // The Lagrange points, under their planet.
    for (p, at) in at_points {
        let Some(point) = points.get(&p) else {
            continue;
        };
        add_place(&mut nodes, &point.planet, None);
        let mut groups: Vec<PlaceGroup> = Vec::new();
        if !at.direct.is_empty() {
            groups.push(PlaceGroup {
                title: None,
                detail: None,
                areas: at.direct,
            });
        }
        // Pockets spawning the same tables: one group (CRU L2-A, -B, -C).
        let mut pockets: Vec<(Vec<String>, Vec<String>)> = Vec::new();
        for (g, mut areas) in at.pockets {
            areas.sort();
            let name = src.place_name(&g).unwrap_or_else(|| "?".into());
            match pockets.iter_mut().find(|(a, _)| *a == areas) {
                Some((_, names)) => names.push(name),
                None => pockets.push((areas, vec![name])),
            }
        }
        pockets.sort_by(|a, b| a.1.iter().min().cmp(&b.1.iter().min()));
        for (areas, mut names) in pockets {
            names.sort();
            let title = if names.len() > 1 {
                "Gas pockets"
            } else {
                "Gas pocket"
            };
            groups.push(PlaceGroup {
                title: Some(title.into()),
                detail: Some(names.join(", ")),
                areas,
            });
        }
        // Claims: the table every claim carries, then each yield table with
        // the claims carrying it (claim names repeat across points, so a
        // claim is always named with its point beside it).
        let claims = at.claims.len();
        let mut by_table: HashMap<String, Vec<String>> = HashMap::new();
        for (g, areas) in &at.claims {
            let name = src.place_name(g).unwrap_or_else(|| "?".into());
            for a in areas {
                by_table.entry(a.clone()).or_default().push(name.clone());
            }
        }
        let record_for = |area: &str| {
            area.parse::<Guid>()
                .map(|g| record_of(&g))
                .unwrap_or_default()
        };
        let mut tables: Vec<(String, Vec<String>)> = by_table.into_iter().collect();
        // The claim table (not a yield table) on every claim reads as such;
        // a yield table keeps its yield even where every claim carries it
        // (ARC L1: all eight low-yield).
        let every = |record: &str, n: usize| {
            n == claims && lagrange::claim_label(record) == "Mining claims"
        };
        tables.sort_by_key(|(a, names)| {
            let record = record_for(a);
            let order = if every(&record, names.len()) {
                0
            } else if record.contains("High_Yield") {
                1
            } else if record.contains("Medium_Yield") {
                2
            } else {
                3
            };
            (order, record)
        });
        for (area, mut names) in tables {
            names.sort();
            let group = if every(&record_for(&area), names.len()) {
                PlaceGroup {
                    title: Some("At every mining claim".into()),
                    detail: Some(format!(
                        "{claims} claim{}",
                        if claims == 1 { "" } else { "s" }
                    )),
                    areas: vec![area],
                }
            } else {
                PlaceGroup {
                    title: Some(format!(
                        "{} · {}",
                        lagrange::claim_label(&record_for(&area)),
                        names.len()
                    )),
                    detail: Some(names.join(" · ")),
                    areas: vec![area],
                }
            };
            groups.push(group);
        }
        let mut areas: Vec<String> = groups.iter().flat_map(|g| g.areas.clone()).collect();
        areas.sort();
        areas.dedup();
        nodes.insert(
            p.to_string(),
            Building {
                name: point.name.clone(),
                kind: "lpoint",
                areas,
                count: claims as u32,
                parent: Some(point.planet),
                system: star_of(&p).unwrap_or_else(|| "Other".into()),
                rest_stop: point.rest_stop.clone(),
                groups,
            },
        );
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
        // Moons, then other places, then the Lagrange points (L1..L5).
        let order = |k: &str| match k {
            "body" => 0,
            "place" => 1,
            "lpoint" => 2,
            _ => 3,
        };
        children.sort_by(|a, b| {
            order(&a.kind)
                .cmp(&order(&b.kind))
                .then(a.name.cmp(&b.name))
        });
        PlaceNode {
            id: key.to_string(),
            name: b.name.clone(),
            kind: b.kind.to_string(),
            areas: b.areas.clone(),
            count: b.count,
            rest_stop: b.rest_stop.clone(),
            groups: b.groups.clone(),
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
    let points = Points::build(src.locations, src.locale);
    let mut odds_cache: HashMap<Guid, Vec<OreOdds>> = HashMap::new();
    let mut mass_cache: HashMap<String, Option<MassRange>> = HashMap::new();
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
                    let shapes = src.shapes(e.rock, e.geometry_tags);
                    let rollable = composition.is_some() && !shapes.is_empty();
                    let dist = dist_key(src, e.rock, e.geometry_tags);
                    let mass = match rollable {
                        true => *mass_cache
                            .entry(dist.clone())
                            .or_insert_with(|| mass_range(src, catalog, e.rock, &shapes)),
                        false => None,
                    };
                    AreaRock {
                        rock_type: e.rock.guid.to_string(),
                        name: src.rock_name(e.rock),
                        share: e.share,
                        rollable,
                        ores,
                        dist,
                        mass,
                    }
                })
                .collect();
            rocks.sort_by(|a, b| b.share.total_cmp(&a.share));
            let (name, from_record, places) = src.area_places(guid);
            let (name, from_record) = match point_area_name(src, &points, guid) {
                Some(n) => (n, false),
                None => (name, from_record),
            };
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

/// What decides a rock type's rolls, as a key: its composition, scale range,
/// filled factor, piece count and meshes. Rock types differ by GUID per
/// planet group while rolling the same rocks (Quantainium on Aberdeen,
/// Arial, Cellin, Wala), so the GUID is not part of it.
fn dist_key(src: &Sources<'_>, rock: &RockType, geometry_tags: &[Guid]) -> String {
    let mut meshes: Vec<String> = rock
        .geometries_for(src.tags, geometry_tags)
        .iter()
        .map(|g| g.path.to_string())
        .collect();
    meshes.sort();
    meshes.dedup();
    format!(
        "{}|{}-{}|{}|{}|{}",
        rock.composition.map(|c| c.to_string()).unwrap_or_default(),
        rock.min_scale,
        rock.max_scale,
        rock.filled_factor,
        rock.pieces_override,
        meshes.join(",")
    )
}

/// A distribution's lightest, most common and heaviest rolls, over the
/// class table's sample (the same seeds, so the finder's rows and lists
/// describe the same rocks).
fn mass_range(
    src: &Sources<'_>,
    catalog: &Catalog,
    rock: &RockType,
    shapes: &[&Shape],
) -> Option<MassRange> {
    let spawner = Spawner {
        rock_type: rock,
        ruleset: &catalog.ruleset,
        elements: &catalog.elements,
        compositions: &src.index.compositions,
        shapes,
    };
    let mut masses: Vec<f32> = (0..TABLE_SAMPLE)
        .filter_map(|seed| spawner.spawn(seed))
        .map(|sp| sp.rock.mass)
        .collect();
    if masses.is_empty() {
        return None;
    }
    masses.sort_by(f32::total_cmp);
    let top = masses[((masses.len() as f32 * 0.98) as usize).min(masses.len() - 1)].max(1.0);
    let width = top / MASS_BUCKETS as f32;
    let mut counts = [0u32; MASS_BUCKETS];
    for m in &masses {
        counts[((m / width) as usize).min(MASS_BUCKETS - 1)] += 1;
    }
    // The fullest bar; the lighter one on a tie.
    let fullest = (0..MASS_BUCKETS)
        .max_by_key(|&i| (counts[i], std::cmp::Reverse(i)))
        .unwrap_or(0);
    Some(MassRange {
        min: masses[0],
        mode: (fullest as f32 + 0.5) * width,
        max: masses[masses.len() - 1],
    })
}

/// An area spawning only at Lagrange points, named the way the finder
/// shows those: `Bloom L1–L5` for the points themselves (Pyro's tables),
/// `HUR L1-A, HUR L4-A` for gas pockets, `Low-yield claims (124)` for claims.
fn point_area_name(src: &Sources<'_>, points: &Points, provider: &Guid) -> Option<String> {
    let all = src.places?.locations_of(provider);
    if all.is_empty() {
        return None;
    }
    let roles = all
        .iter()
        .map(|g| points.of(src.locations, g).map(|(p, r)| (*g, p, r)))
        .collect::<Option<Vec<_>>>()?;
    if roles.iter().all(|r| r.2 == Role::Point) {
        let mut by_planet: Vec<(String, Vec<u8>)> = Vec::new();
        for (_, p, _) in &roles {
            let point = points.get(p)?;
            let planet = src.place_name(&point.planet)?;
            let n = point
                .name
                .rsplit_once('L')
                .and_then(|(_, n)| n.parse().ok())?;
            match by_planet.iter_mut().find(|(name, _)| *name == planet) {
                Some((_, ns)) => ns.push(n),
                None => by_planet.push((planet, vec![n])),
            }
        }
        by_planet.sort();
        return Some(
            by_planet
                .into_iter()
                .map(|(planet, ns)| format!("{planet} {}", lagrange::ranges(ns)))
                .collect::<Vec<_>>()
                .join(" · "),
        );
    }
    // Claims, with any pockets or points it also spawns at counted on
    // (Lagrange C: 158 claims and nine pockets and points).
    let claims = roles.iter().filter(|r| r.2 == Role::Claim).count();
    if claims > 0 {
        let record = src
            .index
            .provider_names
            .iter()
            .find(|p| p.guid == *provider)
            .map(|p| p.record_name.as_str())
            .unwrap_or_default();
        let more = all.len() - claims;
        let more = if more > 0 {
            format!(" + {more} more")
        } else {
            String::new()
        };
        return Some(format!(
            "{} ({claims}){more}",
            lagrange::claim_label(record)
        ));
    }
    if roles.iter().all(|r| r.2 == Role::Pocket) {
        let mut names: Vec<String> = all.iter().filter_map(|g| src.place_name(g)).collect();
        names.sort();
        return Some(names.join(", "));
    }
    None
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
pub fn grade(ruleset: &Ruleset, rock: &Rock, setup: &Setup, runs: u32) -> RockClass {
    grade_share(ruleset, rock, setup, runs).0
}

/// [`grade`] and, where the player model decided it, the share of tries
/// that cracked without losing a piece (`None` where breakability or power
/// decide the class).
pub fn grade_share(
    ruleset: &Ruleset,
    rock: &Rock,
    setup: &Setup,
    runs: u32,
) -> (RockClass, Option<f32>) {
    let m = ModifiedRock::new(ruleset, rock, setup);
    if let Some(class) = decided(ruleset, &m) {
        return (class, None);
    }
    let beam_off = crate::player::beam_off(ruleset, rock, setup);
    let d = crate::player::difficulty(&m, &beam_off, ruleset, &SimConfig::default(), runs);
    let class = match by_share(d.controlled) {
        RockClass::Easy if crate::player::narrow_window(&m) => RockClass::Manageable,
        class => class,
    };
    (class, Some(d.controlled))
}

/// Every rock's class: exact where breakability or power decide it, else
/// the class our player model gives the nearest of a few graded masses
/// (quantiles of the rocks, the median rock's composition) — a simulation
/// per rock would be thousands of them. Empty once `superseded`.
/// Better first: easy, manageable, hard, too much power, can't break.
fn rank(c: RockClass) -> u8 {
    match c {
        RockClass::Easy => 0,
        RockClass::Manageable => 1,
        RockClass::Hard => 2,
        RockClass::TooMuchPower => 3,
        RockClass::Impossible => 4,
    }
}

/// Each rock's class for the setup. Where every laser at its minimum is
/// too much power, a crew stops a head (a MOLE operator holds fire) rather
/// than overcharge a small rock: those rocks take the best class any
/// smaller set of the lasers gets. Without it a three-head MOLE read as too
/// much power on most small rocks it cracks fine with one head.
fn classes_of(
    ruleset: &Ruleset,
    elements: &Elements,
    rocks: &[&Rock],
    setup: &Setup,
    superseded: Superseded<'_>,
) -> Vec<RockClass> {
    let mut best = classes_with(ruleset, elements, rocks, setup, superseded);
    let n = setup.lasers.len().min(4);
    if n < 2 || !best.contains(&RockClass::TooMuchPower) {
        return best;
    }
    for mask in 1..(1u32 << n) - 1 {
        let fewer = Setup {
            lasers: (0..n)
                .filter(|i| mask & (1 << i) != 0)
                .map(|i| setup.lasers[i].clone())
                .collect(),
            gadgets: setup.gadgets.clone(),
            activations: vec![],
        };
        let over: Vec<usize> = (0..rocks.len())
            .filter(|&i| best[i] == RockClass::TooMuchPower)
            .collect();
        if over.is_empty() {
            break;
        }
        let subset: Vec<&Rock> = over.iter().map(|&i| rocks[i]).collect();
        let classes = classes_with(ruleset, elements, &subset, &fewer, superseded);
        if superseded() {
            return vec![];
        }
        for (k, c) in over.into_iter().zip(classes) {
            if rank(c) < rank(best[k]) {
                best[k] = c;
            }
        }
    }
    best
}

fn classes_with(
    ruleset: &Ruleset,
    elements: &Elements,
    rocks: &[&Rock],
    setup: &Setup,
    superseded: Superseded<'_>,
) -> Vec<RockClass> {
    let mut by_mass: Vec<&Rock> = rocks.to_vec();
    by_mass.sort_by(|a, b| a.mass.total_cmp(&b.mass));
    let Some(median) = by_mass.get(by_mass.len() / 2) else {
        return vec![];
    };
    let n = SPREAD_GRADES.min(by_mass.len());
    let graded: Vec<(f32, RockClass)> = (0..n)
        .filter_map(|k| {
            if superseded() {
                return None;
            }
            let mass = by_mass[((k as f32 + 0.5) / n as f32 * by_mass.len() as f32) as usize].mass;
            let rock = Rock::from_mass(ruleset, elements, &median.entries, mass)?;
            let m = ModifiedRock::new(ruleset, &rock, setup);
            // Only simulated grades stand in for other rocks.
            decided(ruleset, &m)
                .is_none()
                .then(|| (mass, grade(ruleset, &rock, setup, GRADE_RUNS)))
        })
        .collect();
    if superseded() {
        return vec![];
    }
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
                    .unwrap_or_else(|| grade(ruleset, r, setup, GRADE_RUNS))
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
    superseded: Superseded<'_>,
) -> Result<Vec<RockClasses>, SolveError> {
    let by_dist: HashMap<String, ClassShares> = comfort(src, catalog, loadout, superseded)?
        .into_iter()
        .map(|c| (c.dist, c.classes))
        .collect();
    let mut out = Vec::new();
    for (guid, _) in src.providers.iter() {
        if LEFTOVERS.contains(&guid.to_string().as_str()) {
            continue;
        }
        for e in ship_entries(src, catalog, guid) {
            if let Some(shares) = by_dist.get(&dist_key(src, e.rock, e.geometry_tags)) {
                out.push(RockClasses {
                    area: guid.to_string(),
                    rock_type: e.rock.guid.to_string(),
                    classes: *shares,
                });
            }
        }
    }
    Ok(out)
}

/// Every distribution that spawns, with the loadout: class shares and mass
/// bars counted by class, over the class table's sample (the same seeds the
/// finder's mass ranges use).
pub fn comfort(
    src: &Sources<'_>,
    catalog: &Catalog,
    loadout: &LoadoutInput,
    superseded: Superseded<'_>,
) -> Result<Vec<Comfort>, SolveError> {
    let (lasers, gadgets) = setup_of(catalog, loadout)?;
    let setup = Setup {
        lasers,
        gadgets,
        activations: vec![],
    };
    let rs = &catalog.ruleset;
    let mut done: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out = Vec::new();
    for (guid, _) in src.providers.iter() {
        if LEFTOVERS.contains(&guid.to_string().as_str()) {
            continue;
        }
        for e in ship_entries(src, catalog, guid) {
            let dist = dist_key(src, e.rock, e.geometry_tags);
            if !done.insert(dist.clone()) {
                continue;
            }
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
            let classes = classes_of(rs, &catalog.elements, &rocks, &setup, superseded);
            if superseded() {
                return Err(SolveError::Superseded);
            }
            if classes.is_empty() {
                continue;
            }
            out.push(Comfort {
                classes: shares_of(&classes),
                bars: comfort_bars(&rocks, &classes),
                dist,
            });
        }
    }
    Ok(out)
}

/// A sample's masses as [`MASS_BUCKETS`] bars up to the 98th percentile
/// (heavier rolls in the last), each counting its rolls by class.
fn comfort_bars(rocks: &[&Rock], classes: &[RockClass]) -> Vec<ComfortBar> {
    let mut masses: Vec<f32> = rocks.iter().map(|r| r.mass).collect();
    masses.sort_by(f32::total_cmp);
    let top = masses[((masses.len() as f32 * 0.98) as usize).min(masses.len() - 1)].max(1.0);
    let width = top / MASS_BUCKETS as f32;
    let order = [
        RockClass::TooMuchPower,
        RockClass::Easy,
        RockClass::Manageable,
        RockClass::Hard,
        RockClass::Impossible,
    ];
    let mut bars: Vec<ComfortBar> = (0..MASS_BUCKETS)
        .map(|b| ComfortBar {
            from: b as f32 * width,
            to: (b + 1) as f32 * width,
            counts: [0; 5],
        })
        .collect();
    for (r, c) in rocks.iter().zip(classes) {
        let b = ((r.mass / width) as usize).min(MASS_BUCKETS - 1);
        let k = order.iter().position(|x| x == c).unwrap_or(4);
        bars[b].counts[k] += 1;
    }
    bars
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
    superseded: Superseded<'_>,
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
    let setup = match loadout.filter(|l| l.lasers.iter().any(|x| !x.off)) {
        Some(l) => {
            let (lasers, gadgets) = setup_of(catalog, l).map_err(GenerateError::Loadout)?;
            Some(Setup {
                lasers,
                gadgets,
                activations: vec![],
            })
        }
        None => None,
    };
    let rs = &catalog.ruleset;
    let classes: Option<Vec<RockClass>> = setup.as_ref().map(|s| {
        let rocks: Vec<&Rock> = rolled.iter().map(|r| &r.spawn.rock).collect();
        classes_of(rs, &catalog.elements, &rocks, s, superseded)
    });
    if superseded() {
        return Err(GenerateError::Superseded);
    }
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
            // Ship rocks in space use the asteroid meshes, on planets and
            // moons mining_rock_01 (inferred from the meshes; sc-holotable
            // is tracing what tells them apart).
            surface: Some(
                !chosen
                    .spawn
                    .shape
                    .path
                    .to_ascii_lowercase()
                    .contains("asteroid"),
            ),
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
