//! Mission text preview — what the patch does to each mission's title and
//! description, without touching any file.
//!
//! Pure: the cooked data's base text, the stacked ops ([`crate::merge::stack_ops`])
//! and the user overrides in, before/after texts out — the same per-key
//! steps [`crate::merge::apply_patches`] and
//! [`crate::merge::apply_user_overrides`] run over the whole INI. The base is
//! the English text the data was cooked with; a language pack isn't
//! overlaid.

use std::collections::{BTreeMap, HashMap, HashSet};

use svc_data::{CookedData, LocaleMap};

use crate::merge::{apply_ops, is_placeholder_value};
use crate::ops::PatchOp;

/// One INI value before and after patching.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PreviewText {
    /// The INI key (stripped, no `@`); `None` when the mission has none.
    pub key: Option<String>,
    pub original: String,
    pub patched: String,
}

/// The title and description a mission shows in the contracts panel. Pool
/// members sharing both keys show the same text in game, so they share one
/// preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissionPreview {
    pub title: PreviewText,
    pub description: PreviewText,
}

/// One preview per distinct (title key, description key) pair across the
/// cooked missions, in catalog order.
pub fn mission_previews(
    cooked: &CookedData,
    stacked: &HashMap<String, Vec<PatchOp>>,
    overrides: &BTreeMap<String, String>,
) -> Vec<MissionPreview> {
    let mut seen: HashSet<(Option<&str>, Option<&str>)> = HashSet::new();
    let mut out = Vec::new();
    for m in &cooked.missions {
        let title_key = stripped(m.title_key.as_deref());
        let description_key = stripped(m.description_key.as_deref());
        if (title_key, description_key) == (None, None)
            || !seen.insert((title_key, description_key))
        {
            continue;
        }
        out.push(MissionPreview {
            title: preview_text(title_key, &cooked.locale, stacked, overrides),
            description: preview_text(description_key, &cooked.locale, stacked, overrides),
        });
    }
    out
}

fn stripped(key: Option<&str>) -> Option<&str> {
    key.map(|k| k.strip_prefix('@').unwrap_or(k))
        .filter(|k| !k.is_empty())
}

/// One key through the apply steps: a user override replaces the value
/// outright; otherwise the op stack applies, except to CIG's placeholder
/// sentinels, which are never patched.
fn preview_text(
    key: Option<&str>,
    locale: &LocaleMap,
    stacked: &HashMap<String, Vec<PatchOp>>,
    overrides: &BTreeMap<String, String>,
) -> PreviewText {
    let Some(key) = key else {
        return PreviewText::default();
    };
    let original = locale.resolve(key).unwrap_or_default().to_string();
    let patched = if let Some(user) = overrides.get(key) {
        user.clone()
    } else if is_placeholder_value(&original) {
        original.clone()
    } else {
        stacked
            .get(key)
            .map_or_else(|| original.clone(), |ops| apply_ops(&original, ops))
    };
    PreviewText {
        key: Some(key.to_string()),
        original,
        patched,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn locale() -> LocaleMap {
        let mut l = LocaleMap::new();
        l.set("desc_a", "Base text.");
        l.set("LOC_PLACEHOLDER", "S1 ???0A <= PLACEHOLDER =>");
        l
    }

    fn stack(key: &str, op: PatchOp) -> HashMap<String, Vec<PatchOp>> {
        HashMap::from([(key.to_string(), vec![op])])
    }

    #[test]
    fn ops_apply_to_the_base_text() {
        let s = stack("desc_a", PatchOp::Suffix("\\n\\nMission Info".into()));
        let t = preview_text(Some("desc_a"), &locale(), &s, &BTreeMap::new());
        assert_eq!(t.original, "Base text.");
        assert_eq!(t.patched, "Base text.\\n\\nMission Info");
    }

    #[test]
    fn user_override_wins_and_placeholders_stay_unpatched() {
        let s = stack("desc_a", PatchOp::Prefix("[x] ".into()));
        let overrides = BTreeMap::from([("desc_a".to_string(), "Mine.".to_string())]);
        let t = preview_text(Some("desc_a"), &locale(), &s, &overrides);
        assert_eq!(t.patched, "Mine.");

        let s = stack("LOC_PLACEHOLDER", PatchOp::Suffix("!".into()));
        let t = preview_text(Some("LOC_PLACEHOLDER"), &locale(), &s, &BTreeMap::new());
        assert_eq!(t.patched, t.original);
    }
}
