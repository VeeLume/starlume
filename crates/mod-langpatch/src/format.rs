//! Shared text-formatting helpers for INI value patches — the port of
//! sc-langpatch's `formatter_helpers.rs` (unchanged semantics).
//!
//! Star Citizen's `global.ini` rendering pipeline interprets a small markup
//! vocabulary: emphasis tags `<EM0>`..`<EM4>` for color, and `\n` literals
//! for line breaks (the INI parser sees the two characters `\` `n`, not a
//! real newline byte).

/// Single in-value line break — renders as a newline in the game.
pub const NEWLINE: &str = "\\n";

/// Blank line / paragraph break between sections.
pub const PARAGRAPH_BREAK: &str = "\\n\\n";

/// In-game emphasis levels (`<EMn>` tags). Named by player-visible intent
/// in the contracts panel — only `Underline` (EM3) and `Highlight` (EM4)
/// render distinctly there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Plain,
    Faint,
    Soft,
    Underline,
    Highlight,
}

impl Color {
    fn tag(&self) -> &'static str {
        match self {
            Self::Plain => "EM0",
            Self::Faint => "EM1",
            Self::Soft => "EM2",
            Self::Underline => "EM3",
            Self::Highlight => "EM4",
        }
    }
}

/// Wrap `text` in a color tag pair.
pub fn apply_color(color: Color, text: impl AsRef<str>) -> String {
    let tag = color.tag();
    format!("<{tag}>{}</{tag}>", text.as_ref())
}

/// Standard section-header label — wrapped in `Color::Highlight`.
pub fn header(label: impl AsRef<str>) -> String {
    apply_color(Color::Highlight, label)
}

/// Wrap text in square brackets — the title-tag convention (`[BP]`, …).
pub fn bracket(label: impl AsRef<str>) -> String {
    format!("[{}]", label.as_ref())
}

/// A list-item line: `"- {text}"`.
pub fn bullet(text: impl AsRef<str>) -> String {
    format!("- {}", text.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers_match_langpatch_shapes() {
        assert_eq!(header("Mission Info"), "<EM4>Mission Info</EM4>");
        assert_eq!(apply_color(Color::Underline, "x"), "<EM3>x</EM3>");
        assert_eq!(bracket("BP"), "[BP]");
        assert_eq!(bullet("Bracer"), "- Bracer");
    }
}
