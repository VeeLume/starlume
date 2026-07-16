//! Patch-operation model + patcher configuration — the vocabulary shared by
//! derive (patchers → ops), the cache (ops on disk per build), and apply
//! (ops → patched `global.ini`).
//!
//! Ported from sc-langpatch's `module.rs`, minus specta (svc-* / module
//! crates stay specta-free; the shell mirrors what crosses the IPC) and
//! minus the `ModuleContext` (patchers derive from `svc_data::CookedData`
//! now — see [`crate::Patcher`]).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

// ── Patch operations ────────────────────────────────────────────────────────

/// How a single INI key's value is modified.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum PatchOp {
    /// Completely replace the original value. Discards community-pack text
    /// for that key — patchers emitting these declare
    /// [`crate::Patcher::uses_replace_ops`] so the UI can badge them.
    Replace(String),
    /// Prepend to the current value.
    Prefix(String),
    /// Append to the current value.
    Suffix(String),
}

/// Rename an INI key (keeping its value). Applied in phase 1, before value
/// patches, so later phases see corrected keys.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyRename {
    pub from: String,
    pub to: String,
}

/// One patcher's derive result — the cacheable artifact. Deriving needs
/// [`svc_data::CookedData`]; applying an `OpSet` needs only the base INI.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OpSet {
    pub renames: Vec<KeyRename>,
    /// `(ini_key, op)` pairs. Keys are the stripped form (no `@`).
    pub patches: Vec<(String, PatchOp)>,
}

impl OpSet {
    pub fn is_empty(&self) -> bool {
        self.renames.is_empty() && self.patches.is_empty()
    }
}

// ── Patcher options ─────────────────────────────────────────────────────────

/// A configurable option a patcher exposes (rendered by the UI).
#[derive(Debug, Clone, Serialize)]
pub struct PatcherOption {
    /// Machine-readable identifier (config key).
    pub id: String,
    pub label: String,
    pub description: String,
    pub kind: OptionKind,
    /// Default value, stringly (matches sc-langpatch's model: bools are
    /// `"true"`/`"false"`, choices are the choice value).
    pub default: String,
}

#[derive(Debug, Clone, Serialize)]
pub enum OptionKind {
    Bool,
    Choice { choices: Vec<ChoiceOption> },
}

#[derive(Debug, Clone, Serialize)]
pub struct ChoiceOption {
    pub value: String,
    pub label: String,
}

/// Per-patcher configuration: enabled + chosen option values.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PatcherConfig {
    /// `None` → the patcher's `default_enabled` decides.
    pub enabled: Option<bool>,
    /// Option values by option id. Missing ids fall back to the option's
    /// declared default. BTreeMap for stable serialization (the config
    /// hash depends on it).
    #[serde(default)]
    pub options: BTreeMap<String, String>,
}

impl PatcherConfig {
    /// String value of an option, or the given default.
    pub fn get_str<'a>(&'a self, id: &str, default: &'a str) -> &'a str {
        self.options.get(id).map(String::as_str).unwrap_or(default)
    }

    /// Bool value of an option, or the given default.
    pub fn get_bool(&self, id: &str, default: bool) -> bool {
        self.options.get(id).map(|v| v == "true").unwrap_or(default)
    }
}

// ── Module configuration ────────────────────────────────────────────────────

/// The langpatch module's whole durable configuration
/// (`langpatch/config.json` under the app data root).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LangpatchConfig {
    /// The maintained-state switch: re-patch automatically on
    /// `InstallChanged` / startup staleness. Default ON — this is the
    /// product improvement over standalone langpatch.
    pub auto_patch: bool,
    /// Channel keys (lowercase) the user wants patched. Installs not
    /// listed are left untouched.
    pub channels: Vec<String>,
    /// Community language pack: a local file path or an `https://` URL
    /// (fetched by the shell behind the online gate, cached on disk so
    /// offline re-patches keep working).
    pub language_pack: Option<String>,
    /// Per-patcher enable/options, keyed by patcher id.
    pub patchers: BTreeMap<String, PatcherConfig>,
}

impl Default for LangpatchConfig {
    fn default() -> Self {
        Self {
            auto_patch: true,
            channels: vec!["live".into()],
            language_pack: None,
            patchers: BTreeMap::new(),
        }
    }
}

impl LangpatchConfig {
    /// Effective enabled state for one patcher.
    pub fn patcher_enabled(&self, patcher: &dyn crate::Patcher) -> bool {
        self.patchers
            .get(patcher.id())
            .and_then(|c| c.enabled)
            .unwrap_or_else(|| patcher.default_enabled())
    }

    /// The patcher's config (default when the user never touched it).
    pub fn patcher_config(&self, id: &str) -> PatcherConfig {
        self.patchers.get(id).cloned().unwrap_or_default()
    }
}

/// Stable hash of a serializable value — fingerprint building block.
pub(crate) fn stable_hash<T: Serialize>(value: &T) -> String {
    use sha2::{Digest, Sha256};
    let json = serde_json::to_vec(value).expect("serializable");
    hex::encode(Sha256::digest(&json))
}
