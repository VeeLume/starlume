//! App plumbing shared by the shell and (eventually) the service crates:
//! data-dir resolution, atomic file writes, and JSON load/save helpers.
//!
//! Deliberately free of Tauri, tokio, and domain types — anything here must be
//! usable from a plain test or a future CLI.

use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;

/// Dev profile for multi-instance testing (two Discord accounts side by
/// side). Honored in **debug builds only** — release users can't footgun
/// themselves into a shadow profile. A profile gets its own data dir, its
/// own keyring slot, no single-instance lock, and a loopback (instead of
/// deep-link) auth callback — see CONTRIBUTING.md, "Testing with two
/// accounts".
pub fn profile() -> Option<String> {
    if !cfg!(debug_assertions) {
        return None;
    }
    std::env::var("STARLUME_PROFILE")
        .ok()
        .filter(|p| !p.is_empty())
}

/// Root directory for all persisted app data.
///
/// Debug and release builds are namespaced apart (the Hearth pattern) so a dev
/// schema experiment never touches real data:
/// - debug   → `%APPDATA%\starlume-dev\` (or `starlume-dev-<profile>`)
/// - release → `%APPDATA%\starlume\`
/// - `STARLUME_DATA_DIR` overrides both.
pub fn app_data_root() -> PathBuf {
    if let Ok(dir) = std::env::var("STARLUME_DATA_DIR") {
        return PathBuf::from(dir);
    }
    let base = dirs::config_dir().expect("no platform config dir");
    match profile() {
        Some(p) => base.join(format!("starlume-dev-{p}")),
        None if cfg!(debug_assertions) => base.join("starlume-dev"),
        None => base.join("starlume"),
    }
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

/// [`load_json`] for what the user made (saved loadouts): an unparseable
/// file is moved aside to `<name>.bad-<unix seconds>` before the defaults
/// come back, so the next save cannot overwrite it and it stays at hand to
/// recover.
pub fn load_json_or_set_aside<T: DeserializeOwned + Default>(path: &Path) -> T {
    let Ok(bytes) = std::fs::read(path) else {
        return T::default();
    };
    serde_json::from_slice(&bytes).unwrap_or_else(|e| {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let mut aside = path.as_os_str().to_owned();
        aside.push(format!(".bad-{secs}"));
        match std::fs::rename(path, &aside) {
            Ok(()) => tracing::warn!(
                "failed to parse {}: {e}; moved aside to {}",
                path.display(),
                Path::new(&aside).display()
            ),
            Err(r) => tracing::warn!(
                "failed to parse {}: {e}; could not move it aside: {r}",
                path.display()
            ),
        }
        T::default()
    })
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
    fn unparseable_user_data_is_moved_aside() {
        let dir = std::env::temp_dir().join("starlume-app-kit-aside");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("loadouts.json");
        atomic_write(&path, b"{ not json").unwrap();
        let v: Vec<String> = load_json_or_set_aside(&path);
        assert!(v.is_empty());
        assert!(!path.exists());
        let aside: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
        assert_eq!(aside.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_json_defaults_on_missing() {
        let v: Vec<String> = load_json(Path::new("Z:/does/not/exist.json"));
        assert!(v.is_empty());
    }
}
