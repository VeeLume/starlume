//! UEX service — community prices from UEX Corp's public API 2.0
//! (uexcorp.space): what the shops ask for each part, where ships are sold
//! and rented, and what players ask on UEX's marketplace. The client data
//! has no shop inventories (they are server-side), so this is the "where
//! to buy it" the Ships page shows. The data is crowd-reported; every row
//! carries the date of its last report.
//!
//! # Online-policy contract — READ THIS
//!
//! Public API reads, anonymous (no token). UEX is a third party, not an
//! RSI/CIG API, so the gate is `require_online()`, not the unofficial-API
//! opt-in. [`UexService::refresh`] is network and **must** be called only
//! after the shell passed `AppState::require_online()`.
//! [`UexService::snapshot`] and [`UexService::stale`] touch no network.
//!
//! # Posture
//!
//! Six lists ([`Feed`]), each one request, kept in its own file under the
//! service's dir. Each is fetched again only once its answer's
//! `Cache-Control: max-age` has run out (12 h; the marketplace 1 h), as
//! UEX asks: every user's app reads for itself, so the request rate is one
//! person's. The lists are loaded per question and dropped (memory rule:
//! nothing resident while the app idles).

pub mod feeds;
pub mod lookup;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

pub use feeds::{Feed, ItemPrice, MarketAverage, Terminal, Vehicle, VehiclePrice};
pub use lookup::{Index, ItemOffers, Offer, Snapshot, VehicleOffers, norm};

pub const BASE: &str = "https://api.uexcorp.uk/2.0/";

/// Never refetch a list sooner than this, whatever the header says.
const MIN_AGE: i64 = 15 * 60;

/// The rows' shape in the cache. A change (a field added) bumps it, and a
/// cache of another shape counts as never fetched.
/// 2: vehicles' `pad_type`.
pub const SCHEMA: u32 = 2;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("UEX request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("UEX answered {status} for {endpoint}: {message}")]
    Refused {
        endpoint: &'static str,
        status: String,
        message: String,
    },
    #[error("UEX's {endpoint} list did not parse: {source}")]
    Parse {
        endpoint: &'static str,
        source: serde_json::Error,
    },
    #[error("could not write the UEX cache: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// When each list was fetched and how long it stays fresh, apart from the
/// lists themselves (the item prices are megabytes; the hourly check reads
/// only this).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Meta {
    /// [`SCHEMA`] the cached rows were written with; 0: before it existed.
    pub schema: u32,
    pub feeds: BTreeMap<String, FeedMeta>,
}

impl Meta {
    /// When `feed` was fetched and how long it stays fresh; `None` when it
    /// never was, or the cache is of another [`SCHEMA`].
    pub fn feed(&self, feed: Feed) -> Option<&FeedMeta> {
        (self.schema == SCHEMA)
            .then(|| self.feeds.get(feed.endpoint()))
            .flatten()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FeedMeta {
    /// Unix seconds; 0: never.
    pub fetched_at: i64,
    /// Seconds it stays fresh (the answer's `max-age`).
    pub max_age: i64,
    pub rows: usize,
}

impl FeedMeta {
    pub fn stale(&self, now: i64) -> bool {
        self.fetched_at == 0 || now - self.fetched_at >= self.max_age.max(MIN_AGE)
    }
}

/// What one refresh did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RefreshReport {
    /// The lists fetched, by endpoint.
    pub fetched: Vec<&'static str>,
    /// The first fill: nothing was cached before.
    pub first: bool,
}

#[derive(Debug, Clone)]
pub struct UexService {
    dir: PathBuf,
}

impl UexService {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn meta_path(&self) -> PathBuf {
        self.dir.join("meta.json")
    }

    fn feed_path(&self, feed: Feed) -> PathBuf {
        self.dir.join(format!("{}.json", feed.endpoint()))
    }

    pub fn meta(&self) -> Meta {
        app_kit::load_json(&self.meta_path())
    }

    /// Any list due at `now` (Unix seconds). No network.
    pub fn stale(&self, now: i64) -> bool {
        let meta = self.meta();
        Feed::ALL
            .iter()
            .any(|f| meta.feed(*f).is_none_or(|m| m.stale(now)))
    }

    /// When the item prices were last fetched; `None`: never.
    pub fn fetched_at(&self) -> Option<i64> {
        self.meta()
            .feed(Feed::ItemPrices)
            .map(|m| m.fetched_at)
            .filter(|&t| t > 0)
    }

    /// Everything cached, for one question. No network.
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            item_prices: self.rows(Feed::ItemPrices),
            purchases: self.rows(Feed::VehiclePurchases),
            rentals: self.rows(Feed::VehicleRentals),
            vehicles: self.rows(Feed::Vehicles),
            terminals: self.rows(Feed::Terminals),
            market: self.rows(Feed::Marketplace),
        }
    }

    fn rows<T: DeserializeOwned>(&self, feed: Feed) -> Vec<T> {
        app_kit::load_json(&self.feed_path(feed))
    }

    /// Fetch every list that is due at `now`, one request each, one after
    /// another. A list that fails keeps its old copy; the first error is
    /// returned after the others had their turn. **Network — gated by the
    /// caller.**
    pub async fn refresh(&self, user_agent: &str, now: i64) -> Result<RefreshReport> {
        let client = reqwest::Client::builder()
            .user_agent(user_agent)
            .timeout(Duration::from_secs(60))
            .build()?;
        let mut meta = self.meta();
        if meta.schema != SCHEMA {
            // Rows of another shape: fetch every list again.
            meta = Meta {
                schema: SCHEMA,
                feeds: BTreeMap::new(),
            };
        }
        let mut report = RefreshReport {
            first: meta.feeds.is_empty(),
            ..RefreshReport::default()
        };
        let mut first_error = None;
        for feed in Feed::ALL {
            if !meta.feed(feed).is_none_or(|m| m.stale(now)) {
                continue;
            }
            let fetched = match feed {
                Feed::ItemPrices => self.fetch::<ItemPrice>(&client, feed).await,
                Feed::VehiclePurchases | Feed::VehicleRentals => {
                    self.fetch::<VehiclePrice>(&client, feed).await
                }
                Feed::Vehicles => self.fetch::<Vehicle>(&client, feed).await,
                Feed::Terminals => self.fetch::<Terminal>(&client, feed).await,
                Feed::Marketplace => self.fetch::<MarketAverage>(&client, feed).await,
            };
            match fetched {
                Ok((rows, max_age)) => {
                    meta.feeds.insert(
                        feed.endpoint().to_string(),
                        FeedMeta {
                            fetched_at: now,
                            max_age: max_age.unwrap_or_else(|| feed.default_max_age()),
                            rows,
                        },
                    );
                    app_kit::save_json(&self.meta_path(), &meta)?;
                    report.fetched.push(feed.endpoint());
                }
                Err(e) => {
                    tracing::info!("UEX {} not refreshed: {e}", feed.endpoint());
                    first_error.get_or_insert(e);
                }
            }
        }
        match first_error {
            Some(e) => Err(e),
            None => Ok(report),
        }
    }

    /// One list: read, keep the rows that parse, save. The row count and
    /// the answer's `max-age`.
    async fn fetch<T: DeserializeOwned + Serialize>(
        &self,
        client: &reqwest::Client,
        feed: Feed,
    ) -> Result<(usize, Option<i64>)> {
        let endpoint = feed.endpoint();
        let response = client.get(format!("{BASE}{endpoint}")).send().await?;
        let max_age = response
            .headers()
            .get(reqwest::header::CACHE_CONTROL)
            .and_then(|v| v.to_str().ok())
            .and_then(max_age);
        let body = response.error_for_status()?.bytes().await?;
        let rows: Vec<T> = parse(endpoint, &body)?;
        // Compact, not pretty: the item prices are megabytes, read whole
        // per question.
        let bytes =
            serde_json::to_vec(&rows).map_err(|source| Error::Parse { endpoint, source })?;
        app_kit::atomic_write(&self.feed_path(feed), &bytes)?;
        Ok((rows.len(), max_age))
    }
}

/// UEX's envelope (`{"status": "ok", "data": [...]}`), its rows read one by
/// one: a row that doesn't parse is skipped, not the list.
fn parse<T: DeserializeOwned>(endpoint: &'static str, body: &[u8]) -> Result<Vec<T>> {
    #[derive(Deserialize)]
    struct Envelope {
        status: String,
        #[serde(default)]
        message: String,
        #[serde(default)]
        data: Vec<serde_json::Value>,
    }
    let env: Envelope =
        serde_json::from_slice(body).map_err(|source| Error::Parse { endpoint, source })?;
    if env.status != "ok" {
        return Err(Error::Refused {
            endpoint,
            status: env.status,
            message: env.message,
        });
    }
    let total = env.data.len();
    let rows: Vec<T> = env
        .data
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect();
    if rows.len() < total {
        tracing::info!(
            "UEX {endpoint}: {} of {total} rows skipped",
            total - rows.len()
        );
    }
    Ok(rows)
}

/// `max-age` from a `Cache-Control` header, seconds.
fn max_age(header: &str) -> Option<i64> {
    header
        .split(',')
        .find_map(|part| part.trim().strip_prefix("max-age="))
        .and_then(|v| v.trim().parse().ok())
}

/// The service's directory under `root` (the app data root).
pub fn dir_in(root: &Path) -> PathBuf {
    root.join("uex")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_control() {
        assert_eq!(
            max_age("public, max-age=43200, must-revalidate"),
            Some(43200)
        );
        assert_eq!(max_age("no-store"), None);
    }

    #[test]
    fn envelope_skips_bad_rows_and_reports_refusals() {
        let ok = br#"{"status":"ok","data":[{"id":1,"uuid":"a","name":"M50","name_full":"Origin M50"},{"name":"no id"}]}"#;
        let rows: Vec<Vehicle> = parse("vehicles", ok).unwrap();
        assert_eq!(rows.len(), 1);
        let limit = br#"{"status":"requests_limit_reached","message":"slow down","data":[]}"#;
        assert!(matches!(
            parse::<Vehicle>("vehicles", limit),
            Err(Error::Refused { .. })
        ));
    }

    #[test]
    fn staleness_follows_max_age_with_a_floor() {
        let m = FeedMeta {
            fetched_at: 1000,
            max_age: 3600,
            rows: 1,
        };
        assert!(!m.stale(1000 + 3599));
        assert!(m.stale(1000 + 3600));
        let short = FeedMeta {
            fetched_at: 1000,
            max_age: 1,
            rows: 1,
        };
        assert!(!short.stale(1000 + 60));
        assert!(FeedMeta::default().stale(0));
    }

    #[test]
    fn snapshot_of_an_empty_dir_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let s = UexService::new(dir.path().to_path_buf());
        assert!(s.stale(0));
        assert!(s.snapshot().item_prices.is_empty());
        assert_eq!(s.fetched_at(), None);
    }
}
