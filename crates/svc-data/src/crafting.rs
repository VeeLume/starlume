//! Crafting reference data — the `CraftingGameplayPropertyDef` records
//! (sc-crafting's [`GameplayProperties`]): what a crafted item's stat
//! factors are. Reference framework (README module rule 4): mod-items names
//! the factors of the player's crafted items through it, and the blueprint
//! library / crafting planner build on the same feature later.
//!
//! Only the property definitions are cooked for now. The blueprints are
//! the planner's data and stay out of the snapshot until it lands (no
//! consumer yet — the same rule the weapons index applies to FPS weapons).
//!
//! Raw locale keys, resolved at query time ([`crate::cooked`] philosophy).

use sc_holotable::asset::{Datacore, class_crc};
use sc_holotable::crafting::{GameplayProperties, GameplayStat};
use serde::{Deserialize, Serialize};

/// The cooked crafting bundle.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CraftingIndex {
    /// Every gameplay property a recipe can modify, sorted by record name.
    pub properties: Vec<CraftingProperty>,
}

/// One `CraftingGameplayPropertyDef`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CraftingProperty {
    /// Record GUID, hex-string form.
    pub guid: String,
    /// The GUID's class CRC: how an inventory's crafted record names the
    /// property.
    pub crc: u32,
    /// DCB record name (`CraftingGameplayPropertyDef.GPP_Weapon_Damage`):
    /// stable across patches, unlike the GUID.
    pub record_name: String,
    /// Raw `propertyName` key (`@StatName_…`).
    pub name_key: String,
    /// Raw `unitFormat` key (`@LOC_EMPTY` when unitless).
    pub unit_key: String,
    /// Whether a factor above 1 is the better roll. Not in the data: an
    /// editorial call per stat ([`factor_up_is_better`]), `None` where it is
    /// unclear or the stat is unmodelled.
    pub factor_up_is_better: Option<bool>,
}

impl CraftingIndex {
    /// The property an inventory crafted record names by `crc`.
    pub fn by_crc(&self, crc: u32) -> Option<&CraftingProperty> {
        self.properties.iter().find(|p| p.crc == crc)
    }
}

/// Which way a factor helps. Recoil, spread and fuel use shrink when they
/// improve; everything else grows. Both temperature limits widen with a
/// factor above 1 (the minimum is negative), so a bigger factor is better
/// for both.
fn factor_up_is_better(stat: &GameplayStat) -> Option<bool> {
    use GameplayStat::*;
    match stat {
        WeaponFireRate
        | WeaponDamage
        | ArmorDamageMitigation
        | ArmorTemperatureMin
        | ArmorTemperatureMax
        | ArmorRadiationDissipation
        | Integrity
        | QuantumSpeed
        | ShieldMaxHealth
        | CoolantGeneration
        | PowerGeneration => Some(true),
        WeaponRecoilKick
        | WeaponRecoilHandling
        | WeaponRecoilSmoothness
        | WeaponSpread
        | QuantumFuelRequirement => Some(false),
        // Stats sc-crafting doesn't model yet, by record-name suffix: tractor
        // beams, hull scraping and the crafter all improve as they grow.
        // Radar assist distances and reload speed stay open: which way
        // helps depends on what the value measures.
        Unknown(suffix)
            if suffix.starts_with("GPP_Weapon_Tractor_")
                || suffix.starts_with("GPP_Weapon_HullScraping_")
                || suffix.starts_with("GPP_Crafter_")
                || suffix == "GPP_Armor_RadiationCapacity" =>
        {
            Some(true)
        }
        RadarMinAimAssist | RadarMaxAimAssist | Unknown(_) => None,
    }
}

pub(crate) fn build_crafting(datacore: &Datacore) -> CraftingIndex {
    let mut properties: Vec<CraftingProperty> = GameplayProperties::build(datacore)
        .iter()
        .map(|p| CraftingProperty {
            guid: p.guid.to_string(),
            crc: class_crc(&p.guid),
            record_name: p.record_name.clone(),
            name_key: p.property_name_key.as_str().to_string(),
            unit_key: p.unit_format_key.as_str().to_string(),
            factor_up_is_better: factor_up_is_better(&p.stat()),
        })
        .collect();
    properties.sort_by(|a, b| a.record_name.cmp(&b.record_name));
    CraftingIndex { properties }
}

#[cfg(test)]
mod probe {
    //! Ignored-by-default measurement on a real raw extract: what the
    //! crafting cook costs, and what cooking the blueprints too would add.
    //!
    //! ```text
    //! STARLUME_REAL_BUILD_DIR="C:\tmp\cache\builds\12913470" \
    //!   cargo test -p svc-data --release crafting::probe -- --ignored --nocapture
    //! ```
    use std::time::Instant;

    use sc_holotable::asset::{AssetConfig, ExtractSnapshot, ProcessedSnapshot, SnapshotMeta};
    use sc_holotable::crafting::Blueprints;

    #[test]
    #[ignore = "needs a real raw extract — set STARLUME_REAL_BUILD_DIR"]
    fn crafting_cook_cost() {
        let dir = std::path::PathBuf::from(
            std::env::var("STARLUME_REAL_BUILD_DIR").expect("set STARLUME_REAL_BUILD_DIR"),
        );
        let t = Instant::now();
        let snap = ExtractSnapshot::load(&dir.join("extract.snap")).expect("extract.snap");
        let (asset_data, datacore) = snap
            .hydrate(&AssetConfig::standard())
            .expect("datacore from the snapshot");
        eprintln!("load raw extract: {:?}", t.elapsed());

        let t = Instant::now();
        let foundations = sc_holotable::build_foundations(&datacore);
        eprintln!("foundations: {:?}", t.elapsed());

        let t = Instant::now();
        let crafting = super::build_crafting(&datacore);
        eprintln!(
            "crafting properties: {:?} ({} properties, {} named stats)",
            t.elapsed(),
            crafting.properties.len(),
            crafting
                .properties
                .iter()
                .filter(|p| p.factor_up_is_better.is_some())
                .count()
        );
        for p in &crafting.properties {
            eprintln!(
                "  {:>10} {} {} {:?}",
                p.crc,
                p.record_name,
                asset_data.locale.resolve(&p.name_key).unwrap_or("?"),
                p.factor_up_is_better
            );
        }

        let t = Instant::now();
        let blueprints = Blueprints::build(&datacore, &foundations.items);
        eprintln!("blueprints: {:?}", t.elapsed());

        let meta = SnapshotMeta {
            schema_version: 0,
            game_version: String::new(),
            build_id: String::new(),
            extracted_at: String::new(),
        };
        let size = |name: &str, f: &dyn Fn(&std::path::Path)| {
            let path = std::env::temp_dir().join(name);
            f(&path);
            let n = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let _ = std::fs::remove_file(&path);
            n
        };
        let props = size("probe-crafting.cook", &|p| {
            ProcessedSnapshot::new(meta.clone(), 1, crafting.clone())
                .save(p)
                .unwrap()
        });
        let bps = size("probe-blueprints.cook", &|p| {
            ProcessedSnapshot::new(meta.clone(), 1, blueprints.clone())
                .save(p)
                .unwrap()
        });
        eprintln!("compressed: properties {props} B, blueprints {bps} B");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_factor_helps_by_what_the_stat_measures() {
        assert_eq!(factor_up_is_better(&GameplayStat::WeaponDamage), Some(true));
        assert_eq!(
            factor_up_is_better(&GameplayStat::WeaponSpread),
            Some(false)
        );
        // The minimum temperature is negative: a bigger factor widens it.
        assert_eq!(
            factor_up_is_better(&GameplayStat::ArmorTemperatureMin),
            Some(true)
        );
        let unmodelled = |s: &str| factor_up_is_better(&GameplayStat::from_gpp_name(s));
        assert_eq!(
            unmodelled("CraftingGameplayPropertyDef.GPP_Weapon_Tractor_Force"),
            Some(true)
        );
        assert_eq!(
            unmodelled("CraftingGameplayPropertyDef.GPP_Weapon_ReloadSpeed"),
            None
        );
    }
}
