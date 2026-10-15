//! Spectrum service — RSI's chat/forum platform over its REST API, on the RSI
//! launcher session: the CIG staff feed (the dev tracker), paced,
//! throttle-aware, deduped and cached on disk.
//!
//! Wraps [`sc_dossier::spectrum`]. The library deliberately leaves pacing and
//! filtering to its callers; this service owns the policy, app-wide:
//!
//! - **Pacing** — at most [`MAX_REQUESTS_PER_SECOND`] per endpoint, however
//!   many callers (the server limit is ~1 req/s per IP and route).
//! - **Throttle backoff** — `ErrThrottleLimit` (an HTTP 200, not a 429) shuts
//!   the endpoint for [`THROTTLE_BACKOFF`]; calls in that window fail fast
//!   with [`Error::Throttled`] without touching the network, and
//!   [`SpectrumService::throttled_until`] surfaces the state.
//! - **Dedup + cache** — posts merge into one [`Feed`] by
//!   [`FeedPost::key`], persisted atomically so the feed survives a restart.
//! - **Backfill** — the server's `range: "month"` is broken, so backfills
//!   search `range: "year"` and cut off client-side.
//!
//! Not here, by design: **filtering** (the content filter is mod-devtracker's)
//! and **cadence** (when to poll is the consumer's call — this crate starts no
//! background task). The Spectrum chat WebSocket will live here too when a
//! consumer needs it; the pacer and backoff are keyed by endpoint for that.
//!
//! # Online-policy contract — READ THIS
//!
//! Spectrum is RSI's private API; Starlume is not an official client, so every
//! call is ToS-grey — the same footing as the game-services gRPC. Every method
//! that touches the network ([`SpectrumService::fetch_newest`],
//! [`SpectrumService::backfill`]) **must** be called only after the shell has
//! passed the gate `AppState::require_unofficial("devtracker")`. This crate
//! cannot see settings, so it cannot gate itself — the gate lives at the call
//! site, exactly like svc-dossier's. The cached reads
//! ([`SpectrumService::cached_feed`], [`SpectrumService::throttled_until`])
//! touch no network and need no gate.
//!
//! # Posture
//!
//! Reads only, on the user's own launcher session (anonymous reads miss
//! restricted lobbies such as `sc-testing-chat`). A fetch reads the launcher
//! credentials, makes its paced requests and drops the client — no long-lived
//! connection. The feed is loaded from disk per call and dropped after: it is
//! never resident while the app idles in the tray.

use std::future::Future;
use std::path::PathBuf;
use std::sync::Mutex;

use sc_dossier::spectrum::{Range, SearchPage, SearchQuery, Spectrum};
use tokio::time::Instant;

mod feed;
mod policy;

pub use feed::{
    Author, Feed, FeedPost, FetchReport, MAX_CACHED_POSTS, PAGE_SIZE, Place, PostKind, ThreadRef,
};
pub use policy::{MAX_REQUESTS_PER_SECOND, MIN_INTERVAL, THROTTLE_BACKOFF};

use feed::{Mode, Next, Run};
use policy::Policy;

/// Pages [`SpectrumService::fetch_newest`] requests at most while catching up
/// to the cache (200 posts). Past it the gap stays, and coverage says so — a
/// [`SpectrumService::backfill`] fills it.
pub const MAX_CATCHUP_PAGES: u32 = 8;

/// Pages one [`SpectrumService::backfill`] requests at most (1000 posts,
/// ~50 s at the pace). A capped backfill records how far it got; the next one
/// skips ahead into what is already covered.
pub const MAX_BACKFILL_PAGES: u32 = 40;

/// The content-search route — the pacing/backoff key for the feed.
const SEARCH: &str = "/search/content/extended";

/// Errors from a Spectrum fetch.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The endpoint is backing off after `ErrThrottleLimit` (just now, or
    /// earlier — a call during the backoff fails fast without a request).
    /// `until` is when it reopens, Unix seconds. Whatever the fetch got before
    /// the throttle is cached.
    #[error("Spectrum is rate-limiting requests — paused until the backoff ends")]
    Throttled {
        /// When the backoff ends, Unix seconds.
        until: i64,
    },
    /// From [`sc_dossier`]: a missing/stale launcher session
    /// ([`sc_dossier::Error::StoreNotFound`], [`sc_dossier::Error::MissingCredential`]),
    /// an outdated launcher ([`sc_dossier::Error::LauncherOutdated`]), a
    /// Spectrum error envelope ([`sc_dossier::Error::Spectrum`]), transport.
    #[error(transparent)]
    Spectrum(#[from] sc_dossier::Error),
}

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Where search pages come from: Spectrum in production, a fake in tests.
trait Source: Sync {
    fn search(
        &self,
        query: &SearchQuery,
    ) -> impl Future<Output = sc_dossier::Result<SearchPage>> + Send;
}

impl Source for Spectrum {
    fn search(
        &self,
        query: &SearchQuery,
    ) -> impl Future<Output = sc_dossier::Result<SearchPage>> + Send {
        Spectrum::search(self, query)
    }
}

/// The Spectrum feed + its policy state + its on-disk cache. One per app —
/// the pace is app-wide.
pub struct SpectrumService {
    cache_root: PathBuf,
    /// Pacing slots + backoffs. Locked briefly, never across an await.
    policy: Mutex<Policy>,
    /// Serializes fetch runs: each loads, merges and saves the whole feed.
    run_lock: tokio::sync::Mutex<()>,
}

impl SpectrumService {
    /// `cache_root` is where the cache files live — the shell passes a
    /// subdirectory of `app_kit::app_data_root()`.
    pub fn new(cache_root: PathBuf) -> Self {
        Self {
            cache_root,
            policy: Mutex::new(Policy::default()),
            run_lock: tokio::sync::Mutex::new(()),
        }
    }

    fn feed_path(&self) -> PathBuf {
        self.cache_root.join("feed.json")
    }

    /// The cached staff feed, newest first, or `None` when nothing has ever
    /// been fetched. Cheap, no network — safe to call ungated, so the feed
    /// renders before (or without) a fetch.
    pub fn cached_feed(&self) -> Option<Feed> {
        let feed: Feed = app_kit::load_json(&self.feed_path());
        (feed.fetched_at != 0).then_some(feed)
    }

    /// When the current throttle backoff ends (Unix seconds), or `None` when
    /// nothing is backing off. No network.
    pub fn throttled_until(&self) -> Option<i64> {
        self.policy.lock().unwrap().latest_backoff(Instant::now())
    }

    /// Drop the cached feed (sign-out / account switch). No network.
    pub fn clear_feed(&self) {
        let path = self.feed_path();
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
    }

    /// Fetch what is new since the cache: the newest page of the staff feed,
    /// and further pages only while they hold nothing cached yet (a long
    /// absence), up to [`MAX_CATCHUP_PAGES`]. A regular poll is one request.
    ///
    /// **Gated:** the caller must have passed `require_unofficial("devtracker")`
    /// first (see the module contract). `user_agent` is the one the shell
    /// supplies. Errors with [`Error::Throttled`] during a backoff.
    pub async fn fetch_newest(&self, user_agent: &str) -> Result<FetchReport> {
        let client = launcher_client(user_agent).await?;
        self.run(&client, Mode::Newest, MAX_CATCHUP_PAGES).await
    }

    /// Page back through the staff feed until the cache holds every post since
    /// `cutoff_ms` (Unix milliseconds), up to [`MAX_BACKFILL_PAGES`]. Posts
    /// older than the cutoff are dropped. Stops early once it meets a part of
    /// the cache already known to be gap-free back to the cutoff — on a warm
    /// cache that is one request.
    ///
    /// **Gated:** like [`SpectrumService::fetch_newest`].
    pub async fn backfill(&self, user_agent: &str, cutoff_ms: i64) -> Result<FetchReport> {
        let client = launcher_client(user_agent).await?;
        self.run(
            &client,
            Mode::Backfill { cutoff: cutoff_ms },
            MAX_BACKFILL_PAGES,
        )
        .await
    }

    /// One fetch run: page through the feed per `mode`, merge, save. What a
    /// run fetched before an error is still saved.
    async fn run<S: Source>(&self, source: &S, mode: Mode, max_pages: u32) -> Result<FetchReport> {
        let _serial = self.run_lock.lock().await;
        let mut feed: Feed = app_kit::load_json(&self.feed_path());
        let mut run = Run::new(&feed, mode);
        let mut page = 1;
        // Set while the current page is a jump: where to resume if it missed.
        let mut resume_at = None;
        let outcome = loop {
            if run.pages() >= max_pages {
                break Ok(());
            }
            let posts = match self.search_page(source, page).await {
                Ok(posts) => posts,
                Err(e) => break Err(e),
            };
            if let Some(back) = resume_at.take()
                && !run.verify_jump(&posts)
            {
                run.discard_page();
                page = back;
                continue;
            }
            if run.absorb(&mut feed, posts) == Next::Stop {
                break Ok(());
            }
            match run.jump_page() {
                Some(jump) => {
                    resume_at = Some(page + 1);
                    page = jump;
                }
                None => page += 1,
            }
        };
        let report = run.finish(&mut feed, chrono::Utc::now().timestamp_millis());
        if report.pages > 0 {
            write_cache(&self.feed_path(), &feed);
        }
        tracing::info!(
            pages = report.pages,
            added = report.added.len(),
            updated = report.updated,
            ok = outcome.is_ok(),
            "spectrum feed fetch"
        );
        outcome.map(|()| report)
    }

    /// One page of the staff feed (`range: "year"`, newest first), paced.
    async fn search_page<S: Source>(&self, source: &S, page: u32) -> Result<Vec<FeedPost>> {
        let query = SearchQuery::staff().range(Range::Year).page(page);
        let found = self.paced(SEARCH, || source.search(&query)).await?;
        Ok(found.posts.iter().map(FeedPost::from).collect())
    }

    /// Send one request to `endpoint` under the policy: fail fast during a
    /// backoff, wait for the pacing slot, and turn a throttle into a backoff.
    async fn paced<T, F>(&self, endpoint: &'static str, request: impl FnOnce() -> F) -> Result<T>
    where
        F: Future<Output = sc_dossier::Result<T>>,
    {
        let wait = {
            let mut policy = self.policy.lock().unwrap();
            let now = Instant::now();
            if let Some(until) = policy.backing_off(endpoint, now) {
                return Err(Error::Throttled { until });
            }
            policy.reserve(endpoint, now)
        };
        tokio::time::sleep(wait).await;
        // Another caller may have been throttled while this one waited.
        if let Some(until) = self
            .policy
            .lock()
            .unwrap()
            .backing_off(endpoint, Instant::now())
        {
            return Err(Error::Throttled { until });
        }
        match request().await {
            Err(sc_dossier::Error::Throttled { .. }) => {
                let until = self.policy.lock().unwrap().throttled(
                    endpoint,
                    Instant::now(),
                    chrono::Utc::now().timestamp(),
                );
                tracing::warn!(endpoint, until, "spectrum throttled — backing off");
                Err(Error::Throttled { until })
            }
            other => other.map_err(Error::from),
        }
    }
}

/// A Spectrum client on the launcher session. Reading the credentials scans
/// the launcher's `app.asar` — blocking IO, so off the async workers.
async fn launcher_client(user_agent: &str) -> Result<Spectrum> {
    let user_agent = user_agent.to_string();
    match tokio::task::spawn_blocking(move || Spectrum::from_launcher(&user_agent)).await {
        Ok(client) => Ok(client?),
        Err(e) if e.is_panic() => std::panic::resume_unwind(e.into_panic()),
        Err(_) => Err(sc_dossier::Error::Store(
            "reading the launcher session was cancelled".into(),
        )
        .into()),
    }
}

/// Persist the feed; a write failure is logged, not fatal (the fetched posts
/// are in hand — a stale cache is a next-launch problem, not this one).
fn write_cache(path: &std::path::Path, feed: &Feed) {
    if let Err(e) = app_kit::save_json(path, feed) {
        tracing::warn!("failed to write spectrum cache to {}: {e}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, VecDeque};
    use std::sync::atomic::{AtomicUsize, Ordering};

    use sc_dossier::spectrum::{Member, Post};

    use super::*;

    /// Serves queued responses and records when each request went out.
    #[derive(Default)]
    struct Fake {
        responses: Mutex<VecDeque<sc_dossier::Result<Vec<i64>>>>,
        requested: Mutex<Vec<(u32, Instant)>>,
        calls: AtomicUsize,
    }

    impl Fake {
        fn serving(responses: impl IntoIterator<Item = sc_dossier::Result<Vec<i64>>>) -> Self {
            Self {
                responses: Mutex::new(responses.into_iter().collect()),
                ..Self::default()
            }
        }

        fn pages_requested(&self) -> Vec<u32> {
            self.requested.lock().unwrap().iter().map(|r| r.0).collect()
        }
    }

    fn hit(time_ms: i64) -> Post {
        Post {
            id: time_ms.to_string(),
            kind: sc_dossier::spectrum::PostKind::Chat,
            community_id: "1".into(),
            community_slug: "SC".into(),
            member: Member {
                id: "1".into(),
                nickname: "Dev-CIG".into(),
                displayname: "Dev".into(),
                is_gm: true,
                roles: BTreeMap::new(),
            },
            body: String::new(),
            highlight_role_id: Some(2),
            time_created: String::new(),
            time_ms,
            lobby: None,
            channel: None,
            thread: None,
            url: String::new(),
        }
    }

    impl Source for Fake {
        fn search(
            &self,
            query: &SearchQuery,
        ) -> impl Future<Output = sc_dossier::Result<SearchPage>> + Send {
            // The service always asks for the staff feed over the year window.
            assert_eq!(query.highlight_role_id, Some(2));
            assert_eq!(query.range, Range::Year);
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.requested
                .lock()
                .unwrap()
                .push((query.page, Instant::now()));
            let response = self
                .responses
                .lock()
                .unwrap()
                .pop_front()
                .expect("an unexpected request");
            async move {
                response.map(|times| SearchPage {
                    total: 0,
                    posts: times.into_iter().map(hit).collect(),
                })
            }
        }
    }

    fn throttled() -> sc_dossier::Result<Vec<i64>> {
        Err(sc_dossier::Error::Throttled {
            path: SEARCH.into(),
        })
    }

    fn full_page(newest: i64) -> sc_dossier::Result<Vec<i64>> {
        Ok((0..PAGE_SIZE as i64).map(|i| newest - i).collect())
    }

    fn now_ms() -> i64 {
        chrono::Utc::now().timestamp_millis()
    }

    #[tokio::test(start_paused = true)]
    async fn pages_are_paced_and_cached() {
        let dir = tempfile::tempdir().unwrap();
        let svc = SpectrumService::new(dir.path().to_path_buf());
        assert!(svc.cached_feed().is_none());

        let t = now_ms();
        let fake = Fake::serving([full_page(t), full_page(t - 100), Ok(vec![t - 200])]);
        let report = svc
            .run(&fake, Mode::Backfill { cutoff: 0 }, MAX_BACKFILL_PAGES)
            .await
            .unwrap();
        assert_eq!(report.pages, 3);
        assert_eq!(report.added.len(), 51);
        assert_eq!(fake.pages_requested(), [1, 2, 3]);

        // Never two requests to the endpoint closer than the interval.
        let at: Vec<Instant> = fake.requested.lock().unwrap().iter().map(|r| r.1).collect();
        for pair in at.windows(2) {
            assert!(pair[1] - pair[0] >= MIN_INTERVAL);
        }

        // A fresh service (a restart) reads the feed back, no network.
        let svc = SpectrumService::new(dir.path().to_path_buf());
        let feed = svc.cached_feed().expect("cached");
        assert_eq!(feed.posts.len(), 51);
        assert_eq!(feed.posts[0].time_ms, t);
        svc.clear_feed();
        assert!(svc.cached_feed().is_none());
    }

    #[tokio::test(start_paused = true)]
    async fn a_throttle_backs_off_without_further_requests() {
        let dir = tempfile::tempdir().unwrap();
        let svc = SpectrumService::new(dir.path().to_path_buf());
        let t = now_ms();
        // The last page reaches back into the cache, so the catch-up stops.
        let fake = Fake::serving([full_page(t), throttled(), full_page(t + 10)]);

        // Throttled mid-backfill: the error carries the backoff, and the page
        // fetched before it is cached.
        let err = svc
            .run(&fake, Mode::Backfill { cutoff: 0 }, MAX_BACKFILL_PAGES)
            .await
            .unwrap_err();
        let Error::Throttled { until } = err else {
            panic!("expected a throttle, got {err:?}");
        };
        assert!(until >= chrono::Utc::now().timestamp() + 120);
        assert_eq!(svc.throttled_until(), Some(until));
        assert_eq!(svc.cached_feed().unwrap().posts.len(), PAGE_SIZE);

        // During the backoff a fetch fails fast: no request goes out.
        let calls = fake.calls.load(Ordering::SeqCst);
        let err = svc
            .run(&fake, Mode::Newest, MAX_CATCHUP_PAGES)
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Throttled { .. }));
        assert_eq!(fake.calls.load(Ordering::SeqCst), calls);

        // After it, fetching resumes.
        tokio::time::advance(THROTTLE_BACKOFF).await;
        assert_eq!(svc.throttled_until(), None);
        let report = svc
            .run(&fake, Mode::Newest, MAX_CATCHUP_PAGES)
            .await
            .unwrap();
        assert_eq!(report.pages, 1);
    }

    #[tokio::test(start_paused = true)]
    async fn concurrent_fetches_share_the_pace() {
        let dir = tempfile::tempdir().unwrap();
        let svc = SpectrumService::new(dir.path().to_path_buf());
        let t = now_ms();
        let fake = Fake::serving([Ok(vec![t]), Ok(vec![t + 1]), Ok(vec![t + 2])]);
        let (a, b, c) = tokio::join!(
            svc.run(&fake, Mode::Newest, 1),
            svc.run(&fake, Mode::Newest, 1),
            svc.run(&fake, Mode::Newest, 1),
        );
        assert!(a.is_ok() && b.is_ok() && c.is_ok());
        let at: Vec<Instant> = fake.requested.lock().unwrap().iter().map(|r| r.1).collect();
        assert_eq!(at.len(), 3);
        for pair in at.windows(2) {
            assert!(pair[1] - pair[0] >= MIN_INTERVAL);
        }
        assert_eq!(svc.cached_feed().unwrap().posts.len(), 3);
    }

    #[tokio::test(start_paused = true)]
    async fn a_warm_backfill_skips_ahead_into_the_covered_region() {
        let dir = tempfile::tempdir().unwrap();
        let svc = SpectrumService::new(dir.path().to_path_buf());
        let t = now_ms();
        // A capped backfill: 10 full pages, gap-free back to t - 249.
        let fake = Fake::serving((0..10).map(|i| full_page(t - i * 25)));
        svc.run(&fake, Mode::Backfill { cutoff: 0 }, 10)
            .await
            .unwrap();
        assert_eq!(svc.cached_feed().unwrap().covered_since_ms, Some(t - 249));

        // The next backfill meets the cache on page 1, jumps to page 10 (the
        // bottom of what is covered) and carries on from there.
        let fake = Fake::serving([
            full_page(t + 2),
            full_page(t - 225),
            full_page(t - 250),
            Ok(vec![t - 275]),
        ]);
        let report = svc
            .run(&fake, Mode::Backfill { cutoff: 0 }, MAX_BACKFILL_PAGES)
            .await
            .unwrap();
        assert_eq!(fake.pages_requested(), [1, 10, 11, 12]);
        assert_eq!(report.added.len(), 2 + 25 + 1);

        // A jump that misses (the region moved) is discarded; paging resumes
        // at page 2.
        let fake = Fake::serving([
            full_page(t + 2),
            full_page(t + 5000),
            full_page(t - 23),
            Ok(vec![]),
        ]);
        svc.run(&fake, Mode::Backfill { cutoff: 1 }, MAX_BACKFILL_PAGES)
            .await
            .unwrap();
        assert_eq!(fake.pages_requested(), [1, 11, 2, 3]);
        assert!(
            svc.cached_feed()
                .unwrap()
                .posts
                .iter()
                .all(|p| p.time_ms != t + 5000)
        );
    }
}
