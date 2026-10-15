//! Patch-notes service — the LIVE release notes from RSI's public patch-note
//! pages, parsed by [`sc_dossier::patchnotes`], paced and cached on disk.
//!
//! App framework, not a module (decision 2026-10-15): Home's "is the game up
//! to date" and the Patch Notes page both read it, whatever modules are on.
//!
//! # Online-policy contract — READ THIS
//!
//! These are public website pages read anonymously — the footing of the RSI
//! profile lookup, not the unofficial-API opt-in. Still network: every method
//! that touches it ([`PatchNotesService::refresh`]) **must** be called only
//! after the shell passed `AppState::require_online()`. This crate cannot see
//! settings, so it cannot gate itself. [`PatchNotesService::cached`] touches
//! no network and needs no gate.
//!
//! # Posture
//!
//! Rare reads: a refresh is the index plus the newest article (its point
//! releases and edits land there) plus any of the newest
//! [`ARTICLE_WINDOW`] not cached yet — so the first run reads a handful of
//! articles and later ones read two pages. Requests are spaced by
//! [`MIN_INTERVAL`]. Older articles stay index entries (title + link). The
//! cache is loaded per call and dropped: nothing resident in the tray.

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

pub use sc_dossier::patchnotes::{
    Block, Build, Counts, PatchNoteArticle, PatchNoteRef, PatchNotes, Release, Section, SectionKind,
};

/// How many of the newest articles are kept parsed. Each covers one minor
/// version (some also hold its point releases); older ones are listed only.
pub const ARTICLE_WINDOW: usize = 6;

/// Spacing between requests (the Spectrum rule; the site's limit is
/// unmeasured, see sc-grpc FINDINGS "Comm-links").
pub const MIN_INTERVAL: Duration = Duration::from_millis(1250);

/// What is cached on disk.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PatchNotesCache {
    /// Every article, newest first.
    pub index: Vec<PatchNoteRef>,
    /// The parsed ones (up to [`ARTICLE_WINDOW`]), newest first.
    pub articles: Vec<PatchNoteArticle>,
    /// When the last refresh completed, Unix seconds. `0` = never.
    pub fetched_at: i64,
}

impl PatchNotesCache {
    /// Every parsed release with its article, newest first.
    pub fn releases(&self) -> impl Iterator<Item = (&PatchNoteArticle, &Release)> {
        self.articles
            .iter()
            .flat_map(|a| a.releases.iter().map(move |r| (a, r)))
    }

    /// The newest release — what LIVE is on, as far as the notes say.
    pub fn latest(&self) -> Option<(&PatchNoteArticle, &Release)> {
        self.releases().next()
    }
}

/// What a refresh found.
#[derive(Debug, Clone, Default)]
pub struct RefreshReport {
    /// Releases the cache didn't have, newest first. On the very first
    /// refresh everything is new — see [`RefreshReport::first`].
    pub new_releases: Vec<Release>,
    /// The cache was empty before (don't announce the backlog).
    pub first: bool,
    /// Articles read.
    pub articles_fetched: u32,
    /// Articles that failed to read or parse (url, error); their previous
    /// copy, if any, is kept.
    pub failed: Vec<(String, String)>,
}

/// The patch notes + their on-disk cache.
pub struct PatchNotesService {
    cache_root: PathBuf,
    /// Serializes refreshes: each loads, merges and saves the whole cache.
    run_lock: tokio::sync::Mutex<()>,
}

impl PatchNotesService {
    /// `cache_root` is where the cache file lives — the shell passes a
    /// subdirectory of `app_kit::app_data_root()`.
    pub fn new(cache_root: PathBuf) -> Self {
        Self {
            cache_root,
            run_lock: tokio::sync::Mutex::new(()),
        }
    }

    fn cache_path(&self) -> PathBuf {
        self.cache_root.join("patchnotes.json")
    }

    /// The cached patch notes, or `None` when nothing was ever fetched. No
    /// network.
    pub fn cached(&self) -> Option<PatchNotesCache> {
        let cache: PatchNotesCache = app_kit::load_json(&self.cache_path());
        (cache.fetched_at != 0).then_some(cache)
    }

    /// Read the index and the articles that need it, merge, save.
    ///
    /// **Gated:** the caller must have passed `require_online()` (see the
    /// module contract). An index failure is the error; an article failure
    /// is reported and keeps the previous copy.
    pub async fn refresh(&self, user_agent: &str) -> sc_dossier::Result<RefreshReport> {
        let _serial = self.run_lock.lock().await;
        let old: PatchNotesCache = app_kit::load_json(&self.cache_path());
        let client = PatchNotes::new(user_agent);
        let index = client.index().await?;
        let mut fetched = Vec::new();
        let mut failed = Vec::new();
        for url in to_fetch(&old, &index) {
            tokio::time::sleep(MIN_INTERVAL).await;
            match client.article(&url).await {
                Ok(article) => fetched.push(article),
                Err(e) => {
                    tracing::warn!(url, "patch-notes article failed: {e}");
                    failed.push((url, e.to_string()));
                }
            }
        }
        let articles_fetched = fetched.len() as u32;
        let (cache, new_releases) = merge(&old, index, fetched, chrono::Utc::now().timestamp());
        if let Err(e) = app_kit::save_json(&self.cache_path(), &cache) {
            tracing::warn!("failed to write the patch-notes cache: {e}");
        }
        tracing::info!(
            articles = articles_fetched,
            new = new_releases.len(),
            failed = failed.len(),
            "patch notes refreshed"
        );
        Ok(RefreshReport {
            first: old.fetched_at == 0,
            new_releases,
            articles_fetched,
            failed,
        })
    }
}

/// Which articles a refresh reads: the newest always (point releases and
/// edits land in it), and any other of the newest [`ARTICLE_WINDOW`] the
/// cache lacks.
fn to_fetch(old: &PatchNotesCache, index: &[PatchNoteRef]) -> Vec<String> {
    index
        .iter()
        .take(ARTICLE_WINDOW)
        .enumerate()
        .filter(|(i, r)| *i == 0 || !old.articles.iter().any(|a| a.id == r.id))
        .map(|(_, r)| r.url.clone())
        .collect()
}

/// A release's identity across refreshes.
fn release_key(r: &Release) -> (&str, &str) {
    (r.title.as_str(), r.date_text.as_str())
}

/// The new cache: the fresh index, its newest [`ARTICLE_WINDOW`] articles
/// from this fetch or the old cache, and the releases that weren't cached.
fn merge(
    old: &PatchNotesCache,
    index: Vec<PatchNoteRef>,
    fetched: Vec<PatchNoteArticle>,
    now: i64,
) -> (PatchNotesCache, Vec<Release>) {
    let articles: Vec<PatchNoteArticle> = index
        .iter()
        .take(ARTICLE_WINDOW)
        .filter_map(|r| {
            fetched
                .iter()
                .find(|a| a.id == r.id)
                .or_else(|| old.articles.iter().find(|a| a.id == r.id))
                .cloned()
        })
        .collect();
    let known: Vec<(&str, &str)> = old.releases().map(|(_, r)| release_key(r)).collect();
    let new_releases = articles
        .iter()
        .flat_map(|a| a.releases.iter())
        .filter(|r| !known.contains(&release_key(r)))
        .cloned()
        .collect();
    (
        PatchNotesCache {
            index,
            articles,
            fetched_at: now,
        },
        new_releases,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index_ref(id: u64) -> PatchNoteRef {
        PatchNoteRef {
            id,
            url: format!("https://example.test/{id}"),
            title: format!("Star Citizen Alpha 4.{id}"),
            version: Some(format!("4.{id}")),
            thumbnail: None,
            age_text: "1 day ago".into(),
        }
    }

    fn release(version: &str, date: &str) -> Release {
        Release {
            date_text: date.into(),
            date: None,
            title: format!("Star Citizen Alpha {version} LIVE"),
            version: Some(version.into()),
            channel: Some("LIVE".into()),
            name: None,
            build: None,
            build_info: Vec::new(),
            highlights: Vec::new(),
            counts: Counts::default(),
            sections: Vec::new(),
        }
    }

    fn article(id: u64, releases: Vec<Release>) -> PatchNoteArticle {
        PatchNoteArticle {
            id,
            url: format!("https://example.test/{id}"),
            title: format!("Star Citizen Alpha 4.{id}"),
            releases,
        }
    }

    #[test]
    fn first_refresh_reads_the_window_then_only_the_newest() {
        let index: Vec<_> = (1..=9).rev().map(index_ref).collect();
        let empty = PatchNotesCache::default();
        assert_eq!(to_fetch(&empty, &index).len(), ARTICLE_WINDOW);

        let cached = PatchNotesCache {
            articles: index[..ARTICLE_WINDOW]
                .iter()
                .map(|r| article(r.id, vec![]))
                .collect(),
            ..PatchNotesCache::default()
        };
        assert_eq!(to_fetch(&cached, &index), vec![index[0].url.clone()]);
        // A new article on top: it, plus nothing else (the rest is cached).
        let mut grown = index.clone();
        grown.insert(0, index_ref(10));
        assert_eq!(to_fetch(&cached, &grown), vec![grown[0].url.clone()]);
    }

    #[test]
    fn merge_keeps_old_copies_and_reports_only_new_releases() {
        let old = PatchNotesCache {
            index: vec![index_ref(2), index_ref(1)],
            articles: vec![
                article(2, vec![release("4.2", "May 1")]),
                article(1, vec![release("4.1", "April 1")]),
            ],
            fetched_at: 1,
        };
        // Article 2 gained a point release; article 3 is new; 1 failed to
        // fetch this time (not in `fetched`) and keeps its old copy.
        let fetched = vec![
            article(3, vec![release("4.3", "June 1")]),
            article(2, vec![release("4.2.1", "May 9"), release("4.2", "May 1")]),
        ];
        let index = vec![index_ref(3), index_ref(2), index_ref(1)];
        let (cache, new) = merge(&old, index, fetched, 99);
        assert_eq!(
            cache.articles.iter().map(|a| a.id).collect::<Vec<_>>(),
            vec![3, 2, 1]
        );
        let versions: Vec<_> = new.iter().map(|r| r.version.clone().unwrap()).collect();
        assert_eq!(versions, vec!["4.3", "4.2.1"]);
        assert_eq!(cache.latest().unwrap().1.version.as_deref(), Some("4.3"));
        assert_eq!(cache.fetched_at, 99);
    }
}
