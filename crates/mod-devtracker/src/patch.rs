//! Patch notes — CIG posts them as thread openers in Spectrum's "Patch
//! Notes" forum, and the staff feed carries them. The titles are regular
//! enough to read: "[All Backer] Star Citizen Alpha 4.10.2 PTU RC2 Patch
//! Notes 12878398", "Star Citizen Alpha 4.10.2 LIVE Release Notes".

use std::sync::LazyLock;

use regex::Regex;
use svc_spectrum::{FeedPost, PostKind};

/// The SC community's "Patch Notes" forum channel.
pub const PATCH_NOTES_CHANNEL: &str = "190048";

/// What a patch-note title says. Every part is optional: titles drift.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PatchNote {
    /// Game version, e.g. `"4.10.2"`.
    pub version: Option<String>,
    /// Release channel as written in the launcher: `"LIVE"`, `"PTU"`,
    /// `"EPTU"`, `"Tech Preview"`, `"Hotfix"`, `"Evocati"`.
    pub channel: Option<String>,
    /// Build changelist, e.g. `"12878398"`.
    pub build: Option<String>,
    /// Release candidate or wave, e.g. `"RC2"`, `"Wave 1"`.
    pub stage: Option<String>,
}

static VERSION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\balpha\s+(\d+\.\d+(?:\.\d+)?[a-z]?)").expect("version regex")
});
static CHANNEL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(eptu|ptu|live|tech[\s-]?preview|hotfix|evocati)\b").expect("channel regex")
});
static BUILD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(\d{8})\b").expect("build regex"));
static STAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(rc\s?\d+|wave\s?\d+)\b").expect("stage regex"));

/// Read a patch-note title.
pub fn parse_title(subject: &str) -> PatchNote {
    let group = |re: &Regex| re.captures(subject).map(|c| c[1].to_owned());
    PatchNote {
        version: group(&VERSION),
        channel: group(&CHANNEL).map(|c| match c.to_lowercase().as_str() {
            "live" => "LIVE".to_owned(),
            "ptu" => "PTU".to_owned(),
            "eptu" => "EPTU".to_owned(),
            "hotfix" => "Hotfix".to_owned(),
            "evocati" => "Evocati".to_owned(),
            _ => "Tech Preview".to_owned(),
        }),
        build: group(&BUILD),
        stage: group(&STAGE).map(|s| {
            let s = s.to_uppercase().replace(' ', "");
            match s.strip_prefix("WAVE") {
                Some(n) => format!("Wave {n}"),
                None => s,
            }
        }),
    }
}

/// The patch note a post is, if it is one: a thread opener in the Patch
/// Notes forum.
pub fn patch_note(post: &FeedPost) -> Option<PatchNote> {
    let in_channel = post
        .channel
        .as_ref()
        .is_some_and(|c| c.id == PATCH_NOTES_CHANNEL);
    if post.kind != PostKind::Thread || !in_channel {
        return None;
    }
    Some(parse_title(&post.thread.as_ref()?.subject))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_ptu_titles() {
        let n = parse_title("[All Backer] Star Citizen Alpha 4.10.2 PTU RC2 Patch Notes 12878398");
        assert_eq!(n.version.as_deref(), Some("4.10.2"));
        assert_eq!(n.channel.as_deref(), Some("PTU"));
        assert_eq!(n.build.as_deref(), Some("12878398"));
        assert_eq!(n.stage.as_deref(), Some("RC2"));
    }

    #[test]
    fn reads_live_release_notes_without_a_build() {
        let n = parse_title("Star Citizen Alpha 4.10.2 LIVE Release Notes");
        assert_eq!(n.version.as_deref(), Some("4.10.2"));
        assert_eq!(n.channel.as_deref(), Some("LIVE"));
        assert_eq!(n.build, None);
        assert_eq!(n.stage, None);
    }

    #[test]
    fn eptu_is_not_read_as_ptu_and_waves_normalise() {
        let n = parse_title("[Wave 1] Star Citizen Alpha 4.11.0 EPTU Patch Notes 13000001");
        assert_eq!(n.channel.as_deref(), Some("EPTU"));
        assert_eq!(n.stage.as_deref(), Some("Wave 1"));
        assert_eq!(
            parse_title("Tech-Preview build notes").channel.as_deref(),
            Some("Tech Preview")
        );
    }
}
