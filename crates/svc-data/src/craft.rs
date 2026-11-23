//! What crafting does to an item: a recipe's quality curves evaluated at
//! the qualities of its material slots. Reference framework (CONTRIBUTING
//! module rule 4): Ships and Mining ask "what if this part were crafted at
//! Q" through [`Recipes`]; Crafting's sliders run the same maths in
//! TypeScript (`src/lib/modules/crafting/model.ts`), held to it by the
//! shared cases in `tests/craft_cases.json`.
//!
//! The rules (sc-crafting's, checked against scmdb): a band of a curve is
//! linear and clamped to its ends; a quality outside every band takes the
//! nearest one; factors of several slots add (`1 + Σ(fᵢ − 1)`: two −10%
//! slots are −20%, not −19%); additive bands add their amount.

use std::collections::HashMap;

use sc_holotable::crafting::GameplayStat;
use serde::{Deserialize, Serialize};

use crate::blueprints::{
    BlueprintEntry, BlueprintIndex, BlueprintSlot, BlueprintStat, ModifierRange,
};
use crate::crafting::CraftingIndex;

/// The highest quality a material has.
pub const MAX_QUALITY: i32 = 1000;

/// An `f32` as the decimal it prints as: the value the frontend gets
/// through JSON (`1.2`, not `1.2000000476…`), so both sides agree to the
/// last digit and a stock quality reads as no change at all.
pub fn wide(v: f32) -> f64 {
    v.to_string().parse().unwrap_or(f64::from(v))
}

/// One band's value at `q`: linear across it, clamped to its ends.
pub fn eval_range(r: &ModifierRange, q: f64) -> f64 {
    let (start, end) = (f64::from(r.start_quality), f64::from(r.end_quality));
    let (at_start, at_end) = (wide(r.at_start), wide(r.at_end));
    if end <= start {
        return at_start;
    }
    let clamped = q.clamp(start, end);
    at_start + (at_end - at_start) * (clamped - start) / (end - start)
}

/// A modifier's value at a quality.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Effect {
    pub value: f64,
    /// An added amount, not a factor.
    pub additive: bool,
}

/// A modifier at `q`: the band holding it, else the nearest (the first of
/// two as near). `None` without bands.
pub fn eval_modifier(ranges: &[ModifierRange], q: f64) -> Option<Effect> {
    let distance = |r: &ModifierRange| {
        let (start, end) = (f64::from(r.start_quality), f64::from(r.end_quality));
        if q < start {
            start - q
        } else {
            (q - end).max(0.0)
        }
    };
    let band = ranges.iter().reduce(|best, r| {
        if distance(r) < distance(best) {
            r
        } else {
            best
        }
    })?;
    Some(Effect {
        value: eval_range(band, q),
        additive: band.additive,
    })
}

/// What a recipe does to one property at its slots' qualities.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Change {
    /// `1 + Σ(fᵢ − 1)` over the factor bands; 1 when there are none.
    pub factor: f64,
    /// The sum of the additive bands.
    pub added: f64,
}

impl Change {
    pub const NONE: Self = Self {
        factor: 1.0,
        added: 0.0,
    };

    /// `base` crafted.
    pub fn apply(&self, base: f64) -> f64 {
        base * self.factor + self.added
    }

    pub fn is_none(&self) -> bool {
        (self.factor - 1.0).abs() <= 1e-6 && self.added.abs() <= 1e-6
    }
}

/// `property` over every slot, slot `i` at `qualities[i]` (a missing
/// quality is the last one given, so one quality fills every slot).
pub fn eval_property(property: &str, slots: &[BlueprintSlot], qualities: &[f64]) -> Change {
    let mut change = Change::NONE;
    for (i, slot) in slots.iter().enumerate() {
        let Some(&q) = qualities.get(i).or(qualities.last()) else {
            continue;
        };
        for m in &slot.modifiers {
            if m.property.as_deref() != Some(property) {
                continue;
            }
            let Some(e) = eval_modifier(&m.ranges, q) else {
                continue;
            };
            if e.additive {
                change.added += e.value;
            } else {
                change.factor += e.value - 1.0;
            }
        }
    }
    change
}

/// A stat-sheet row crafted.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StatResult {
    /// The value as crafted; `None` when the item has no absolute figure.
    pub value: Option<f64>,
    /// Percent change; `None` when unchanged.
    pub pct: Option<f64>,
    /// Better for the item; `None` when unchanged or the direction is
    /// unclear.
    pub improved: Option<bool>,
}

/// A stat-sheet row at each slot's quality: Crafting's `evalStat`.
pub fn eval_stat(stat: &BlueprintStat, slots: &[BlueprintSlot], qualities: &[f64]) -> StatResult {
    let change = stat
        .property
        .as_deref()
        .map_or(Change::NONE, |p| eval_property(p, slots, qualities));
    let base = stat.base.map(wide);
    let value = base.map(|b| change.apply(b));
    let changed = match (value, base) {
        (Some(v), Some(b)) => (v - b).abs() > 1e-6,
        _ => (change.factor - 1.0).abs() > 1e-6,
    };
    if !changed {
        return StatResult {
            value,
            pct: None,
            improved: None,
        };
    }
    let (pct, up) = match (value, base) {
        (Some(v), Some(b)) if b != 0.0 => ((v - b) / b.abs() * 100.0, v > b),
        (Some(v), Some(b)) => ((change.factor - 1.0) * 100.0, v > b),
        _ => ((change.factor - 1.0) * 100.0, change.factor > 1.0),
    };
    StatResult {
        value,
        pct: Some(pct),
        improved: stat.higher_is_better.map(|h| up == h),
    }
}

/// One property a crafted part changes.
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyChange {
    /// The property's GUID.
    pub property: String,
    /// What it moves, by its record name; `None` for a property the cook
    /// doesn't know.
    pub stat: Option<GameplayStat>,
    pub change: Change,
}

/// A stat-sheet row of a crafted part, changed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Gain {
    pub label: String,
    pub unit: String,
    /// The stock figure; `None` for a percent-only row.
    pub base: Option<f64>,
    pub value: Option<f64>,
    pub pct: f64,
    pub improved: Option<bool>,
}

/// An item crafted at a quality per material slot.
#[derive(Debug, Clone, PartialEq)]
pub struct Crafted {
    /// The blueprint (`CraftingBlueprintRecord` GUID): what Plans takes.
    pub blueprint: String,
    /// The highest of the slots' qualities: the one quality when all are
    /// alike.
    pub quality: i32,
    /// Each slot's quality, in the recipe's slot order; empty when every
    /// slot is at `quality`.
    pub slots: Vec<i32>,
    pub changes: Vec<PropertyChange>,
}

impl Crafted {
    /// What it does to properties of `stat`, together (one property each
    /// today; summed the way slots sum, should two ever share a stat).
    pub fn of(&self, stat: &GameplayStat) -> Change {
        self.changes
            .iter()
            .filter(|c| c.stat.as_ref() == Some(stat))
            .fold(Change::NONE, |acc, c| Change {
                factor: acc.factor + (c.change.factor - 1.0),
                added: acc.added + c.change.added,
            })
    }
}

/// The blueprints by the item they make, for "what if it were crafted".
/// Empty when the build has no cooked blueprints: nothing is craftable.
#[derive(Debug, Default)]
pub struct Recipes<'a> {
    by_item: HashMap<&'a str, &'a BlueprintEntry>,
    crafting: Option<&'a CraftingIndex>,
}

impl<'a> Recipes<'a> {
    pub fn new(blueprints: Option<&'a BlueprintIndex>, crafting: &'a CraftingIndex) -> Self {
        let mut by_item = HashMap::new();
        for b in blueprints.iter().flat_map(|i| &i.blueprints) {
            // A colourway recipe is the same recipe: the first is as good.
            if let Some(e) = b.entity_guid.as_deref() {
                by_item.entry(e).or_insert(b);
            }
        }
        Self {
            by_item,
            crafting: Some(crafting),
        }
    }

    /// The blueprint that makes `item` (its entity GUID).
    pub fn of(&self, item: &str) -> Option<&'a BlueprintEntry> {
        self.by_item.get(item).copied()
    }

    /// `item`'s slots at the qualities another recipe's slots were set
    /// to, for comparing parts: `by_property` holds, per crafting property
    /// GUID, the qualities of the slots that move it in slot order. A slot
    /// takes, for each stat it moves, the quality of the same-numbered
    /// slot moving that stat there (the last when this recipe has more),
    /// the highest when it moves several; a slot that moves none of them
    /// keeps its floor. The qualities are taken as they are, not snapped to
    /// what this recipe's materials come in. `None` when no blueprint makes
    /// it.
    pub fn slots_for(
        &self,
        item: &str,
        by_property: &HashMap<String, Vec<i32>>,
    ) -> Option<Vec<i32>> {
        let bp = self.of(item)?;
        let mut seen: HashMap<&str, usize> = HashMap::new();
        Some(
            bp.slots
                .iter()
                .map(|s| {
                    s.modifiers
                        .iter()
                        .filter_map(|m| {
                            let p = m.property.as_deref()?;
                            let n = seen.entry(p).or_insert(0);
                            let qs = by_property.get(p)?;
                            let q = qs.get(*n).or(qs.last()).copied();
                            *n += 1;
                            q
                        })
                        .max()
                        .unwrap_or(s.min_quality)
                        .max(s.min_quality)
                })
                .collect(),
        )
    }

    /// `item` crafted with every material at `quality` (clamped to
    /// 0..=1000); `None` when no blueprint makes it.
    pub fn crafted(&self, item: &str, quality: i32) -> Option<Crafted> {
        self.crafted_at(item, &[quality])
    }

    /// `item` crafted at a quality per material slot, in the recipe's slot
    /// order (each clamped to 0..=1000; a slot past the end takes the
    /// last); `None` when no blueprint makes it or `qualities` is empty.
    pub fn crafted_at(&self, item: &str, qualities: &[i32]) -> Option<Crafted> {
        let bp = self.of(item)?;
        let slots: Vec<i32> = qualities
            .iter()
            .map(|q| (*q).clamp(0, MAX_QUALITY))
            .collect();
        let quality = *slots.iter().max()?;
        let q: Vec<f64> = slots.iter().map(|v| f64::from(*v)).collect();
        let mut seen: Vec<&str> = Vec::new();
        for m in bp.slots.iter().flat_map(|s| &s.modifiers) {
            if let Some(p) = m.property.as_deref()
                && !seen.contains(&p)
            {
                seen.push(p);
            }
        }
        let changes = seen
            .into_iter()
            .map(|p| PropertyChange {
                property: p.to_owned(),
                stat: self.stat_of(p),
                change: eval_property(p, &bp.slots, &q),
            })
            .collect();
        Some(Crafted {
            blueprint: bp.guid.clone(),
            quality,
            slots: if slots.iter().all(|v| *v == quality) {
                Vec::new()
            } else {
                slots
            },
            changes,
        })
    }

    /// The rows of `item`'s stat sheet that crafting at `quality` changes.
    pub fn gains(&self, item: &str, quality: i32) -> Vec<Gain> {
        self.gains_at(item, &[quality])
    }

    /// The rows of `item`'s stat sheet that crafting at a quality per slot
    /// changes ([`Self::crafted_at`]).
    pub fn gains_at(&self, item: &str, qualities: &[i32]) -> Vec<Gain> {
        let Some(bp) = self.of(item) else {
            return Vec::new();
        };
        if qualities.is_empty() {
            return Vec::new();
        }
        let q: Vec<f64> = qualities
            .iter()
            .map(|v| f64::from((*v).clamp(0, MAX_QUALITY)))
            .collect();
        bp.stats
            .iter()
            .filter_map(|s| {
                let r = eval_stat(s, &bp.slots, &q);
                Some(Gain {
                    label: s.label.clone(),
                    unit: s.unit.clone(),
                    base: s.base.map(wide),
                    value: r.value,
                    pct: r.pct?,
                    improved: r.improved,
                })
            })
            .collect()
    }

    /// What a property moves, by its record name; `None` for one the cook
    /// doesn't know.
    pub fn stat_of(&self, property: &str) -> Option<GameplayStat> {
        let p = self
            .crafting?
            .properties
            .iter()
            .find(|p| p.guid == property)?;
        Some(GameplayStat::from_gpp_name(&p.record_name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blueprints::{BlueprintEntry, BlueprintIndex, ModifierTransform, SlotModifier};
    use crate::crafting::CraftingIndex;

    /// The cases Crafting's TypeScript runs too (`model.test.ts`).
    #[derive(Deserialize)]
    struct Cases {
        modifiers: Vec<ModifierCase>,
        stats: Vec<StatCase>,
    }

    #[derive(Deserialize)]
    struct ModifierCase {
        name: String,
        ranges: Vec<RangeJson>,
        q: f64,
        expect: Option<EffectJson>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct RangeJson {
        additive: bool,
        start_quality: i32,
        end_quality: i32,
        at_start: f32,
        at_end: f32,
    }

    #[derive(Deserialize)]
    struct EffectJson {
        value: f64,
        additive: bool,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct StatCase {
        name: String,
        base: Option<f32>,
        higher_is_better: Option<bool>,
        /// Per slot: its modifiers' bands, all on the stat's property.
        slots: Vec<Vec<Vec<RangeJson>>>,
        qualities: Vec<f64>,
        expect: StatJson,
    }

    #[derive(Deserialize)]
    struct StatJson {
        value: Option<f64>,
        pct: Option<f64>,
        improved: Option<bool>,
    }

    fn range(r: &RangeJson) -> ModifierRange {
        ModifierRange {
            additive: r.additive,
            start_quality: r.start_quality,
            end_quality: r.end_quality,
            at_start: r.at_start,
            at_end: r.at_end,
        }
    }

    fn cases() -> Cases {
        serde_json::from_str(include_str!("../tests/craft_cases.json")).expect("craft_cases.json")
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-4 * b.abs().max(1.0)
    }

    #[test]
    fn modifiers_match_the_shared_cases() {
        for c in cases().modifiers {
            let ranges: Vec<_> = c.ranges.iter().map(range).collect();
            let got = eval_modifier(&ranges, c.q);
            match (got, c.expect) {
                (None, None) => {}
                (Some(g), Some(e)) => {
                    assert!(
                        close(g.value, e.value),
                        "{}: {} vs {}",
                        c.name,
                        g.value,
                        e.value
                    );
                    assert_eq!(g.additive, e.additive, "{}", c.name);
                }
                (g, e) => panic!("{}: {g:?} vs {:?}", c.name, e.map(|e| e.value)),
            }
        }
    }

    #[test]
    fn stats_match_the_shared_cases() {
        for c in cases().stats {
            let slots: Vec<BlueprintSlot> = c
                .slots
                .iter()
                .map(|mods| BlueprintSlot {
                    name: None,
                    material: "Ore".into(),
                    scu: Some(1.0),
                    count: None,
                    min_quality: 0,
                    modifiers: mods
                        .iter()
                        .map(|ranges| SlotModifier {
                            property: Some("p".into()),
                            name: None,
                            transform: ModifierTransform::Raw,
                            ranges: ranges.iter().map(range).collect(),
                        })
                        .collect(),
                })
                .collect();
            let stat = BlueprintStat {
                group: None,
                label: c.name.clone(),
                property: Some("p".into()),
                unit: String::new(),
                higher_is_better: c.higher_is_better,
                base: c.base,
            };
            let got = eval_stat(&stat, &slots, &c.qualities);
            let opt = |a: Option<f64>, b: Option<f64>| match (a, b) {
                (None, None) => true,
                (Some(a), Some(b)) => close(a, b),
                _ => false,
            };
            assert!(
                opt(got.value, c.expect.value),
                "{}: value {:?} vs {:?}",
                c.name,
                got.value,
                c.expect.value
            );
            assert!(
                opt(got.pct, c.expect.pct),
                "{}: pct {:?} vs {:?}",
                c.name,
                got.pct,
                c.expect.pct
            );
            assert_eq!(got.improved, c.expect.improved, "{}: improved", c.name);
        }
    }

    #[test]
    fn one_quality_fills_every_slot() {
        let band = |lo: f32, hi: f32| ModifierRange {
            additive: false,
            start_quality: 0,
            end_quality: 1000,
            at_start: lo,
            at_end: hi,
        };
        let slot = |r: ModifierRange| BlueprintSlot {
            name: None,
            material: "Ore".into(),
            scu: Some(1.0),
            count: None,
            min_quality: 0,
            modifiers: vec![SlotModifier {
                property: Some("dmg".into()),
                name: None,
                transform: ModifierTransform::Raw,
                ranges: vec![r],
            }],
        };
        let slots = [slot(band(0.8, 1.2)), slot(band(0.8, 1.2))];
        // Two +20% slots at Q1000 are +40%.
        let c = eval_property("dmg", &slots, &[1000.0]);
        assert!(close(c.factor, 1.4));
        assert!(eval_property("dmg", &slots, &[500.0]).is_none());
    }

    #[test]
    fn another_part_takes_the_fitted_slots_qualities_stat_by_stat() {
        let slot = |props: &[&str], min: i32| BlueprintSlot {
            name: None,
            material: "Ore".into(),
            scu: Some(1.0),
            count: None,
            min_quality: min,
            modifiers: props
                .iter()
                .map(|p| SlotModifier {
                    property: Some((*p).into()),
                    name: None,
                    transform: ModifierTransform::Raw,
                    ranges: Vec::new(),
                })
                .collect(),
        };
        let entry = |slots: Vec<BlueprintSlot>| BlueprintEntry {
            guid: "bp".into(),
            entity_guid: Some("gun".into()),
            name: "Gun".into(),
            family: "gun".into(),
            family_name: "Gun".into(),
            variant: None,
            category: None,
            item_type: "WeaponGun".into(),
            sub_type: String::new(),
            size: 3,
            grade: 1,
            class: None,
            manufacturer: None,
            craft_seconds: None,
            slots,
            stats: Vec::new(),
            granted_by: Vec::new(),
        };
        // This gun: one damage slot, two integrity slots, one plain slot.
        let index = BlueprintIndex {
            default_quality: 500,
            blueprints: vec![entry(vec![
                slot(&["dmg"], 0),
                slot(&["int"], 0),
                slot(&["int"], 300),
                slot(&[], 100),
            ])],
        };
        let crafting = CraftingIndex::default();
        let recipes = Recipes::new(Some(&index), &crafting);
        // The fitted gun had two damage slots (912, 530) and one integrity
        // slot (325).
        let by = HashMap::from([
            ("dmg".to_owned(), vec![912, 530]),
            ("int".to_owned(), vec![325]),
        ]);
        // Its first damage slot takes the first; its second integrity slot
        // the last there is, and keeps its own floor of 300 only when
        // that is higher; the plain slot its floor.
        assert_eq!(
            recipes.slots_for("gun", &by),
            Some(vec![912, 325, 325, 100])
        );
        assert_eq!(recipes.slots_for("other", &by), None);
    }
}
