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

use std::collections::{BTreeMap, BTreeSet};
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
    /// The owned-blueprint salt (`crate::owned_salt`) — `None` when no
    /// enabled patcher renders ownership. When the owned set changes and a
    /// patcher depends on it, this moves, so `plan_for` returns `Apply` and
    /// the mission text re-renders through the normal write-gates.
    pub owned_salt: Option<String>,
}

impl Fingerprint {
    pub fn new(
        staleness_key: &str,
        config: &LangpatchConfig,
        pack_hash: Option<String>,
        owned_salt: Option<String>,
    ) -> Self {
        Self {
            staleness_key: staleness_key.to_string(),
            config_hash: stable_hash(&config.patchers),
            pack_hash,
            owned_salt,
        }
    }
}

/// What we last applied to one install.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallPatchState {
    pub staleness_key: String,
    pub config_hash: String,
    pub pack_hash: Option<String>,
    /// Owned-blueprint salt at write time (`#[serde(default)]` so pre-owned
    /// state files load as `None` and re-apply once ownership renders).
    #[serde(default)]
    pub owned_salt: Option<String>,
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
            && self.owned_salt == desired.owned_salt
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

    /// Every recorded output hash across **all** channels — the "is this file
    /// ours?" set for [`plan_for`]. Cross-channel so a folder rename
    /// (`hotfix`→`live`) that moves our file to a different channel key isn't
    /// mistaken for a foreign writer.
    pub fn known_outputs(&self) -> BTreeSet<String> {
        self.installs
            .values()
            .map(|s| s.output_sha256.clone())
            .collect()
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
/// `disk_sha` is the sha256 of the override currently on disk (`None` when no
/// override file exists). `known_outputs` is every `output_sha256` recorded
/// across **all** channels (see [`PatchStateFile::known_outputs`]) — the "is
/// this file ours?" set.
///
/// Foreign is decided by that set alone: a file whose hash we have never
/// written is someone else's; a file whose hash we *have* written is ours,
/// even when it landed under a different channel key (a `hotfix`→`live`
/// folder rename) or when this channel's own record is stale. "Ours but not
/// current" is a re-apply, never a pause.
pub fn plan_for(
    state: Option<&InstallPatchState>,
    desired: &Fingerprint,
    disk_sha: Option<&str>,
    known_outputs: &BTreeSet<String>,
) -> PatchPlan {
    let Some(disk) = disk_sha else {
        // No override on disk — fresh, or ours was wiped (launcher verify).
        return PatchPlan::Apply;
    };
    // A hash we've never produced → someone else wrote it (SC Deutsch
    // Launcher, a manual edit, a standalone langpatch run).
    if !known_outputs.contains(disk) {
        return PatchPlan::Foreign;
    }
    // The file is ours. Up to date only when THIS channel's record matches
    // both the on-disk hash and the desired fingerprint; anything else (stale
    // build/config/pack, or a file recorded under another channel) re-applies.
    match state {
        Some(s) if disk == s.output_sha256 && s.matches(desired) => PatchPlan::UpToDate,
        _ => PatchPlan::Apply,
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
            owned_salt: None,
        }
    }

    fn applied(build: &str, sha: &str) -> InstallPatchState {
        InstallPatchState {
            staleness_key: build.into(),
            config_hash: "cfg1".into(),
            pack_hash: None,
            owned_salt: None,
            output_sha256: sha.into(),
            patched_at: "2026-07-04T00:00:00Z".into(),
        }
    }

    /// A known-outputs set from the given hashes.
    fn known(shas: &[&str]) -> BTreeSet<String> {
        shas.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn fresh_install_applies() {
        assert_eq!(
            plan_for(None, &fp("b1"), None, &known(&[])),
            PatchPlan::Apply
        );
    }

    #[test]
    fn unknown_existing_file_is_foreign() {
        // A file on disk whose hash we've never written.
        assert_eq!(
            plan_for(None, &fp("b1"), Some("abc"), &known(&[])),
            PatchPlan::Foreign
        );
    }

    #[test]
    fn matching_fingerprint_and_our_file_is_up_to_date() {
        let s = applied("b1", "sha");
        assert_eq!(
            plan_for(Some(&s), &fp("b1"), Some("sha"), &known(&["sha"])),
            PatchPlan::UpToDate
        );
    }

    #[test]
    fn build_change_applies() {
        let s = applied("b1", "sha");
        assert_eq!(
            plan_for(Some(&s), &fp("b2"), Some("sha"), &known(&["sha"])),
            PatchPlan::Apply
        );
    }

    #[test]
    fn config_change_applies() {
        let s = applied("b1", "sha");
        let mut desired = fp("b1");
        desired.config_hash = "cfg2".into();
        assert_eq!(
            plan_for(Some(&s), &desired, Some("sha"), &known(&["sha"])),
            PatchPlan::Apply
        );
    }

    #[test]
    fn pack_change_applies() {
        let s = applied("b1", "sha");
        let mut desired = fp("b1");
        desired.pack_hash = Some("pack1".into());
        assert_eq!(
            plan_for(Some(&s), &desired, Some("sha"), &known(&["sha"])),
            PatchPlan::Apply
        );
    }

    #[test]
    fn owned_salt_change_applies() {
        // The owned-blueprint set changed under a still-current build/config →
        // re-apply so the mission text re-renders ownership.
        let s = applied("b1", "sha");
        let mut desired = fp("b1");
        desired.owned_salt = Some("owned-v2".into());
        assert_eq!(
            plan_for(Some(&s), &desired, Some("sha"), &known(&["sha"])),
            PatchPlan::Apply
        );
    }

    #[test]
    fn vanished_file_reapplies() {
        let s = applied("b1", "sha");
        assert_eq!(
            plan_for(Some(&s), &fp("b1"), None, &known(&["sha"])),
            PatchPlan::Apply
        );
    }

    #[test]
    fn foreign_rewrite_pauses() {
        // Our record says "sha", but the disk holds "other" — a hash we never
        // wrote → foreign.
        let s = applied("b1", "sha");
        assert_eq!(
            plan_for(Some(&s), &fp("b1"), Some("other"), &known(&["sha"])),
            PatchPlan::Foreign
        );
    }

    #[test]
    fn our_file_under_another_channel_reapplies_not_foreign() {
        // The `hotfix`→`live` folder-rename case: no record for THIS channel,
        // but the disk file's hash matches one we wrote under another channel
        // → ours → re-apply (re-record here), never a foreign pause.
        assert_eq!(
            plan_for(None, &fp("b1"), Some("hotfix_sha"), &known(&["hotfix_sha"])),
            PatchPlan::Apply
        );
    }

    #[test]
    fn our_file_with_stale_channel_record_reapplies_not_foreign() {
        // This channel has a stale record (old build/hash), but the disk file
        // is one we wrote (matches another channel's output) → re-apply.
        let stale = applied("old-build", "old_sha");
        assert_eq!(
            plan_for(
                Some(&stale),
                &fp("b1"),
                Some("hotfix_sha"),
                &known(&["old_sha", "hotfix_sha"]),
            ),
            PatchPlan::Apply
        );
    }
}
