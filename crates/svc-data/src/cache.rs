//! The layered snapshot cache (the Hearth `sc_loader::cache` port, slimmed
//! by sc-holotable v0.15 APIs — `AssetConfig::standard()` parses the locale
//! and `ExtractSnapshot::hydrate` replaces the manual re-parse).
//!
//! Two tiers per build directory (`cache/builds/<key>/`, see
//! [`crate::builds`]), both stamped with the build key so a snapshot can
//! never be served for a different build:
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

/// Snapshot provenance from an install's identity fields.
pub(crate) fn meta_for(install: &InstallRef) -> SnapshotMeta {
    SnapshotMeta {
        schema_version: ExtractSnapshot::SCHEMA_VERSION,
        game_version: install.version.clone(),
        build_id: install.build_key(),
        extracted_at: chrono::Utc::now().to_rfc3339(),
    }
}

/// Which cache tier the next load will *likely* use. Cheap — reads only the
/// tiny key sidecars written beside each snapshot, never the snapshot. A
/// snapshot whose key doesn't match this install's build (an SC patch) or
/// the current [`DATA_COOK_VERSION`] (a cook-rev bump) predicts the slower
/// tier the load will actually fall through to, so the UI never promises
/// "~1s" for a rebuild. Not a guarantee: a corrupt snapshot still falls
/// through at load time.
pub(crate) fn predict_tier(dir: &Path, install: &InstallRef) -> LoadTier {
    if dir.join(PROCESSED_SNAPSHOT_NAME).exists()
        && read_key(dir, PROCESSED_SNAPSHOT_NAME).as_deref() == Some(&processed_key(install))
    {
        LoadTier::Processed
    } else if dir.join(EXTRACT_SNAPSHOT_NAME).exists()
        && read_key(dir, EXTRACT_SNAPSHOT_NAME).as_deref() == Some(&extract_key(install))
    {
        LoadTier::Extract
    } else {
        LoadTier::Live
    }
}

/// What a processed snapshot must have been written for to be current.
fn processed_key(install: &InstallRef) -> String {
    format!("{DATA_COOK_VERSION}\n{}", install.build_key())
}

/// What an extract snapshot must have been written for to be current.
fn extract_key(install: &InstallRef) -> String {
    format!(
        "{}\n{}",
        ExtractSnapshot::SCHEMA_VERSION,
        install.build_key()
    )
}

fn key_path(dir: &Path, snapshot: &str) -> PathBuf {
    dir.join(format!("{snapshot}.key"))
}

fn read_key(dir: &Path, snapshot: &str) -> Option<String> {
    std::fs::read_to_string(key_path(dir, snapshot)).ok()
}

/// Record which build/version a snapshot is current for. Written after each
/// save and after each successful load (snapshots predating the sidecar get
/// one on their first use). Best-effort: a missing key only makes the
/// prediction pessimistic.
fn write_key(dir: &Path, snapshot: &str, key: &str) {
    if read_key(dir, snapshot).as_deref() == Some(key) {
        return;
    }
    if let Err(e) = std::fs::write(key_path(dir, snapshot), key) {
        tracing::debug!("failed to write {snapshot} key: {e}");
    }
}

/// Was a snapshot stamped `stamp` written for this install's build? New
/// snapshots carry the build key. Ones from the per-channel era carry the
/// install's staleness key: the version label (whose changelist is the same
/// key) where `build_manifest.id` was `"None"`, or the real manifest id
/// where CIG ships one — that one only ever matches its own install.
fn same_build(stamp: &str, install: &InstallRef) -> bool {
    crate::builds::key_of(stamp) == install.build_key() || stamp == install.build_id
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
    if !same_build(&snap.meta.build_id, install) {
        tracing::info!(
            snapshot_build_id = %snap.meta.build_id,
            install_build = %install.build_key(),
            "processed snapshot stale (SC patched); falling back"
        );
        return None;
    }
    write_key(dir, PROCESSED_SNAPSHOT_NAME, &processed_key(install));
    Some(snap.into_index())
}

/// The provenance stamped into a directory's extract snapshot, if it has a
/// readable one. A full read — for one-time adoption of legacy dirs only.
pub(crate) fn extract_meta(dir: &Path) -> Option<SnapshotMeta> {
    ExtractSnapshot::load(&dir.join(EXTRACT_SNAPSHOT_NAME))
        .ok()
        .map(|snap| snap.meta)
}

/// Delete the cooked snapshot (and its key) — slimming an archived build to
/// its extract.
pub(crate) fn drop_processed(dir: &Path) -> std::io::Result<()> {
    let _ = std::fs::remove_file(key_path(dir, PROCESSED_SNAPSHOT_NAME));
    std::fs::remove_file(dir.join(PROCESSED_SNAPSHOT_NAME))
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
        write_key(dir, PROCESSED_SNAPSHOT_NAME, &processed_key(install));
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
    if !same_build(&snap.meta.build_id, install) {
        tracing::info!(
            snapshot_build_id = %snap.meta.build_id,
            install_build = %install.build_key(),
            "extract snapshot stale (SC patched); falling back to live parse"
        );
        return None;
    }
    match snap.hydrate(&AssetConfig::standard()) {
        Ok(pair) => {
            write_key(dir, EXTRACT_SNAPSHOT_NAME, &extract_key(install));
            Some(pair)
        }
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
        write_key(dir, EXTRACT_SNAPSHOT_NAME, &extract_key(install));
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        tracing::info!(bytes = size, "wrote extract snapshot to {}", path.display());
    }
}
