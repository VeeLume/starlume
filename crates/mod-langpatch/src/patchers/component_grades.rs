//! Component grades — label ship components with class + grade
//! (`"Bracer Military C"` / `"M1C Bracer"` / `"MIL1C Bracer"`).
//!
//! Port of sc-langpatch's `component_grades.rs` patch decisions. The data
//! walk changed: instead of traversing raw `EntityClassDefinition` records,
//! grade/size/type come straight off the cooked item index (`AttachDef` is
//! exactly what `sc-items` extracts), and the class still parses out of the
//! description text — both already in [`svc_data::CookedData`].

use std::collections::BTreeMap;

use svc_data::{CookedData, RecordCollection};

use crate::ops::{ChoiceOption, OpSet, OptionKind, PatchOp, PatcherConfig, PatcherOption};

/// Component item types that get grade/class labels.
const COMPONENT_TYPES: &[&str] = &["Cooler", "PowerPlant", "Radar", "Shield", "QuantumDrive"];

/// Map numeric grade (from the DCB) to its letter.
fn grade_letter(grade: i32) -> &'static str {
    match grade {
        1 => "A",
        2 => "B",
        3 => "C",
        _ => "D",
    }
}

/// Parse the class from the item description (`"Class: Military"` on one
/// of its `\n`-separated lines).
fn parse_class(description: &str) -> Option<&str> {
    for segment in description.split("\\n") {
        if let Some(class) = segment.trim().strip_prefix("Class: ") {
            return Some(class.trim());
        }
    }
    None
}

pub struct ComponentGrades;

impl crate::Patcher for ComponentGrades {
    fn id(&self) -> &'static str {
        "component_grades"
    }

    fn name(&self) -> &'static str {
        "Component Grades"
    }

    fn description(&self) -> &'static str {
        "Label ship components with class and grade (e.g. 'Bracer Military C')"
    }

    fn uses_replace_ops(&self) -> bool {
        true
    }

    fn options(&self) -> Vec<PatcherOption> {
        vec![PatcherOption {
            id: "format".into(),
            label: "Name format".into(),
            description: "How to format the component name".into(),
            kind: OptionKind::Choice {
                choices: vec![
                    ChoiceOption {
                        value: "name_class_grade".into(),
                        label: "Name Class Grade (Bracer Military C)".into(),
                    },
                    ChoiceOption {
                        value: "compact_prefix".into(),
                        label: "Compact prefix (M1C Bracer)".into(),
                    },
                    ChoiceOption {
                        value: "short_prefix".into(),
                        label: "Short prefix (MIL1C Bracer)".into(),
                    },
                ],
            },
            default: "name_class_grade".into(),
        }]
    }

    fn derive(&self, cooked: &CookedData, config: &PatcherConfig) -> anyhow::Result<OpSet> {
        let Some(items) = cooked.holotable.items.as_ref() else {
            return Ok(OpSet::default());
        };
        let format = config.get_str("format", "name_class_grade");

        // Several entities share one name key (colorways, capital-ship
        // variants) — dedupe by key; BTreeMap keeps the output stable.
        let mut by_key: BTreeMap<String, PatchOp> = BTreeMap::new();
        for (_, item) in items.iter() {
            if !COMPONENT_TYPES.contains(&item.item_type.as_dcb_str()) {
                continue;
            }
            let Some(name_key) = item.name_key.as_ref() else {
                continue;
            };
            let key = name_key
                .as_str()
                .strip_prefix('@')
                .unwrap_or(name_key.as_str());
            // Only patch keys that exist in the base INI.
            let Some(display_name) = cooked.locale.resolve(key).filter(|s| !s.is_empty()) else {
                continue;
            };

            let class = item
                .desc_key
                .as_ref()
                .and_then(|k| cooked.locale.resolve(k.as_str()))
                .and_then(parse_class)
                .unwrap_or("Unknown");
            let grade = grade_letter(item.grade);

            let new_value = match format {
                "compact_prefix" => {
                    let code = match class {
                        "Military" => "M",
                        "Civilian" => "C",
                        "Industrial" => "I",
                        "Stealth" => "S",
                        "Competition" => "X",
                        _ => "?",
                    };
                    format!("{code}{}{grade} {display_name}", item.size)
                }
                "short_prefix" => {
                    let abbr = match class {
                        "Military" => "MIL",
                        "Civilian" => "CIV",
                        "Industrial" => "IND",
                        "Stealth" => "STL",
                        "Competition" => "CMP",
                        _ => "???",
                    };
                    format!("{abbr}{}{grade} {display_name}", item.size)
                }
                _ => format!("{display_name} {class} {grade}"),
            };
            by_key.insert(key.to_string(), PatchOp::Replace(new_value));
        }

        Ok(OpSet {
            renames: Vec::new(),
            patches: by_key.into_iter().collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grade_mapping() {
        assert_eq!(grade_letter(1), "A");
        assert_eq!(grade_letter(2), "B");
        assert_eq!(grade_letter(3), "C");
        assert_eq!(grade_letter(4), "D");
        assert_eq!(grade_letter(0), "D");
    }

    #[test]
    fn parse_class_from_description() {
        let desc =
            r"Item Type: Cooler\nManufacturer: Aegis\nSize: 1\nGrade: C\nClass: Military\n\nText.";
        assert_eq!(parse_class(desc), Some("Military"));
        assert_eq!(parse_class(r"Class: Stealth\nGrade: A"), Some("Stealth"));
        assert_eq!(parse_class(r"Item Type: Cooler\nGrade: B"), None);
    }
}
