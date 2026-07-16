//! Legality index cooking — `Jurisdiction` records → per-commodity legality
//! rows. Reference-catalog framework data (README module rule 4): the same
//! index feeds the resource catalog's "Illegal in …" facts and
//! mod-langpatch's illegal-goods derive.
//!
//! Ported from sc-langpatch's `illegal_goods.rs` collection pass (the patch
//! *decisions* — prefix/suffix rendering — stay in the consumer). Per the
//! snapshot philosophy ([`crate::cooked`]), rows keep **raw locale keys**;
//! display names resolve at query time.
//!
//! Sources, per jurisdiction:
//! - `prohibitedResources[]` → [`LegalityKind::Contraband`]
//! - `controlledSubstanceClasses[].resources[]` → [`LegalityKind::Drug`]
//!
//! Drug wins when a resource appears in both (the langpatch precedence
//! rule). `prohibitedGoods[]` and `controlledSubstanceClasses[].commodities`
//! are not modelled yet — parity first; extend when a consumer needs them.

use std::collections::HashMap;

use sc_holotable::asset::{Datacore, Guid, RecordCollection};
use sc_holotable::resources::Resources;
use serde::{Deserialize, Serialize};

/// Why a commodity is illegal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LegalityKind {
    /// Controlled substance (`controlledSubstanceClasses`).
    Drug,
    /// Prohibited good (`prohibitedResources`).
    Contraband,
}

/// One jurisdiction that outlaws a commodity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JurisdictionRef {
    /// Raw locale key of the jurisdiction name (`@`-prefixed as authored);
    /// `None` when the record carries an empty key.
    pub name_key: Option<String>,
    /// DCB record name — stable fallback label + cross-ref.
    pub record_name: String,
}

/// One illegal commodity with everywhere it's outlawed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegalityEntry {
    /// `ResourceType` record GUID, hex-string form — joins the resource
    /// catalog.
    pub resource_guid: String,
    /// DCB record name (e.g. `"WiDoW"`).
    pub record_name: String,
    /// Raw locale key from `ResourceType.displayName` (`@`-prefixed as
    /// authored) — the commodity's INI name key. Its `_desc` sibling is the
    /// description key.
    pub name_key: String,
    pub kind: LegalityKind,
    /// Jurisdictions outlawing this commodity, sorted by record name.
    pub jurisdictions: Vec<JurisdictionRef>,
}

/// Cook the legality index from one parsed `Datacore`. `resources` comes
/// from the foundations build the caller already paid for (primary name-key
/// source; raw `displayName` walk as fallback for refs outside the index).
pub(crate) fn build_legality(datacore: &Datacore, resources: &Resources) -> Vec<LegalityEntry> {
    let store = datacore.records();
    let db = datacore.db();

    let mut by_guid: HashMap<Guid, LegalityEntry> = HashMap::new();

    for (&jur_guid, &handle) in &store.records.multi_feature.jurisdiction {
        let Some(j) = handle.get(&store.pools) else {
            continue;
        };
        let jur = JurisdictionRef {
            name_key: {
                let key = j.name.as_str();
                (!key.is_empty()).then(|| key.to_string())
            },
            record_name: db
                .record(&jur_guid)
                .and_then(|r| r.name())
                .unwrap_or("Unknown")
                .to_string(),
        };

        for guid in &j.prohibited_resources {
            add(
                db,
                resources,
                *guid,
                LegalityKind::Contraband,
                &jur,
                &mut by_guid,
            );
        }
        for class_handle in &j.controlled_substance_classes {
            let Some(class) = class_handle.get(&store.pools) else {
                continue;
            };
            for guid in &class.resources {
                add(db, resources, *guid, LegalityKind::Drug, &jur, &mut by_guid);
            }
        }
    }

    let mut out: Vec<LegalityEntry> = by_guid.into_values().collect();
    for entry in &mut out {
        entry
            .jurisdictions
            .sort_by(|a, b| a.record_name.cmp(&b.record_name));
    }
    out.sort_by(|a, b| {
        a.record_name
            .cmp(&b.record_name)
            .then_with(|| a.resource_guid.cmp(&b.resource_guid))
    });
    out
}

/// Resolve one prohibited/controlled reference and merge it into the map.
/// Drug takes precedence over contraband when a resource appears as both.
fn add(
    db: &sc_holotable::asset::DataCoreDatabase,
    resources: &Resources,
    guid: Guid,
    kind: LegalityKind,
    jurisdiction: &JurisdictionRef,
    out: &mut HashMap<Guid, LegalityEntry>,
) {
    let record = db.record(&guid);
    let record_name = record
        .as_ref()
        .and_then(|r| r.name())
        .unwrap_or("")
        .to_string();
    // Name key: resource index first, raw `displayName` walk as fallback.
    let name_key = resources
        .get(&guid)
        .map(|r| r.name_key.as_str().to_string())
        .or_else(|| {
            record
                .as_ref()
                .and_then(|r| r.as_instance().get_str("displayName"))
                .map(str::to_string)
        })
        .filter(|k| !k.is_empty());
    let (Some(name_key), false) = (name_key, record_name.is_empty()) else {
        return;
    };

    let entry = out.entry(guid).or_insert_with(|| LegalityEntry {
        resource_guid: guid.to_string(),
        record_name,
        name_key,
        kind,
        jurisdictions: Vec::new(),
    });
    if kind == LegalityKind::Drug {
        entry.kind = LegalityKind::Drug;
    }
    if !entry.jurisdictions.contains(jurisdiction) {
        entry.jurisdictions.push(jurisdiction.clone());
    }
}
