//! The derive stage + its op-set cache.
//!
//! Derive is the only stage that needs game data ([`svc_data::CookedData`]);
//! its output — one [`OpSet`] per patcher — is cached on disk per
//! (build staleness key, patcher, options hash). Toggling patchers or
//! re-applying after a foreign write then never re-derives, and re-deriving
//! never re-parses (the CookedData comes from svc-data's snapshot cache).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::Patcher;
use crate::ops::{LangpatchConfig, OpSet, stable_hash};

/// One patcher's cached derive result.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    options_hash: String,
    ops: OpSet,
}

/// Per-channel cache file: all patcher op-sets for one build.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct CacheFile {
    staleness_key: String,
    entries: BTreeMap<String, CacheEntry>,
}

fn cache_path(data_dir: &Path, channel_key: &str) -> PathBuf {
    data_dir.join(format!("ops-{channel_key}.json"))
}

/// One enabled patcher's ops, ready for the apply stage.
#[derive(Debug, Clone)]
pub struct PatcherOps {
    pub id: String,
    pub priority: u32,
    pub ops: OpSet,
}

/// Do the cached op-sets already cover every enabled patcher for this
/// build + config? When true, [`derive_ops`] won't call any
/// [`Patcher::derive`] — callers can skip loading `CookedData` entirely
/// (`derive_ops` then accepts `cooked: None`).
pub fn cache_complete(
    data_dir: &Path,
    channel_key: &str,
    staleness_key: &str,
    config: &LangpatchConfig,
    patchers: &[Box<dyn Patcher>],
) -> bool {
    let enabled: Vec<_> = patchers
        .iter()
        .filter(|p| config.patcher_enabled(p.as_ref()))
        .collect();
    if enabled.is_empty() {
        return true;
    }
    let cache: CacheFile = app_kit::load_json(&cache_path(data_dir, channel_key));
    if cache.staleness_key != staleness_key {
        return false;
    }
    enabled.iter().all(|p| {
        cache
            .entries
            .get(p.id())
            .is_some_and(|e| e.options_hash == stable_hash(&config.patcher_config(p.id())))
    })
}

/// Cache-miss error: derive needed game data the caller didn't provide.
#[derive(Debug, thiserror::Error)]
#[error("derive needs cooked game data for {missing:?} but none was provided")]
pub struct DeriveError {
    pub missing: Vec<String>,
}

/// Produce the op-sets for every enabled patcher, in apply order
/// (priority, then id). Cached entries are reused; misses derive from
/// `cooked` and refresh the cache. Pass `cooked: None` only when
/// [`cache_complete`] said so — otherwise this fails with the list of
/// patchers that needed data.
pub fn derive_ops(
    data_dir: &Path,
    channel_key: &str,
    staleness_key: &str,
    cooked: Option<&svc_data::CookedData>,
    config: &LangpatchConfig,
    patchers: &[Box<dyn Patcher>],
) -> anyhow::Result<Vec<PatcherOps>> {
    let path = cache_path(data_dir, channel_key);
    let mut cache: CacheFile = app_kit::load_json(&path);
    if cache.staleness_key != staleness_key {
        // New build → every cached op-set is garbage.
        cache = CacheFile {
            staleness_key: staleness_key.to_string(),
            entries: BTreeMap::new(),
        };
    }

    let mut out = Vec::new();
    let mut dirty = false;
    let mut missing = Vec::new();

    for patcher in patchers {
        if !config.patcher_enabled(patcher.as_ref()) {
            continue;
        }
        let id = patcher.id();
        let options_hash = stable_hash(&config.patcher_config(id));

        let ops = match cache
            .entries
            .get(id)
            .filter(|e| e.options_hash == options_hash)
        {
            Some(entry) => entry.ops.clone(),
            None => {
                let Some(cooked) = cooked else {
                    missing.push(id.to_string());
                    continue;
                };
                let ops = patcher
                    .derive(cooked, &config.patcher_config(id))
                    .map_err(|e| e.context(format!("derive failed for patcher '{id}'")))?;
                tracing::info!(
                    patcher = id,
                    renames = ops.renames.len(),
                    patches = ops.patches.len(),
                    "derived op-set"
                );
                cache.entries.insert(
                    id.to_string(),
                    CacheEntry {
                        options_hash,
                        ops: ops.clone(),
                    },
                );
                dirty = true;
                ops
            }
        };
        out.push(PatcherOps {
            id: id.to_string(),
            priority: patcher.priority(),
            ops,
        });
    }

    if !missing.is_empty() {
        return Err(DeriveError { missing }.into());
    }
    if dirty && let Err(e) = app_kit::save_json(&path, &cache) {
        tracing::warn!("failed to persist op-set cache: {e}");
    }

    out.sort_by(|a, b| a.priority.cmp(&b.priority).then_with(|| a.id.cmp(&b.id)));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::{PatchOp, PatcherConfig};

    struct FakePatcher {
        id: &'static str,
        derives: std::sync::atomic::AtomicUsize,
    }

    impl FakePatcher {
        fn new(id: &'static str) -> Self {
            Self {
                id,
                derives: std::sync::atomic::AtomicUsize::new(0),
            }
        }
    }

    impl Patcher for FakePatcher {
        fn id(&self) -> &'static str {
            self.id
        }
        fn name(&self) -> &'static str {
            "Fake"
        }
        fn description(&self) -> &'static str {
            "test"
        }
        fn derive(
            &self,
            _cooked: &svc_data::CookedData,
            config: &PatcherConfig,
        ) -> anyhow::Result<OpSet> {
            self.derives
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(OpSet {
                renames: Vec::new(),
                patches: vec![(
                    "some_key".into(),
                    PatchOp::Prefix(format!("[{}]", config.get_str("tag", "x"))),
                )],
            })
        }
    }

    fn cooked() -> svc_data::CookedData {
        svc_data::CookedData::default()
    }

    #[test]
    fn cache_hits_skip_derive_and_survive_options_change() {
        let dir = tempfile::tempdir().unwrap();
        let config = LangpatchConfig::default();
        let patchers: Vec<Box<dyn Patcher>> = vec![Box::new(FakePatcher::new("fake"))];

        assert!(!cache_complete(
            dir.path(),
            "live",
            "b1",
            &config,
            &patchers
        ));
        let first = derive_ops(
            dir.path(),
            "live",
            "b1",
            Some(&cooked()),
            &config,
            &patchers,
        )
        .unwrap();
        assert_eq!(first.len(), 1);
        assert!(cache_complete(dir.path(), "live", "b1", &config, &patchers));

        // Second run: cache hit, cooked not needed at all.
        let second = derive_ops(dir.path(), "live", "b1", None, &config, &patchers).unwrap();
        assert_eq!(second[0].ops, first[0].ops);

        // Option change → cache miss for that patcher.
        let mut changed = config.clone();
        changed
            .patchers
            .entry("fake".into())
            .or_default()
            .options
            .insert("tag".into(), "y".into());
        assert!(!cache_complete(
            dir.path(),
            "live",
            "b1",
            &changed,
            &patchers
        ));
        let third = derive_ops(
            dir.path(),
            "live",
            "b1",
            Some(&cooked()),
            &changed,
            &patchers,
        )
        .unwrap();
        assert_eq!(third[0].ops.patches[0].1, PatchOp::Prefix("[y]".into()));

        // New build → cache invalid even with the old config.
        assert!(!cache_complete(
            dir.path(),
            "live",
            "b2",
            &config,
            &patchers
        ));
    }

    #[test]
    fn missing_cooked_on_cache_miss_errors_with_patcher_list() {
        let dir = tempfile::tempdir().unwrap();
        let config = LangpatchConfig::default();
        let patchers: Vec<Box<dyn Patcher>> = vec![Box::new(FakePatcher::new("fake"))];
        let err = derive_ops(dir.path(), "live", "b1", None, &config, &patchers).unwrap_err();
        let derive_err = err.downcast_ref::<DeriveError>().expect("DeriveError");
        assert_eq!(derive_err.missing, vec!["fake".to_string()]);
    }

    #[test]
    fn disabled_patchers_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = LangpatchConfig::default();
        config.patchers.entry("fake".into()).or_default().enabled = Some(false);
        let patchers: Vec<Box<dyn Patcher>> = vec![Box::new(FakePatcher::new("fake"))];
        let ops = derive_ops(dir.path(), "live", "b1", None, &config, &patchers).unwrap();
        assert!(ops.is_empty());
        assert!(cache_complete(dir.path(), "live", "b1", &config, &patchers));
    }
}
