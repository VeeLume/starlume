//! The cooked payload svc-data caches per build, plus the query surface the
//! shell exposes to the frontend.
//!
//! [`CookedData`] wraps sc-holotable's [`HolotableSnapshot`] (every
//! foundational index, all-`Option`) and adds the one thing that bundle
//! deliberately doesn't carry: the [`LocaleMap`]. Domain indices keep raw
//! `LocaleKey`s; display names resolve at query time against the bundled
//! locale, so the snapshot stays patch-honest (names come from the same
//! `global.ini` the indices were cooked from).

use std::path::Path;
use std::str::FromStr;

use sc_holotable::asset::{Guid, LocaleMap, ProcessedSnapshot, RecordCollection, SnapshotMeta};
use sc_holotable::{HOLOTABLE_COOK_VERSION, HolotableSnapshot};
use serde::{Deserialize, Serialize};

/// Starlume's own cook revision. Bump when [`CookedData`]'s shape or the
/// query-relevant projection changes.
///
/// rev 2: mission catalog ([`CookedData::missions`]).
/// rev 3: mission loc keys + pool facts + crimestat; legality index
/// ([`CookedData::legality`]); weapons index ([`CookedData::weapons`]).
/// rev 4: `cooldown_seconds` actually in seconds (upstream feeds the
/// minutes-authored `personal_cooldown_time` into a seconds-named field;
/// the cook now converts).
pub const STARLUME_COOK_REV: u32 = 4;

/// The version guard for the processed snapshot on disk — composes the
/// upstream cook version so *either* bump invalidates cleanly.
pub const DATA_COOK_VERSION: u32 = STARLUME_COOK_REV * 1000 + HOLOTABLE_COOK_VERSION;

/// Cooked foundational indices + the locale they were cooked with.
/// Serialized whole into `foundations.cook` (see [`crate::cache`]).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CookedData {
    /// sc-holotable's bundled foundational indices (items, tags,
    /// manufacturers, resources, locations, gathering, record paths).
    pub holotable: HolotableSnapshot,
    /// Parsed `english/global.ini` — what [`HolotableSnapshot`] doesn't
    /// carry. Keys resolve with or without the leading `@`.
    pub locale: LocaleMap,
    /// The pooled mission catalog, fully resolved at cook time (see
    /// [`crate::missions`] for why missions don't defer to query time).
    #[serde(default)]
    pub missions: Vec<crate::missions::MissionEntry>,
    /// Per-commodity legality rows from `Jurisdiction` records (raw locale
    /// keys — see [`crate::legality`]).
    #[serde(default)]
    pub legality: Vec<crate::legality::LegalityEntry>,
    /// Ship weapons + missiles with combat stats (raw locale keys — see
    /// [`crate::weapons`]).
    #[serde(default)]
    pub weapons: crate::weapons::WeaponsIndex,
}

/// One page of item search results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemPage {
    /// Total matches before pagination.
    pub total: usize,
    pub rows: Vec<ItemRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemRow {
    pub guid: String,
    pub name: String,
    pub item_type: String,
    pub item_sub_type: String,
    pub size: i32,
    pub grade: i32,
}

/// Full per-item view for the detail panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemDetail {
    pub guid: String,
    pub name: String,
    pub short_name: Option<String>,
    pub description: Option<String>,
    pub item_type: String,
    pub item_sub_type: String,
    pub size: i32,
    pub grade: i32,
    /// DCB record path (`libs/foundry/records/...`), when known.
    pub record_path: Option<String>,
    /// Combat stats when this item is a ship weapon (the weapons-index
    /// join — README rule 4: enrichments are framework reference data).
    pub ship_weapon: Option<crate::weapons::ShipWeaponEntry>,
    /// Combat stats when this item is a missile / torpedo.
    pub missile: Option<crate::weapons::MissileEntry>,
}

/// A resource's legality verdict — the legality-index join, resolved for
/// display (kind + jurisdiction names).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLegality {
    pub kind: crate::legality::LegalityKind,
    /// Resolved jurisdiction names (record-name fallback).
    pub jurisdictions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRow {
    pub guid: String,
    pub name: String,
    pub description: Option<String>,
    /// Resolved name of the resource this one refines into, if any.
    pub refined_into: Option<String>,
    /// Density normalized to kg/m³, when the record carries one.
    pub density_kg_per_m3: Option<f32>,
    /// Drug/contraband verdict when any jurisdiction outlaws this resource.
    pub legality: Option<ResourceLegality>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManufacturerRow {
    pub guid: String,
    pub code: String,
    pub name: Option<String>,
}

/// Item search parameters. `text` matches case-insensitively against the
/// resolved display name and the GUID; empty text matches everything.
#[derive(Debug, Clone, Default)]
pub struct ItemQuery {
    pub text: String,
    /// Exact DCB type string (from [`CookedData::item_type_facets`]), if
    /// filtering by type.
    pub item_type: Option<String>,
    pub offset: usize,
    pub limit: usize,
}

impl CookedData {
    /// Resolve a locale key to display text, falling back to the raw key
    /// with its `@` stripped (better a key than a blank cell).
    fn resolve_or_key<'a>(&'a self, key: &'a str) -> &'a str {
        self.locale
            .resolve(key)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| key.strip_prefix('@').unwrap_or(key))
    }

    /// Number of real inventory items (the search corpus).
    pub fn item_count(&self) -> usize {
        self.holotable
            .items
            .as_ref()
            .map(|items| items.values().filter(|i| i.is_inventory_item()).count())
            .unwrap_or(0)
    }

    pub fn resource_count(&self) -> usize {
        self.holotable
            .resources
            .as_ref()
            .map(RecordCollection::len)
            .unwrap_or(0)
    }

    /// Number of pooled mission templates in the catalog.
    pub fn mission_count(&self) -> usize {
        self.missions.len()
    }

    /// Search inventory items. Results sort by resolved name (then GUID for
    /// a stable tiebreak), so pagination is deterministic across calls.
    pub fn search_items(&self, query: &ItemQuery) -> ItemPage {
        let Some(items) = self.holotable.items.as_ref() else {
            return ItemPage {
                total: 0,
                rows: Vec::new(),
            };
        };
        let needle = query.text.to_lowercase();
        let mut matches: Vec<(String, String, &sc_holotable::items::Item)> = items
            .iter()
            .filter(|(_, item)| item.is_inventory_item())
            .filter(|(_, item)| match &query.item_type {
                Some(t) => item.item_type.as_dcb_str() == t,
                None => true,
            })
            .filter_map(|(guid, item)| {
                let name = item
                    .name_key
                    .as_ref()
                    .map(|k| self.resolve_or_key(k.as_str()).to_string())?;
                let guid = guid.to_string();
                (needle.is_empty()
                    || name.to_lowercase().contains(&needle)
                    || guid.to_lowercase().contains(&needle))
                .then_some((name, guid, item))
            })
            .collect();
        matches.sort_by(|a, b| (a.0.to_lowercase(), &a.1).cmp(&(b.0.to_lowercase(), &b.1)));

        let total = matches.len();
        let rows = matches
            .into_iter()
            .skip(query.offset)
            .take(query.limit)
            .map(|(name, guid, item)| ItemRow {
                guid,
                name,
                item_type: item.item_type.as_dcb_str().to_string(),
                item_sub_type: item.item_sub_type.as_dcb_str().to_string(),
                size: item.size,
                grade: item.grade,
            })
            .collect();
        ItemPage { total, rows }
    }

    /// Full detail for one item. `None` on an unknown or unparsable GUID.
    pub fn item_detail(&self, guid: &str) -> Option<ItemDetail> {
        let guid = Guid::from_str(guid).ok()?;
        let item = self.holotable.items.as_ref()?.get(&guid)?;
        Some(ItemDetail {
            guid: guid.to_string(),
            name: item
                .name_key
                .as_ref()
                .map(|k| self.resolve_or_key(k.as_str()).to_string())
                .unwrap_or_else(|| guid.to_string()),
            short_name: item
                .short_name_key
                .as_ref()
                .map(|k| self.resolve_or_key(k.as_str()).to_string()),
            description: item
                .desc_key
                .as_ref()
                .map(|k| self.resolve_or_key(k.as_str()).to_string()),
            item_type: item.item_type.as_dcb_str().to_string(),
            item_sub_type: item.item_sub_type.as_dcb_str().to_string(),
            size: item.size,
            grade: item.grade,
            record_path: self
                .holotable
                .paths
                .as_ref()
                .and_then(|p| p.get(&guid))
                .map(|r| r.path.clone()),
            ship_weapon: {
                let g = guid.to_string();
                self.weapons
                    .ship_weapons
                    .iter()
                    .find(|w| w.guid == g)
                    .cloned()
            },
            missile: {
                let g = guid.to_string();
                self.weapons.missiles.iter().find(|m| m.guid == g).cloned()
            },
        })
    }

    /// Every resource, sorted by resolved name.
    pub fn resources(&self) -> Vec<ResourceRow> {
        let Some(resources) = self.holotable.resources.as_ref() else {
            return Vec::new();
        };
        // Legality index keyed by resource GUID for the per-row join.
        let legality_by_guid: std::collections::HashMap<&str, &crate::legality::LegalityEntry> =
            self.legality
                .iter()
                .map(|e| (e.resource_guid.as_str(), e))
                .collect();
        let mut rows: Vec<ResourceRow> = resources
            .values()
            .map(|r| ResourceRow {
                guid: r.guid.to_string(),
                name: self.resolve_or_key(r.name_key.as_str()).to_string(),
                description: {
                    let d = self.resolve_or_key(r.description_key.as_str());
                    (!d.is_empty()).then(|| d.to_string())
                },
                refined_into: r
                    .refined_version
                    .and_then(|next| resources.get(&next))
                    .map(|next| self.resolve_or_key(next.name_key.as_str()).to_string()),
                density_kg_per_m3: r
                    .density
                    .as_ref()
                    .and_then(|d| d.unit.as_ref())
                    .and_then(|u| u.to_kg_per_m3()),
                legality: legality_by_guid.get(r.guid.to_string().as_str()).map(|e| {
                    ResourceLegality {
                        kind: e.kind.clone(),
                        jurisdictions: e
                            .jurisdictions
                            .iter()
                            .map(|j| {
                                j.name_key
                                    .as_deref()
                                    .map(|k| self.resolve_or_key(k).to_string())
                                    .filter(|s| !s.is_empty())
                                    .unwrap_or_else(|| j.record_name.clone())
                            })
                            .collect(),
                    }
                }),
            })
            .collect();
        rows.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        rows
    }

    /// Every manufacturer, sorted by code.
    pub fn manufacturers(&self) -> Vec<ManufacturerRow> {
        let Some(manufacturers) = self.holotable.manufacturers.as_ref() else {
            return Vec::new();
        };
        let mut rows: Vec<ManufacturerRow> = manufacturers
            .values()
            .map(|m| ManufacturerRow {
                guid: m.guid.to_string(),
                code: m.code.clone(),
                name: m
                    .name_key
                    .as_ref()
                    .map(|k| self.resolve_or_key(k.as_str()).to_string()),
            })
            .collect();
        rows.sort_by(|a, b| a.code.cmp(&b.code));
        rows
    }

    /// Distinct item-type DCB strings over inventory items, with counts,
    /// sorted by count descending (then name). Feeds the type filter.
    pub fn item_type_facets(&self) -> Vec<(String, usize)> {
        let Some(items) = self.holotable.items.as_ref() else {
            return Vec::new();
        };
        let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for item in items.values().filter(|i| i.is_inventory_item()) {
            *counts.entry(item.item_type.as_dcb_str()).or_default() += 1;
        }
        let mut facets: Vec<(String, usize)> = counts
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        facets.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        facets
    }

    /// Serialize and write atomically (zstd + msgpack, version-guarded with
    /// [`DATA_COOK_VERSION`]).
    pub fn save(&self, meta: SnapshotMeta, path: &Path) -> sc_holotable::asset::Result<()> {
        ProcessedSnapshot::new(meta, DATA_COOK_VERSION, self.clone()).save(path)
    }
}
