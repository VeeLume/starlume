//! Illegal-goods markers — `[!]` prefix on commodity names + an "Illegal
//! in: …" block on descriptions, from svc-data's cooked legality index.
//!
//! Port of sc-langpatch's `illegal_goods.rs` patch decisions; the
//! jurisdiction traversal moved into svc-data's cook (`legality.rs`), so
//! this patcher is a pure rendering of [`svc_data::LegalityEntry`] rows.

use svc_data::{CookedData, LegalityKind};

use crate::format::{Color, apply_color};
use crate::ops::{ChoiceOption, OpSet, OptionKind, PatchOp, PatcherConfig, PatcherOption};

fn category_color(kind: LegalityKind) -> Color {
    match kind {
        LegalityKind::Drug => Color::Underline,
        LegalityKind::Contraband => Color::Highlight,
    }
}

pub struct IllegalGoods;

impl crate::Patcher for IllegalGoods {
    fn id(&self) -> &'static str {
        "illegal_goods"
    }

    fn name(&self) -> &'static str {
        "Illegal Goods Markers"
    }

    fn description(&self) -> &'static str {
        "Mark illegal commodities (drugs, contraband) with a [!] prefix and list where they're outlawed"
    }

    fn options(&self) -> Vec<PatcherOption> {
        vec![PatcherOption {
            id: "display".into(),
            label: "Display style".into(),
            description: "How to mark illegal goods in the commodity name".into(),
            kind: OptionKind::Choice {
                choices: vec![
                    ChoiceOption {
                        value: "color_coded".into(),
                        label: "Emphasised (distinct style for drugs vs contraband)".into(),
                    },
                    ChoiceOption {
                        value: "simple".into(),
                        label: "Plain [!] prefix".into(),
                    },
                ],
            },
            default: "color_coded".into(),
        }]
    }

    fn derive(&self, cooked: &CookedData, config: &PatcherConfig) -> anyhow::Result<OpSet> {
        let display = config.get_str("display", "color_coded");
        let mut patches = Vec::new();

        // Entries come pre-sorted from the cook — output stays stable.
        for good in &cooked.legality {
            let key = good.name_key.strip_prefix('@').unwrap_or(&good.name_key);
            if cooked
                .locale
                .resolve(key)
                .filter(|v| !v.is_empty())
                .is_none()
            {
                continue;
            }

            let prefix = match display {
                "simple" => "[!] ".to_string(),
                _ => format!("{} ", apply_color(category_color(good.kind), "[!]")),
            };
            patches.push((key.to_string(), PatchOp::Prefix(prefix)));

            let desc_key = format!("{key}_desc");
            let desc_ok = cooked
                .locale
                .resolve(&desc_key)
                .is_some_and(|v| !v.is_empty() && !v.contains("LOC_EMPTY"));
            if desc_ok {
                let category_label = match good.kind {
                    LegalityKind::Drug => "Controlled Substance",
                    LegalityKind::Contraband => "Prohibited Good",
                };
                let jurisdictions = if good.jurisdictions.is_empty() {
                    "All jurisdictions".to_string()
                } else {
                    good.jurisdictions
                        .iter()
                        .map(|j| {
                            j.name_key
                                .as_deref()
                                .and_then(|k| cooked.locale.resolve(k))
                                .filter(|n| !n.is_empty())
                                .unwrap_or(&j.record_name)
                                .to_string()
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                let suffix = format!(
                    "\\n\\n{}\\nIllegal in: {jurisdictions}",
                    apply_color(category_color(good.kind), category_label)
                );
                patches.push((desc_key, PatchOp::Suffix(suffix)));
            }
        }

        Ok(OpSet {
            renames: Vec::new(),
            patches,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drug_uses_underline_contraband_highlight() {
        assert_eq!(
            apply_color(category_color(LegalityKind::Drug), "[!]"),
            "<EM3>[!]</EM3>"
        );
        assert_eq!(
            apply_color(category_color(LegalityKind::Contraband), "[!]"),
            "<EM4>[!]</EM4>"
        );
    }
}
