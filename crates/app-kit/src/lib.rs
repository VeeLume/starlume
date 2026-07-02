//! App plumbing shared by the shell and (eventually) the service crates:
//! data-dir resolution, atomic file writes, and JSON load/save helpers.
//!
//! Deliberately free of Tauri, tokio, and domain types — anything here must be
//! usable from a plain test or a future CLI.

use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;

/// Root directory for all persisted app data.
///
/// Debug and release builds are namespaced apart (the Hearth pattern) so a dev
/// schema experiment never touches real data:
/// - debug   → `%APPDATA%\starlume-dev\`
/// - release → `%APPDATA%\starlume\`
/// - `STARLUME_DATA_DIR` overrides both.
pub fn app_data_root() -> PathBuf {
    if let Ok(dir) = std::env::var("STARLUME_DATA_DIR") {
        return PathBuf::from(dir);
    }
    let base = dirs::config_dir().expect("no platform config dir");
    let name = if cfg!(debug_assertions) {
        "starlume-dev"
    } else {
        "starlume"
    };
    base.join(name)
}

/// Write `bytes` to `path` atomically: write a sibling `.tmp` file, then swap
/// it into place. Creates parent directories as needed.
///
/// Windows can't rename onto an existing file, so the swap is remove+rename —
/// a tiny non-atomic window, but readers never observe a half-written file,
/// which is the property consumers (and external watchers) actually need.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    std::fs::rename(&tmp, path)
}

/// Load a JSON file into `T`, falling back to `T::default()` when the file is
/// missing or unparseable (a parse failure is logged, not fatal — settings
/// corruption must never brick the app).
pub fn load_json<T: DeserializeOwned + Default>(path: &Path) -> T {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            tracing::warn!("failed to parse {}: {e} — using defaults", path.display());
            T::default()
        }),
        Err(_) => T::default(),
    }
}

/// Serialize `value` as pretty JSON and write it atomically.
pub fn save_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    atomic_write(path, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_roundtrip_and_overwrite() {
        let dir = std::env::temp_dir().join("starlume-app-kit-test");
        let path = dir.join("file.json");
        atomic_write(&path, b"one").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"one");
        atomic_write(&path, b"two").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"two");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_json_defaults_on_missing() {
        let v: Vec<String> = load_json(Path::new("Z:/does/not/exist.json"));
        assert!(v.is_empty());
    }
}
