//! The built-in patcher registry.
//!
//! Lineup per the 2026-07-04 feature decisions: the four code patchers are
//! the core (component grades, illegal goods, weapons, mission enhancer),
//! label_fixes survives as maintainer-curated embedded TOML, and the
//! legacy TOML patchers superseded by code patchers (drug_markers,
//! component_grades.toml, blueprint_markers/rewards) plus the empty
//! key_fixes were dropped in the port.

mod component_grades;
mod illegal_goods;
mod missions;
mod weapons;

use crate::Patcher;

/// Every built-in patcher, in registry order.
pub fn builtin_patchers() -> Vec<Box<dyn Patcher>> {
    vec![
        Box::new(component_grades::ComponentGrades),
        Box::new(illegal_goods::IllegalGoods),
        Box::new(weapons::WeaponEnhancer),
        Box::new(missions::MissionEnhancer),
        crate::toml_patcher::label_fixes(),
    ]
}
