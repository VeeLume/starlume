//! Build identity, the build-keyed cache layout, and archive retention.
//!
//! The cache is keyed by **build**, not by channel: `cache/builds/<key>/`
//! holds a build's snapshots plus a small `build.json` describing it. The
//! key is the changelist from the version label (`4.10.2-live.12881860` →
//! `12881860`), never the release: PTU waves and Live hotfixes keep the
//! release label but change the data. A build promoted PTU → Live with the
//! same changelist therefore shares one directory and one parse.
//!
//! When a channel moves to a new build, the old build's directory stays as
//! an **archive** — the raw extract snapshot only, since it can be re-cooked
//! by newer cook code (the cooked snapshot is a disposable cache). Which
//! archives survive is [`plan_prune`]'s job: grouped by release, newest
//! first, Live/Hotfix only unless the user includes test channels. Builds
//! of currently installed channels are never touched. The archive exists so
//! patch diffs ("what changed") can be computed later.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::InstallRef;

/// Subdirectory of the cache root holding one directory per build.
pub(crate) const BUILDS_DIR: &str = "builds";
const META_NAME: &str = "build.json";

/// The changelist in a version label (`4.10.2-live.12881860` →
/// `12881860`), or a bare changelist (`12881860`, the form build keys and
/// new snapshot metadata use). `None` for labels without one — a manifest
/// version like `4.10.2` must not read as changelist 2.
pub fn changelist(label: &str) -> Option<u64> {
    if !label.is_empty() && label.bytes().all(|b| b.is_ascii_digit()) {
        return label.parse().ok();
    }
    let (_, tail) = label.rsplit_once('-')?;
    let (_, digits) = tail.rsplit_once('.')?;
    digits.parse().ok()
}

/// The release a version label belongs to (`4.10.2-live.12881860` →
/// `4.10.2`). Retention groups builds by it.
pub fn release(label: &str) -> &str {
    label.split_once('-').map_or(label, |(release, _)| release)
}

/// Live and Hotfix are the builds people play; everything else (PTU, EPTU,
/// Tech Preview) is a test channel.
pub fn is_prod_channel(channel_key: &str) -> bool {
    matches!(channel_key, "live" | "hotfix")
}

/// The build key for any label or id: its changelist when it has one, else
/// the label itself made filesystem-safe.
pub(crate) fn key_of(label: &str) -> String {
    match changelist(label) {
        Some(cl) => cl.to_string(),
        None => label
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                    c
                } else {
                    '_'
                }
            })
            .collect(),
    }
}

impl InstallRef {
    /// This install's build key — the cache directory name. The version
    /// label's changelist when it has one (the normal case; `build_id` is
    /// literally `"None"` on current builds), else the staleness key.
    pub fn build_key(&self) -> String {
        if changelist(&self.version).is_some() {
            key_of(&self.version)
        } else {
            key_of(&self.build_id)
        }
    }

    /// The release this install's build belongs to (`4.10.2`).
    pub fn release(&self) -> String {
        release(&self.version).to_string()
    }
}

/// `build.json` — what a build directory holds, readable without opening
/// a snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildMeta {
    pub key: String,
    /// Version label as last seen (`4.10.2-live.12881860`).
    pub version: String,
    pub release: String,
    pub changelist: Option<u64>,
    /// Every channel this build has been installed on (`live`, `ptu`, …).
    pub channels: BTreeSet<String>,
    /// RFC 3339; updated whenever the build is loaded.
    pub last_used: String,
}

impl BuildMeta {
    /// Was this build ever a Live/Hotfix build?
    pub fn is_prod(&self) -> bool {
        self.channels.iter().any(|c| is_prod_channel(c))
    }
}

/// Which archived builds to keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetentionPolicy {
    /// How many releases (newest first) keep an archived build. At least 1.
    pub releases: usize,
    /// Keep every build within a kept release, not just its newest one.
    pub all_builds: bool,
    /// Archive PTU / Tech Preview builds too (default: Live/Hotfix only).
    pub test_channels: bool,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            releases: 3,
            all_builds: false,
            test_channels: false,
        }
    }
}

/// The builds to delete. Pure — the retention rules in one place:
///
/// - builds in `current` (installed right now) are never deleted;
/// - a build counts only if it ran on Live/Hotfix, or test channels are
///   included; uncounted archives are deleted;
/// - releases are ranked by their newest counted changelist and the top
///   `policy.releases` keep archives; older releases' archives go;
/// - within a kept release only the newest counted build survives (a
///   hotfix replaces its release's archive) unless `all_builds` is set —
///   the current build counts as that newest one.
pub fn plan_prune(
    builds: &[BuildMeta],
    current: &BTreeSet<String>,
    policy: RetentionPolicy,
) -> Vec<String> {
    let counted = |b: &&BuildMeta| policy.test_channels || b.is_prod();
    let cl = |b: &BuildMeta| b.changelist.unwrap_or(0);

    // release → newest counted changelist in it.
    let mut newest: BTreeMap<&str, u64> = BTreeMap::new();
    for b in builds.iter().filter(counted) {
        let entry = newest.entry(b.release.as_str()).or_insert(0);
        *entry = (*entry).max(cl(b));
    }
    let mut ranked: Vec<(&str, u64)> = newest.iter().map(|(r, c)| (*r, *c)).collect();
    ranked.sort_by_key(|(_, cl)| std::cmp::Reverse(*cl));
    let kept: BTreeSet<&str> = ranked
        .iter()
        .take(policy.releases.max(1))
        .map(|(r, _)| *r)
        .collect();

    builds
        .iter()
        .filter(|b| !current.contains(&b.key))
        .filter(|b| {
            if !counted(b) || !kept.contains(b.release.as_str()) {
                return true;
            }
            !policy.all_builds && cl(b) < newest[b.release.as_str()]
        })
        .map(|b| b.key.clone())
        .collect()
}

// ── On-disk layout ─────────────────────────────────────────────────────────

pub(crate) fn builds_root(cache_root: &Path) -> PathBuf {
    cache_root.join(BUILDS_DIR)
}

pub(crate) fn read_meta(dir: &Path) -> Option<BuildMeta> {
    let bytes = std::fs::read(dir.join(META_NAME)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Create or refresh a build's `build.json`: record the channel it is
/// installed on and when it was last used. Best-effort.
pub(crate) fn touch_meta(dir: &Path, install: &InstallRef) {
    let mut meta = read_meta(dir).unwrap_or_else(|| BuildMeta {
        key: install.build_key(),
        version: install.version.clone(),
        release: install.release(),
        changelist: changelist(&install.version),
        channels: BTreeSet::new(),
        last_used: String::new(),
    });
    meta.version = install.version.clone();
    meta.channels.insert(install.channel_key.clone());
    meta.last_used = chrono::Utc::now().to_rfc3339();
    if let Err(e) = std::fs::create_dir_all(dir).and_then(|()| {
        let json = serde_json::to_vec_pretty(&meta).map_err(std::io::Error::other)?;
        std::fs::write(dir.join(META_NAME), json)
    }) {
        tracing::debug!("failed to write {}: {e}", dir.join(META_NAME).display());
    }
}

/// Every build directory that has a `build.json`, with its size on disk.
/// Directories without one (mid-creation, foreign) are left alone.
pub(crate) fn list(cache_root: &Path) -> Vec<(PathBuf, BuildMeta, u64)> {
    let Ok(entries) = std::fs::read_dir(builds_root(cache_root)) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .filter_map(|p| {
            let meta = read_meta(&p)?;
            let bytes = dir_size(&p);
            Some((p, meta, bytes))
        })
        .collect()
}

fn dir_size(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|e| e.metadata().ok())
                .filter(|m| m.is_file())
                .map(|m| m.len())
                .sum()
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(key: &str, release: &str, channels: &[&str]) -> BuildMeta {
        BuildMeta {
            key: key.into(),
            version: format!("{release}-x.{key}"),
            release: release.into(),
            changelist: key.parse().ok(),
            channels: channels.iter().map(|c| c.to_string()).collect(),
            last_used: String::new(),
        }
    }

    fn keys(v: &[&str]) -> BTreeSet<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn pruned(builds: &[BuildMeta], current: &[&str], policy: RetentionPolicy) -> Vec<String> {
        let mut out = plan_prune(builds, &keys(current), policy);
        out.sort();
        out
    }

    #[test]
    fn changelist_and_release_parse_labels() {
        assert_eq!(changelist("4.10.2-live.12881860"), Some(12881860));
        assert_eq!(changelist("12881860"), Some(12881860));
        // A bare release must not read as changelist 2.
        assert_eq!(changelist("4.10.2"), None);
        assert_eq!(changelist("4.8.0-test"), None);
        assert_eq!(changelist(""), None);
        assert_eq!(release("4.10.2-ptu.12900001"), "4.10.2");
        assert_eq!(release("4.10.2"), "4.10.2");
    }

    #[test]
    fn build_key_is_the_changelist_shared_across_channels() {
        let live = InstallRef {
            channel_key: "live".into(),
            p4k_path: PathBuf::new(),
            build_id: "4.10.2-live.12881860".into(),
            version: "4.10.2-live.12881860".into(),
        };
        let ptu = InstallRef {
            channel_key: "ptu".into(),
            version: "4.10.2-ptu.12881860".into(),
            build_id: "4.10.2-ptu.12881860".into(),
            ..live.clone()
        };
        assert_eq!(live.build_key(), "12881860");
        assert_eq!(ptu.build_key(), live.build_key());
        // No changelist anywhere → the staleness key, filesystem-safe.
        let odd = InstallRef {
            version: "4.10.2".into(),
            build_id: "abc/def".into(),
            ..live
        };
        assert_eq!(odd.build_key(), "abc_def");
    }

    #[test]
    fn default_keeps_newest_live_build_of_the_last_three_releases() {
        let builds = [
            build("500", "4.10.2", &["live"]), // current
            build("490", "4.10.2", &["live"]), // replaced by the hotfix above
            build("400", "4.10.1", &["live"]),
            build("390", "4.10.1", &["hotfix"]),
            build("300", "4.10.0", &["live"]),
            build("200", "4.9.3", &["live"]), // fourth release
            build("510", "4.11.0", &["ptu"]), // test channel, not current
        ];
        assert_eq!(
            pruned(&builds, &["500"], RetentionPolicy::default()),
            ["200", "390", "490", "510"]
        );
    }

    #[test]
    fn current_builds_survive_whatever_the_policy() {
        let builds = [
            build("510", "4.11.0", &["ptu"]),
            build("500", "4.10.2", &["live"]),
            build("100", "4.8.0", &["live"]),
        ];
        let policy = RetentionPolicy {
            releases: 1,
            ..RetentionPolicy::default()
        };
        // The PTU build is current: kept, but it doesn't push the Live
        // release out of the count.
        assert_eq!(pruned(&builds, &["510", "500"], policy), ["100"]);
    }

    #[test]
    fn power_user_options_widen_the_archive() {
        let builds = [
            build("500", "4.10.2", &["live"]),
            build("490", "4.10.2", &["live"]),
            build("480", "4.10.2", &["ptu"]),
            build("520", "4.11.0", &["ptu"]),
            build("300", "4.10.0", &["live"]),
        ];
        let all = RetentionPolicy {
            releases: 3,
            all_builds: true,
            test_channels: true,
        };
        assert!(pruned(&builds, &["500"], all).is_empty());

        // Test channels count toward the release ranking when included.
        let two = RetentionPolicy {
            releases: 2,
            all_builds: false,
            test_channels: true,
        };
        assert_eq!(pruned(&builds, &[], two), ["300", "480", "490"]);
    }

    #[test]
    fn a_build_seen_on_live_counts_even_if_it_started_on_ptu() {
        let builds = [build("500", "4.10.2", &["ptu", "live"])];
        assert!(pruned(&builds, &[], RetentionPolicy::default()).is_empty());
    }
}
