//! The layered snapshot cache (the Hearth `sc_loader::cache` port, slimmed
//! by sc-holotable v0.15 APIs — `AssetConfig::standard()` parses the locale
//! and `ExtractSnapshot::hydrate` replaces the manual re-parse).
//!
//! Two tiers per channel, both keyed by SC `build_id` so a patch
//! invalidates them:
//!
//! 1. **Processed snapshot** (`foundations.cook`) — the cooked
//!    [`CookedData`] serialized whole. Sub-second; no parsing. Also
//!    invalidated by a [`DATA_COOK_VERSION`] bump.
//! 2. **Raw extract snapshot** (`extract.snap`) — captured DCB +
//!    `global.ini` bytes; skips p4k reads but still pays the DCB parse.
//!    Also the designated future "fleet reference upload" artifact.
//!
//! Snapshot failures (missing file, version mismatch, staleness, decode
//! error) are non-fatal: they log at info level and the orchestrator in
//! [`crate::DataService::load`] falls through to the next tier. Writes are
//! atomic (`.tmp` + rename) inside sc-holotable.

use std::path::{Path, PathBuf};

use sc_holotable::asset::{
    AssetConfig, AssetData, AssetSource, Datacore, ExtractSnapshot, ProcessedSnapshot,
    SnapshotCaptureConfig, SnapshotMeta,
};

use crate::cooked::{CookedData, DATA_COOK_VERSION};
use crate::{InstallRef, LoadTier};

pub(crate) const PROCESSED_SNAPSHOT_NAME: &str = "foundations.cook";
pub(crate) const EXTRACT_SNAPSHOT_NAME: &str = "extract.snap";

/// Per-channel cache directory under the injected cache root.
pub(crate) fn channel_dir(cache_root: &Path, channel_key: &str) -> PathBuf {
    cache_root.join(channel_key)
}

/// Snapshot provenance from an install's identity fields.
pub(crate) fn meta_for(install: &InstallRef) -> SnapshotMeta {
    SnapshotMeta {
        schema_version: ExtractSnapshot::SCHEMA_VERSION,
        game_version: install.version.clone(),
        build_id: install.build_id.clone(),
        extracted_at: chrono::Utc::now().to_rfc3339(),
    }
}

/// Which cache tier the next load will *likely* use, from snapshot-file
/// existence (cheap, no parse). Not a guarantee — a stale snapshot after an
/// SC patch still falls through to a slower tier, which is why the UI pairs
/// it with a "may take longer" note.
pub(crate) fn predict_tier(dir: &Path) -> LoadTier {
    if dir.join(PROCESSED_SNAPSHOT_NAME).exists() {
        LoadTier::Processed
    } else if dir.join(EXTRACT_SNAPSHOT_NAME).exists() {
        LoadTier::Extract
    } else {
        LoadTier::Live
    }
}

/// Try to load the cooked data directly. `None` on any failure — the caller
/// falls through to the next tier. The build_id check catches SC patches
/// since the snapshot was written.
pub(crate) fn try_load_processed(dir: &Path, install: &InstallRef) -> Option<CookedData> {
    let path = dir.join(PROCESSED_SNAPSHOT_NAME);
    if !path.exists() {
        return None;
    }
    let snap = match ProcessedSnapshot::<CookedData>::load(&path, DATA_COOK_VERSION) {
        Ok(s) => s,
        Err(e) => {
            tracing::info!("processed snapshot unusable ({e}); falling back");
            return None;
        }
    };
    if snap.meta.build_id != install.build_id {
        tracing::info!(
            snapshot_build_id = %snap.meta.build_id,
            install_build_id = %install.build_id,
            "processed snapshot stale (SC patched); falling back"
        );
        return None;
    }
    Some(snap.into_index())
}

/// Persist the cooked data. Failure is non-fatal (logged): the load already
/// succeeded, the next session just re-cooks.
pub(crate) fn save_processed(dir: &Path, install: &InstallRef, cooked: &CookedData) {
    let path = dir.join(PROCESSED_SNAPSHOT_NAME);
    if let Err(e) = cooked.save(meta_for(install), &path) {
        tracing::warn!(
            "failed to save processed snapshot to {}: {e}",
            path.display()
        );
    } else {
        tracing::debug!("wrote processed snapshot to {}", path.display());
    }
}

/// Try to hydrate the raw extract snapshot into a live `Datacore` +
/// `AssetData` (locale included via `AssetConfig::standard()`). `None` on
/// any failure. Skips p4k reads entirely; still pays the DCB-parse cost.
pub(crate) fn try_load_extract(dir: &Path, install: &InstallRef) -> Option<(AssetData, Datacore)> {
    let path = dir.join(EXTRACT_SNAPSHOT_NAME);
    if !path.exists() {
        return None;
    }
    let snap = match ExtractSnapshot::load(&path) {
        Ok(s) => s,
        Err(e) => {
            tracing::info!("extract snapshot unusable ({e}); falling back to live parse");
            return None;
        }
    };
    if snap.meta.build_id != install.build_id {
        tracing::info!(
            snapshot_build_id = %snap.meta.build_id,
            install_build_id = %install.build_id,
            "extract snapshot stale (SC patched); falling back to live parse"
        );
        return None;
    }
    match snap.hydrate(&AssetConfig::standard()) {
        Ok(pair) => Some(pair),
        Err(e) => {
            tracing::info!("extract snapshot hydrate failed ({e}); falling back to live parse");
            None
        }
    }
}

/// Capture + persist the raw extract snapshot while the p4k source is still
/// open. Failure is non-fatal (logged).
pub(crate) fn save_extract(dir: &Path, install: &InstallRef, assets: &AssetSource) {
    let path = dir.join(EXTRACT_SNAPSHOT_NAME);
    let snap = match ExtractSnapshot::capture(
        assets,
        meta_for(install),
        &SnapshotCaptureConfig::standard(),
    ) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("failed to capture extract snapshot: {e}");
            return;
        }
    };
    if let Err(e) = snap.save(&path) {
        tracing::warn!("failed to save extract snapshot to {}: {e}", path.display());
    } else {
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        tracing::info!(bytes = size, "wrote extract snapshot to {}", path.display());
    }
}
