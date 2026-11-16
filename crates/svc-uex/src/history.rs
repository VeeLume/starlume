//! Marketplace price history, recorded by Starlume itself. UEX gives each
//! marketplace average only as it stands now (plus its own day, week and
//! month averages), so whether a price is stable or jumps, and how supply
//! and demand move, needs our own record (decision 'Plans pick one source
//! per need with a switch...', 2026-11-16: trust is a score from history).
//!
//! On every marketplace fetch the new list is compared with the cached one
//! it replaces, and each row that changed (or is new, or is gone) is
//! appended as one [`MarketPoint`] to a JSON-lines file per month under
//! `market-history/`. An unchanged price writes nothing, so the files grow
//! with the market's movement, not with the fetch rate. The very first
//! fetch writes every row once, as the baseline.
//!
//! Append-only and written outside the atomic-write path on purpose: a line
//! cut short by a crash is skipped on read, and nothing before it is lost.
//! Local only: recording sends nothing anywhere.

use std::collections::{HashMap, HashSet};
use std::fs::OpenOptions;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::MarketAverage;

/// One recorded marketplace average, as it stood at `t`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketPoint {
    /// When Starlume saw it, Unix seconds.
    pub t: i64,
    /// UEX's item id.
    pub item: u32,
    /// The item's GUID as UEX gives it (our DataCore record GUID), when it
    /// has one; kept so history joins our data without UEX's ids.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
    /// UEX's quality tier; 0: none.
    pub tier: i64,
    /// "sell" (players selling) or "buy" (players wanting).
    pub op: String,
    /// Listings behind the average (supply on "sell", demand on "buy").
    pub listings: i64,
    pub avg: f64,
    pub week: f64,
    pub month: f64,
    /// When UEX last changed the row, Unix seconds.
    pub modified: i64,
    /// The row is no longer in UEX's list.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub gone: bool,
}

type Key = (u32, i64, String);

fn key(r: &MarketAverage) -> Key {
    (r.id_item, r.quality_tier, r.operation.clone())
}

fn point(r: &MarketAverage, t: i64) -> MarketPoint {
    MarketPoint {
        t,
        item: r.id_item,
        uuid: r.item_uuid.clone(),
        tier: r.quality_tier,
        op: r.operation.clone(),
        listings: r.listings_count,
        avg: r.price_avg,
        week: r.price_avg_week,
        month: r.price_avg_month,
        modified: r.date_modified,
        gone: false,
    }
}

/// Whether two rows of one key say the same thing.
fn same(a: &MarketAverage, b: &MarketAverage) -> bool {
    a.listings_count == b.listings_count
        && a.price_avg == b.price_avg
        && a.price_avg_week == b.price_avg_week
        && a.price_avg_month == b.price_avg_month
        && a.date_modified == b.date_modified
}

/// The points between the list `previous` and the list `current` that
/// replaces it, seen at `t`: new and changed rows, and rows that are gone.
pub fn changes(previous: &[MarketAverage], current: &[MarketAverage], t: i64) -> Vec<MarketPoint> {
    let before: HashMap<Key, &MarketAverage> = previous.iter().map(|r| (key(r), r)).collect();
    let mut seen: HashSet<Key> = HashSet::with_capacity(current.len());
    let mut out = Vec::new();
    for r in current {
        let k = key(r);
        if !before.get(&k).is_some_and(|b| same(b, r)) {
            out.push(point(r, t));
        }
        seen.insert(k);
    }
    for r in previous {
        if !seen.contains(&key(r)) {
            out.push(MarketPoint {
                gone: true,
                ..point(r, t)
            });
        }
    }
    out
}

/// The history's directory under the service's dir.
pub fn dir_in(uex_dir: &Path) -> PathBuf {
    uex_dir.join("market-history")
}

/// The month file a point at `t` goes to: `2026-11.jsonl`.
fn month_file(dir: &Path, t: i64) -> PathBuf {
    let (y, m) = year_month(t);
    dir.join(format!("{y:04}-{m:02}.jsonl"))
}

/// Append `points` to the month file of their time. The count written.
pub fn append(dir: &Path, points: &[MarketPoint]) -> io::Result<usize> {
    let Some(first) = points.first() else {
        return Ok(0);
    };
    std::fs::create_dir_all(dir)?;
    let mut lines = Vec::new();
    for p in points {
        serde_json::to_writer(&mut lines, p).map_err(io::Error::other)?;
        lines.push(b'\n');
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(month_file(dir, first.t))?;
    file.write_all(&lines)?;
    Ok(points.len())
}

/// Every recorded point, oldest file first; a line that does not parse (cut
/// short by a crash) is skipped. No network.
pub fn read(dir: &Path) -> Vec<MarketPoint> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
        .collect();
    files.sort();
    let mut out = Vec::new();
    for path in files {
        let Ok(file) = std::fs::File::open(&path) else {
            continue;
        };
        out.extend(
            BufReader::new(file)
                .lines()
                .map_while(Result::ok)
                .filter_map(|l| serde_json::from_str(&l).ok()),
        );
    }
    out
}

/// Year and month (1-12) of Unix seconds `t`, UTC (days to civil date,
/// after Howard Hinnant's algorithm).
fn year_month(t: i64) -> (i64, u32) {
    let z = t.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(item: u32, op: &str, n: i64, avg: f64) -> MarketAverage {
        MarketAverage {
            id_item: item,
            quality_tier: 0,
            operation: op.into(),
            listings_count: n,
            price_avg: avg,
            price_avg_week: avg,
            price_avg_month: avg,
            game_version: None,
            date_modified: 1,
            item_name: format!("Item {item}"),
            item_uuid: None,
        }
    }

    #[test]
    fn only_new_changed_and_gone_rows_are_points() {
        let before = [
            row(1, "sell", 3, 185_000.0),
            row(2, "sell", 1, 11.0),
            row(3, "buy", 2, 5.0),
        ];
        let after = [
            row(1, "sell", 3, 185_000.0),
            row(2, "sell", 2, 12.0),
            row(4, "sell", 1, 7.0),
        ];
        let pts = changes(&before, &after, 100);
        let got: Vec<(u32, bool)> = pts.iter().map(|p| (p.item, p.gone)).collect();
        assert_eq!(got, [(2, false), (4, false), (3, true)]);
        assert!(pts.iter().all(|p| p.t == 100));
    }

    #[test]
    fn the_first_fetch_records_every_row_and_a_quiet_one_nothing() {
        let rows = [row(1, "sell", 3, 1.0), row(1, "buy", 1, 2.0)];
        assert_eq!(changes(&[], &rows, 5).len(), 2);
        assert!(changes(&rows, &rows, 6).is_empty());
    }

    #[test]
    fn points_append_per_month_and_read_back_past_a_cut_line() {
        let dir = tempfile::tempdir().unwrap();
        let nov = 1_794_816_000; // 2026-11-16
        let dec = 1_796_169_600; // 2026-12-02
        append(dir.path(), &changes(&[], &[row(1, "sell", 1, 1.0)], nov)).unwrap();
        append(dir.path(), &changes(&[], &[row(2, "sell", 1, 2.0)], dec)).unwrap();
        // A crash mid-line leaves a fragment behind.
        let nov_file = dir.path().join("2026-11.jsonl");
        assert!(nov_file.exists());
        let mut f = OpenOptions::new().append(true).open(&nov_file).unwrap();
        f.write_all(b"{\"t\":17948").unwrap();
        let items: Vec<u32> = read(dir.path()).iter().map(|p| p.item).collect();
        assert_eq!(items, [1, 2]);
        assert!(dir.path().join("2026-12.jsonl").exists());
    }

    #[test]
    fn months_of_unix_time() {
        assert_eq!(year_month(0), (1970, 1));
        assert_eq!(year_month(951_782_400), (2000, 2)); // 2000-02-29
        assert_eq!(year_month(1_794_816_000), (2026, 11));
    }
}
