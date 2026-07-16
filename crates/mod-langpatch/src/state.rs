//! Patch state — the fingerprint that turns "patched" from an action into a
//! maintained state (the scoping's core reframe).
//!
//! Per selected install we persist what we last wrote: the build's
//! staleness key, the config hash, the language-pack hash, and the sha256
//! of the file we wrote. Any mismatch between fingerprint and reality is a
//! reconciliation plan:
//!
//! - fingerprint matches + our file on disk → [`PatchPlan::UpToDate`]
//! - anything stale / missing → [`PatchPlan::Apply`]
//! - a file we didn't write → [`PatchPlan::Foreign`] — **warn + pause**
//!   (2026-07-04 coexistence decision: don't fight the SC Deutsch Launcher;
//!   the user gets a "take over" action instead)
//!
//! sc-langpatch had none of this — no record of what it last patched.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::ops::{LangpatchConfig, stable_hash};

/// What the override on disk *should* correspond to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fingerprint {
    /// The install's build staleness key (`InstallInfo::staleness_key`).
    pub staleness_key: String,
    /// Hash over the patcher enable/options map — the part of the config
    /// that changes patch output.
    pub config_hash: String,
    /// Content hash of the language pack in use, `None` when none.
    pub pack_hash: Option<String>,
}

impl Fingerprint {
    pub fn new(staleness_key: &str, config: &LangpatchConfig, pack_hash: Option<String>) -> Self {
        Self {
            staleness_key: staleness_key.to_string(),
            config_hash: stable_hash(&config.patchers),
            pack_hash,
        }
    }
}

/// What we last applied to one install.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallPatchState {
    pub staleness_key: String,
    pub config_hash: String,
    pub pack_hash: Option<String>,
    /// sha256 of the `global.ini` override we wrote — the foreign-writer
    /// detector.
    pub output_sha256: String,
    /// RFC 3339 timestamp of the write.
    pub patched_at: String,
}

impl InstallPatchState {
    fn matches(&self, desired: &Fingerprint) -> bool {
        self.staleness_key == desired.staleness_key
            && self.config_hash == desired.config_hash
            && self.pack_hash == desired.pack_hash
    }
}

/// The durable state file (`langpatch/state.json`): channel key → applied
/// state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PatchStateFile {
    #[serde(default)]
    pub installs: BTreeMap<String, InstallPatchState>,
}

impl PatchStateFile {
    pub fn path(data_dir: &Path) -> PathBuf {
        data_dir.join("state.json")
    }

    pub fn load(data_dir: &Path) -> Self {
        app_kit::load_json(&Self::path(data_dir))
    }

    pub fn save(&self, data_dir: &Path) -> std::io::Result<()> {
        app_kit::save_json(&Self::path(data_dir), self)
    }
}

/// The reconciliation decision for one install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchPlan {
    /// Fingerprint matches and the file on disk is the one we wrote.
    UpToDate,
    /// Needs (re-)applying: never patched, build/config/pack changed, or
    /// our file vanished (launcher verify).
    Apply,
    /// The override on disk isn't ours — someone else wrote it. Auto
    /// pauses for this install; the UI offers "take over".
    Foreign,
}

/// Decide what one install needs.
///
/// `disk_sha` is the sha256 of the override currently on disk (`None`
/// when no override file exists).
pub fn plan_for(
    state: Option<&InstallPatchState>,
    desired: &Fingerprint,
    disk_sha: Option<&str>,
) -> PatchPlan {
    match (state, disk_sha) {
        // Never patched, no file → fresh apply.
        (None, None) => PatchPlan::Apply,
        // Never patched but a file exists → someone else's (SC Deutsch
        // Launcher, manual edit, a standalone langpatch run).
        (None, Some(_)) => PatchPlan::Foreign,
        // We patched but the file is gone (launcher verify wiped it).
        (Some(_), None) => PatchPlan::Apply,
        (Some(s), Some(disk)) => {
            if disk != s.output_sha256 {
                PatchPlan::Foreign
            } else if s.matches(desired) {
                PatchPlan::UpToDate
            } else {
                PatchPlan::Apply
            }
        }
    }
}

/// sha256 of a file, `None` when it doesn't exist / can't be read.
pub fn sha256_file(path: &Path) -> Option<String> {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(path).ok()?;
    Some(hex::encode(Sha256::digest(&bytes)))
}

/// sha256 of a byte buffer.
pub fn sha256_bytes(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fp(build: &str) -> Fingerprint {
        Fingerprint {
            staleness_key: build.into(),
            config_hash: "cfg1".into(),
            pack_hash: None,
        }
    }

    fn applied(build: &str, sha: &str) -> InstallPatchState {
        InstallPatchState {
            staleness_key: build.into(),
            config_hash: "cfg1".into(),
            pack_hash: None,
            output_sha256: sha.into(),
            patched_at: "2026-07-04T00:00:00Z".into(),
        }
    }

    #[test]
    fn fresh_install_applies() {
        assert_eq!(plan_for(None, &fp("b1"), None), PatchPlan::Apply);
    }

    #[test]
    fn unknown_existing_file_is_foreign() {
        assert_eq!(plan_for(None, &fp("b1"), Some("abc")), PatchPlan::Foreign);
    }

    #[test]
    fn matching_fingerprint_and_our_file_is_up_to_date() {
        let s = applied("b1", "sha");
        assert_eq!(
            plan_for(Some(&s), &fp("b1"), Some("sha")),
            PatchPlan::UpToDate
        );
    }

    #[test]
    fn build_change_applies() {
        let s = applied("b1", "sha");
        assert_eq!(plan_for(Some(&s), &fp("b2"), Some("sha")), PatchPlan::Apply);
    }

    #[test]
    fn config_change_applies() {
        let s = applied("b1", "sha");
        let mut desired = fp("b1");
        desired.config_hash = "cfg2".into();
        assert_eq!(plan_for(Some(&s), &desired, Some("sha")), PatchPlan::Apply);
    }

    #[test]
    fn pack_change_applies() {
        let s = applied("b1", "sha");
        let mut desired = fp("b1");
        desired.pack_hash = Some("pack1".into());
        assert_eq!(plan_for(Some(&s), &desired, Some("sha")), PatchPlan::Apply);
    }

    #[test]
    fn vanished_file_reapplies() {
        let s = applied("b1", "sha");
        assert_eq!(plan_for(Some(&s), &fp("b1"), None), PatchPlan::Apply);
    }

    #[test]
    fn foreign_rewrite_pauses() {
        let s = applied("b1", "sha");
        assert_eq!(
            plan_for(Some(&s), &fp("b1"), Some("other")),
            PatchPlan::Foreign
        );
    }
}
