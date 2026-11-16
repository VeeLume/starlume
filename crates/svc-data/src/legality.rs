//! Legality index cooking: sc-holotable's typed jurisdictions (`sc-law`) →
//! per-commodity legality rows. Reference-catalog framework data (CONTRIBUTING
//! module rule 4): the same index feeds the resource catalog's "Illegal in …"
//! facts and mod-langpatch's illegal-goods derive.
//!
//! Ported from sc-langpatch's `illegal_goods.rs` collection pass (the patch
//! *decisions* — prefix/suffix rendering — stay in the consumer). Per the
//! snapshot philosophy ([`crate::cooked`]), rows keep **raw locale keys**;
//! display names resolve at query time. No raw DataCore here ('No raw
//! DataCore access in Starlume'): jurisdictions come from
//! `Foundations::jurisdictions`, names from `Resources` and `RecordPaths`.
//!
//! Sources, per jurisdiction:
//! - `prohibited_resources` → [`LegalityKind::Contraband`]
//! - `controlled_substances[].resources` → [`LegalityKind::Drug`]
//!
//! Drug wins when a resource appears in both (the langpatch precedence
//! rule). `prohibited_goods` and `controlled_substances[].commodities`
//! (items, not resources) are not modelled yet, nor the parent jurisdiction's
//! laws (`respects_parent_laws`, unverified in game): parity first.

use std::collections::HashMap;

use sc_holotable::asset::{Guid, RecordCollection, RecordPaths};
use sc_holotable::law::Jurisdictions;
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
    /// Record name without its type prefix (`ArcCorp`): a fallback label,
    /// never a join key.
    pub record_name: String,
}

/// One illegal commodity with everywhere it's outlawed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegalityEntry {
    /// `ResourceType` record GUID, hex-string form — joins the resource
    /// catalog.
    pub resource_guid: String,
    /// Record name without its type prefix (e.g. `"Widow"`).
    pub record_name: String,
    /// Raw locale key from `ResourceType.displayName` (`@`-prefixed as
    /// authored) — the commodity's INI name key. Its `_desc` sibling is the
    /// description key.
    pub name_key: String,
    pub kind: LegalityKind,
    /// Jurisdictions outlawing this commodity, sorted by record name.
    pub jurisdictions: Vec<JurisdictionRef>,
}

/// Cook the legality index from the foundations build the caller already
/// paid for. Every jurisdiction's resource references resolve in
/// `resources` (sc-holotable's probe asserts it, 66 of 66 on LIVE 4.10); one
/// that does not is skipped, not guessed at.
pub(crate) fn build_legality(
    jurisdictions: &Jurisdictions,
    resources: &Resources,
    paths: &RecordPaths,
) -> Vec<LegalityEntry> {
    let label = |guid: &Guid| paths.get(guid).map(|p| p.short_name().to_string());
    let mut by_guid: HashMap<Guid, LegalityEntry> = HashMap::new();

    for (guid, j) in jurisdictions.iter() {
        let jur = JurisdictionRef {
            name_key: j
                .name_key
                .as_ref()
                .map(|k| k.as_str().to_string())
                .filter(|k| !k.is_empty()),
            record_name: label(guid).unwrap_or_else(|| "Unknown".to_string()),
        };
        for g in &j.prohibited_resources {
            add(
                resources,
                &label,
                *g,
                LegalityKind::Contraband,
                &jur,
                &mut by_guid,
            );
        }
        for class in &j.controlled_substances {
            for g in &class.resources {
                add(
                    resources,
                    &label,
                    *g,
                    LegalityKind::Drug,
                    &jur,
                    &mut by_guid,
                );
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
    resources: &Resources,
    label: &impl Fn(&Guid) -> Option<String>,
    guid: Guid,
    kind: LegalityKind,
    jurisdiction: &JurisdictionRef,
    out: &mut HashMap<Guid, LegalityEntry>,
) {
    let Some(resource) = resources.get(&guid) else {
        return;
    };
    let name_key = resource.name_key.as_str().to_string();
    let Some(record_name) = label(&guid) else {
        return;
    };
    if name_key.is_empty() || record_name.is_empty() {
        return;
    }

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
