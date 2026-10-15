//! Comm-links service — the newest entries of RSI's public comm-link
//! listing, read through [`sc_dossier::commlinks`], paced and cached on disk.
//!
//! App framework beside the patch notes (decision 2026-10-15): the News page
//! reads it whatever modules are on.
//!
//! **Dates.** The listing only gives relative ages ("11 hours ago"). The
//! cache records when Starlume first saw each comm-link (by its stable id);
//! on the very first fill, which has no history, that time is estimated from
//! the coarse age instead. So new items get a close time, the first fill's
//! backlog an approximate one.
//!
//! # Online-policy contract — READ THIS
//!
//! Public website pages read anonymously (the patch notes' footing). Still
//! network: [`CommLinksService::refresh`] **must** be called only after the
//! shell passed `AppState::require_online()`. [`CommLinksService::cached`]
//! touches no network and needs no gate.
//!
//! # Posture
//!
//! A refresh reads [`PAGES`] listing pages ([`MIN_INTERVAL`] apart). The
//! cache is loaded per call and dropped: nothing resident in the tray.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

pub use sc_dossier::commlinks::{
    Age, AgeUnit, CommLink, CommLinkKind, CommLinkPage, CommLinkQuery, CommLinks,
};

/// Listing pages per refresh (10 cards each).
pub const PAGES: u32 = 2;

/// Spacing between requests (the Spectrum rule; the listing's own limit is
/// unmeasured).
pub const MIN_INTERVAL: Duration = Duration::from_millis(1250);

/// What is cached on disk.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CommLinksCache {
    /// The newest comm-links, newest first.
    pub items: Vec<CommLink>,
    /// When each item (by id) was first seen, Unix seconds — estimated from
    /// its age on the first fill. Kept for the cached items only.
    pub first_seen: BTreeMap<u64, i64>,
    /// When the last refresh completed, Unix seconds. `0` = never.
    pub fetched_at: i64,
}

impl CommLinksCache {
    /// When an item appeared, Unix seconds, as well as Starlume can tell.
    pub fn published(&self, item: &CommLink) -> Option<i64> {
        self.first_seen.get(&item.id).copied()
    }
}

/// What a refresh found.
#[derive(Debug, Clone, Default)]
pub struct RefreshReport {
    /// Items the cache didn't have, newest first. On the first fill
    /// everything is new — see [`RefreshReport::first`].
    pub new_items: Vec<CommLink>,
    /// The cache was empty before (don't announce the backlog).
    pub first: bool,
}

/// The comm-links + their on-disk cache.
pub struct CommLinksService {
    cache_root: PathBuf,
    /// Serializes refreshes: each loads, merges and saves the whole cache.
    run_lock: tokio::sync::Mutex<()>,
}

impl CommLinksService {
    /// `cache_root` is where the cache file lives — the shell passes a
    /// subdirectory of `app_kit::app_data_root()`.
    pub fn new(cache_root: PathBuf) -> Self {
        Self {
            cache_root,
            run_lock: tokio::sync::Mutex::new(()),
        }
    }

    fn cache_path(&self) -> PathBuf {
        self.cache_root.join("commlinks.json")
    }

    /// The cached comm-links, or `None` when nothing was ever fetched. No
    /// network.
    pub fn cached(&self) -> Option<CommLinksCache> {
        let cache: CommLinksCache = app_kit::load_json(&self.cache_path());
        (cache.fetched_at != 0).then_some(cache)
    }

    /// Read the newest [`PAGES`] listing pages, merge, save.
    ///
    /// **Gated:** the caller must have passed `require_online()` (see the
    /// module contract). A failed page after the first keeps what was read.
    pub async fn refresh(&self, user_agent: &str) -> sc_dossier::Result<RefreshReport> {
        let _serial = self.run_lock.lock().await;
        let old: CommLinksCache = app_kit::load_json(&self.cache_path());
        let client = CommLinks::new(user_agent);
        let mut fetched: Vec<CommLink> = Vec::new();
        for page in 1..=PAGES {
            if page > 1 {
                tokio::time::sleep(MIN_INTERVAL).await;
            }
            let query = CommLinkQuery {
                page,
                ..CommLinkQuery::default()
            };
            match client.page(&query).await {
                Ok(p) => {
                    fetched.extend(p.items);
                    if p.is_last {
                        break;
                    }
                }
                Err(e) if page > 1 => {
                    tracing::warn!(page, "comm-link page failed: {e}");
                    break;
                }
                Err(e) => return Err(e),
            }
        }
        let first = old.fetched_at == 0;
        let (cache, new_items) = merge(&old, fetched, chrono::Utc::now().timestamp());
        if let Err(e) = app_kit::save_json(&self.cache_path(), &cache) {
            tracing::warn!("failed to write the comm-links cache: {e}");
        }
        tracing::info!(
            items = cache.items.len(),
            new = new_items.len(),
            "comm-links refreshed"
        );
        Ok(RefreshReport { new_items, first })
    }
}

/// The new cache and the items it didn't have. First-seen times: kept for
/// known items, `now` for new ones — or, on the first fill (no history),
/// `now` minus the item's coarse age.
fn merge(
    old: &CommLinksCache,
    fetched: Vec<CommLink>,
    now: i64,
) -> (CommLinksCache, Vec<CommLink>) {
    let first = old.fetched_at == 0;
    let mut items: Vec<CommLink> = Vec::new();
    for item in fetched {
        if !items.iter().any(|i| i.id == item.id) {
            items.push(item);
        }
    }
    let mut first_seen = BTreeMap::new();
    let mut new_items = Vec::new();
    for item in &items {
        let seen = match old.first_seen.get(&item.id) {
            Some(&t) => t,
            None => {
                new_items.push(item.clone());
                if first {
                    now - item.age.map_or(0, |a| a.approx_seconds() as i64)
                } else {
                    now
                }
            }
        };
        first_seen.insert(item.id, seen);
    }
    (
        CommLinksCache {
            items,
            first_seen,
            fetched_at: now,
        },
        new_items,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: u64, age: Option<Age>) -> CommLink {
        CommLink {
            id,
            url: format!("https://example.test/{id}"),
            title: format!("Comm-link {id}"),
            kind: CommLinkKind::Post,
            channel: Some("transmission".into()),
            summary: String::new(),
            thumbnail: None,
            comments: None,
            age_text: String::new(),
            age,
        }
    }

    fn hours(n: u32) -> Option<Age> {
        Some(Age {
            value: n,
            unit: AgeUnit::Hour,
        })
    }

    #[test]
    fn first_fill_estimates_from_age_later_ones_record_now() {
        let empty = CommLinksCache::default();
        let (cache, new) = merge(&empty, vec![item(2, hours(1)), item(1, hours(5))], 100_000);
        assert_eq!(new.len(), 2);
        assert_eq!(cache.first_seen[&2], 100_000 - 3600);
        assert_eq!(cache.first_seen[&1], 100_000 - 5 * 3600);

        // Next refresh: one new item on top, #1 fell off the two pages.
        let (next, new) = merge(&cache, vec![item(3, hours(2)), item(2, hours(3))], 200_000);
        assert_eq!(new.iter().map(|i| i.id).collect::<Vec<_>>(), vec![3]);
        assert_eq!(next.first_seen[&3], 200_000, "seen now, not estimated");
        assert_eq!(next.first_seen[&2], 100_000 - 3600, "kept");
        assert!(!next.first_seen.contains_key(&1), "pruned with its item");
    }

    #[test]
    fn duplicates_across_pages_are_dropped() {
        let (cache, _) = merge(
            &CommLinksCache::default(),
            vec![item(1, None), item(1, None)],
            10,
        );
        assert_eq!(cache.items.len(), 1);
    }
}
