//! The content filter — which staff posts are dev news.
//!
//! A port of the sc-grpc prototype (`.scratch/spectrum-search/tools/
//! chat_filter.py`, "v2"), tuned on 90 days of the staff feed (2026-07-15 to
//! 10-13: 3,336 posts, 3,001 of them chat). Forum posts always pass. Chat:
//!
//! 1. drop when fewer than [`MIN_CHARS`] remain after stripping mentions,
//!    emoji shortcodes and links;
//! 2. drop on moderation / channel policing / "not my department";
//! 3. keep on one **strong** game-ops signal (PTU, case-sensitive `LIVE`,
//!    hotfix, versions, build and shard ids, maintenance, known issue, "team
//!    is investigating", …);
//! 4. else keep on two distinct **weak** signals (bug, fix, crash, build,
//!    server, …), or one in a testing lobby, where the lobby's own vocabulary
//!    ("test", "devs", "changes") never counts alone.
//!
//! Generic words are weak on purpose: they all show up in small talk. Blind
//! sample: precision ~87%, recall ~75-80% — which is why the page has a
//! "show filtered-out" toggle. The regexes are kept textually close to the
//! prototype so the two can be diffed when it is re-tuned.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use regex::Regex;

/// Chat shorter than this (in characters, after stripping mentions, emoji
/// shortcodes and links) is a reaction, not news.
pub const MIN_CHARS: usize = 15;

/// Lobbies where staff chat is testing-focused by policy: one weak signal
/// suffices there.
pub const TESTING_LOBBIES: &[&str] = &["sc-testing-chat"];

/// Moderation, channel policing, "not my area" → always drop.
static MODERATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)(
    please\s+(refrain|choose|avoid|keep|don'?t|do\ not|aim) | choose\s+(to\s+avoid|more\s+suitable) |
    look\s+to\s+avoid | avoid\s+(these|this|that|arguing|the\ off) | naming\s+and\s+shaming |
    follow\s+the\s+(thoughtful\s+)?advice | forum\s+rules | not\s+a\s+suitable\s+place |
    (outside|not)\s+(of\s+)?my\s+area | my\s+focus\s+(for|is) | better\s+question\s+for |
    keep\s+it\s+(friendly|civil|on[-\ ]topic) | needs?\s+to\s+be\s+civil | be\s+respectful |
    reserved\s+for\s+testing | speculat | parroting | real-world |
    off[-\ ]topic | chit[-\ ]?chat | derail | refocus | back\s+on\s+track | regroup |
    (this|the)\s+(channel|chat)('s)?\s+(has|is|purpose|focus|isn) | specific\s+focus |
    (i'?m|i\s+am)\s+on\s+the\s+community\s+team | don'?t\s+(really\s+)?have\s+any\s+involvement |
    mod(eration)?\s+actions | (insulting|inappropriate)\s+(comments|speculations) |
    testing[\s-](focused|specific|discussions?|subjects?|chat) | snark | bickering | discontinue |
    unproductive | keep\s+the\s+(comments|content) | questions?\s+better\s+asked | cooler\s+heads |
    customer\s+service | uncivil
)",
    )
    .expect("moderation regex")
});

/// `LIVE` counts only in capitals — "live" is everyday English.
static STRONG_LIVE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bLIVE\b").expect("LIVE regex"));

/// Strong game-ops signals: one is enough.
static STRONG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)(
    \bptu\b | \bhotfix | tech[\s-]?preview | \bLTP\b |
    patch\s*notes | \b\d\.\d{1,2}\.\d+\b | \b\d{8}(_\d{3})?\b |
    maintenance | downtime | servers?\s+(will|are|is|going|offline|online|side|back) |
    \bshard(s|locked)?\b | \b(use|usw|euw|euc|apse|aps)\d\w* |
    rolled?\s+out | rolling\s+(out|back) | available\s+for\s+download | \bwave\s+\d | wave\s+(1|2|3|one|two) |
    known\s+issue | workaround | working\s+on\s+a\s+fix | temporarily\s+disabled | status\s+page |
    (poked|pinged|told|informed)\s+the\s+teams? | (raised|flagged|passed)\s+(it|this|that)?\s*(on|along|internally|to\s+the) |
    internally | \bmotd\b | \b(raised|flagged|escalated)\s+(it|this|that)\b | follow\s+up\s+on |
    \b0[1-9]0\b | \bQA\b | server\s+captures | the\s+devs?\s+(managed|fixed|are|have) |
    (we('re|\s+are)|team\s+is|devs?\s+(is|are))\s+(already\s+)?(aware|looking\s+into|investigating) |
    (open|opening)\s+(for|to)\s+evocati | evocati\s+wave | focus\s+test | playtest |
    # French ops
    correctifs? | d[ée]ploie?ment | d[ée]ployer | serveurs | remont[ée]\s+(le|les|ton|aux) | j'ai\s+track[ée]
)",
    )
    .expect("strong regex")
});

/// Weak signals: two distinct ones are needed (one in a testing lobby).
static WEAK: LazyLock<Vec<(&'static str, Regex)>> = LazyLock::new(|| {
    [
        ("bug", r"\bbugs?\b"),
        ("fix", r"\bfix(ed|es|ing)?\b"),
        ("crash", r"\bcrash"),
        ("build", r"\bbuilds?\b"),
        ("server", r"\bservers?\b"),
        ("deploy", r"\bdeploy"),
        ("ic", r"issue\s+council"),
        ("roadmap", r"roadmap"),
        ("launcher", r"launcher"),
        ("promo", r"\bpromo\b"),
        ("investig", r"investigat|look(ing)?\s+into"),
        ("client", r"\bclient\b"),
        ("news", r"(more\s+)?news\s+(on|about)|no\s+news"),
        ("patch", r"\bpatch\b"),
        ("release", r"\breleas"),
        ("test", r"\btest(ing)?\b"),
        ("version", r"\b4\.\d{1,2}\b|alpha\s+\d\.\d+"),
        ("issue", r"\bissues?\b"),
        ("devs", r"\bdevs?\b|development"),
        ("changes", r"\bchanges?\b"),
    ]
    .into_iter()
    .map(|(k, p)| (k, Regex::new(&format!("(?i){p}")).expect("weak regex")))
    .collect()
});

/// A testing lobby's own vocabulary: never enough alone.
const LOBBY_VOCABULARY: &[&str] = &["test", "devs", "changes"];

/// What remains of a chat line once mentions, emoji shortcodes and links go.
static NOISE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(@\S+|:[a-z0-9_+-]+:|https?://\S+)").expect("noise regex"));

/// Why the filter decided what it did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    /// A forum post — always news.
    Forum,
    /// Kept on a strong signal (the matched text).
    Strong(String),
    /// Kept on weak signals (their names, sorted).
    Weak(Vec<&'static str>),
    /// Dropped: too short once mentions, emoji and links are gone.
    Short,
    /// Dropped: moderation or channel policing.
    Moderation,
    /// Dropped: no game-ops signal.
    NoSignal,
}

/// The filter's decision on one post.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub keep: bool,
    pub reason: Reason,
}

impl Reason {
    /// A short user-facing label ("strong: PTU", "moderation").
    pub fn label(&self) -> String {
        match self {
            Reason::Forum => "forum post".into(),
            Reason::Strong(m) => format!("ops signal: {m}"),
            Reason::Weak(ws) => format!("signals: {}", ws.join(", ")),
            Reason::Short => "too short".into(),
            Reason::Moderation => "moderation".into(),
            Reason::NoSignal => "no ops signal".into(),
        }
    }
}

/// The body as plain text: HTML entities decoded, one line, trimmed.
/// Spectrum search delivers plain text with entities escaped (`&#039;`).
pub fn plain_text(body: &str) -> String {
    decode_entities(body).replace('\n', " ").trim().to_owned()
}

/// The first strong signal in `text`, if any — `LIVE` (case-sensitive)
/// first, as in the prototype.
pub fn strong_signal(text: &str) -> Option<String> {
    STRONG_LIVE
        .find(text)
        .or_else(|| STRONG.find(text))
        .map(|m| m.as_str().to_owned())
}

fn weak_signals(text: &str) -> BTreeSet<&'static str> {
    WEAK.iter()
        .filter(|(_, re)| re.is_match(text))
        .map(|(k, _)| *k)
        .collect()
}

/// Classify one post. `is_chat` = a lobby message (forum threads and replies
/// always pass); `lobby` = the chat lobby's name.
pub fn classify(is_chat: bool, body: &str, lobby: Option<&str>) -> Verdict {
    let keep = |reason| Verdict { keep: true, reason };
    let drop = |reason| Verdict {
        keep: false,
        reason,
    };
    if !is_chat {
        return keep(Reason::Forum);
    }
    let text = plain_text(body);
    if NOISE.replace_all(&text, "").trim().chars().count() < MIN_CHARS {
        return drop(Reason::Short);
    }
    if MODERATION.is_match(&text) {
        return drop(Reason::Moderation);
    }
    if let Some(m) = strong_signal(&text) {
        return keep(Reason::Strong(m));
    }
    let weak = weak_signals(&text);
    let testing = lobby.is_some_and(|l| TESTING_LOBBIES.contains(&l));
    if testing && weak.iter().all(|w| LOBBY_VOCABULARY.contains(w)) {
        return drop(Reason::NoSignal);
    }
    let need = if testing { 1 } else { 2 };
    if weak.len() >= need {
        return keep(Reason::Weak(weak.into_iter().collect()));
    }
    drop(Reason::NoSignal)
}

/// Decode the HTML entities Spectrum escapes: numeric ones and the five XML
/// names (plus `&nbsp;`). Anything else is left as written.
pub fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let decoded = rest.find(';').filter(|&end| end <= 10).and_then(|end| {
            let entity = &rest[1..end];
            let ch = match entity {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some('\u{a0}'),
                _ => entity.strip_prefix('#').and_then(|num| {
                    let code = match num.strip_prefix(['x', 'X']) {
                        Some(hex) => u32::from_str_radix(hex, 16).ok(),
                        None => num.parse().ok(),
                    };
                    code.and_then(char::from_u32)
                }),
            };
            ch.map(|c| (c, end))
        });
        match decoded {
            Some((c, end)) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chat(body: &str) -> Verdict {
        classify(true, body, Some("general"))
    }

    #[test]
    fn forum_posts_always_pass() {
        assert_eq!(classify(false, "hi", None).reason, Reason::Forum);
    }

    #[test]
    fn short_chat_is_dropped_after_stripping_noise() {
        assert_eq!(chat("@Someone :thumbsup: thanks!").reason, Reason::Short);
    }

    #[test]
    fn moderation_beats_ops_words() {
        let v = chat("Please refrain from posting PTU leaks in this channel, thanks.");
        assert_eq!(v.reason, Reason::Moderation);
    }

    #[test]
    fn live_counts_only_in_capitals() {
        assert_eq!(
            chat("The new build is going to LIVE later today.").reason,
            Reason::Strong("LIVE".into())
        );
        assert_eq!(
            chat("I live in a hangar with my cat and stuff").reason,
            Reason::NoSignal
        );
    }

    #[test]
    fn one_strong_signal_is_enough() {
        let v = chat("Servers will be down for maintenance at 14:00 UTC.");
        assert!(v.keep);
        assert!(matches!(v.reason, Reason::Strong(_)));
    }

    #[test]
    fn weak_signals_need_two_outside_testing_lobbies() {
        assert_eq!(
            chat("That bug is a funny one honestly mate").reason,
            Reason::NoSignal
        );
        assert_eq!(
            chat("That bug with the elevator should be fixed by now").reason,
            Reason::Weak(vec!["bug", "fix"])
        );
    }

    #[test]
    fn testing_lobby_needs_one_weak_signal_but_not_its_own_vocabulary() {
        let testing = |b| classify(true, b, Some("sc-testing-chat"));
        assert!(testing("Anyone else seeing this crash on quantum?").keep);
        assert_eq!(
            testing("Thanks for testing everyone, the devs appreciate it").reason,
            Reason::NoSignal
        );
    }

    #[test]
    fn entities_are_decoded() {
        assert_eq!(
            plain_text("it&#039;s &amp; &lt;b&gt; &#x41;\n"),
            "it's & <b> A"
        );
        assert_eq!(plain_text("AT&T &unknown; & done"), "AT&T &unknown; & done");
    }
}
