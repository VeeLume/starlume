//! The cached feed and the merge/coverage bookkeeping of a fetch run.
//!
//! A *run* is one call's sequence of search pages, newest first. Each page is
//! deduped into the [`Feed`]; when the run ends, the feed's coverage — how far
//! back it is known to be gap-free — is recomputed from what the run saw.
//! Coverage is what lets a backfill stop early: once a run bridges into a
//! region the cache already holds without gaps, and that region reaches the
//! cutoff, there is nothing left to fetch. Pure logic, no IO — the service
//! feeds it pages.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

/// Hits per search page (the server's fixed page size).
pub const PAGE_SIZE: usize = 25;

/// Posts the on-disk feed keeps, newest first. A few weeks of the staff feed;
/// disk only, never resident (the feed is loaded per call and dropped).
pub const MAX_CACHED_POSTS: usize = 5000;

/// One year in milliseconds — the server window a `range: "year"` search
/// covers, so an exhausted run is complete back to here.
const YEAR_MS: i64 = 365 * 24 * 60 * 60 * 1000;

/// The staff feed as cached on disk — every post the service has seen, deduped
/// and newest first. Unfiltered: the content filter is the consumer's.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Feed {
    /// Posts, newest first, unique by [`FeedPost::key`].
    pub posts: Vec<FeedPost>,
    /// When the last fetch completed, Unix seconds. `0` = never (treated as
    /// no cache — see `SpectrumService::cached_feed`).
    pub fetched_at: i64,
    /// Coverage: every staff post from this time (Unix ms) up to the newest
    /// cached post is in [`Feed::posts`] — the feed has no gap there. `None`
    /// = no such guarantee. Errs on the side of claiming less.
    pub covered_since_ms: Option<i64>,
}

/// What a post is (mirrors [`sc_dossier::spectrum::PostKind`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PostKind {
    /// A chat lobby message.
    Chat,
    /// A forum thread opener.
    Thread,
    /// A forum reply.
    Reply,
}

/// A cached Spectrum post — the serializable projection of
/// [`sc_dossier::spectrum::Post`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeedPost {
    /// Message / thread / reply id. Unique **within its kind** — see
    /// [`FeedPost::key`].
    pub id: String,
    /// What this is.
    pub kind: PostKind,
    /// Community slug (e.g. `"SC"`).
    pub community_slug: String,
    /// Author.
    pub author: Author,
    /// Text as delivered: plain text with HTML entities escaped (`&#039;`).
    pub body: String,
    /// Role the post is highlighted with (`2` = staff in the SC community).
    pub highlight_role_id: Option<u32>,
    /// Creation time, ISO 8601 UTC.
    pub time_created: String,
    /// Creation time, Unix milliseconds — the feed's sort key.
    pub time_ms: i64,
    /// The chat lobby, for [`PostKind::Chat`].
    pub lobby: Option<Place>,
    /// The forum channel, for threads and replies.
    pub channel: Option<Place>,
    /// The forum thread, for threads and replies.
    pub thread: Option<ThreadRef>,
    /// Link to the post on the website.
    pub url: String,
}

/// A post's author.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Author {
    /// Member id.
    pub id: String,
    /// Handle, e.g. `"Wakapedia-CIG"`.
    pub nickname: String,
    /// Display name.
    pub displayname: String,
    /// The `isGM` flag.
    pub is_gm: bool,
}

/// A chat lobby or forum channel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Place {
    /// Id.
    pub id: String,
    /// Display name.
    pub name: String,
}

/// The forum thread a thread opener or reply belongs to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThreadRef {
    /// Thread id.
    pub id: String,
    /// URL slug.
    pub slug: String,
    /// Title.
    pub subject: String,
    /// Id of the opener's content reply — what RSI's dev tracker links to.
    pub content_reply_id: Option<String>,
}

impl FeedPost {
    /// Dedup key. Spectrum ids are unique only within a kind (chat messages,
    /// threads and replies are separate id spaces), so the key is both.
    pub fn key(&self) -> (PostKind, &str) {
        (self.kind, &self.id)
    }
}

impl From<&sc_dossier::spectrum::Post> for FeedPost {
    fn from(p: &sc_dossier::spectrum::Post) -> Self {
        use sc_dossier::spectrum::PostKind as K;
        let place = |pl: &sc_dossier::spectrum::Place| Place {
            id: pl.id.clone(),
            name: pl.name.clone(),
        };
        Self {
            id: p.id.clone(),
            kind: match p.kind {
                K::Chat => PostKind::Chat,
                K::Thread => PostKind::Thread,
                K::Reply => PostKind::Reply,
            },
            community_slug: p.community_slug.clone(),
            author: Author {
                id: p.member.id.clone(),
                nickname: p.member.nickname.clone(),
                displayname: p.member.displayname.clone(),
                is_gm: p.member.is_gm,
            },
            body: p.body.clone(),
            highlight_role_id: p.highlight_role_id,
            time_created: p.time_created.clone(),
            time_ms: p.time_ms,
            lobby: p.lobby.as_ref().map(place),
            channel: p.channel.as_ref().map(place),
            thread: p.thread.as_ref().map(|t| ThreadRef {
                id: t.id.clone(),
                slug: t.slug.clone(),
                subject: t.subject.clone(),
                content_reply_id: t.content_reply_id.clone(),
            }),
            url: p.url.clone(),
        }
    }
}

type Key = (PostKind, String);

fn owned_key(p: &FeedPost) -> Key {
    (p.kind, p.id.clone())
}

/// Newest first; ties broken by kind and id so the order is stable.
fn sort_newest_first(posts: &mut [FeedPost]) {
    posts.sort_by(|a, b| {
        b.time_ms
            .cmp(&a.time_ms)
            .then_with(|| b.kind.cmp_key().cmp(&a.kind.cmp_key()))
            .then_with(|| b.id.cmp(&a.id))
    });
}

impl PostKind {
    fn cmp_key(self) -> u8 {
        match self {
            PostKind::Chat => 0,
            PostKind::Thread => 1,
            PostKind::Reply => 2,
        }
    }
}

/// What a fetch did to the feed.
#[derive(Debug, Clone, Default)]
pub struct FetchReport {
    /// Posts the cache didn't have before, newest first — what a consumer
    /// notifies about.
    pub added: Vec<FeedPost>,
    /// Posts already cached and fetched again (edits replace the cached copy).
    pub updated: usize,
    /// Search pages requested.
    pub pages: u32,
    /// The feed's coverage after the run ([`Feed::covered_since_ms`]).
    pub covered_since_ms: Option<i64>,
}

/// How a run decides it is done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Catch up from the top: stop at the first page that meets the cache.
    Newest,
    /// Page back until the feed covers everything since `cutoff` (Unix ms).
    Backfill {
        /// Posts older than this are dropped, and reaching it ends the run.
        cutoff: i64,
    },
}

/// One fetch run's bookkeeping. Build with [`Run::new`] before the first page,
/// [`Run::absorb`] each page, [`Run::finish`] once (also after an error — what
/// was absorbed is still valid).
pub(crate) struct Run {
    mode: Mode,
    /// The feed's coverage before the run.
    covered_since: Option<i64>,
    /// Keys of the posts inside that coverage — meeting one means the run
    /// joined the gap-free region.
    covered_keys: HashSet<Key>,
    /// Keys of every post cached before the run.
    known_keys: HashSet<Key>,
    /// Some page met the gap-free region.
    bridged: bool,
    /// A page went past the cutoff.
    reached_cutoff: bool,
    /// A page came back short — the server has nothing older in its window.
    exhausted: bool,
    /// Oldest kept post of the run's contiguous pages.
    oldest_kept: Option<i64>,
    added: Vec<FeedPost>,
    updated: usize,
    pages: u32,
}

/// What the run should do after a page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Next {
    /// Fetch the following page.
    Continue,
    /// Done.
    Stop,
}

impl Run {
    pub(crate) fn new(feed: &Feed, mode: Mode) -> Self {
        let covered_since = feed.covered_since_ms;
        let covered_keys = match covered_since {
            Some(c) => feed
                .posts
                .iter()
                .filter(|p| p.time_ms >= c)
                .map(owned_key)
                .collect(),
            None => HashSet::new(),
        };
        Self {
            mode,
            covered_since,
            covered_keys,
            known_keys: feed.posts.iter().map(owned_key).collect(),
            bridged: false,
            reached_cutoff: false,
            exhausted: false,
            oldest_kept: None,
            added: Vec::new(),
            updated: 0,
            pages: 0,
        }
    }

    fn cutoff(&self) -> Option<i64> {
        match self.mode {
            Mode::Newest => None,
            Mode::Backfill { cutoff } => Some(cutoff),
        }
    }

    /// The page a backfill may jump to after page 1 bridged into the
    /// gap-free region: deep inside that region, so the pages between (all
    /// cached) are skipped. `None` when no jump saves a request. The jump
    /// is verified — see [`Run::verify_jump`].
    pub(crate) fn jump_page(&self) -> Option<u32> {
        let (Mode::Backfill { cutoff }, Some(covered)) = (self.mode, self.covered_since) else {
            return None;
        };
        if !self.bridged || self.pages != 1 || covered <= cutoff {
            return None;
        }
        // New posts above the cache push the region down, so aim at its
        // lower part; a page that misses it is discarded.
        let page = (self.covered_keys.len() / PAGE_SIZE) as u32;
        (page >= 3).then_some(page)
    }

    /// Whether a jumped-to page landed in the gap-free region. A page that
    /// didn't must be discarded (not absorbed) and the run continue at page 2.
    pub(crate) fn verify_jump(&self, page: &[FeedPost]) -> bool {
        page.iter()
            .any(|p| self.covered_keys.contains(&owned_key(p)))
    }

    /// Requests made so far, discarded jumps included.
    pub(crate) fn pages(&self) -> u32 {
        self.pages
    }

    /// Count a request whose page was discarded.
    pub(crate) fn discard_page(&mut self) {
        self.pages += 1;
    }

    /// Merge one page into `feed` and say whether to continue.
    pub(crate) fn absorb(&mut self, feed: &mut Feed, page: Vec<FeedPost>) -> Next {
        self.pages += 1;
        if page.len() < PAGE_SIZE {
            self.exhausted = true;
        }
        let met_known = page.iter().any(|p| self.known_keys.contains(&owned_key(p)));
        if page
            .iter()
            .any(|p| self.covered_keys.contains(&owned_key(p)))
        {
            self.bridged = true;
        }
        let cutoff = self.cutoff();
        if let Some(c) = cutoff
            && page.iter().any(|p| p.time_ms < c)
        {
            self.reached_cutoff = true;
        }

        let mut index: HashMap<Key, usize> = feed
            .posts
            .iter()
            .enumerate()
            .map(|(i, p)| (owned_key(p), i))
            .collect();
        for post in page {
            if cutoff.is_some_and(|c| post.time_ms < c) {
                continue;
            }
            self.oldest_kept = Some(
                self.oldest_kept
                    .map_or(post.time_ms, |o| o.min(post.time_ms)),
            );
            let key = owned_key(&post);
            match index.get(&key) {
                Some(&i) => {
                    // Fetched again: the fresh copy wins (edits).
                    feed.posts[i] = post;
                    self.updated += 1;
                }
                None => {
                    index.insert(key, feed.posts.len());
                    self.added.push(post.clone());
                    feed.posts.push(post);
                }
            }
        }
        sort_newest_first(&mut feed.posts);

        let done = self.exhausted
            || self.reached_cutoff
            || match self.mode {
                // Met the cache (or there was none to meet): caught up.
                Mode::Newest => met_known || self.known_keys.is_empty(),
                // Joined the gap-free region, and it already reaches back to
                // the cutoff.
                Mode::Backfill { cutoff } => {
                    self.bridged && self.covered_since.is_some_and(|c| c <= cutoff)
                }
            };
        if done { Next::Stop } else { Next::Continue }
    }

    /// Close the run: recompute coverage, cap the cache, stamp the fetch.
    /// `now_ms` is the wall clock in Unix ms.
    pub(crate) fn finish(mut self, feed: &mut Feed, now_ms: i64) -> FetchReport {
        if self.pages > 0 && (self.oldest_kept.is_some() || self.exhausted || self.reached_cutoff) {
            let cutoff = self.cutoff();
            // How far back the run alone is gap-free.
            let run_claim = if self.reached_cutoff {
                cutoff
            } else if self.exhausted {
                let year_ago = now_ms - YEAR_MS;
                Some(cutoff.map_or(year_ago, |c| c.max(year_ago)))
            } else {
                self.oldest_kept
            };
            feed.covered_since_ms = match (self.bridged, self.covered_since, run_claim) {
                // Joined the old gap-free region: the union is gap-free.
                (true, Some(c), Some(r)) => Some(c.min(r)),
                (true, Some(c), None) => Some(c),
                // A gap may separate the run from older cached posts.
                (_, _, r) => r,
            };
        }
        if feed.posts.len() > MAX_CACHED_POSTS {
            feed.posts.truncate(MAX_CACHED_POSTS);
            let oldest = feed.posts.last().map(|p| p.time_ms);
            feed.covered_since_ms = match (feed.covered_since_ms, oldest) {
                (Some(c), Some(o)) => Some(c.max(o)),
                (c, _) => c,
            };
        }
        if self.pages > 0 {
            feed.fetched_at = now_ms / 1000;
        }
        sort_newest_first(&mut self.added);
        FetchReport {
            added: self.added,
            updated: self.updated,
            pages: self.pages,
            covered_since_ms: feed.covered_since_ms,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A staff chat post at `time_ms`, id derived from it.
    pub(crate) fn post(time_ms: i64) -> FeedPost {
        FeedPost {
            id: time_ms.to_string(),
            kind: PostKind::Chat,
            community_slug: "SC".into(),
            author: Author {
                id: "1".into(),
                nickname: "Dev-CIG".into(),
                displayname: "Dev".into(),
                is_gm: true,
            },
            body: format!("post {time_ms}"),
            highlight_role_id: Some(2),
            time_created: String::new(),
            time_ms,
            lobby: None,
            channel: None,
            thread: None,
            url: String::new(),
        }
    }

    /// A page of posts at the given times (newest first, as served).
    pub(crate) fn page(times: impl IntoIterator<Item = i64>) -> Vec<FeedPost> {
        times.into_iter().map(post).collect()
    }

    /// A full page: `PAGE_SIZE` posts from `newest` down, one ms apart.
    pub(crate) fn full_page(newest: i64) -> Vec<FeedPost> {
        page((0..PAGE_SIZE as i64).map(|i| newest - i))
    }

    // A year after the epoch: the server window starts at 0, below every
    // test post.
    const NOW: i64 = YEAR_MS;

    #[test]
    fn duplicates_merge_by_kind_and_id() {
        let mut feed = Feed::default();
        let mut run = Run::new(&feed, Mode::Newest);
        let mut first = page([300, 200]);
        let mut reply = post(200);
        reply.kind = PostKind::Reply; // same id, other kind: a different post
        first.push(reply);
        run.absorb(&mut feed, first);
        let report = run.finish(&mut feed, NOW);
        assert_eq!(report.added.len(), 3);
        assert_eq!(feed.posts.len(), 3);

        // Re-fetching an edited post replaces it instead of duplicating it.
        let mut run = Run::new(&feed, Mode::Newest);
        let mut edited = post(300);
        edited.body = "edited".into();
        run.absorb(&mut feed, vec![post(400), edited]);
        let report = run.finish(&mut feed, NOW);
        assert_eq!(report.added, vec![post(400)]);
        assert_eq!(report.updated, 1);
        assert_eq!(feed.posts.len(), 4);
        assert_eq!(feed.posts[0].time_ms, 400, "newest first");
        assert_eq!(feed.posts[1].body, "edited");
    }

    #[test]
    fn newest_stops_when_it_meets_the_cache() {
        let mut feed = Feed::default();
        // Empty cache: one page is the whole catch-up.
        let mut run = Run::new(&feed, Mode::Newest);
        assert_eq!(run.absorb(&mut feed, full_page(1000)), Next::Stop);
        let report = run.finish(&mut feed, NOW);
        // A full page alone is gap-free back to its oldest post.
        assert_eq!(report.covered_since_ms, Some(1000 - 24));
        assert_eq!(feed.fetched_at, NOW / 1000);

        // More than a page arrived since: keep paging until the cache shows.
        let mut run = Run::new(&feed, Mode::Newest);
        assert_eq!(run.absorb(&mut feed, full_page(2000)), Next::Continue);
        // 1010..=986: ten new posts, then the cache.
        assert_eq!(run.absorb(&mut feed, full_page(1010)), Next::Stop);
        let report = run.finish(&mut feed, NOW);
        assert_eq!(report.added.len(), 25 + 10);
        // Bridged into the old gap-free region: coverage extends, not resets.
        assert_eq!(report.covered_since_ms, Some(1000 - 24));
    }

    #[test]
    fn a_gap_resets_coverage_to_the_run() {
        let mut feed = Feed::default();
        let mut run = Run::new(&feed, Mode::Newest);
        run.absorb(&mut feed, full_page(1000));
        run.finish(&mut feed, NOW);

        // Interrupted after one full page that didn't reach the cache: the
        // old posts stay, but the gap-free claim only covers the new page.
        let mut run = Run::new(&feed, Mode::Newest);
        assert_eq!(run.absorb(&mut feed, full_page(5000)), Next::Continue);
        let report = run.finish(&mut feed, NOW);
        assert_eq!(report.covered_since_ms, Some(5000 - 24));
        assert_eq!(feed.posts.len(), 50);
    }

    #[test]
    fn backfill_drops_posts_past_the_cutoff_and_stops_there() {
        let mut feed = Feed::default();
        let mut run = Run::new(&feed, Mode::Backfill { cutoff: 990 });
        assert_eq!(run.absorb(&mut feed, full_page(1000)), Next::Stop);
        let report = run.finish(&mut feed, NOW);
        // 1000..=990 kept, 989..=976 dropped client-side.
        assert_eq!(feed.posts.len(), 11);
        assert!(feed.posts.iter().all(|p| p.time_ms >= 990));
        assert_eq!(report.covered_since_ms, Some(990));
    }

    #[test]
    fn backfill_stops_once_it_joins_a_region_that_reaches_the_cutoff() {
        let mut feed = Feed::default();
        let mut run = Run::new(&feed, Mode::Backfill { cutoff: 500 });
        run.absorb(&mut feed, full_page(1000));
        run.absorb(&mut feed, page([700, 600]));
        let report = run.finish(&mut feed, NOW);
        // Short page: the server's window is exhausted — complete to cutoff.
        assert_eq!(report.covered_since_ms, Some(500));

        // Next start: a few new posts, then the first page meets the cache.
        let mut run = Run::new(&feed, Mode::Backfill { cutoff: 500 });
        let mut top = page([1200, 1100]);
        top.extend(full_page(1000).into_iter().take(23));
        assert_eq!(run.absorb(&mut feed, top), Next::Stop);
        let report = run.finish(&mut feed, NOW);
        assert_eq!(report.pages, 1);
        assert_eq!(report.added.len(), 2);
        assert_eq!(report.covered_since_ms, Some(500));

        // A wider cutoff than the coverage: the same meeting doesn't stop it.
        let mut run = Run::new(&feed, Mode::Backfill { cutoff: 100 });
        assert_eq!(
            run.absorb(&mut feed, page((0..25).map(|i| 1200 - i))),
            Next::Continue
        );
    }

    #[test]
    fn an_exhausted_newest_run_covers_the_server_window() {
        let mut feed = Feed::default();
        let mut run = Run::new(&feed, Mode::Newest);
        run.absorb(&mut feed, page([10, 5]));
        let report = run.finish(&mut feed, NOW);
        assert_eq!(report.covered_since_ms, Some(NOW - YEAR_MS));
    }

    #[test]
    fn a_run_without_pages_changes_nothing() {
        let mut feed = Feed {
            covered_since_ms: Some(42),
            ..Feed::default()
        };
        let run = Run::new(&feed, Mode::Newest);
        let report = run.finish(&mut feed, NOW);
        assert_eq!(report.pages, 0);
        assert_eq!(feed.covered_since_ms, Some(42));
        assert_eq!(feed.fetched_at, 0);
    }

    #[test]
    fn the_cache_is_capped_and_coverage_follows_the_cap() {
        let mut feed = Feed {
            posts: (0..MAX_CACHED_POSTS as i64)
                .rev()
                .map(|t| post(t + 100))
                .collect(),
            covered_since_ms: Some(100),
            fetched_at: 1,
        };
        let mut run = Run::new(&feed, Mode::Newest);
        run.absorb(
            &mut feed,
            page([1_000_000, 1_000_001 - 2, MAX_CACHED_POSTS as i64 + 99]),
        );
        let report = run.finish(&mut feed, NOW);
        assert_eq!(feed.posts.len(), MAX_CACHED_POSTS);
        // The two oldest fell off the end; coverage can't claim them.
        assert_eq!(report.covered_since_ms, Some(102));
    }

    #[test]
    fn jump_targets_the_lower_part_of_the_covered_region() {
        // 200 covered posts, 1000..=801, gap-free back to 801.
        let mut feed = Feed {
            posts: (801..=1000).rev().map(post).collect(),
            covered_since_ms: Some(801),
            ..Feed::default()
        };
        let mut run = Run::new(&feed, Mode::Backfill { cutoff: 10 });
        assert_eq!(run.jump_page(), None, "only after page 1");
        assert_eq!(run.absorb(&mut feed, full_page(1001)), Next::Continue);
        assert_eq!(run.jump_page(), Some(8));
        assert!(run.verify_jump(&full_page(830)));
        assert!(!run.verify_jump(&full_page(500)));

        // A cutoff the coverage already reaches never jumps (page 1 stops).
        let run = Run::new(&feed, Mode::Backfill { cutoff: 900 });
        assert_eq!(run.jump_page(), None);
    }
}
