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
//! **Images.** Each comm-link has a card image on RSI's media host, served
//! in several sizes. A refresh downloads the size each position on the News
//! page shows ([`wanted_images`]: the newest one wide, the next
//! [`CARDS`] as cards, every item as a square thumbnail) into
//! `images/` beside the cache, and deletes files no position wants any more.
//! The page shows the local files, so the webview never fetches from RSI.
//!
//! # Online-policy contract — READ THIS
//!
//! Public website pages read anonymously (the patch notes' footing). Still
//! network: [`CommLinksService::refresh`] (which also fetches the images)
//! **must** be called only after the shell passed
//! `AppState::require_online()`. [`CommLinksService::cached`] and
//! [`CommLinksService::image_path`] touch no network and need no gate.
//!
//! # Posture
//!
//! A refresh reads [`PAGES`] listing pages ([`MIN_INTERVAL`] apart), then
//! the missing images ([`IMAGE_INTERVAL`] apart; usually none or one). The
//! cache is loaded per call and dropped: nothing resident in the tray.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

pub mod media;
pub use media::{IMAGE_INTERVAL, ImageSize, WantedImage};

pub use sc_dossier::commlinks::{
    Age, AgeUnit, CommLink, CommLinkKind, CommLinkPage, CommLinkQuery, CommLinks,
};

/// Listing pages per refresh (10 cards each).
pub const PAGES: u32 = 2;

/// Spacing between requests (the Spectrum rule; the listing's own limit is
/// unmeasured).
pub const MIN_INTERVAL: Duration = Duration::from_millis(1250);

/// How many comm-links after the newest one show as image cards.
pub const CARDS: usize = 4;

/// The size the News page shows the item at `position` (0 = newest) in.
pub fn size_for(position: usize) -> ImageSize {
    match position {
        0 => ImageSize::Wide,
        p if p <= CARDS => ImageSize::Card,
        _ => ImageSize::Thumb,
    }
}

/// The images the current items need: every item's thumbnail (an item
/// moves down the page as new ones arrive) plus the size its position
/// shows. Items without a thumbnail on RSI's media host get none.
pub fn wanted_images(items: &[CommLink]) -> Vec<WantedImage> {
    let mut out = Vec::new();
    for (position, item) in items.iter().enumerate() {
        let Some(thumb) = item.thumbnail.as_deref() else {
            continue;
        };
        let mut sizes = vec![ImageSize::Thumb];
        if size_for(position) != ImageSize::Thumb {
            sizes.push(size_for(position));
        }
        out.extend(
            sizes
                .into_iter()
                .filter_map(|size| WantedImage::new(item.id, thumb, size)),
        );
    }
    out
}

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

    /// Where the downloaded images live. The shell allows the webview to
    /// read this directory (and nothing else) through the asset protocol.
    pub fn images_dir(&self) -> PathBuf {
        self.cache_root.join("images")
    }

    /// The best image on disk for the item at `position`: the size that
    /// position shows, else a smaller one already there. No network.
    pub fn image_path(&self, item: &CommLink, position: usize) -> Option<PathBuf> {
        media::best_on_disk(
            &self.images_dir(),
            item.id,
            item.thumbnail.as_deref()?,
            size_for(position),
        )
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
        let images =
            media::sync(&self.images_dir(), &wanted_images(&cache.items), user_agent).await;
        tracing::info!(
            items = cache.items.len(),
            new = new_items.len(),
            images,
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

    fn with_thumb(id: u64, code: &str, ext: &str) -> CommLink {
        CommLink {
            thumbnail: Some(format!(
                "https://media.robertsspaceindustries.com/{code}/heap_infobox.{ext}"
            )),
            ..item(id, None)
        }
    }

    #[test]
    fn positions_want_wide_then_cards_then_thumbs() {
        let items: Vec<CommLink> = (0..7)
            .map(|i| with_thumb(100 + i, &format!("c{i}"), "jpg"))
            .chain([item(200, None)])
            .collect();
        let files: Vec<String> = wanted_images(&items).into_iter().map(|w| w.file).collect();
        assert_eq!(&files[..2], ["100-thumb.jpg", "100-wide.jpg"]);
        assert_eq!(&files[2..4], ["101-thumb.jpg", "101-card.jpg"]);
        assert!(files.contains(&"104-card.jpg".to_string()));
        assert!(
            !files.contains(&"105-card.jpg".to_string()),
            "past the cards"
        );
        assert!(files.contains(&"106-thumb.jpg".to_string()));
        assert!(
            !files.iter().any(|f| f.starts_with("200-")),
            "no thumbnail, no image"
        );
        assert_eq!(files.len(), 7 + 1 + CARDS);
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
