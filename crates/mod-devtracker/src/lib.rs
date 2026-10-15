//! Dev tracker — CIG staff posts from Spectrum, the chat the official dev
//! tracker never shows included, filtered by content.
//!
//! The feed comes from `svc-spectrum` (module rule 1: no network, no files
//! here — the shell persists [`DevtrackerConfig`] and [`ReadState`] and runs
//! the poll). This crate is the module's judgement:
//!
//! - [`filter`] — the content filter (which staff chat is dev news);
//! - [`AuthorMode`] — the user's per-author override on top of it: always
//!   show, filter by content (the default), or mute;
//! - [`ReadState`] — what the user has seen;
//! - the follow list — the user's curated accounts, seeded with
//!   [`SEED_AUTHORS`] (the known staff) until the user edits it;
//! - [`patch`] — patch-note threads and what their titles say (version,
//!   channel, build).
//!
//! The defaults are data: resetting the follow list restores the seed and
//! clears the overrides, and every author falls back to [`AuthorMode::Filter`].

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use svc_spectrum::{FeedPost, PostKind};

pub mod filter;
pub mod patch;

pub use filter::{Reason, Verdict};

/// How one staff account's posts are treated.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthorMode {
    /// Every post shows, chat included, whatever the content filter says.
    Always,
    /// The content filter decides (the default).
    #[default]
    Filter,
    /// Nothing from this account shows (except with "show filtered-out").
    Mute,
}

/// The module's settings. Persisted by the shell as one JSON file; additive
/// like `AppSettings` (add fields, never rename).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DevtrackerConfig {
    /// Per-author overrides by Spectrum member id. Absent = [`AuthorMode::Filter`].
    pub authors: BTreeMap<String, AuthorMode>,
    /// Raise a notification for new posts with a strong game-ops signal
    /// (maintenance, PTU waves, hotfixes, known issues).
    pub notify_ops: bool,
    /// The follow list, in the user's order. `None` = the defaults
    /// ([`SEED_AUTHORS`]); the first edit stores the whole list.
    pub follows: Option<Vec<Followed>>,
}

/// One account on the follow list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Followed {
    /// Spectrum member id — the stable key.
    pub id: String,
    /// Handle when it was added (the feed's current one wins for display).
    pub nickname: String,
}

impl Default for DevtrackerConfig {
    fn default() -> Self {
        Self {
            authors: BTreeMap::new(),
            notify_ops: true,
            follows: None,
        }
    }
}

impl DevtrackerConfig {
    pub fn mode(&self, member_id: &str) -> AuthorMode {
        self.authors.get(member_id).copied().unwrap_or_default()
    }

    /// Set one author's mode. The default is stored as absence, so a reset
    /// and "set back to Filter" leave the same file.
    pub fn set_mode(&mut self, member_id: &str, mode: AuthorMode) {
        if mode == AuthorMode::Filter {
            self.authors.remove(member_id);
        } else {
            self.authors.insert(member_id.to_owned(), mode);
        }
    }

    /// The follow list: the user's, or the seed until they edit it.
    pub fn follow_list(&self) -> Vec<Followed> {
        self.follows.clone().unwrap_or_else(|| {
            SEED_AUTHORS
                .iter()
                .map(|(id, nickname)| Followed {
                    id: (*id).to_owned(),
                    nickname: (*nickname).to_owned(),
                })
                .collect()
        })
    }

    pub fn is_followed(&self, member_id: &str) -> bool {
        self.follow_list().iter().any(|f| f.id == member_id)
    }

    /// Add an account to the end of the list (no-op when it is on it).
    pub fn follow(&mut self, member_id: &str, nickname: &str) {
        let mut list = self.follow_list();
        if !list.iter().any(|f| f.id == member_id) {
            list.push(Followed {
                id: member_id.to_owned(),
                nickname: nickname.to_owned(),
            });
        }
        self.follows = Some(list);
    }

    /// Take an account off the list. Its override goes too: an account off
    /// the list is filtered by content like any other.
    pub fn unfollow(&mut self, member_id: &str) {
        let mut list = self.follow_list();
        list.retain(|f| f.id != member_id);
        self.follows = Some(list);
        self.authors.remove(member_id);
    }

    /// Back to the defaults: the seeded list, no overrides.
    pub fn reset_follows(&mut self) {
        self.follows = None;
        self.authors.clear();
    }
}

/// Why a post is (not) on the feed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    /// On the feed (without "show filtered-out").
    pub shown: bool,
    /// The author's mode at the time.
    pub mode: AuthorMode,
    /// The content filter's verdict (computed whatever the mode, so the page
    /// can say why a filtered-out post was dropped).
    pub verdict: Verdict,
    /// A strong game-ops signal in the post — the notification trigger.
    pub ops: bool,
}

/// Decide one post under the config.
pub fn decide(post: &FeedPost, config: &DevtrackerConfig) -> Decision {
    let mode = config.mode(&post.author.id);
    let is_chat = post.kind == PostKind::Chat;
    let verdict = filter::classify(
        is_chat,
        &post.body,
        post.lobby.as_ref().map(|l| l.name.as_str()),
    );
    let shown = match mode {
        AuthorMode::Always => true,
        AuthorMode::Mute => false,
        AuthorMode::Filter => verdict.keep,
    };
    // A forum post's subject carries the signal as often as its body
    // ("Star Citizen Alpha 4.4 PTU Patch Notes").
    let subject = post
        .thread
        .as_ref()
        .map(|t| t.subject.as_str())
        .unwrap_or("");
    let ops = shown
        && verdict.reason != Reason::Moderation
        && filter::strong_signal(&filter::plain_text(&format!("{subject} {}", post.body)))
            .is_some();
    Decision {
        shown,
        mode,
        verdict,
        ops,
    }
}

/// The key a post is read under: kind + id (ids are unique per kind only).
pub fn read_key(post: &FeedPost) -> String {
    let kind = match post.kind {
        PostKind::Chat => "chat",
        PostKind::Thread => "thread",
        PostKind::Reply => "reply",
    };
    format!("{kind}:{}", post.id)
}

/// What the user has seen: everything up to a watermark, plus single posts
/// read after it. Persisted by the shell (frontend rule 6: nothing durable
/// lives only in JS).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ReadState {
    /// Posts created at or before this (Unix ms) are read. `0` = never set.
    pub read_through_ms: i64,
    /// Read keys ([`read_key`]) of posts newer than the watermark.
    pub read: BTreeSet<String>,
}

impl ReadState {
    pub fn is_read(&self, post: &FeedPost) -> bool {
        post.time_ms <= self.read_through_ms || self.read.contains(&read_key(post))
    }

    pub fn mark(&mut self, key: String) {
        self.read.insert(key);
    }

    /// Everything up to `through_ms` is read; single marks below it go.
    pub fn mark_all(&mut self, through_ms: i64, posts: &[FeedPost]) {
        self.read_through_ms = self.read_through_ms.max(through_ms);
        let older: BTreeSet<String> = posts
            .iter()
            .filter(|p| p.time_ms <= self.read_through_ms)
            .map(read_key)
            .collect();
        self.read.retain(|k| !older.contains(k));
    }
}

/// Known staff accounts (Spectrum member id, nickname), most active first —
/// the staff tally of the 90-day sample the filter was tuned on (2026-07-15
/// to 10-13). Seeds the follow list; accounts the feed sees later join it.
pub const SEED_AUTHORS: &[(&str, &str)] = &[
    ("419208", "Nightrider-CIG"),
    ("310556", "Synthwave-CIG"),
    ("6257233", "Keanis-CIG"),
    ("4713987", "Nicou-CIG"),
    ("3736518", "Swordfish_CIG"),
    ("429390", "Wakapedia-CIG"),
    ("4094960", "Freyja-CIG"),
    ("530431", "Shark-CIG"),
    ("6335183", "Nebula-Ronin-CIG"),
    ("5711108", "Liryn-CIG"),
    ("581144", "Silvan-CIG"),
    ("2521973", "BigTO-CIG"),
    ("5078956", "mkale-CIG"),
    ("6211466", "500servererror-CIG"),
    ("3615334", "LoujoD"),
    ("4392587", "Armeggadon-CIG"),
    ("4178", "Spectral-CIG"),
    ("5697405", "Voidspire-CIG"),
    ("6642", "Zyloh-CIG"),
    ("25252", "CIG_Tiki_Bartender"),
    ("2690759", "Highlander-CIG"),
    ("3538916", "Stamp-CIG"),
    ("6575261", "Caelvyn-CIG"),
    ("4040073", "Wiaz-CIG"),
    ("5199977", "nschiro"),
    ("3126689", "XLB-CIG"),
    ("3154801", "ZacPreece_CIG"),
    ("3567739", "TheDJBuntin-CIG"),
    ("4180203", "Raven-CIG"),
    ("153733", "Baior-CIG"),
    ("3287043", "Hyperion-Turb"),
    ("3386804", "Fil-CIG"),
    ("28507", "CIG-SeanT"),
    ("1", "Bault"),
    ("478", "Proxus-CIG"),
    ("3129993", "Taqwakore"),
    ("400237", "PeTong-CIG"),
    ("211229", "Skelting-CIG"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use svc_spectrum::{Author, Place, ThreadRef};

    fn post(kind: PostKind, id: &str, author: &str, body: &str, time_ms: i64) -> FeedPost {
        FeedPost {
            id: id.into(),
            kind,
            community_slug: "SC".into(),
            author: Author {
                id: author.into(),
                nickname: format!("{author}-CIG"),
                displayname: author.into(),
                is_gm: true,
            },
            body: body.into(),
            highlight_role_id: Some(2),
            time_created: String::new(),
            time_ms,
            lobby: (kind == PostKind::Chat).then(|| Place {
                id: "1".into(),
                name: "general".into(),
            }),
            channel: None,
            thread: (kind != PostKind::Chat).then(|| ThreadRef {
                id: "t".into(),
                slug: "s".into(),
                subject: "Star Citizen Alpha 4.4 PTU Patch Notes".into(),
                content_reply_id: None,
            }),
            url: String::new(),
            context: None,
            context_checked: false,
        }
    }

    #[test]
    fn author_mode_overrides_the_filter() {
        let small_talk = post(
            PostKind::Chat,
            "1",
            "a",
            "Good morning everyone, have a nice day",
            1,
        );
        let mut config = DevtrackerConfig::default();
        assert!(!decide(&small_talk, &config).shown);
        config.set_mode("a", AuthorMode::Always);
        assert!(decide(&small_talk, &config).shown);
        config.set_mode("a", AuthorMode::Mute);
        let ops = post(
            PostKind::Chat,
            "2",
            "a",
            "Servers will be down for maintenance soon",
            2,
        );
        assert!(!decide(&ops, &config).shown);
        // Back to the default is stored as absence.
        config.set_mode("a", AuthorMode::Filter);
        assert!(config.authors.is_empty());
    }

    #[test]
    fn ops_reads_the_forum_subject_and_needs_the_post_shown() {
        let config = DevtrackerConfig::default();
        let reply = post(PostKind::Reply, "1", "a", "Thanks all for the feedback!", 1);
        assert!(decide(&reply, &config).ops);
        let chat = post(
            PostKind::Chat,
            "2",
            "a",
            "Good morning everyone, have a nice day",
            2,
        );
        assert!(!decide(&chat, &config).ops);
    }

    #[test]
    fn follow_list_starts_as_the_seed_and_edits_store_it() {
        let mut config = DevtrackerConfig::default();
        assert_eq!(config.follow_list().len(), SEED_AUTHORS.len());
        assert!(config.is_followed("429390"));
        config.set_mode("429390", AuthorMode::Always);
        config.unfollow("429390");
        assert!(!config.is_followed("429390"));
        assert_eq!(
            config.mode("429390"),
            AuthorMode::Filter,
            "override goes with it"
        );
        config.follow("42", "New-CIG");
        config.follow("42", "New-CIG");
        let list = config.follow_list();
        assert_eq!(list.len(), SEED_AUTHORS.len());
        assert_eq!(list.last().unwrap().id, "42");
        config.reset_follows();
        assert!(config.follows.is_none());
        assert!(config.is_followed("429390"));
    }

    #[test]
    fn read_state_watermark_and_single_marks() {
        let posts = vec![
            post(PostKind::Chat, "1", "a", "x", 10),
            post(PostKind::Chat, "2", "a", "x", 20),
            post(PostKind::Reply, "2", "a", "x", 30),
        ];
        let mut read = ReadState::default();
        read.mark(read_key(&posts[1]));
        assert!(read.is_read(&posts[1]));
        // Same id, other kind: not read.
        assert!(!read.is_read(&posts[2]));
        read.mark_all(20, &posts);
        assert!(read.is_read(&posts[0]));
        assert!(read.read.is_empty(), "marks under the watermark are pruned");
        assert!(!read.is_read(&posts[2]));
    }
}
