//! The declarative TOML patcher — sc-langpatch's `toml_module.rs`
//! interpreter ported onto the derive pipeline, plus the embedded
//! `label_fixes` definition it exists for.
//!
//! A definition is plain TOML: `[[patch]]` rules match locale keys —
//! exact (`key` / `keys`) or via a `key_pattern` glob where `*` matches
//! anything and `{name}` captures a run of non-`_` characters for reuse
//! in the op template — optionally gated on the current value
//! (`value_contains`) and on option values (`when = { option_id = "v" }`
//! against `[[option]]` declarations, which surface as
//! [`PatcherOption`]s). `[[rename]]` entries pass through as
//! [`KeyRename`]s.
//!
//! Changes from the sc-langpatch interpreter: the glob engine is a small
//! backtracking matcher instead of a `regex` dependency, `[[remove]]` is
//! gone (the [`OpSet`] model has no cross-patcher key suppression), and
//! invalid rules fail parsing loudly instead of being silently skipped.
//! Pattern-rule matches are sorted by key so a derive is deterministic
//! regardless of [`svc_data::LocaleMap`]'s hash order — op-sets are
//! cached and hashed on disk.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::ops::{
    ChoiceOption, KeyRename, OpSet, OptionKind, PatchOp, PatcherConfig, PatcherOption,
};

// ── TOML schema ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct TomlFile {
    module: TomlMeta,
    #[serde(default)]
    option: Vec<TomlOption>,
    #[serde(default)]
    patch: Vec<TomlPatchEntry>,
    #[serde(default)]
    rename: Vec<TomlRenameEntry>,
}

#[derive(Deserialize)]
struct TomlMeta {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default = "default_true")]
    default_enabled: bool,
    #[serde(default = "default_priority")]
    priority: u32,
}

fn default_true() -> bool {
    true
}

fn default_priority() -> u32 {
    100
}

#[derive(Deserialize)]
struct TomlOption {
    id: String,
    label: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    kind: TomlOptionKind,
    #[serde(default)]
    choices: Vec<TomlChoice>,
    /// Stringly default, matching [`PatcherConfig`]: bools are
    /// `"true"`/`"false"`, choices are the choice value.
    default: String,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "lowercase")]
enum TomlOptionKind {
    #[default]
    Bool,
    Choice,
}

#[derive(Deserialize)]
struct TomlChoice {
    value: String,
    label: String,
}

#[derive(Deserialize)]
struct TomlPatchEntry {
    /// Single exact key.
    key: Option<String>,
    /// Multiple exact keys.
    keys: Option<Vec<String>>,
    /// Glob-style pattern with optional `{name}` captures.
    key_pattern: Option<String>,

    /// Only apply if the current value contains this string.
    value_contains: Option<String>,
    /// Only apply if every listed option has this effective value.
    #[serde(default)]
    when: BTreeMap<String, String>,

    /// Patch operations (exactly one must be set).
    replace: Option<String>,
    prefix: Option<String>,
    suffix: Option<String>,
}

#[derive(Deserialize)]
struct TomlRenameEntry {
    from: String,
    to: String,
}

// ── Compiled definition ─────────────────────────────────────────────────────

enum KeyMatcher {
    /// Match specific keys exactly (leading `@` stripped at compile time).
    Exact(Vec<String>),
    /// Match keys against a compiled glob with named captures.
    Pattern(GlobPattern),
}

struct CompiledRule {
    matcher: KeyMatcher,
    value_contains: Option<String>,
    when: BTreeMap<String, String>,
    /// Template string that may contain `{capture_name}` placeholders.
    template: String,
    op_kind: OpKind,
}

#[derive(Clone, Copy)]
enum OpKind {
    Replace,
    Prefix,
    Suffix,
}

/// One parsed + compiled TOML patcher definition.
struct Definition {
    id: &'static str,
    name: String,
    description: String,
    default_enabled: bool,
    priority: u32,
    uses_replace: bool,
    options: Vec<PatcherOption>,
    /// Declared option defaults, for resolving `when` conditions.
    option_defaults: BTreeMap<String, String>,
    rules: Vec<CompiledRule>,
    renames: Vec<KeyRename>,
}

impl Definition {
    fn parse(id: &'static str, toml_str: &str) -> Result<Self> {
        let file: TomlFile =
            toml::from_str(toml_str).with_context(|| format!("invalid patcher TOML for {id}"))?;

        let mut options = Vec::new();
        let mut option_defaults = BTreeMap::new();
        for opt in file.option {
            let kind = match opt.kind {
                TomlOptionKind::Bool => OptionKind::Bool,
                TomlOptionKind::Choice => {
                    if opt.choices.is_empty() {
                        bail!("option '{}' is a choice but declares no choices", opt.id);
                    }
                    OptionKind::Choice {
                        choices: opt
                            .choices
                            .into_iter()
                            .map(|c| ChoiceOption {
                                value: c.value,
                                label: c.label,
                            })
                            .collect(),
                    }
                }
            };
            option_defaults.insert(opt.id.clone(), opt.default.clone());
            options.push(PatcherOption {
                id: opt.id,
                label: opt.label,
                description: opt.description,
                kind,
                default: opt.default,
            });
        }

        let rules: Vec<CompiledRule> = file
            .patch
            .iter()
            .map(compile_rule)
            .collect::<Result<_>>()
            .with_context(|| format!("invalid patch rule in {id}"))?;
        let uses_replace = rules.iter().any(|r| matches!(r.op_kind, OpKind::Replace));

        let renames = file
            .rename
            .into_iter()
            .map(|r| KeyRename {
                from: strip_at(&r.from).to_string(),
                to: strip_at(&r.to).to_string(),
            })
            .collect();

        Ok(Self {
            id,
            name: file.module.name,
            description: file.module.description,
            default_enabled: file.module.default_enabled,
            priority: file.module.priority,
            uses_replace,
            options,
            option_defaults,
            rules,
            renames,
        })
    }

    /// Derive the op-set against the parsed base `global.ini`.
    fn derive(&self, locale: &svc_data::LocaleMap, config: &PatcherConfig) -> OpSet {
        let mut patches = Vec::new();

        for rule in &self.rules {
            if !self.when_satisfied(&rule.when, config) {
                continue;
            }
            match &rule.matcher {
                KeyMatcher::Exact(keys) => {
                    for key in keys {
                        if let Some(value) = locale.get(key)
                            && rule.matches_value(value)
                        {
                            patches.push((key.clone(), rule.make_op(&rule.template)));
                        }
                    }
                }
                KeyMatcher::Pattern(glob) => {
                    let mut hits = Vec::new();
                    for (key, value) in locale.iter() {
                        if let Some(caps) = glob.captures(key)
                            && rule.matches_value(value)
                        {
                            let resolved = resolve_template(&rule.template, &caps);
                            hits.push((key.to_string(), rule.make_op(&resolved)));
                        }
                    }
                    hits.sort_by(|a, b| a.0.cmp(&b.0));
                    patches.append(&mut hits);
                }
            }
        }

        OpSet {
            renames: self.renames.clone(),
            patches,
        }
    }

    /// True when every `when` entry matches the option's effective value
    /// (config override, else the declared default).
    fn when_satisfied(&self, when: &BTreeMap<String, String>, config: &PatcherConfig) -> bool {
        when.iter().all(|(id, required)| {
            let default = self
                .option_defaults
                .get(id)
                .map(String::as_str)
                .unwrap_or("");
            config.get_str(id, default) == required
        })
    }
}

impl CompiledRule {
    fn matches_value(&self, value: &str) -> bool {
        match &self.value_contains {
            Some(needle) => value.contains(needle.as_str()),
            None => true,
        }
    }

    fn make_op(&self, resolved_template: &str) -> PatchOp {
        match self.op_kind {
            OpKind::Replace => PatchOp::Replace(resolved_template.to_string()),
            OpKind::Prefix => PatchOp::Prefix(resolved_template.to_string()),
            OpKind::Suffix => PatchOp::Suffix(resolved_template.to_string()),
        }
    }
}

// ── Rule compilation ────────────────────────────────────────────────────────

fn compile_rule(entry: &TomlPatchEntry) -> Result<CompiledRule> {
    let (template, op_kind) = resolve_op(entry)?;
    let matcher = resolve_matcher(entry)?;

    Ok(CompiledRule {
        matcher,
        value_contains: entry.value_contains.clone(),
        when: entry.when.clone(),
        template,
        op_kind,
    })
}

fn resolve_op(entry: &TomlPatchEntry) -> Result<(String, OpKind)> {
    let ops: Vec<_> = [
        entry.replace.as_ref().map(|v| (v.clone(), OpKind::Replace)),
        entry.prefix.as_ref().map(|v| (v.clone(), OpKind::Prefix)),
        entry.suffix.as_ref().map(|v| (v.clone(), OpKind::Suffix)),
    ]
    .into_iter()
    .flatten()
    .collect();

    match ops.len() {
        0 => bail!("patch entry must have one of replace, prefix, or suffix"),
        1 => Ok(ops.into_iter().next().unwrap()),
        _ => bail!("patch entry must have only one of replace, prefix, or suffix"),
    }
}

fn resolve_matcher(entry: &TomlPatchEntry) -> Result<KeyMatcher> {
    let exact =
        |keys: &[String]| KeyMatcher::Exact(keys.iter().map(|k| strip_at(k).to_string()).collect());

    match (&entry.key, &entry.keys, &entry.key_pattern) {
        (Some(k), None, None) => Ok(exact(std::slice::from_ref(k))),
        (None, Some(ks), None) => Ok(exact(ks)),
        (None, None, Some(p)) => Ok(KeyMatcher::Pattern(GlobPattern::compile(p)?)),
        (None, None, None) => bail!("patch entry must have key, keys, or key_pattern"),
        _ => bail!("patch entry must have only one of key, keys, or key_pattern"),
    }
}

/// Strip the `@` locale-reference prefix — [`OpSet`] keys are the bare form.
fn strip_at(key: &str) -> &str {
    key.strip_prefix('@').unwrap_or(key)
}

// ── Glob patterns ───────────────────────────────────────────────────────────

/// A compiled `key_pattern` like `item_Name*_S{size}_*`: `*` matches any
/// run of characters, `{name}` captures a run of non-`_` characters, and
/// everything else matches literally. Anchored at both ends.
struct GlobPattern {
    tokens: Vec<Token>,
}

enum Token {
    Literal(String),
    Star,
    Capture(String),
}

impl GlobPattern {
    fn compile(pattern: &str) -> Result<Self> {
        let mut tokens = Vec::new();
        let mut literal = String::new();
        let mut chars = pattern.chars();

        let flush = |literal: &mut String, tokens: &mut Vec<Token>| {
            if !literal.is_empty() {
                tokens.push(Token::Literal(std::mem::take(literal)));
            }
        };

        while let Some(ch) = chars.next() {
            match ch {
                '*' => {
                    flush(&mut literal, &mut tokens);
                    tokens.push(Token::Star);
                }
                '{' => {
                    flush(&mut literal, &mut tokens);
                    let mut name = String::new();
                    let mut closed = false;
                    for c in chars.by_ref() {
                        if c == '}' {
                            closed = true;
                            break;
                        }
                        name.push(c);
                    }
                    if !closed {
                        bail!("Unclosed capture in pattern: {pattern}");
                    }
                    if name.is_empty() {
                        bail!("Empty capture name in pattern: {pattern}");
                    }
                    tokens.push(Token::Capture(name));
                }
                _ => literal.push(ch),
            }
        }
        flush(&mut literal, &mut tokens);

        Ok(Self { tokens })
    }

    /// Match a key against the pattern; on success, return the captured
    /// `{name}` values.
    fn captures(&self, key: &str) -> Option<BTreeMap<String, String>> {
        let mut caps = BTreeMap::new();
        match_tokens(&self.tokens, key, &mut caps).then_some(caps)
    }
}

/// Greedy backtracking matcher — same language and capture behaviour as
/// the old interpreter's `^…$` regex with `.*` for `*` and `[^_]*` for
/// `{name}`.
fn match_tokens(tokens: &[Token], text: &str, caps: &mut BTreeMap<String, String>) -> bool {
    let Some((first, rest)) = tokens.split_first() else {
        return text.is_empty();
    };
    match first {
        Token::Literal(lit) => text
            .strip_prefix(lit.as_str())
            .is_some_and(|tail| match_tokens(rest, tail, caps)),
        Token::Star => (0..=text.len())
            .rev()
            .filter(|&i| text.is_char_boundary(i))
            .any(|i| match_tokens(rest, &text[i..], caps)),
        Token::Capture(name) => {
            let run_end = text.find('_').unwrap_or(text.len());
            let prior = caps.get(name.as_str()).cloned();
            for end in (0..=run_end).rev() {
                if !text.is_char_boundary(end) {
                    continue;
                }
                caps.insert(name.clone(), text[..end].to_string());
                if match_tokens(rest, &text[end..], caps) {
                    return true;
                }
            }
            if let Some(v) = prior {
                caps.insert(name.clone(), v);
            } else {
                caps.remove(name.as_str());
            }
            false
        }
    }
}

/// Substitute `{name}` placeholders in a template with captured values.
fn resolve_template(template: &str, caps: &BTreeMap<String, String>) -> String {
    let mut result = template.to_string();
    for (name, value) in caps {
        result = result.replace(&format!("{{{name}}}"), value);
    }
    result
}

// ── The Patcher wrapper ─────────────────────────────────────────────────────

/// A [`crate::Patcher`] over an embedded, statically-parsed [`Definition`].
struct TomlPatcher {
    def: &'static Definition,
}

impl crate::Patcher for TomlPatcher {
    fn id(&self) -> &'static str {
        self.def.id
    }

    fn name(&self) -> &'static str {
        &self.def.name
    }

    fn description(&self) -> &'static str {
        &self.def.description
    }

    fn default_enabled(&self) -> bool {
        self.def.default_enabled
    }

    fn uses_replace_ops(&self) -> bool {
        self.def.uses_replace
    }

    fn priority(&self) -> u32 {
        self.def.priority
    }

    fn options(&self) -> Vec<PatcherOption> {
        self.def.options.clone()
    }

    fn derive(
        &self,
        cooked: &svc_data::CookedData,
        config: &PatcherConfig,
        _owned: Option<&crate::OwnedSet>,
    ) -> anyhow::Result<OpSet> {
        Ok(self.def.derive(&cooked.locale, config))
    }
}

/// The curated label-fixes patcher (embedded TOML, parsed once).
pub(crate) fn label_fixes() -> Box<dyn crate::Patcher> {
    static DEF: OnceLock<Definition> = OnceLock::new();
    let def = DEF.get_or_init(|| {
        Definition::parse("label_fixes", include_str!("patchers/label_fixes.toml"))
            .expect("embedded label_fixes.toml is valid")
    });
    Box::new(TomlPatcher { def })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(toml: &str) -> Definition {
        Definition::parse("test", toml).expect("test definition parses")
    }

    fn locale_with(entries: &[(&str, &str)]) -> svc_data::LocaleMap {
        let mut locale = svc_data::LocaleMap::new();
        for (k, v) in entries {
            locale.set(*k, *v);
        }
        locale
    }

    #[test]
    fn exact_key_replace() {
        let def = parse(
            r#"
            [module]
            name = "Test"
            [[patch]]
            key = "item_Name"
            replace = "New Name"
        "#,
        );
        let locale = locale_with(&[("item_Name", "Old Name"), ("other", "Untouched")]);

        let ops = def.derive(&locale, &PatcherConfig::default());
        assert_eq!(
            ops.patches,
            vec![(
                "item_Name".to_string(),
                PatchOp::Replace("New Name".to_string())
            )]
        );
    }

    #[test]
    fn multiple_keys_prefix() {
        let def = parse(
            r#"
            [module]
            name = "Test"
            [[patch]]
            keys = ["a", "b", "c"]
            prefix = "[!] "
        "#,
        );
        let locale = locale_with(&[
            ("a", "Alpha"),
            ("b", "Beta"),
            ("c", "Charlie"),
            ("d", "Delta"),
        ]);

        let ops = def.derive(&locale, &PatcherConfig::default());
        assert_eq!(ops.patches.len(), 3);
        for (key, op) in &ops.patches {
            assert_ne!(key, "d");
            assert_eq!(*op, PatchOp::Prefix("[!] ".to_string()));
        }
    }

    #[test]
    fn pattern_matching_sorted_by_key() {
        let def = parse(
            r#"
            [module]
            name = "Test"
            [[patch]]
            key_pattern = "item_Name*_SCItem"
            prefix = "[W] "
        "#,
        );
        let locale = locale_with(&[
            ("item_NameWEAP_Laser_SCItem", "Laser"),
            ("item_NameCOOL_Fan_SCItem", "Fan"),
            ("item_NameOther", "Other"),
            ("unrelated_key", "Nope"),
        ]);

        let ops = def.derive(&locale, &PatcherConfig::default());
        let keys: Vec<&str> = ops.patches.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(
            keys,
            vec!["item_NameCOOL_Fan_SCItem", "item_NameWEAP_Laser_SCItem"],
            "matches must be present and sorted for deterministic op-sets",
        );
    }

    #[test]
    fn pattern_with_captures() {
        let def = parse(
            r#"
            [module]
            name = "Test"
            [[patch]]
            key_pattern = "item_Name*_S{size}_*"
            suffix = " [S{size}]"
        "#,
        );
        let locale = locale_with(&[
            ("item_NameCOOL_AEGS_S01_Bracer", "Bracer"),
            ("item_NamePOWR_AMRS_S03_Turbo", "Turbo"),
            ("item_NameNoSize", "Plain"),
        ]);

        let ops = def.derive(&locale, &PatcherConfig::default());
        assert_eq!(ops.patches.len(), 2);
        assert!(ops.patches.contains(&(
            "item_NameCOOL_AEGS_S01_Bracer".to_string(),
            PatchOp::Suffix(" [S01]".to_string())
        )));
        assert!(ops.patches.contains(&(
            "item_NamePOWR_AMRS_S03_Turbo".to_string(),
            PatchOp::Suffix(" [S03]".to_string())
        )));
    }

    #[test]
    fn value_contains_condition() {
        let def = parse(
            r#"
            [module]
            name = "Test"
            [[patch]]
            key_pattern = "item_Desc*"
            value_contains = "Grade: A"
            prefix = "[A] "
        "#,
        );
        let locale = locale_with(&[
            ("item_DescWeapon1", "Type: Weapon\\nGrade: A\\nSize: 2"),
            ("item_DescWeapon2", "Type: Weapon\\nGrade: C\\nSize: 1"),
        ]);

        let ops = def.derive(&locale, &PatcherConfig::default());
        assert_eq!(
            ops.patches,
            vec![(
                "item_DescWeapon1".to_string(),
                PatchOp::Prefix("[A] ".to_string())
            )]
        );
    }

    #[test]
    fn missing_key_produces_no_patch() {
        let def = parse(
            r#"
            [module]
            name = "Test"
            [[patch]]
            key = "nonexistent"
            replace = "Nope"
        "#,
        );
        let locale = locale_with(&[("other_key", "Value")]);

        let ops = def.derive(&locale, &PatcherConfig::default());
        assert!(ops.patches.is_empty());
    }

    #[test]
    fn at_prefix_stripped_from_exact_keys() {
        let def = parse(
            r#"
            [module]
            name = "Test"
            [[patch]]
            key = "@item_Name"
            replace = "New"
        "#,
        );
        let locale = locale_with(&[("item_Name", "Old")]);

        let ops = def.derive(&locale, &PatcherConfig::default());
        assert_eq!(
            ops.patches,
            vec![("item_Name".to_string(), PatchOp::Replace("New".to_string()))]
        );
    }

    #[test]
    fn renames_pass_through() {
        let def = parse(
            r#"
            [module]
            name = "Test"
            [[rename]]
            from = "typo_key"
            to = "correct_key"
        "#,
        );
        let locale = locale_with(&[("old_key", "Val")]);

        let ops = def.derive(&locale, &PatcherConfig::default());
        assert_eq!(
            ops.renames,
            vec![KeyRename {
                from: "typo_key".to_string(),
                to: "correct_key".to_string()
            }]
        );
    }

    #[test]
    fn default_enabled_parsing() {
        let implicit = parse("[module]\nname = \"Test\"\n");
        assert!(implicit.default_enabled);

        let explicit = parse("[module]\nname = \"Test\"\ndefault_enabled = false\n");
        assert!(!explicit.default_enabled);
    }

    #[test]
    fn when_condition_uses_declared_default() {
        let def = parse(
            r#"
            [module]
            name = "Test"
            [[option]]
            id = "style"
            label = "Style"
            kind = "choice"
            choices = [
                { value = "fancy", label = "Fancy" },
                { value = "plain", label = "Plain" },
            ]
            default = "fancy"
            [[patch]]
            key = "a"
            prefix = "[F] "
            when = { style = "fancy" }
        "#,
        );
        assert_eq!(def.options.len(), 1);
        let locale = locale_with(&[("a", "Alpha")]);

        let ops = def.derive(&locale, &PatcherConfig::default());
        assert_eq!(ops.patches.len(), 1, "default option value satisfies when");
    }

    #[test]
    fn when_condition_respects_config_override() {
        let def = parse(
            r#"
            [module]
            name = "Test"
            [[option]]
            id = "markers"
            label = "Markers"
            kind = "bool"
            default = "true"
            [[patch]]
            key = "a"
            prefix = "[!] "
            when = { markers = "true" }
        "#,
        );
        let locale = locale_with(&[("a", "Alpha")]);

        let config = PatcherConfig {
            enabled: None,
            options: [("markers".to_string(), "false".to_string())].into(),
        };
        let ops = def.derive(&locale, &config);
        assert!(
            ops.patches.is_empty(),
            "overridden option gates the rule off"
        );
    }

    #[test]
    fn invalid_rules_fail_parse() {
        // No op.
        assert!(
            Definition::parse("t", "[module]\nname = \"T\"\n[[patch]]\nkey = \"a\"\n").is_err()
        );
        // Two ops.
        assert!(
            Definition::parse(
                "t",
                "[module]\nname = \"T\"\n[[patch]]\nkey = \"a\"\nreplace = \"x\"\nprefix = \"y\"\n",
            )
            .is_err()
        );
        // No matcher.
        assert!(
            Definition::parse("t", "[module]\nname = \"T\"\n[[patch]]\nreplace = \"x\"\n").is_err()
        );
        // Empty capture name.
        assert!(
            Definition::parse(
                "t",
                "[module]\nname = \"T\"\n[[patch]]\nkey_pattern = \"a{}b\"\nreplace = \"x\"\n",
            )
            .is_err()
        );
    }

    #[test]
    fn embedded_label_fixes_derives_expected_ops() {
        let patcher = label_fixes();
        assert_eq!(patcher.id(), "label_fixes");
        assert_eq!(patcher.name(), "Label Fixes");
        assert_eq!(
            patcher.description(),
            "Shorten labels that cause text overlap in the game UI"
        );
        assert!(patcher.default_enabled());
        assert!(patcher.uses_replace_ops());
        assert_eq!(patcher.priority(), 100);

        let mut cooked = svc_data::CookedData::default();
        cooked
            .locale
            .set("hud_mining_scanning_instability", "Instability:");
        cooked
            .locale
            .set("items_commodities_hephaestanite", "Hephaestanite");
        cooked.locale.set("unrelated", "Untouched");

        let ops = patcher
            .derive(&cooked, &PatcherConfig::default(), None)
            .expect("derive succeeds");
        assert_eq!(ops.patches.len(), 2, "only present keys are patched");
        assert!(ops.patches.contains(&(
            "hud_mining_scanning_instability".to_string(),
            PatchOp::Replace("Instab.:".to_string())
        )));
        assert!(ops.patches.contains(&(
            "items_commodities_hephaestanite".to_string(),
            PatchOp::Replace("Heph".to_string())
        )));
    }
}
