//! What Discord shows: the activity object of `SET_ACTIVITY`, as Discord
//! names its fields, so it serializes straight into the command.
//!
//! Discord's own limits: every text 2 to 128 characters, at most two
//! buttons with labels up to 32. Ours is tighter: a line the profile shows
//! (details, state) is at most [`LINE_MAX`] characters, cut at a word by
//! [`cut`], because the profile popout cuts at about 30 (measured in the
//! Rich Presence visualizer, 2026-11-23) and its cut is mid-word.

use serde::{Deserialize, Serialize};

/// The longest line we send for details and state.
pub const LINE_MAX: usize = 28;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Activity {
    /// The top line: what you are doing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    /// The second line: where.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamps: Option<Timestamps>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assets: Option<Assets>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub buttons: Vec<Button>,
}

/// Unix seconds. Discord counts up from `start`; with none it counts from
/// when the activity was first set.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Timestamps {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub start: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "specta", specta(type = Option<f64>))]
    pub end: Option<i64>,
}

/// An image is an asset key uploaded to the Discord application, or an
/// https URL Discord fetches itself; its text is the hover.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Assets {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub large_image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub large_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub small_image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub small_text: Option<String>,
}

/// A link under the activity. Discord hides buttons on your own profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Button {
    pub label: String,
    pub url: String,
}

/// `s` in at most `max` characters: whole, or cut at the last word that
/// fits (a cut word is never shown) with an ellipsis. A word longer than
/// the line is cut where it must be.
pub fn cut(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let room: String = s.chars().take(max.saturating_sub(1)).collect();
    // The cut falls inside a word unless the next character is a space.
    let next_is_space = s
        .chars()
        .nth(room.chars().count())
        .is_some_and(char::is_whitespace);
    let kept = if next_is_space {
        room.as_str()
    } else {
        match room.rfind(char::is_whitespace) {
            Some(at) if at > 0 => &room[..at],
            _ => room.as_str(),
        }
    };
    let kept = kept.trim_end_matches(|c: char| c.is_whitespace() || ",:;-–".contains(c));
    format!("{kept}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_that_fits_stays_whole() {
        assert_eq!(
            cut("Mercenary for Headhunters", LINE_MAX),
            "Mercenary for Headhunters"
        );
        assert_eq!(cut("  On foot ", LINE_MAX), "On foot");
    }

    #[test]
    fn a_long_line_is_cut_at_a_word() {
        assert_eq!(
            cut("Little Surprise for My Daughter", LINE_MAX),
            "Little Surprise for My…"
        );
        assert_eq!(
            cut("Jorrit Dossier: Retrieve Security Data", LINE_MAX),
            "Jorrit Dossier: Retrieve…"
        );
    }

    #[test]
    fn a_cut_drops_trailing_punctuation() {
        assert_eq!(
            cut("Jorrit Dossier: Retrieve Security Data", 18),
            "Jorrit Dossier…"
        );
    }

    #[test]
    fn a_cut_right_before_a_space_keeps_the_whole_word() {
        assert_eq!(
            cut("Hauling for Covalex Shipping", 20),
            "Hauling for Covalex…"
        );
    }

    #[test]
    fn one_long_word_is_cut_where_it_must_be() {
        assert_eq!(cut("Supercalifragilisticexpialidocious", 10), "Supercali…");
    }

    #[test]
    fn a_cut_line_is_never_longer_than_the_limit() {
        for s in [
            "To the Nyx–Castra jump point",
            "Retrieve Security Data from Research Wing",
        ] {
            assert!(cut(s, LINE_MAX).chars().count() <= LINE_MAX, "{s}");
        }
    }

    #[test]
    fn empty_fields_are_not_sent() {
        let a = Activity {
            details: Some("On foot".into()),
            ..Activity::default()
        };
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            r#"{"details":"On foot"}"#
        );
    }
}
