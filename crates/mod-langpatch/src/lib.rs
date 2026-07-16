//! mod-langpatch — the SC localization-enrichment engine, ported from
//! sc-langpatch onto Starlume's services (design doc: "mod-langpatch
//! subscribing to `InstallChanged` *is* the re-patch-before-SC-starts
//! feature").
//!
//! # The two-stage pipeline (the port's architecture upgrade)
//!
//! - **Derive** ([`derive`]): patchers turn [`svc_data::CookedData`] into
//!   [`ops::OpSet`]s — once per (build, patcher, options), cached on disk.
//!   This is the only stage that needs game data, and it rides svc-data's
//!   parse window: no p4k, no DataCore, no sc-holotable pin in this crate.
//! - **Apply** ([`merge`]): base `global.ini` + language-pack overlay +
//!   enabled op-sets + user overrides (always last, always win) → the
//!   loose-file override in the install dir. Sub-second, no game data —
//!   toggling a patcher never re-parses.
//!
//! # Maintained state, not an action
//!
//! [`state`] holds the per-install fingerprint (build staleness key +
//! config hash + pack hash + written-file hash) and plans reconciliation:
//! the override on disk matches the fingerprint, or it comes off (vanilla
//! beats an entire stale localization — the override shadows the p4k copy
//! wholesale). Orchestration (bus subscription, game-running gate, pack
//! fetch behind the online gate) lives in the shell; this crate stays
//! synchronous and I/O-scoped to the install dir + its own data dir.

pub mod format;
pub mod merge;
pub mod ops;
pub mod state;

mod derive;
mod patchers;
mod toml_patcher;

pub use derive::{DeriveError, PatcherOps, cache_complete, derive_ops};
pub use ops::{
    ChoiceOption, KeyRename, LangpatchConfig, OpSet, OptionKind, PatchOp, PatcherConfig,
    PatcherOption,
};
pub use patchers::builtin_patchers;
pub use state::{
    Fingerprint, InstallPatchState, PatchPlan, PatchStateFile, plan_for, sha256_bytes, sha256_file,
};

/// One enrichment patcher: derives INI patch operations from the cooked
/// game data. Pure — no I/O, no game files; everything a patcher reads is
/// in [`svc_data::CookedData`] (extend the svc-data cook when a patcher
/// needs more, never reach around it).
pub trait Patcher: Send + Sync {
    /// Stable id (config key, cache key, UI key).
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn default_enabled(&self) -> bool {
        true
    }
    /// Emits `PatchOp::Replace` — overwrites community-pack text for those
    /// keys; the UI badges this for translation-pack users.
    fn uses_replace_ops(&self) -> bool {
        false
    }
    /// Lower runs first when op-sets are combined at apply time. Key-fix
    /// style patchers use 0; enrichment defaults to 100.
    fn priority(&self) -> u32 {
        100
    }
    fn options(&self) -> Vec<ops::PatcherOption> {
        Vec::new()
    }
    /// Derive this patcher's op-set. Key-existence checks go against
    /// `cooked.locale` (parsed from the same base `global.ini` the ops are
    /// later applied to).
    fn derive(
        &self,
        cooked: &svc_data::CookedData,
        config: &ops::PatcherConfig,
    ) -> anyhow::Result<ops::OpSet>;
}
