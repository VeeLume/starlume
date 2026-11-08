//! Ship-store service — the ships on RSI's pledge store, read through
//! [`sc_dossier::pledge`], and their store images, cached on disk.
//!
//! The Ships page shows a picture per hull: the store's `store_small`
//! image (351 × 210). A refresh reads the list (one request per 30 ships),
//! then downloads every image not on disk yet into `images/` beside the
//! cache, and deletes the ones no listed ship has any more. The page shows
//! the local files, so the webview never fetches from RSI.
//!
//! Store entries carry no id the game data shares; joining them to game
//! records is the Ships module's (by name).
//!
//! # Online-policy contract — READ THIS
//!
//! Public website pages read anonymously (the comm-links' footing).
//! [`ShipStoreService::refresh`] is network and **must** be called only
//! after the shell passed `AppState::require_online()`.
//! [`ShipStoreService::cached`] and [`ShipStoreService::image_path`] touch no
//! network and need no gate.
//!
//! # Posture
//!
//! The list changes when a ship goes on sale (every few weeks):
//! [`MAX_AGE`] is a week. Images are paced
//! ([`svc_commlinks::IMAGE_INTERVAL`]); only the first fill downloads
//! many (~250 × 70 KB). The cache is loaded per call and dropped.

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use svc_commlinks::media::{self, ImageSize, WantedImage};

pub use sc_dossier::pledge::{PledgeStore, ShipMedia, StoreShip};

/// Refresh when the cached list is older than this.
pub const MAX_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// The size the Ships page shows.
const SIZE: ImageSize = ImageSize::Card;

/// What is cached on disk.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShipStoreCache {
    pub ships: Vec<StoreShip>,
    /// When the last refresh completed, Unix seconds. `0` = never.
    pub fetched_at: i64,
}

impl ShipStoreCache {
    /// Older than [`MAX_AGE`] at `now` (Unix seconds), or never filled.
    pub fn stale(&self, now: i64) -> bool {
        self.fetched_at == 0 || now - self.fetched_at >= MAX_AGE.as_secs() as i64
    }
}

/// The images the listed ships need.
pub fn wanted_images(ships: &[StoreShip]) -> Vec<WantedImage> {
    ships
        .iter()
        .filter_map(|s| WantedImage::new(u64::from(s.id), s.media.thumbnail.as_deref()?, SIZE))
        .collect()
}

pub struct ShipStoreService {
    cache_root: PathBuf,
    run_lock: tokio::sync::Mutex<()>,
}

impl ShipStoreService {
    pub fn new(cache_root: PathBuf) -> Self {
        Self {
            cache_root,
            run_lock: tokio::sync::Mutex::new(()),
        }
    }

    fn cache_path(&self) -> PathBuf {
        self.cache_root.join("shipstore.json")
    }

    /// Where the images go; the shell lets the webview read it.
    pub fn images_dir(&self) -> PathBuf {
        self.cache_root.join("images")
    }

    /// `ship`'s image on disk, if downloaded. No network.
    pub fn image_path(&self, ship: &StoreShip) -> Option<PathBuf> {
        media::best_on_disk(
            &self.images_dir(),
            u64::from(ship.id),
            ship.media.thumbnail.as_deref()?,
            SIZE,
        )
    }

    /// The cached list; `None` before the first refresh. No network.
    pub fn cached(&self) -> Option<ShipStoreCache> {
        let cache: ShipStoreCache = app_kit::load_json(&self.cache_path());
        (cache.fetched_at != 0).then_some(cache)
    }

    /// Read the store's ships, save them, fetch the missing images.
    /// Returns how many images were downloaded. **Network** — see the
    /// module contract.
    pub async fn refresh(&self, user_agent: &str) -> sc_dossier::Result<usize> {
        let _serial = self.run_lock.lock().await;
        let ships = PledgeStore::new(user_agent).ships().await?;
        let cache = ShipStoreCache {
            ships,
            fetched_at: chrono::Utc::now().timestamp(),
        };
        if let Err(e) = app_kit::save_json(&self.cache_path(), &cache) {
            tracing::warn!("failed to write the ship-store cache: {e}");
        }
        let images =
            media::sync(&self.images_dir(), &wanted_images(&cache.ships), user_agent).await;
        tracing::info!(ships = cache.ships.len(), images, "ship store refreshed");
        Ok(images)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ship(id: u32, thumb: Option<&str>) -> StoreShip {
        StoreShip {
            id,
            name: format!("Ship {id}"),
            url: String::new(),
            msrp_cents: None,
            published_at: None,
            media: ShipMedia {
                thumbnail: thumb.map(str::to_string),
                ..ShipMedia::default()
            },
        }
    }

    #[test]
    fn wants_one_card_per_ship_with_an_image() {
        let ships = [
            ship(
                1,
                Some("https://media.robertsspaceindustries.com/abc/store_small.jpg"),
            ),
            ship(2, None),
            ship(3, Some("https://example.test/x/store_small.jpg")),
        ];
        let wanted = wanted_images(&ships);
        assert_eq!(wanted.len(), 1);
        assert_eq!(wanted[0].file, "1-card.jpg");
        assert_eq!(
            wanted[0].url,
            "https://media.robertsspaceindustries.com/abc/store_small.jpg"
        );
    }

    #[test]
    fn stale_after_a_week() {
        let mut c = ShipStoreCache::default();
        assert!(c.stale(100));
        c.fetched_at = 100;
        assert!(!c.stale(100 + 3600));
        assert!(c.stale(100 + MAX_AGE.as_secs() as i64));
    }
}
