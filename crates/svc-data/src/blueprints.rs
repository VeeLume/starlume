//! The blueprint catalog: every craftable item, its recipe's material slots,
//! how each slot's quality reshapes the item, and which missions grant it.
//! Reference framework (CONTRIBUTING module rule 4).
//!
//! Written beside the cook and loaded only when something asks, so it costs
//! nothing while Crafting is closed. Names are resolved here: the file is
//! self-contained and a query needs no other game data (the missions'
//! reasoning, [`crate::missions`]).
//!
//! The slider maths stays with the frontend, which has to re-evaluate as a
//! quality moves: this ships the curves and the base values.

use std::collections::{HashMap, HashSet};

use sc_holotable::Foundations;
use sc_holotable::armor::{Armor, ArmorStats, DamageResistance, ResistanceEntry};
use sc_holotable::asset::{Datacore, Guid, LocaleKey, LocaleMap, RecordCollection};
use sc_holotable::crafting::{
    Blueprint, Blueprints, Categories, Cost, DisplayTransformation, GameplayProperties,
    GameplayPropertyModifier, GameplayStat, GlobalParams, Process, SlotName, ValueRange,
};
use sc_holotable::fps_weapons::{Damage as FpsDamage, FpsWeaponStats, FpsWeapons};
use sc_holotable::items::{Item, ItemCatalog, ItemClass};
use sc_holotable::ship_components::{ShipComponentStats, ShipComponents};
use sc_holotable::ship_weapons::{Damage as ShipDamage, ShipWeaponStats, ShipWeapons};
use serde::{Deserialize, Serialize};

use crate::missions::MissionEntry;

/// The catalog for one build.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BlueprintIndex {
    /// The quality a recipe's curves are anchored on: stock (500 today).
    pub default_quality: i32,
    /// Sorted by name.
    pub blueprints: Vec<BlueprintEntry>,
}

impl BlueprintIndex {
    pub fn get(&self, guid: &str) -> Option<&BlueprintEntry> {
        self.blueprints.iter().find(|b| b.guid == guid)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlueprintEntry {
    /// The `CraftingBlueprintRecord` GUID: what owned blueprints name.
    pub guid: String,
    /// The crafted item's record GUID.
    pub entity_guid: Option<String>,
    pub name: String,
    /// The variant family it folds into: shared by colourways of one item,
    /// see [`assign_families`]. A lone blueprint is a family of one.
    pub family: String,
    /// The family's name, without any colourway.
    pub family_name: String,
    /// What sets this one apart in its family ("Snow Camo"); `None` for the
    /// plain one and for a family of one.
    pub variant: Option<String>,
    /// CIG's category record, prefix cut (`FPSArmours`, `VehicleWeaponsS3`).
    pub category: Option<String>,
    pub item_type: String,
    pub sub_type: String,
    pub size: i32,
    pub grade: i32,
    /// Military, Civilian, Industrial, Stealth or Competition.
    pub class: Option<String>,
    /// From the item description's `Manufacturer:` line.
    pub manufacturer: Option<String>,
    pub craft_seconds: Option<f32>,
    pub slots: Vec<BlueprintSlot>,
    pub stats: Vec<BlueprintStat>,
    pub granted_by: Vec<GrantingMission>,
}

/// One named material slot of a recipe.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlueprintSlot {
    /// "Frame", "Cabling"; `None` for slots the game hasn't named.
    pub name: Option<String>,
    pub material: String,
    /// Bulk materials, in SCU.
    pub scu: Option<f32>,
    /// Discrete items (hand-mined gems), counted.
    pub count: Option<i32>,
    pub min_quality: i32,
    pub modifiers: Vec<SlotModifier>,
}

/// One gameplay property a slot's quality reshapes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlotModifier {
    /// The property's GUID: joins to [`BlueprintStat::property`].
    pub property: Option<String>,
    pub name: Option<String>,
    pub transform: ModifierTransform,
    pub ranges: Vec<ModifierRange>,
}

/// How the game displays a modifier's value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModifierTransform {
    /// `×0.92` as `-8%`.
    FactorToPercent,
    /// `×0.92` as `+8%`: the stat improves as it shrinks.
    FactorToNegatedPercent,
    /// The factor as it is.
    Raw,
}

/// A quality band of a modifier's curve.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModifierRange {
    /// An added amount, not a factor.
    pub additive: bool,
    pub start_quality: i32,
    pub end_quality: i32,
    pub at_start: f32,
    pub at_end: f32,
}

/// One line of the crafted item's stat sheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlueprintStat {
    /// Section ("Damage Resistance"); `None` for a flat row.
    pub group: Option<String>,
    pub label: String,
    /// The property the recipe drives this through; `None` when it doesn't.
    pub property: Option<String>,
    /// Suffix, with its leading space where it wants one.
    pub unit: String,
    /// Which way the value is better; `None` when unclear.
    pub higher_is_better: Option<bool>,
    /// `None` when the item has no absolute value for it: percent only.
    pub base: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GrantingMission {
    pub mission_id: String,
    pub title: String,
}

/// The per-domain base stats a crafted item can be covered by.
struct Sheets {
    fps_weapons: FpsWeapons,
    armor: Armor,
    ship_components: ShipComponents,
    ship_weapons: ShipWeapons,
}

pub(crate) fn build_blueprints(
    datacore: &Datacore,
    foundations: &Foundations,
    locale: &LocaleMap,
    missions: &[MissionEntry],
) -> BlueprintIndex {
    let items = &foundations.items;
    let catalog = Blueprints::build(datacore, items);
    let categories = Categories::build(&foundations.paths);
    let properties = GameplayProperties::build(datacore);
    let default_quality = GlobalParams::build(datacore)
        .map(|g| g.default_composition_quality)
        .unwrap_or(500);
    let sheets = Sheets {
        fps_weapons: FpsWeapons::build(datacore, items),
        armor: Armor::build(datacore, items),
        ship_components: ShipComponents::build(datacore, items),
        ship_weapons: ShipWeapons::build(datacore, items),
    };
    let grants = grants_by_blueprint(missions);
    let ctx = Context {
        foundations,
        categories: &categories,
        properties: &properties,
        sheets: &sheets,
        locale,
        grants: &grants,
    };
    let mut blueprints: Vec<BlueprintEntry> = catalog
        .iter()
        .filter(|(_, b)| matches!(b.process, Process::Creation { .. }))
        .filter_map(|(_, b)| ctx.entry(b))
        .collect();
    blueprints
        .sort_by(|a, b| (a.name.to_lowercase(), &a.guid).cmp(&(b.name.to_lowercase(), &b.guid)));
    let models = ItemCatalog::build(items, &foundations.paths, locale);
    assign_families(&mut blueprints, &models, foundations);
    BlueprintIndex {
        default_quality,
        blueprints,
    }
}

/// Fold blueprints into variant families: one row in the catalog for an item
/// and its colourways.
///
/// Two signals say "same item": sc-holotable's [`ItemCatalog`] model, which
/// goes by display name, and the entity record's stem, which goes by CIG's
/// `<item>_NN_NN` numbering. Names split what the records join ("Aztalan
/// Galena Helmet" is `thp_light_helmet_01_01_12`, "Aztalan Helmet" `_01_01_01`),
/// so either signal may join. Nothing joins across a different recipe, stat
/// sheet, type or size: a family shares one recipe sheet, so that has to be
/// true of every member ("Chiron Arms Purgatory Camo" has its own numbers).
/// Geometry is no signal: unrelated items share models.
///
/// On 4.10.2 live: 1,618 blueprints, 762 families.
fn assign_families(
    entries: &mut [BlueprintEntry],
    models: &ItemCatalog,
    foundations: &Foundations,
) {
    let signals: Vec<Signals> = entries
        .iter()
        .map(|b| {
            let entity = b
                .entity_guid
                .as_deref()
                .and_then(|g| g.parse::<Guid>().ok());
            Signals {
                guard: format!(
                    "{:?}|{:?}|{:?}|{}|{}|{}|{}",
                    b.craft_seconds, b.slots, b.stats, b.item_type, b.sub_type, b.size, b.grade
                ),
                model: entity
                    .as_ref()
                    .and_then(|e| models.model_id_of(e))
                    .map(str::to_owned),
                stem: entity
                    .as_ref()
                    .and_then(|e| foundations.paths.get(e))
                    .and_then(|r| record_stem(&r.name))
                    .map(str::to_owned),
            }
        })
        .collect();
    for group in group_families(&signals) {
        let names: Vec<&str> = group.iter().map(|&i| entries[i].name.as_str()).collect();
        let family_name = family_name(&names);
        let family = entries[group[0]].guid.clone();
        let solo = group.len() == 1;
        for &i in &group {
            entries[i].variant = if solo {
                None
            } else {
                variant_label(&entries[i].name, &family_name)
            };
            entries[i].family = family.clone();
            entries[i].family_name = family_name.clone();
        }
    }
}

struct Signals {
    /// Everything a family has to share.
    guard: String,
    model: Option<String>,
    stem: Option<String>,
}

/// Indices per family, each ascending, in order of their first member.
fn group_families(signals: &[Signals]) -> Vec<Vec<usize>> {
    fn root(parent: &mut [usize], i: usize) -> usize {
        let mut r = i;
        while parent[r] != r {
            parent[r] = parent[parent[r]];
            r = parent[r];
        }
        r
    }
    let mut parent: Vec<usize> = (0..signals.len()).collect();
    let mut seen: HashMap<(&str, u8, &str), usize> = HashMap::new();
    for (i, s) in signals.iter().enumerate() {
        let keys = [(0, s.model.as_deref()), (1, s.stem.as_deref())];
        for (kind, key) in keys {
            let Some(key) = key else { continue };
            let first = *seen.entry((s.guard.as_str(), kind, key)).or_insert(i);
            let (a, b) = (root(&mut parent, first), root(&mut parent, i));
            parent[b.max(a)] = b.min(a);
        }
    }
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut slot: HashMap<usize, usize> = HashMap::new();
    for i in 0..signals.len() {
        let r = root(&mut parent, i);
        let at = *slot.entry(r).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[at].push(i);
    }
    groups
}

/// A record name without its colourway numbers: `thp_light_helmet_01_01_12`
/// is `thp_light_helmet_01`. `None` for a name that isn't numbered that way.
fn record_stem(record: &str) -> Option<&str> {
    let name = record
        .strip_prefix("EntityClassDefinition.")
        .unwrap_or(record);
    let numbered = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let (rest, last) = name.rsplit_once('_')?;
    let (stem, before) = rest.rsplit_once('_')?;
    let (_, model) = stem.rsplit_once('_')?;
    (numbered(last) && numbered(before) && numbered(model)).then_some(stem)
}

/// The name a family reads under: the words its members share, when that
/// says what the item is, else its shortest member ("Aztalan" alone is not
/// an item, "Aztalan Helmet" is).
fn family_name(names: &[&str]) -> String {
    let mut shared: Vec<&str> = names[0].split_whitespace().collect();
    for name in &names[1..] {
        let same = shared
            .iter()
            .zip(name.split_whitespace())
            .take_while(|(a, b)| *a == b)
            .count();
        shared.truncate(same);
    }
    let joined = shared.join(" ");
    if shared.len() >= 2 || names.contains(&joined.as_str()) {
        return joined;
    }
    names
        .iter()
        .min_by_key(|n| (n.len(), **n))
        .map(|n| n.to_string())
        .unwrap_or_default()
}

/// What a member's name adds to its family's, quotes and brackets off.
/// `None` when it adds nothing.
fn variant_label(name: &str, family: &str) -> Option<String> {
    let mut family_words: Vec<&str> = family.split_whitespace().collect();
    let rest: Vec<&str> = name
        .split_whitespace()
        .filter(|w| match family_words.iter().position(|f| f == w) {
            Some(at) => {
                family_words.remove(at);
                false
            }
            None => true,
        })
        .collect();
    let label = rest
        .join(" ")
        .trim_matches(|c| matches!(c, '"' | '(' | ')'))
        .to_owned();
    (!label.is_empty()).then_some(label)
}

/// Blueprint GUID → the missions whose reward pools hold it, once each.
fn grants_by_blueprint(missions: &[MissionEntry]) -> HashMap<&str, Vec<GrantingMission>> {
    fold_grants(missions.iter().flat_map(|m| {
        let title = m.title.as_deref().unwrap_or(&m.debug_name);
        m.blueprint_rewards
            .iter()
            .flat_map(|p| &p.blueprints)
            .map(move |e| {
                (
                    e.blueprint_record_guid.as_str(),
                    m.mission_id.as_str(),
                    title,
                )
            })
    }))
}

/// `(blueprint, mission id, title)` triples, folded per blueprint. CIG
/// offers one contract per place, so a title counts once.
fn fold_grants<'a>(
    grants: impl Iterator<Item = (&'a str, &'a str, &'a str)>,
) -> HashMap<&'a str, Vec<GrantingMission>> {
    let mut out: HashMap<&str, Vec<GrantingMission>> = HashMap::new();
    for (blueprint, mission_id, title) in grants {
        let list = out.entry(blueprint).or_default();
        if !list.iter().any(|g| g.title == title) {
            list.push(GrantingMission {
                mission_id: mission_id.to_string(),
                title: title.to_string(),
            });
        }
    }
    for list in out.values_mut() {
        list.sort_by(|a, b| a.title.cmp(&b.title));
    }
    out
}

struct Context<'a> {
    foundations: &'a Foundations,
    categories: &'a Categories,
    properties: &'a GameplayProperties,
    sheets: &'a Sheets,
    locale: &'a LocaleMap,
    grants: &'a HashMap<&'a str, Vec<GrantingMission>>,
}

impl Context<'_> {
    fn entry(&self, b: &Blueprint) -> Option<BlueprintEntry> {
        let recipe = b.tiers.first()?.recipe.as_ref()?;
        let entity = b.crafted_entity_guid();
        let item = entity.as_ref().and_then(|e| self.foundations.items.get(e));
        let mut raw = Vec::new();
        if let Some(mandatory) = recipe.costs.as_ref().and_then(|c| c.mandatory.as_ref()) {
            collect_slots(mandatory, &mut raw);
        }
        let guid = b.blueprint_record_guid.to_string();
        Some(BlueprintEntry {
            name: b
                .display_name(self.locale)
                .map(str::to_owned)
                .unwrap_or_else(|| self.record_name(&b.blueprint_record_guid)),
            entity_guid: entity.as_ref().map(Guid::to_string),
            family: guid.clone(),
            family_name: String::new(),
            variant: None,
            category: b.category.as_ref().and_then(|c| {
                let name = &self.categories.get(c)?.name;
                Some(
                    name.strip_prefix("BlueprintCategoryRecord.")
                        .unwrap_or(name)
                        .to_owned(),
                )
            }),
            item_type: item
                .map(|i| i.item_type.as_dcb_str().to_owned())
                .unwrap_or_default(),
            sub_type: item
                .map(|i| i.item_sub_type.as_dcb_str().to_owned())
                .unwrap_or_default(),
            size: item.map_or(0, |i| i.size),
            grade: item.map_or(0, |i| i.grade),
            class: item.and_then(|i| match i.class(self.locale)? {
                ItemClass::Other(_) => None,
                c => Some(format!("{c:?}")),
            }),
            manufacturer: item.and_then(|i| self.manufacturer(i)),
            craft_seconds: recipe.craft_time.as_ref().map(|d| d.to_seconds()),
            stats: self.stats(&raw, entity.as_ref()),
            slots: raw.iter().map(|s| self.slot(s)).collect(),
            granted_by: self.grants.get(guid.as_str()).cloned().unwrap_or_default(),
            guid,
        })
    }

    /// The record's own name: the last resort for a blueprint the game
    /// hasn't named.
    fn record_name(&self, guid: &Guid) -> String {
        self.foundations
            .paths
            .get(guid)
            .map(|r| {
                r.name
                    .strip_prefix("CraftingBlueprintRecord.")
                    .unwrap_or(&r.name)
                    .to_owned()
            })
            .unwrap_or_else(|| guid.to_string())
    }

    /// The `Manufacturer:` line of an item's description.
    fn manufacturer(&self, item: &Item) -> Option<String> {
        let desc = self.locale.resolve(item.desc_key.as_ref()?)?;
        manufacturer_line(desc)
    }

    fn name_of(&self, key: Option<&LocaleKey>) -> Option<String> {
        self.locale
            .resolve(key?)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    }

    fn slot(&self, raw: &RawSlot) -> BlueprintSlot {
        let name = |key: Option<&LocaleKey>| self.name_of(key).unwrap_or_else(|| "Unknown".into());
        let (material, scu, count, min_quality) = match raw.leaf {
            Cost::Resource(rc) => (
                name(
                    rc.resource
                        .as_ref()
                        .and_then(|g| self.foundations.resources.get(g))
                        .map(|r| &r.name_key),
                ),
                rc.quantity.as_ref().and_then(|q| q.to_scu()),
                None,
                rc.min_quality,
            ),
            Cost::Item(ic) => (
                name(
                    ic.entity_class
                        .as_ref()
                        .and_then(|g| self.foundations.items.name_key(g)),
                ),
                None,
                Some(ic.quantity),
                ic.min_quality,
            ),
            _ => ("Unknown".into(), None, None, 0),
        };
        BlueprintSlot {
            name: raw.name.and_then(|n| {
                self.name_of(Some(&n.display_name))
                    .filter(|s| !s.contains("PLACEHOLDER"))
            }),
            material,
            scu,
            count,
            min_quality,
            modifiers: raw.modifiers.iter().map(|m| self.modifier(m)).collect(),
        }
    }

    fn modifier(&self, m: &GameplayPropertyModifier) -> SlotModifier {
        let prop = m
            .gameplay_property
            .as_ref()
            .and_then(|g| self.properties.get(g));
        SlotModifier {
            property: m.gameplay_property.as_ref().map(Guid::to_string),
            name: prop.and_then(|p| self.name_of(Some(&p.property_name_key))),
            transform: match prop.and_then(|p| p.display_transformation.as_ref()) {
                Some(DisplayTransformation::ConvertFactorToPercentChange) => {
                    ModifierTransform::FactorToPercent
                }
                Some(DisplayTransformation::ConvertFactorToNegatedPercentChange) => {
                    ModifierTransform::FactorToNegatedPercent
                }
                _ => ModifierTransform::Raw,
            },
            ranges: m.value_ranges.iter().filter_map(range).collect(),
        }
    }

    /// The crafted item's full sheet, each row the recipe drives linked to
    /// its property; properties with no absolute value (tractor beams,
    /// hull scraping) follow as percent-only rows.
    fn stats(&self, raw: &[RawSlot], entity: Option<&Guid>) -> Vec<BlueprintStat> {
        let mut seen = HashSet::new();
        let mut driven: Vec<Guid> = Vec::new();
        let mut by_stat: HashMap<String, Guid> = HashMap::new();
        for m in raw.iter().flat_map(|s| &s.modifiers) {
            if m.value_ranges.is_empty() {
                continue;
            }
            if let Some(g) = m.gameplay_property
                && seen.insert(g)
            {
                driven.push(g);
                if let Some(p) = self.properties.get(&g) {
                    by_stat.entry(stat_key(&p.stat())).or_insert(g);
                }
            }
        }

        let rows = entity.map(|e| self.sheets.rows_for(e)).unwrap_or_default();
        let mut covered = HashSet::new();
        let mut out = Vec::with_capacity(rows.len() + driven.len());
        for r in rows {
            let key = r.stat.as_ref().map(stat_key);
            let property = key
                .as_ref()
                .and_then(|k| by_stat.get(k))
                .map(Guid::to_string);
            covered.extend(key);
            out.push(BlueprintStat {
                group: r.group.map(str::to_owned),
                label: r.label,
                property,
                unit: r.unit.to_owned(),
                higher_is_better: r.stat.as_ref().and_then(higher_is_better),
                base: Some(r.base),
            });
        }
        for g in driven {
            let prop = self.properties.get(&g);
            let stat = prop.map(|p| p.stat());
            if stat
                .as_ref()
                .is_some_and(|s| covered.contains(&stat_key(s)))
            {
                continue;
            }
            out.push(BlueprintStat {
                group: None,
                label: prop
                    .and_then(|p| self.name_of(Some(&p.property_name_key)))
                    .unwrap_or_else(|| "Effect".to_owned()),
                property: Some(g.to_string()),
                unit: String::new(),
                higher_is_better: stat.as_ref().and_then(higher_is_better),
                base: None,
            });
        }
        out
    }
}

fn manufacturer_line(desc: &str) -> Option<String> {
    // The game's descriptions carry literal `\n` escapes.
    desc.replace("\\n", "\n")
        .lines()
        .find_map(|l| l.trim().strip_prefix("Manufacturer:"))
        .map(|m| m.trim().to_owned())
        .filter(|m| !m.is_empty())
}

fn range(vr: &ValueRange) -> Option<ModifierRange> {
    match vr {
        ValueRange::Linear {
            start_quality,
            end_quality,
            modifier_at_start,
            modifier_at_end,
        } => Some(ModifierRange {
            additive: false,
            start_quality: *start_quality,
            end_quality: *end_quality,
            at_start: *modifier_at_start,
            at_end: *modifier_at_end,
        }),
        ValueRange::LinearIntegerAdditive {
            start_quality,
            end_quality,
            additive_at_start,
            additive_at_end,
        } => Some(ModifierRange {
            additive: true,
            start_quality: *start_quality,
            end_quality: *end_quality,
            at_start: *additive_at_start as f32,
            at_end: *additive_at_end as f32,
        }),
        ValueRange::Other { .. } => None,
    }
}

/// A material slot in the cost tree, borrowed before names resolve.
struct RawSlot<'a> {
    name: Option<&'a SlotName>,
    leaf: &'a Cost,
    modifiers: Vec<&'a GameplayPropertyModifier>,
}

/// The game shapes a recipe as `Select { N, [Select { 1, [leaf] }] }`: the
/// outer select groups the slots, each inner one is a named slot holding its
/// material. Any select directly holding a leaf is a slot; the rest are
/// walked through. A slot's first leaf wins (the game offers one today).
fn collect_slots<'a>(cost: &'a Cost, out: &mut Vec<RawSlot<'a>>) {
    match cost {
        Cost::Select {
            name_info, options, ..
        } => {
            if let Some(leaf) = options.iter().find(|o| is_leaf(o)) {
                out.push(RawSlot {
                    name: name_info.as_ref(),
                    leaf,
                    modifiers: cost.gameplay_property_modifiers(),
                });
            } else {
                for option in options {
                    collect_slots(option, out);
                }
            }
        }
        Cost::Resource(_) | Cost::Item(_) => out.push(RawSlot {
            name: None,
            leaf: cost,
            modifiers: cost.gameplay_property_modifiers(),
        }),
        Cost::Other { .. } => {}
    }
}

fn is_leaf(cost: &Cost) -> bool {
    matches!(cost, Cost::Resource(_) | Cost::Item(_))
}

/// Which way a value is better. Unlike a factor's direction
/// ([`crate::crafting`]), this judges the value itself: the minimum
/// temperature is better lower. Radar assist stays open, as it does there.
fn higher_is_better(stat: &GameplayStat) -> Option<bool> {
    use GameplayStat::*;
    Some(match stat {
        WeaponFireRate
        | WeaponDamage
        | ArmorDamageMitigation
        | ArmorTemperatureMax
        | ArmorRadiationDissipation
        | Integrity
        | QuantumSpeed
        | ShieldMaxHealth
        | CoolantGeneration
        | PowerGeneration => true,
        WeaponRecoilKick
        | WeaponRecoilHandling
        | WeaponRecoilSmoothness
        | WeaponSpread
        | QuantumFuelRequirement
        | ArmorTemperatureMin => false,
        RadarMinAimAssist | RadarMaxAimAssist | Unknown(_) => return None,
    })
}

/// [`GameplayStat`] isn't hashable upstream; its debug form is stable.
fn stat_key(stat: &GameplayStat) -> String {
    format!("{stat:?}")
}

/// A base-stat row before the recipe's properties are laid over it.
struct BaseRow {
    group: Option<&'static str>,
    label: String,
    unit: &'static str,
    stat: Option<GameplayStat>,
    base: f32,
}

fn row(
    group: Option<&'static str>,
    label: impl Into<String>,
    unit: &'static str,
    stat: Option<GameplayStat>,
    base: f32,
) -> BaseRow {
    BaseRow {
        group,
        label: label.into(),
        unit,
        stat,
        base,
    }
}

impl Sheets {
    fn rows_for(&self, entity: &Guid) -> Vec<BaseRow> {
        if let Some(s) = self.fps_weapons.get(entity) {
            fps_weapon_rows(s)
        } else if let Some(s) = self.armor.get(entity) {
            armor_rows(s)
        } else if let Some(s) = self.ship_components.get(entity) {
            ship_component_rows(s)
        } else if let Some(s) = self.ship_weapons.get(entity) {
            ship_weapon_rows(s)
        } else {
            Vec::new()
        }
    }
}

/// The largest damage type's name; the figure stays the total, which is
/// what the recipe's damage property scales.
fn dominant(kinds: [(&'static str, f32); 6]) -> &'static str {
    kinds
        .into_iter()
        .fold(
            ("", 0.0_f32),
            |best, cur| {
                if cur.1 > best.1 { cur } else { best }
            },
        )
        .0
}

fn damage_label(ty: &str, per: &str) -> String {
    if ty.is_empty() {
        format!("Damage / {per}")
    } else {
        format!("{ty} Damage / {per}")
    }
}

fn fps_kinds(d: &FpsDamage) -> [(&'static str, f32); 6] {
    [
        ("Physical", d.physical),
        ("Energy", d.energy),
        ("Distortion", d.distortion),
        ("Thermal", d.thermal),
        ("Biochemical", d.biochemical),
        ("Stun", d.stun),
    ]
}

fn ship_kinds(d: &ShipDamage) -> [(&'static str, f32); 6] {
    [
        ("Physical", d.physical),
        ("Energy", d.energy),
        ("Distortion", d.distortion),
        ("Thermal", d.thermal),
        ("Biochemical", d.biochemical),
        ("Stun", d.stun),
    ]
}

fn fps_weapon_rows(s: &FpsWeaponStats) -> Vec<BaseRow> {
    use GameplayStat::*;
    let mut out = Vec::new();
    if let Some(v) = s.fire_rate {
        out.push(row(
            None,
            "Fire Rate",
            " RPM",
            Some(WeaponFireRate),
            v as f32,
        ));
    }
    if let Some(d) = &s.damage {
        let label = damage_label(dominant(fps_kinds(d)), "Shot");
        out.push(row(None, label, "", Some(WeaponDamage), d.total()));
    }
    if let Some(v) = s.recoil_pitch {
        out.push(row(None, "Recoil Pitch", "°", Some(WeaponRecoilKick), v));
    }
    if let Some(v) = s.recoil_yaw {
        out.push(row(None, "Recoil Yaw", "°", Some(WeaponRecoilHandling), v));
    }
    if let Some(v) = s.recoil_smooth {
        out.push(row(
            None,
            "Recoil Smoothing",
            " s",
            Some(WeaponRecoilSmoothness),
            v,
        ));
    }
    if let Some(v) = s.spread_max {
        out.push(row(None, "Spread", "°", Some(WeaponSpread), v));
    }
    if let Some(v) = s.ammo_speed {
        out.push(row(None, "Ammo Speed", " m/s", None, v));
    }
    if let Some(v) = s.mag_size {
        out.push(row(None, "Magazine", "", None, v as f32));
    }
    out
}

fn armor_rows(s: &ArmorStats) -> Vec<BaseRow> {
    use GameplayStat::*;
    let mut out = Vec::new();
    if let Some(dr) = &s.damage_resistance {
        for (name, entry) in resistance_entries(dr) {
            if let Some(e) = entry {
                // The mitigated share: a damage-taken multiplier of 0.6 is 40%.
                out.push(row(
                    Some("Damage Resistance"),
                    name,
                    "%",
                    Some(ArmorDamageMitigation),
                    (1.0 - e.multiplier) * 100.0,
                ));
            }
        }
    }
    if let Some(v) = s.temp_resistance_min {
        out.push(row(
            Some("Temperature"),
            "Min",
            "°C",
            Some(ArmorTemperatureMin),
            v,
        ));
    }
    if let Some(v) = s.temp_resistance_max {
        out.push(row(
            Some("Temperature"),
            "Max",
            "°C",
            Some(ArmorTemperatureMax),
            v,
        ));
    }
    if let Some(v) = s.radiation_dissipation {
        out.push(row(
            Some("Radiation"),
            "Dissipation",
            " REM/s",
            Some(ArmorRadiationDissipation),
            v,
        ));
    }
    if let Some(v) = s.radiation_capacity {
        out.push(row(Some("Radiation"), "Capacity", " REM", None, v));
    }
    out
}

fn resistance_entries(dr: &DamageResistance) -> [(&'static str, &Option<ResistanceEntry>); 6] {
    [
        ("Physical", &dr.physical),
        ("Energy", &dr.energy),
        ("Distortion", &dr.distortion),
        ("Thermal", &dr.thermal),
        ("Biochemical", &dr.biochemical),
        ("Stun", &dr.stun),
    ]
}

fn ship_component_rows(s: &ShipComponentStats) -> Vec<BaseRow> {
    use GameplayStat::*;
    let mut out = Vec::new();
    let mut add = |label: &str, unit: &'static str, stat, value: Option<f32>| {
        if let Some(v) = value {
            out.push(row(None, label, unit, stat, v));
        }
    };
    add("Integrity", " HP", Some(Integrity), s.integrity_hp);
    add(
        "Quantum Speed",
        " Mm/s",
        Some(QuantumSpeed),
        s.quantum_drive_speed,
    );
    add(
        "Quantum Fuel / Distance",
        "",
        Some(QuantumFuelRequirement),
        s.quantum_fuel_requirement,
    );
    add(
        "Shield Capacity",
        " HP",
        Some(ShieldMaxHealth),
        s.shield_max_health,
    );
    add("Shield Regen", " HP/s", None, s.shield_regen);
    add(
        "Coolant Output",
        "/s",
        Some(CoolantGeneration),
        s.coolant_rate,
    );
    add(
        "Power Output",
        " pips",
        Some(PowerGeneration),
        s.power_output,
    );
    add(
        "Aim-Assist (Min)",
        " m",
        Some(RadarMinAimAssist),
        s.radar_aim_assist_min,
    );
    add(
        "Aim-Assist (Max)",
        " m",
        Some(RadarMaxAimAssist),
        s.radar_aim_assist_max,
    );
    out
}

fn ship_weapon_rows(s: &ShipWeaponStats) -> Vec<BaseRow> {
    use GameplayStat::*;
    let mut out = Vec::new();
    if let Some(v) = s.integrity_hp {
        out.push(row(None, "Integrity", " HP", Some(Integrity), v));
    }
    if let Some(d) = &s.damage {
        let per = if s.is_beam { "s" } else { "Shot" };
        let label = damage_label(dominant(ship_kinds(d)), per);
        out.push(row(None, label, "", Some(WeaponDamage), d.total()));
    }
    if let Some(v) = s.fire_rate {
        out.push(row(
            None,
            "Fire Rate",
            " RPM",
            Some(WeaponFireRate),
            v as f32,
        ));
    }
    if let Some(v) = s.ammo_speed {
        out.push(row(None, "Ammo Speed", " m/s", None, v));
    }
    out
}

#[cfg(test)]
mod probe {
    //! Ignored-by-default look at the catalog on a real raw extract.
    //!
    //! ```text
    //! STARLUME_REAL_BUILD_DIR="…\cache\builds\12913470" \
    //!   cargo test -p svc-data blueprints::probe -- --ignored --nocapture
    //! ```
    use std::collections::BTreeMap;

    use sc_holotable::asset::{AssetConfig, ExtractSnapshot};

    fn run() {
        let dir = std::path::PathBuf::from(
            std::env::var("STARLUME_REAL_BUILD_DIR").expect("set STARLUME_REAL_BUILD_DIR"),
        );
        let snap = ExtractSnapshot::load(&dir.join("extract.snap")).expect("extract.snap");
        let (asset_data, datacore) = snap
            .hydrate(&AssetConfig::standard())
            .expect("datacore from the snapshot");
        let foundations = sc_holotable::build_foundations(&datacore);
        let missions = crate::missions::build_missions(
            &datacore,
            &foundations.items,
            &foundations.resources,
            &asset_data.locale,
        );
        let index = super::build_blueprints(&datacore, &foundations, &asset_data.locale, &missions);

        let all = &index.blueprints;
        eprintln!("{} blueprints, stock Q{}", all.len(), index.default_quality);
        let mut groups: BTreeMap<String, usize> = BTreeMap::new();
        for b in all {
            let key = format!(
                "{} | {} | {}",
                b.category.as_deref().unwrap_or("-"),
                b.item_type,
                b.sub_type
            );
            *groups.entry(key).or_default() += 1;
        }
        for (k, n) in &groups {
            eprintln!("{n:>5}  {k}");
        }
        let count =
            |f: &dyn Fn(&super::BlueprintEntry) -> bool| all.iter().filter(|b| f(b)).count();
        eprintln!("no slots        {}", count(&|b| b.slots.is_empty()));
        eprintln!("no stats        {}", count(&|b| b.stats.is_empty()));
        eprintln!("no manufacturer {}", count(&|b| b.manufacturer.is_none()));
        eprintln!("no class        {}", count(&|b| b.class.is_none()));
        eprintln!("not granted     {}", count(&|b| b.granted_by.is_empty()));
        eprintln!(
            "unknown material {}",
            count(&|b| b.slots.iter().any(|s| s.material == "Unknown"))
        );
        eprintln!(
            "item materials  {}",
            count(&|b| b.slots.iter().any(|s| s.count.is_some()))
        );
        eprintln!(
            "unnamed         {}",
            count(&|b| b.name.starts_with("BP_") || b.name.contains("PLACEHOLDER"))
        );
        for name in ["Cryo-Star SL", "Geist Armor Core"] {
            if let Some(b) = all.iter().find(|b| b.name.starts_with(name)) {
                eprintln!("{b:#?}");
            }
        }
        let bytes = rmp_size(&index);
        eprintln!("serialized {bytes} bytes");
    }

    fn rmp_size(index: &super::BlueprintIndex) -> usize {
        serde_json::to_vec(index).map_or(0, |v| v.len())
    }

    /// What the catalog's filter fields hold, from a cooked build.
    #[test]
    #[ignore = "needs a cooked build: set STARLUME_REAL_BUILD_DIR"]
    fn filter_values() {
        let dir = std::path::PathBuf::from(
            std::env::var("STARLUME_REAL_BUILD_DIR").expect("set STARLUME_REAL_BUILD_DIR"),
        );
        let snap = sc_holotable::asset::ProcessedSnapshot::<super::BlueprintIndex>::load(
            &dir.join("blueprints.cook"),
            crate::cooked::DATA_COOK_VERSION,
        )
        .expect("blueprints.cook");
        let mut seen: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        let mut families = std::collections::BTreeSet::new();
        for b in &snap.into_index().blueprints {
            let key = format!(
                "{} | {} | {} | grade {}",
                b.item_type,
                b.sub_type,
                b.class.as_deref().unwrap_or("-"),
                b.grade
            );
            let e = seen.entry(key.clone()).or_default();
            e.0 += 1;
            if families.insert((key, b.family.clone())) {
                e.1 += 1;
            }
        }
        for (k, (n, f)) in seen {
            eprintln!("{n:>5} {f:>5}  {k}");
        }
    }

    /// Which grouping signals agree, over the real blueprints.
    fn grouping() {
        use sc_holotable::asset::{Guid, RecordCollection};
        use sc_holotable::items::ItemCatalog;
        use sc_holotable::visuals::Visuals;
        use std::collections::{BTreeMap, BTreeSet};
        use std::str::FromStr;

        let dir = std::path::PathBuf::from(
            std::env::var("STARLUME_REAL_BUILD_DIR").expect("set STARLUME_REAL_BUILD_DIR"),
        );
        let snap = ExtractSnapshot::load(&dir.join("extract.snap")).expect("extract.snap");
        let (asset_data, datacore) = snap
            .hydrate(&AssetConfig::standard())
            .expect("datacore from the snapshot");
        let foundations = sc_holotable::build_foundations(&datacore);
        let locale = &asset_data.locale;
        let index = super::build_blueprints(&datacore, &foundations, locale, &[]);
        let catalog = ItemCatalog::build(&foundations.items, &foundations.paths, locale);
        let visuals = Visuals::build(&datacore, &foundations.items);

        struct R<'a> {
            b: &'a super::BlueprintEntry,
            model: String,
            geometry: String,
            record: String,
            recipe: String,
            stats: String,
        }
        let rows: Vec<R> = index
            .blueprints
            .iter()
            .map(|b| {
                let g = b
                    .entity_guid
                    .as_deref()
                    .and_then(|g| Guid::from_str(g).ok());
                let model = g
                    .as_ref()
                    .and_then(|g| catalog.model_id_of(g))
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("none:{}", b.guid));
                let geometry = g
                    .as_ref()
                    .and_then(|g| visuals.get(g))
                    .and_then(|v| v.geometry.as_ref())
                    .and_then(|n| n.geometry.clone())
                    .unwrap_or_else(|| format!("none:{}", b.guid));
                let record = g
                    .as_ref()
                    .and_then(|g| foundations.paths.get(g))
                    .map(|r| r.name.clone())
                    .unwrap_or_default();
                R {
                    b,
                    model,
                    geometry,
                    record,
                    recipe: format!("{:?}|{:?}", b.craft_seconds, b.slots),
                    stats: format!("{:?}", b.stats),
                }
            })
            .collect();

        let group = |key: &dyn Fn(&R) -> String| {
            let mut m: BTreeMap<String, Vec<&R>> = BTreeMap::new();
            for r in &rows {
                m.entry(key(r)).or_default().push(r);
            }
            m
        };
        let distinct = |g: &[&R], f: &dyn Fn(&R) -> String| {
            g.iter().map(|r| f(r)).collect::<BTreeSet<_>>().len()
        };
        let by_model = group(&|r| r.model.clone());
        let by_geo = group(&|r| r.geometry.clone());
        eprintln!(
            "rows {}, by model {}, by geometry {}",
            rows.len(),
            by_model.len(),
            by_geo.len()
        );

        let show = |title: &str, list: Vec<&Vec<&R>>, f: &dyn Fn(&R) -> String| {
            eprintln!("--- {title}: {}", list.len());
            for g in list.iter().take(25) {
                eprintln!("  group of {}:", g.len());
                for r in g.iter().take(6) {
                    eprintln!(
                        "    {} [{}] {}",
                        r.b.name,
                        r.record,
                        f(r).chars().take(70).collect::<String>()
                    );
                }
            }
        };
        show(
            "models with more than one recipe",
            by_model
                .values()
                .filter(|g| distinct(g, &|r| r.recipe.clone()) > 1)
                .collect(),
            &|r| {
                format!(
                    "{:?}s {}",
                    r.b.craft_seconds,
                    r.b.slots
                        .iter()
                        .map(|s| s.material.as_str())
                        .collect::<Vec<_>>()
                        .join(",")
                )
            },
        );
        show(
            "models with more than one stat sheet",
            by_model
                .values()
                .filter(|g| distinct(g, &|r| r.stats.clone()) > 1)
                .collect(),
            &|r| r.stats.clone(),
        );
        show(
            "models with more than one geometry",
            by_model
                .values()
                .filter(|g| distinct(g, &|r| r.geometry.clone()) > 1)
                .collect(),
            &|r| r.geometry.clone(),
        );
        show(
            "geometries across more than one model",
            by_geo
                .values()
                .filter(|g| distinct(g, &|r| r.model.clone()) > 1)
                .collect(),
            &|r| r.model.clone(),
        );
        let guarded = group(&|r| format!("{}#{}#{}", r.model, r.recipe, r.stats));
        let geo_guarded = group(&|r| {
            format!(
                "{}#{}#{}#{}#{}",
                r.geometry, r.recipe, r.stats, r.b.item_type, r.b.size
            )
        });
        eprintln!(
            "model+recipe+stats {}, geometry+recipe+stats {}",
            guarded.len(),
            geo_guarded.len()
        );
        let both = group(&|r| format!("{}#{}#{}#{}", r.model, r.geometry, r.recipe, r.stats));
        eprintln!("model+geometry+recipe+stats {}", both.len());
        let mut sizes: Vec<&Vec<&R>> = guarded.values().collect();
        sizes.sort_by_key(|g| std::cmp::Reverse(g.len()));
        eprintln!("--- largest guarded groups");
        for g in sizes.iter().take(20) {
            let names: Vec<_> = g.iter().take(8).map(|r| r.b.name.as_str()).collect();
            eprintln!("  {:>3} {}", g.len(), names.join(" | "));
        }
        let mut shelves: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
        for g in guarded.values() {
            let e = shelves.entry(g[0].b.item_type.as_str()).or_default();
            e.0 += 1;
            e.1 += g.len();
        }
        for (k, (groups, n)) in shelves {
            eprintln!("  {k}: {n} blueprints in {groups} groups");
        }
    }

    #[test]
    #[ignore = "needs a real raw extract: set STARLUME_REAL_BUILD_DIR"]
    fn grouping_on_real_data() {
        std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(grouping)
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    #[ignore = "needs a real raw extract: set STARLUME_REAL_BUILD_DIR"]
    fn catalog_on_real_data() {
        // The parse recurses deeply: the loader's stack, not the default.
        std::thread::Builder::new()
            .stack_size(32 * 1024 * 1024)
            .spawn(run)
            .unwrap()
            .join()
            .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_manufacturer_comes_from_the_description_line() {
        let desc = r"Item Type: Cooler\nManufacturer: Aegis Dynamics \nSize: 1\n\nText.";
        assert_eq!(manufacturer_line(desc).as_deref(), Some("Aegis Dynamics"));
        assert_eq!(manufacturer_line("Just text."), None);
    }

    fn signals(guard: &str, model: Option<&str>, stem: Option<&str>) -> Signals {
        Signals {
            guard: guard.into(),
            model: model.map(str::to_owned),
            stem: stem.map(str::to_owned),
        }
    }

    #[test]
    fn either_signal_joins_a_family() {
        let groups = group_families(&[
            signals("r", Some("a"), Some("x")),
            signals("r", Some("b"), Some("x")),
            signals("r", Some("b"), None),
            signals("r", Some("c"), None),
        ]);
        assert_eq!(groups, [vec![0, 1, 2], vec![3]]);
    }

    #[test]
    fn a_different_recipe_never_joins() {
        let groups = group_families(&[
            signals("r1", Some("a"), Some("x")),
            signals("r2", Some("a"), Some("x")),
        ]);
        assert_eq!(groups, [vec![0], vec![1]]);
    }

    #[test]
    fn no_signal_stands_alone() {
        let groups = group_families(&[signals("r", None, None), signals("r", None, None)]);
        assert_eq!(groups, [vec![0], vec![1]]);
    }

    #[test]
    fn a_record_stem_drops_the_colourway_numbers() {
        assert_eq!(
            record_stem("EntityClassDefinition.thp_light_helmet_01_01_12"),
            Some("thp_light_helmet_01")
        );
        assert_eq!(record_stem("COOL_JSPN_S02_CryoStarEX_SCItem"), None);
        assert_eq!(record_stem("cds_helmet_01_crus01_01"), None);
    }

    #[test]
    fn a_family_reads_under_its_shared_words() {
        assert_eq!(
            family_name(&["ADP Arms Aqua", "ADP Arms Black"]),
            "ADP Arms"
        );
        assert_eq!(family_name(&["ADP Core", "ADP Core Aqua"]), "ADP Core");
        // One shared word isn't an item.
        assert_eq!(
            family_name(&["Aztalan Galena Helmet", "Aztalan Helmet"]),
            "Aztalan Helmet"
        );
        assert_eq!(
            family_name(&["A03 \"Canuto\" Sniper Rifle", "A03 Sniper Rifle"]),
            "A03 Sniper Rifle"
        );
    }

    #[test]
    fn a_variant_is_what_its_name_adds() {
        assert_eq!(
            variant_label("ADP Arms Snow Camo", "ADP Arms").as_deref(),
            Some("Snow Camo")
        );
        assert_eq!(
            variant_label("Aztalan Galena Helmet", "Aztalan Helmet").as_deref(),
            Some("Galena")
        );
        assert_eq!(
            variant_label("A03 \"Canuto\" Sniper Rifle", "A03 Sniper Rifle").as_deref(),
            Some("Canuto")
        );
        assert_eq!(
            variant_label("ADP Arms (Modified)", "ADP Arms").as_deref(),
            Some("Modified")
        );
        assert_eq!(variant_label("ADP Arms", "ADP Arms"), None);
    }

    #[test]
    fn a_range_keeps_factors_and_additions_apart() {
        let factor = range(&ValueRange::Linear {
            start_quality: 0,
            end_quality: 1000,
            modifier_at_start: 0.8,
            modifier_at_end: 1.2,
        })
        .unwrap();
        assert!(!factor.additive);
        let added = range(&ValueRange::LinearIntegerAdditive {
            start_quality: 0,
            end_quality: 1000,
            additive_at_start: 1,
            additive_at_end: 5,
        })
        .unwrap();
        assert!(added.additive);
        assert_eq!(added.at_end, 5.0);
    }

    #[test]
    fn a_blueprint_is_granted_once_per_mission_title() {
        let grants = fold_grants(
            [
                ("bp", "2", "Guard"),
                ("bp", "1", "Defend"),
                ("bp", "3", "Guard"),
            ]
            .into_iter(),
        );
        let titles: Vec<_> = grants["bp"].iter().map(|g| g.title.as_str()).collect();
        assert_eq!(titles, ["Defend", "Guard"]);
    }
}
