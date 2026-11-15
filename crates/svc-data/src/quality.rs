//! The qualities a material comes in.
//!
//! The game does not hand out a mined resource at any quality from 0 to
//! 1000: each resource sorts a rolled quality into a few bands and gives
//! every roll in a band the same value (`CraftingQualityQuantization`, read
//! by sc-holotable). In the 4.x data every mined resource has the same
//! eight bands (from 0, 400, 600, 700, 800, 900, 950 and 999) and its own
//! value in each, so a material exists at eight qualities at most. A recipe
//! names the refined material; its qualities are its ore's, since refining
//! keeps the quality.
//!
//! A minimum quality between two of those values asks for the same stock as
//! the next value up, so a control that sets a minimum snaps to them.

use std::collections::HashMap;

use sc_holotable::asset::RecordCollection;
use sc_holotable::resources::{QuantizationBand, QuantizationRef};

use crate::cooked::CookedData;

/// One band of a material's quality: every roll from `start` to `end`
/// comes out as `value`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QualityBand {
    pub start: u32,
    pub end: u32,
    pub value: u32,
}

/// Each material's bands, by its name as recipes and the inventory show it.
#[derive(Debug, Clone, Default)]
pub struct QualityBands {
    by_name: HashMap<String, Vec<QualityBand>>,
}

impl QualityBands {
    /// The bands of `material`, lowest first; empty when the data gives the
    /// material none (it is not mined, or its values do not sit inside
    /// their bands).
    pub fn of(&self, material: &str) -> &[QualityBand] {
        self.by_name
            .get(&material.to_lowercase())
            .map_or(&[], Vec::as_slice)
    }

    /// The qualities `material` comes in, lowest first.
    pub fn values(&self, material: &str) -> Vec<u32> {
        self.of(material).iter().map(|b| b.value).collect()
    }
}

impl CookedData {
    /// Every material's quality bands: a mined resource's own, and for a
    /// refined material its ore's.
    pub fn quality_bands(&self) -> QualityBands {
        let mut by_name = HashMap::new();
        let Some(resources) = self.holotable.resources.as_ref() else {
            return QualityBands { by_name };
        };
        for (guid, resource) in resources.iter() {
            let bands = match resource
                .quality
                .as_ref()
                .and_then(|q| q.quantization.as_ref())
            {
                Some(QuantizationRef::Inline(bands)) => bands.as_slice(),
                Some(QuantizationRef::Record(record)) => {
                    match self.mining.quality.quantization(record) {
                        Some(q) => q.bands.as_slice(),
                        None => continue,
                    }
                }
                _ => continue,
            };
            let Some(bands) = clear_bands(bands) else {
                continue;
            };
            let refined = resources
                .refined_version_of(guid)
                .and_then(|r| self.resource_name(&r.guid.to_string()));
            // A resource's own bands win over the ones it inherits.
            if let Some(name) = self.resource_name(&guid.to_string()) {
                by_name.insert(name.to_lowercase(), bands.clone());
            }
            if let Some(name) = refined {
                by_name.entry(name.to_lowercase()).or_insert(bands);
            }
        }
        QualityBands { by_name }
    }
}

/// Bands that give one value each, lowest first. `None` when they do not:
/// the game leaves a roll as it is when a band's value lies outside the
/// band, and then the material has no fixed set of qualities.
fn clear_bands(bands: &[QuantizationBand]) -> Option<Vec<QualityBand>> {
    let mut out: Vec<QualityBand> = Vec::with_capacity(bands.len());
    for b in bands {
        let inside = (b.start..=b.end).contains(&b.mapped_value);
        if !inside || b.start < 0 || b.end > 1000 || b.start > b.end {
            return None;
        }
        out.push(QualityBand {
            start: b.start as u32,
            end: b.end as u32,
            value: b.mapped_value as u32,
        });
    }
    out.sort_by_key(|b| b.start);
    let ordered = out.windows(2).all(|w| w[0].end < w[1].start);
    (!out.is_empty() && ordered).then_some(out)
}

/// The lowest of `values` at or above `quality`: what a minimum of
/// `quality` really asks for. The highest when `quality` is above them
/// all; `quality` itself when there are no values.
pub fn snap_up(values: &[u32], quality: u32) -> u32 {
    values
        .iter()
        .copied()
        .find(|v| *v >= quality)
        .or(values.last().copied())
        .unwrap_or(quality)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn band(start: i32, end: i32, mapped_value: i32) -> QuantizationBand {
        QuantizationBand {
            start,
            end,
            mapped_value,
        }
    }

    #[test]
    fn bands_with_one_value_each_are_kept_lowest_first() {
        let bands = clear_bands(&[band(400, 599, 523), band(0, 399, 348)]).unwrap();
        assert_eq!(
            bands.iter().map(|b| b.value).collect::<Vec<_>>(),
            [348, 523]
        );
        assert_eq!((bands[1].start, bands[1].end), (400, 599));
    }

    #[test]
    fn a_value_outside_its_band_means_no_clear_bands() {
        // The game leaves such a roll as it is: any quality can come out.
        assert_eq!(clear_bands(&[band(0, 399, 348), band(400, 599, 700)]), None);
        assert_eq!(clear_bands(&[]), None);
        assert_eq!(clear_bands(&[band(0, 500, 300), band(400, 599, 523)]), None);
    }

    #[test]
    fn a_minimum_snaps_up_to_the_next_quality_the_material_comes_in() {
        let values = [348, 523, 686, 717];
        assert_eq!(snap_up(&values, 0), 348);
        assert_eq!(snap_up(&values, 500), 523);
        assert_eq!(snap_up(&values, 523), 523);
        assert_eq!(snap_up(&values, 700), 717);
        assert_eq!(snap_up(&values, 900), 717);
        assert_eq!(snap_up(&[], 640), 640);
    }
}

#[cfg(test)]
mod probe {
    //! Ignored-by-default look at the bands of a real cooked build.
    //!
    //! ```text
    //! STARLUME_REAL_BUILD_DIR="…\cache\builds\13165946" \
    //!   cargo test -p svc-data quality::probe -- --ignored --nocapture
    //! ```
    use std::collections::{BTreeMap, BTreeSet};

    use sc_holotable::asset::ProcessedSnapshot;

    use crate::blueprints::BlueprintIndex;
    use crate::cooked::{CookedData, DATA_COOK_VERSION};

    #[test]
    #[ignore = "needs a cooked build: set STARLUME_REAL_BUILD_DIR"]
    fn quality_bands_of_every_recipe_material() {
        let dir = std::path::PathBuf::from(
            std::env::var("STARLUME_REAL_BUILD_DIR").expect("set STARLUME_REAL_BUILD_DIR"),
        );
        let cooked: CookedData =
            ProcessedSnapshot::<CookedData>::load(&dir.join("foundations.cook"), DATA_COOK_VERSION)
                .expect("foundations.cook")
                .into_index();
        let index = ProcessedSnapshot::<BlueprintIndex>::load(
            &dir.join("blueprints.cook"),
            DATA_COOK_VERSION,
        )
        .expect("blueprints.cook")
        .into_index();
        let bands = cooked.quality_bands();

        let mut edges: BTreeMap<String, usize> = BTreeMap::new();
        let mut with: BTreeSet<&str> = BTreeSet::new();
        let mut without: BTreeMap<&str, usize> = BTreeMap::new();
        let mut slots = (0usize, 0usize);
        for b in &index.blueprints {
            for s in &b.slots {
                let of = bands.of(&s.material);
                if of.is_empty() {
                    *without.entry(&s.material).or_default() += 1;
                    slots.1 += 1;
                } else {
                    with.insert(&s.material);
                    slots.0 += 1;
                }
            }
        }
        for name in &with {
            let shape: Vec<String> = bands
                .of(name)
                .iter()
                .map(|b| format!("{}..{}", b.start, b.end))
                .collect();
            *edges.entry(shape.join(" ")).or_default() += 1;
        }
        eprintln!("recipe slots with bands: {}, without: {}", slots.0, slots.1);
        eprintln!("band edges over the {} materials with bands:", with.len());
        for (shape, n) in &edges {
            eprintln!("{n:>4}  [{shape}]");
        }
        eprintln!("materials without bands (slots): {without:?}");
        for name in ["Agricium", "Hadanite", "Pressurized Ice"] {
            eprintln!("{name}: {:?}", bands.values(name));
        }
        // The recipe this was built for.
        assert_eq!(bands.of("Agricium").len(), 8);
        assert!(!bands.of("Pressurized Ice").is_empty());
    }
}
