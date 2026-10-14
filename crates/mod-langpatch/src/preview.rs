//! Mission text preview — what the patch does to each mission's title and
//! description, without touching any file.
//!
//! Pure: the cooked data's base text, the stacked ops ([`crate::merge::stack_ops`])
//! and the user overrides in, before/after texts out — the same per-key
//! steps [`crate::merge::apply_patches`] and
//! [`crate::merge::apply_user_overrides`] run over the whole INI. The base is
//! the English text the data was cooked with; a language pack isn't
//! overlaid.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use svc_data::{CookedData, LocaleMap};

use crate::merge::{apply_ops, is_placeholder_value};
use crate::ops::{OpSet, OptionKind, PatchOp, PatcherConfig};
use crate::{OwnedSet, Patcher};

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
    mission_keys(cooked)
        .into_iter()
        .map(|(title_key, description_key)| MissionPreview {
            title: preview_text(title_key, &cooked.locale, stacked, overrides),
            description: preview_text(description_key, &cooked.locale, stacked, overrides),
        })
        .collect()
}

/// The distinct (title key, description key) pairs, stripped, in catalog
/// order. Pool members sharing both keys show the same text in game.
fn mission_keys(cooked: &CookedData) -> Vec<(Option<&str>, Option<&str>)> {
    let mut seen = HashSet::new();
    cooked
        .missions
        .iter()
        .map(|m| {
            (
                stripped(m.title_key.as_deref()),
                stripped(m.description_key.as_deref()),
            )
        })
        .filter(|&pair| pair != (None, None) && seen.insert(pair))
        .collect()
}

/// The `title|description` id of a key pair — [`MissionPreview::id`]'s
/// format, computable without rendering.
fn pair_id(title_key: Option<&str>, description_key: Option<&str>) -> String {
    format!(
        "{}|{}",
        title_key.unwrap_or(""),
        description_key.unwrap_or("")
    )
}

impl MissionPreview {
    /// Stable across config changes: the key pair, `title|description`.
    pub fn id(&self) -> String {
        pair_id(self.title.key.as_deref(), self.description.key.as_deref())
    }
}

/// One mission picked to show a patcher's options, and which options it
/// shows (in the patcher's declaration order).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Showcase {
    /// [`MissionPreview::id`].
    pub id: String,
    pub options: Vec<String>,
}

/// The choice value that turns a choice option off.
const OFF: &str = "off";

/// Pick missions so that every option of `patcher` shows in at least one.
///
/// An option shows in a mission when switching it off — from every option
/// on — changes the ops on the mission's title or description key. That is
/// one derive with all options on plus one per option, and it doesn't
/// depend on the user's config: the pick stays put while options change,
/// so a toggle shows its effect on text already on screen. Callers cache
/// it per build.
///
/// Greedy cover: the mission showing the most still-uncovered options
/// first, ties to the one with fewer runtime variables, then the shorter
/// text. Missions whose title or description is nothing but variables
/// (`~mission(…)`), or whose description is just the title, are never
/// picked. Options no mission shows (choices
/// without an off value, owned marks without an owned set) are skipped.
pub fn mission_showcases(
    cooked: &CookedData,
    patcher: &dyn Patcher,
    owned: Option<&OwnedSet>,
) -> anyhow::Result<Vec<Showcase>> {
    let options = patcher.options();
    let mut all_on = PatcherConfig {
        enabled: Some(true),
        options: BTreeMap::new(),
    };
    let mut contrasts = Vec::new();
    for option in &options {
        match &option.kind {
            OptionKind::Bool => {
                all_on.options.insert(option.id.clone(), "true".into());
                contrasts.push((option.id.as_str(), "false"));
            }
            OptionKind::Choice { choices } => {
                let on = if option.default != OFF {
                    Some(option.default.as_str())
                } else {
                    choices.iter().map(|c| c.value.as_str()).find(|v| *v != OFF)
                };
                if let Some(on) = on {
                    all_on.options.insert(option.id.clone(), on.into());
                }
                if choices.iter().any(|c| c.value == OFF) {
                    contrasts.push((option.id.as_str(), OFF));
                }
            }
        }
    }

    let base = by_key(patcher.derive(cooked, &all_on, owned)?);
    let mut shown_at: HashMap<String, BTreeSet<usize>> = HashMap::new();
    for (index, (id, off)) in contrasts.iter().enumerate() {
        let mut config = all_on.clone();
        config.options.insert((*id).to_string(), (*off).to_string());
        let contrast = by_key(patcher.derive(cooked, &config, owned)?);
        for key in changed_keys(&base, &contrast) {
            shown_at.entry(key.to_string()).or_default().insert(index);
        }
    }

    let mut candidates: Vec<Candidate> = mission_keys(cooked)
        .into_iter()
        .filter_map(|(title_key, description_key)| {
            let shows: BTreeSet<usize> = [title_key, description_key]
                .into_iter()
                .flatten()
                .filter_map(|k| shown_at.get(k))
                .flatten()
                .copied()
                .collect();
            if shows.is_empty() {
                return None;
            }
            let title = resolve(title_key, cooked);
            let description = resolve(description_key, cooked);
            // Some missions reuse the title key as their description.
            if !readable(title) || !readable(description) || title == description {
                return None;
            }
            Some(Candidate {
                id: pair_id(title_key, description_key),
                shows,
                variables: title.matches(VARIABLE).count() + description.matches(VARIABLE).count(),
                len: title.len() + description.len(),
            })
        })
        .collect();

    let mut uncovered: BTreeSet<usize> = shown_at.values().flatten().copied().collect();
    let mut picked = Vec::new();
    while !uncovered.is_empty() {
        let gain = |c: &Candidate| c.shows.intersection(&uncovered).count();
        // Ties fall through to the id, so the same build picks the same missions.
        let best = candidates
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| {
                gain(a)
                    .cmp(&gain(b))
                    .then_with(|| b.variables.cmp(&a.variables))
                    .then_with(|| b.len.cmp(&a.len))
                    .then_with(|| b.id.cmp(&a.id))
            })
            .map(|(i, _)| i);
        let Some(best) = best else { break };
        let candidate = candidates.swap_remove(best);
        if gain(&candidate) == 0 {
            break;
        }
        uncovered.retain(|i| !candidate.shows.contains(i));
        picked.push(Showcase {
            id: candidate.id,
            options: candidate
                .shows
                .iter()
                .map(|&i| contrasts[i].0.to_string())
                .collect(),
        });
    }
    Ok(picked)
}

struct Candidate {
    id: String,
    shows: BTreeSet<usize>,
    variables: usize,
    len: usize,
}

/// A runtime variable the game fills in (`~mission(Location)`).
const VARIABLE: &str = "~mission(";

/// Text left once its runtime variables are cut out has something to read.
fn readable(text: &str) -> bool {
    let mut rest = text;
    let mut prose = String::new();
    while let Some(start) = rest.find(VARIABLE) {
        prose.push_str(&rest[..start]);
        rest = rest[start..]
            .find(')')
            .map_or("", |end| &rest[start + end + 1..]);
    }
    prose.push_str(rest);
    prose.chars().any(char::is_alphabetic)
}

fn resolve<'a>(key: Option<&str>, cooked: &'a CookedData) -> &'a str {
    key.and_then(|k| cooked.locale.resolve(k))
        .unwrap_or_default()
}

fn by_key(ops: OpSet) -> HashMap<String, Vec<PatchOp>> {
    let mut map: HashMap<String, Vec<PatchOp>> = HashMap::new();
    for (key, op) in ops.patches {
        map.entry(key).or_default().push(op);
    }
    map
}

/// Keys whose op list differs between two derives.
fn changed_keys<'a>(
    a: &'a HashMap<String, Vec<PatchOp>>,
    b: &'a HashMap<String, Vec<PatchOp>>,
) -> impl Iterator<Item = &'a str> {
    a.iter()
        .filter(|(k, ops)| b.get(*k) != Some(*ops))
        .map(|(k, _)| k.as_str())
        .chain(b.keys().filter(|k| !a.contains_key(*k)).map(String::as_str))
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
    fn readable_needs_prose_outside_the_variables() {
        assert!(readable("Reduce Overpopulation"));
        assert!(readable("Clear ~mission(Location) of pests"));
        assert!(!readable("~mission(Contractor|SearchBodySpaceTitle)"));
        assert!(!readable("~mission(A) ~mission(B)"));
        assert!(!readable(""));
    }

    #[test]
    fn changed_keys_covers_changed_added_and_removed() {
        let a = HashMap::from([
            ("same".to_string(), vec![PatchOp::Suffix("x".into())]),
            ("changed".to_string(), vec![PatchOp::Suffix("x".into())]),
            ("removed".to_string(), vec![PatchOp::Suffix("x".into())]),
        ]);
        let b = HashMap::from([
            ("same".to_string(), vec![PatchOp::Suffix("x".into())]),
            ("changed".to_string(), vec![PatchOp::Suffix("y".into())]),
            ("added".to_string(), vec![PatchOp::Suffix("x".into())]),
        ]);
        let mut keys: Vec<&str> = changed_keys(&a, &b).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["added", "changed", "removed"]);
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
