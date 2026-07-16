//! The apply stage — ported from sc-langpatch's `merge.rs`: base
//! `global.ini` lines + language-pack overlay + key renames + patch ops +
//! user overrides (always last, always win) → UTF-8+BOM bytes → the
//! loose-file override in the install dir. Line-based (`Vec<String>`) so
//! the stages compose without re-splitting, and outcomes come back as
//! return values ([`ApplyStats`], changed counts) instead of eprintln
//! logs — the shell decides what to surface.
//!
//! Two invariants every stage honors:
//!
//! - **`,P` locale-metadata suffixes.** CIG ships some INI keys with a
//!   metadata suffix (`item_Name…,P=…`) while DCB references — and every
//!   key patchers, packs, and user overrides use — are the bare form.
//!   All matching strips the suffix (everything from the first `,`);
//!   every rewrite preserves it so the output stays shape-compatible
//!   with the game's parser.
//! - **Placeholder sentinels are never patched.** The game ships
//!   `LOC_PLACEHOLDER=S1 ???0A <= PLACEHOLDER =>` and
//!   `LOC_UNINITIALIZED=<= UNINITIALIZED =>` as shared fallback targets
//!   for unresolved localization keys; many missions resolve their
//!   title/description there. Patching such a line stacks every
//!   affected mission's enrichment onto one entry, so [`apply_patches`]
//!   skips them (case-insensitively, in case CIG's capitalisation
//!   drifts).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::ops::{KeyRename, PatchOp};

// ── Decode / encode ─────────────────────────────────────────────────────────

/// Decode INI bytes into lines, auto-detecting the encoding by BOM:
/// UTF-16 LE (`FF FE`, what CIG ships in the p4k), UTF-16 BE (`FE FF`),
/// UTF-8 with BOM (`EF BB BF`), otherwise plain UTF-8 — community
/// language packs come in all of the latter three. A residual decoded
/// U+FEFF (double-BOM files exist in the wild) is stripped too.
pub fn decode_ini(bytes: &[u8]) -> Result<Vec<String>> {
    let text = if let Some(body) = bytes.strip_prefix(b"\xFF\xFE") {
        decode_with(encoding_rs::UTF_16LE, body, "UTF-16 LE")?
    } else if let Some(body) = bytes.strip_prefix(b"\xFE\xFF") {
        decode_with(encoding_rs::UTF_16BE, body, "UTF-16 BE")?
    } else if let Some(body) = bytes.strip_prefix(b"\xEF\xBB\xBF") {
        decode_with(encoding_rs::UTF_8, body, "UTF-8")?
    } else {
        decode_with(encoding_rs::UTF_8, bytes, "UTF-8")?
    };
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(&text);
    Ok(text.lines().map(str::to_owned).collect())
}

fn decode_with(
    encoding: &'static encoding_rs::Encoding,
    bytes: &[u8],
    label: &str,
) -> Result<String> {
    let (decoded, _, had_errors) = encoding.decode(bytes);
    if had_errors {
        anyhow::bail!("{label} decoding produced errors");
    }
    Ok(decoded.into_owned())
}

/// Encode lines as the game-ready override file: UTF-8 with BOM
/// (`EF BB BF`), `\n` line endings, trailing newline.
pub fn encode_utf8_bom(lines: &[String]) -> Vec<u8> {
    let mut out = Vec::with_capacity(3 + lines.iter().map(|l| l.len() + 1).sum::<usize>());
    out.extend_from_slice(b"\xEF\xBB\xBF");
    for line in lines {
        out.extend_from_slice(line.as_bytes());
        out.push(b'\n');
    }
    out
}

// ── Parse ───────────────────────────────────────────────────────────────────

/// Parse INI lines into a key → value map, keys `,P`-stripped so they
/// match the bare DCB-reference form. Without the strip, downstream
/// consumers (patcher key-existence checks against `CookedData::locale`)
/// would silently miss every suffixed entry. Lines without `=` are
/// skipped; values keep any embedded `=`.
pub fn parse_ini(lines: &[String]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in lines {
        if let Some(eq) = line.find('=') {
            let (key, _) = split_key(&line[..eq]);
            map.insert(key.to_string(), line[eq + 1..].to_string());
        }
    }
    map
}

/// Split a raw INI key into (bare key, locale-metadata marker) — the
/// marker is everything after the first `,` (e.g. the `P` in
/// `item_Name…,P`).
fn split_key(raw_key: &str) -> (&str, Option<&str>) {
    match raw_key.split_once(',') {
        Some((stem, marker)) => (stem, Some(marker)),
        None => (raw_key, None),
    }
}

// ── Pipeline stages ─────────────────────────────────────────────────────────

/// Overlay a community language pack onto the base lines.
///
/// For every `key=value` line in the pack, replaces the value of the
/// matching (`,P`-stripped) base key in place — packs are authored
/// against bare key names, and the base line keeps its raw key on
/// rewrite. Pack keys with no base match are appended at the end in
/// pack order. Base line order is preserved; pack lines without `=` are
/// ignored. Returns the number of lines replaced or appended.
pub fn apply_language_pack(lines: &mut Vec<String>, pack_lines: &[String]) -> usize {
    let mut overrides: HashMap<&str, &str> = HashMap::new();
    for line in pack_lines {
        if let Some(eq) = line.find('=') {
            overrides.insert(&line[..eq], &line[eq + 1..]);
        }
    }

    let mut seen: HashSet<&str> = HashSet::new();
    let mut changed = 0;
    for line in lines.iter_mut() {
        let Some(eq) = line.find('=') else { continue };
        let (key, _) = split_key(&line[..eq]);
        let Some((&pack_key, &value)) = overrides.get_key_value(key) else {
            continue;
        };
        let replaced = format!("{}={value}", &line[..eq]);
        *line = replaced;
        seen.insert(pack_key);
        changed += 1;
    }

    for line in pack_lines {
        let Some(eq) = line.find('=') else { continue };
        let key = &line[..eq];
        if seen.insert(key) {
            lines.push(format!("{key}={}", overrides[key]));
            changed += 1;
        }
    }
    changed
}

/// Apply key renames in place, keeping values and any `,P` marker on
/// the renamed key. Renames whose `from` key doesn't exist are skipped.
/// Returns the number of lines renamed.
pub fn apply_renames(lines: &mut [String], renames: &[KeyRename]) -> usize {
    let rename_map: HashMap<&str, &str> = renames
        .iter()
        .map(|r| (r.from.as_str(), r.to.as_str()))
        .collect();

    let mut applied = 0;
    for line in lines.iter_mut() {
        let Some(eq) = line.find('=') else { continue };
        let (key, marker) = split_key(&line[..eq]);
        let Some(&new_key) = rename_map.get(key) else {
            continue;
        };
        let renamed = match marker {
            Some(marker) => format!("{new_key},{marker}{}", &line[eq..]),
            None => format!("{new_key}{}", &line[eq..]),
        };
        *line = renamed;
        applied += 1;
    }
    applied
}

/// Outcome of one [`apply_patches`] pass.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApplyStats {
    /// Lines whose value was rewritten.
    pub patched_lines: usize,
    /// Lines left untouched because their value is a CIG placeholder
    /// sentinel.
    pub skipped_placeholders: usize,
    /// Patch keys that matched no line at all.
    pub missing_keys: usize,
}

/// Apply patch-op stacks to the lines in place.
///
/// Each (`,P`-stripped) key can carry a stack of ops, applied in order:
/// `Replace` overwrites the running value outright (wiping any prior
/// op's contribution), `Prefix` / `Suffix` compose around it — so two
/// patchers can both annotate the same key without losing each other's
/// work. Placeholder-valued lines are skipped (see the module docs);
/// the raw key — `,P` marker included — is preserved on rewrite.
pub fn apply_patches(lines: &mut [String], patches: &HashMap<String, Vec<PatchOp>>) -> ApplyStats {
    let mut stats = ApplyStats::default();
    let mut matched: HashSet<&str> = HashSet::new();

    for line in lines.iter_mut() {
        let Some(eq) = line.find('=') else { continue };
        let (key, _) = split_key(&line[..eq]);
        let Some((patch_key, ops)) = patches.get_key_value(key) else {
            continue;
        };
        matched.insert(patch_key.as_str());
        let value = &line[eq + 1..];
        if is_placeholder_value(value) {
            stats.skipped_placeholders += 1;
            continue;
        }
        let patched = format!("{}={}", &line[..eq], apply_ops(value, ops));
        *line = patched;
        stats.patched_lines += 1;
    }

    stats.missing_keys = patches.len() - matched.len();
    stats
}

/// Apply a stacked op list to an original value in order.
fn apply_ops(original: &str, ops: &[PatchOp]) -> String {
    let mut value = original.to_string();
    for op in ops {
        match op {
            PatchOp::Replace(v) => value = v.clone(),
            PatchOp::Prefix(p) => value = format!("{p}{value}"),
            PatchOp::Suffix(s) => value = format!("{value}{s}"),
        }
    }
    value
}

/// Detect CIG's unresolved-localization sentinel values (case-insensitive
/// `<= PLACEHOLDER =>` / `<= UNINITIALIZED =>` markers).
fn is_placeholder_value(value: &str) -> bool {
    let upper = value.to_ascii_uppercase();
    upper.contains("<= PLACEHOLDER =>") || upper.contains("<= UNINITIALIZED =>")
}

/// Apply the user-edit layer — run LAST so user edits always win.
///
/// For each (bare key, value): every line with that `,P`-stripped key
/// gets its value replaced outright (raw key — marker included — kept);
/// keys matching no line are appended as `key=value` at the end, in map
/// order. Placeholder sentinels are *not* exempt here: the user said
/// this exact text, they get it. Returns the number of lines touched.
pub fn apply_user_overrides(
    lines: &mut Vec<String>,
    overrides: &BTreeMap<String, String>,
) -> usize {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut touched = 0;

    for line in lines.iter_mut() {
        let Some(eq) = line.find('=') else { continue };
        let (key, _) = split_key(&line[..eq]);
        let Some((override_key, value)) = overrides.get_key_value(key) else {
            continue;
        };
        let replaced = format!("{}={value}", &line[..eq]);
        *line = replaced;
        seen.insert(override_key.as_str());
        touched += 1;
    }

    for (key, value) in overrides {
        if !seen.contains(key.as_str()) {
            lines.push(format!("{key}={value}"));
            touched += 1;
        }
    }
    touched
}

// ── Install-dir I/O ─────────────────────────────────────────────────────────

/// Where the loose-file localization override lives inside an install:
/// `{install_dir}/data/Localization/english/global.ini`. Loose files
/// shadow the p4k copy wholesale — this path is the entire mechanism.
pub fn override_path(install_dir: &Path) -> PathBuf {
    install_dir
        .join("data")
        .join("Localization")
        .join("english")
        .join("global.ini")
}

/// Write the override file (creating its directories) and upsert
/// `g_language = english` into `{install_dir}/user.cfg` — the setting
/// that makes the game load the `english` localization folder the
/// override lives in. Other user.cfg lines are preserved; the file is
/// created if absent and left untouched when already correct.
pub fn write_patch(install_dir: &Path, ini_bytes: &[u8]) -> Result<()> {
    let ini_path = override_path(install_dir);
    let dir = ini_path.parent().expect("override path has a parent");
    std::fs::create_dir_all(dir).with_context(|| format!("Failed to create {}", dir.display()))?;
    std::fs::write(&ini_path, ini_bytes)
        .with_context(|| format!("Failed to write {}", ini_path.display()))?;

    let cfg_path = install_dir.join("user.cfg");
    let existing = if cfg_path.exists() {
        std::fs::read_to_string(&cfg_path)
            .with_context(|| format!("Failed to read {}", cfg_path.display()))?
    } else {
        String::new()
    };
    let updated = upsert_cfg_key(&existing, "g_language", "english");
    if updated != existing {
        std::fs::write(&cfg_path, &updated)
            .with_context(|| format!("Failed to write {}", cfg_path.display()))?;
    }

    tracing::debug!(path = %ini_path.display(), "wrote localization override");
    Ok(())
}

/// Remove the override file and clean the `g_language` line out of
/// user.cfg (deleting user.cfg entirely if nothing else is left).
/// Returns `true` if an override existed; when it didn't, user.cfg is
/// left alone.
pub fn remove_patch(install_dir: &Path) -> Result<bool> {
    let ini_path = override_path(install_dir);
    if !ini_path.exists() {
        return Ok(false);
    }
    std::fs::remove_file(&ini_path)
        .with_context(|| format!("Failed to remove {}", ini_path.display()))?;

    let cfg_path = install_dir.join("user.cfg");
    if cfg_path.exists()
        && let Ok(content) = std::fs::read_to_string(&cfg_path)
    {
        let cleaned = remove_cfg_key(&content, "g_language");
        if cleaned.trim().is_empty() {
            let _ = std::fs::remove_file(&cfg_path);
        } else if cleaned != content {
            let _ = std::fs::write(&cfg_path, &cleaned);
        }
    }

    tracing::debug!(path = %ini_path.display(), "removed localization override");
    Ok(true)
}

/// Whether a user.cfg line assigns `key` (tolerating whitespace around
/// key and `=`). Longer keys sharing the prefix (`g_languageAudio`)
/// don't match — the char after the key must be `=` after trimming.
fn is_cfg_key_line(line: &str, key: &str) -> bool {
    match line.trim().strip_prefix(key) {
        Some(rest) => rest.trim_start().starts_with('='),
        None => false,
    }
}

/// Insert or update a `key = value` line in user.cfg-style content,
/// preserving all other lines.
fn upsert_cfg_key(content: &str, key: &str, value: &str) -> String {
    let target = format!("{key} = {value}");
    let mut found = false;
    let mut lines: Vec<String> = content
        .lines()
        .map(|l| {
            if is_cfg_key_line(l, key) {
                found = true;
                target.clone()
            } else {
                l.to_string()
            }
        })
        .collect();

    if !found {
        lines.push(target);
    }

    let mut result = lines.join("\n");
    if !result.ends_with('\n') {
        result.push('\n');
    }
    result
}

/// Remove all lines assigning `key` from user.cfg-style content.
fn remove_cfg_key(content: &str, key: &str) -> String {
    let lines: Vec<&str> = content
        .lines()
        .filter(|l| !is_cfg_key_line(l, key))
        .collect();

    if lines.is_empty() {
        return String::new();
    }
    let mut result = lines.join("\n");
    result.push('\n');
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines_of(src: &str) -> Vec<String> {
        src.lines().map(str::to_owned).collect()
    }

    /// Value of the first line whose *raw* key is `raw_key`, or None.
    fn value_of<'a>(lines: &'a [String], raw_key: &str) -> Option<&'a str> {
        let prefix = format!("{raw_key}=");
        lines.iter().find_map(|l| l.strip_prefix(prefix.as_str()))
    }

    fn patch_map(patches: &[(&str, PatchOp)]) -> HashMap<String, Vec<PatchOp>> {
        let mut map: HashMap<String, Vec<PatchOp>> = HashMap::new();
        for (key, op) in patches {
            map.entry(key.to_string()).or_default().push(op.clone());
        }
        map
    }

    fn apply(lines: &mut [String], patches: &[(&str, PatchOp)]) -> ApplyStats {
        apply_patches(lines, &patch_map(patches))
    }

    // ── decode / encode ─────────────────────────────────────────────────

    #[test]
    fn decode_utf16_le_strips_double_bom() {
        // FF FE encoding BOM, then a decoded U+FEFF, then 'A'.
        let bytes = [0xFF, 0xFE, 0xFF, 0xFE, 0x41, 0x00];
        assert_eq!(decode_ini(&bytes).unwrap(), vec!["A".to_string()]);
    }

    #[test]
    fn decode_utf16_be_with_bom() {
        let mut bytes = vec![0xFE, 0xFF];
        for ch in "key=Wert".encode_utf16() {
            bytes.extend_from_slice(&ch.to_be_bytes());
        }
        assert_eq!(decode_ini(&bytes).unwrap(), vec!["key=Wert".to_string()]);
    }

    #[test]
    fn decode_utf8_with_bom() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice("key=Wert\n".as_bytes());
        assert_eq!(decode_ini(&bytes).unwrap(), vec!["key=Wert".to_string()]);
    }

    #[test]
    fn decode_utf8_no_bom() {
        let decoded = decode_ini("a=1\nb=2\n".as_bytes()).unwrap();
        assert_eq!(decoded, vec!["a=1".to_string(), "b=2".to_string()]);
    }

    #[test]
    fn encode_prepends_bom_and_joins_with_newlines() {
        let bytes = encode_utf8_bom(&lines_of("a=1\nb=2"));
        assert_eq!(bytes, b"\xEF\xBB\xBFa=1\nb=2\n");
    }

    // ── parse_ini ───────────────────────────────────────────────────────

    #[test]
    fn parse_ini_basic() {
        let map = parse_ini(&lines_of("alpha=one\nbeta=two\n"));
        assert_eq!(map.get("alpha").unwrap(), "one");
        assert_eq!(map.get("beta").unwrap(), "two");
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn parse_ini_value_with_equals() {
        let map = parse_ini(&lines_of("formula=a=b+c\n"));
        assert_eq!(map.get("formula").unwrap(), "a=b+c");
    }

    #[test]
    fn parse_ini_empty_value() {
        let map = parse_ini(&lines_of("empty=\n"));
        assert_eq!(map.get("empty").unwrap(), "");
    }

    #[test]
    fn parse_ini_skips_non_kv_lines() {
        let map = parse_ini(&lines_of("# comment\nkey=value\n\n"));
        assert_eq!(map.len(), 1);
        assert_eq!(map.get("key").unwrap(), "value");
    }

    #[test]
    fn parse_ini_strips_locale_metadata_suffix() {
        let map = parse_ini(&lines_of("item_NameXYZ,P=Novian\n"));
        assert_eq!(map.get("item_NameXYZ").unwrap(), "Novian");
        assert!(!map.contains_key("item_NameXYZ,P"));
    }

    // ── apply_patches ───────────────────────────────────────────────────

    #[test]
    fn replace_changes_value() {
        let mut lines = lines_of("key_a=Original\nkey_b=Untouched\n");
        let stats = apply(
            &mut lines,
            &[("key_a", PatchOp::Replace("New Value".into()))],
        );
        assert_eq!(value_of(&lines, "key_a").unwrap(), "New Value");
        assert_eq!(value_of(&lines, "key_b").unwrap(), "Untouched");
        assert_eq!(stats.patched_lines, 1);
        assert_eq!(stats.missing_keys, 0);
    }

    #[test]
    fn prefix_prepends_to_value() {
        let mut lines = lines_of("drug=Altruciatoxin\n");
        apply(&mut lines, &[("drug", PatchOp::Prefix("[!] ".into()))]);
        assert_eq!(value_of(&lines, "drug").unwrap(), "[!] Altruciatoxin");
    }

    #[test]
    fn suffix_appends_to_value() {
        let mut lines = lines_of("title=Mining Contract\n");
        apply(&mut lines, &[("title", PatchOp::Suffix(" [BP]".into()))]);
        assert_eq!(value_of(&lines, "title").unwrap(), "Mining Contract [BP]");
    }

    #[test]
    fn no_data_loss_all_lines_preserved() {
        let mut lines = lines_of("key_a=Value A\nkey_b=Value B\nkey_c=Value C\nkey_d=Value D\n");
        apply(&mut lines, &[("key_b", PatchOp::Replace("Changed".into()))]);
        assert_eq!(lines.len(), 4);
        assert_eq!(value_of(&lines, "key_a").unwrap(), "Value A");
        assert_eq!(value_of(&lines, "key_b").unwrap(), "Changed");
        assert_eq!(value_of(&lines, "key_c").unwrap(), "Value C");
        assert_eq!(value_of(&lines, "key_d").unwrap(), "Value D");
    }

    #[test]
    fn unmatched_patch_counts_missing_and_does_not_corrupt() {
        let mut lines = lines_of("exists=Value\n");
        let stats = apply(
            &mut lines,
            &[("nonexistent", PatchOp::Replace("Ghost".into()))],
        );
        assert_eq!(value_of(&lines, "exists").unwrap(), "Value");
        assert!(!lines.iter().any(|l| l.starts_with("nonexistent=")));
        assert_eq!(stats.patched_lines, 0);
        assert_eq!(stats.missing_keys, 1);
    }

    #[test]
    fn values_with_equals_signs_preserved() {
        let mut lines = lines_of("formula=a=b+c=d\n");
        apply(&mut lines, &[]);
        assert_eq!(value_of(&lines, "formula").unwrap(), "a=b+c=d");
    }

    #[test]
    fn suffix_on_value_with_markup() {
        let mut lines = lines_of("desc=Contract details\\nLocation: Pyro\n");
        apply(
            &mut lines,
            &[(
                "desc",
                PatchOp::Suffix("\\n\\nBlueprints:\\n- Item A".into()),
            )],
        );
        assert_eq!(
            value_of(&lines, "desc").unwrap(),
            "Contract details\\nLocation: Pyro\\n\\nBlueprints:\\n- Item A",
        );
    }

    #[test]
    fn empty_value_replace() {
        let mut lines = lines_of("empty=\n");
        apply(
            &mut lines,
            &[("empty", PatchOp::Replace("Now has value".into()))],
        );
        assert_eq!(value_of(&lines, "empty").unwrap(), "Now has value");
    }

    #[test]
    fn empty_value_prefix_suffix() {
        let mut lines = lines_of("empty=\n");
        apply(&mut lines, &[("empty", PatchOp::Prefix("pre".into()))]);
        assert_eq!(value_of(&lines, "empty").unwrap(), "pre");

        let mut lines = lines_of("empty=\n");
        apply(&mut lines, &[("empty", PatchOp::Suffix("suf".into()))]);
        assert_eq!(value_of(&lines, "empty").unwrap(), "suf");
    }

    #[test]
    fn ops_stack_in_order() {
        // Replace then Prefix/Suffix compose; a later Replace wipes both.
        let mut lines = lines_of("key=Original\n");
        apply(
            &mut lines,
            &[
                ("key", PatchOp::Replace("Base".into())),
                ("key", PatchOp::Prefix("<".into())),
                ("key", PatchOp::Suffix(">".into())),
            ],
        );
        assert_eq!(value_of(&lines, "key").unwrap(), "<Base>");
    }

    #[test]
    fn last_replace_wins_on_key_conflict() {
        let mut lines = lines_of("key=Original\n");
        apply(
            &mut lines,
            &[
                ("key", PatchOp::Replace("First".into())),
                ("key", PatchOp::Replace("Second".into())),
            ],
        );
        assert_eq!(value_of(&lines, "key").unwrap(), "Second");
    }

    #[test]
    fn line_order_preserved() {
        let mut lines = lines_of("zebra=Z\nalpha=A\nmiddle=M\n");
        apply(
            &mut lines,
            &[("middle", PatchOp::Replace("Changed".into()))],
        );
        let keys: Vec<&str> = lines.iter().filter_map(|l| l.split('=').next()).collect();
        assert_eq!(keys, vec!["zebra", "alpha", "middle"]);
    }

    #[test]
    fn non_kv_lines_passthrough() {
        let mut lines = lines_of("# comment\nkey=value\n\n; another comment\n");
        apply(&mut lines, &[]);
        assert!(lines.contains(&"# comment".to_string()));
        assert!(lines.contains(&"; another comment".to_string()));
    }

    #[test]
    fn skips_uninitialized_placeholder_value() {
        let mut lines = lines_of("LOC_UNINITIALIZED=<= UNINITIALIZED =>\nother=Untouched\n");
        let stats = apply(
            &mut lines,
            &[(
                "LOC_UNINITIALIZED",
                PatchOp::Suffix("\\n\\n<EM4>Blueprints</EM4>...".into()),
            )],
        );
        assert_eq!(
            value_of(&lines, "LOC_UNINITIALIZED").unwrap(),
            "<= UNINITIALIZED =>",
        );
        assert_eq!(value_of(&lines, "other").unwrap(), "Untouched");
        assert_eq!(stats.skipped_placeholders, 1);
        assert_eq!(stats.patched_lines, 0);
        // Skipped ≠ missing: the key existed, we chose not to patch it.
        assert_eq!(stats.missing_keys, 0);
    }

    #[test]
    fn skips_placeholder_sentinel_value() {
        let mut lines = lines_of("LOC_PLACEHOLDER=S1 ???0A <= PLACEHOLDER =>\n");
        apply(
            &mut lines,
            &[(
                "LOC_PLACEHOLDER",
                PatchOp::Replace("Should not land".into()),
            )],
        );
        assert_eq!(
            value_of(&lines, "LOC_PLACEHOLDER").unwrap(),
            "S1 ???0A <= PLACEHOLDER =>",
        );
    }

    #[test]
    fn placeholder_skip_is_case_insensitive() {
        let mut lines = lines_of("weird_caps=<= placeholder =>\n");
        apply(
            &mut lines,
            &[("weird_caps", PatchOp::Suffix(" tail".into()))],
        );
        assert_eq!(value_of(&lines, "weird_caps").unwrap(), "<= placeholder =>");
    }

    #[test]
    fn placeholder_substring_in_normal_value_still_patches() {
        let mut lines = lines_of("note=Use as placeholder until ready\n");
        apply(&mut lines, &[("note", PatchOp::Suffix(" [done]".into()))]);
        assert_eq!(
            value_of(&lines, "note").unwrap(),
            "Use as placeholder until ready [done]",
        );
    }

    #[test]
    fn comma_suffixed_key_matches_bare_patch_key() {
        let mut lines = lines_of(
            "item_Nameutfl_crossbow_ballistic_01_tint01,P=Novian \"Nighthunter\" Crossbow\n",
        );
        apply(
            &mut lines,
            &[(
                "item_Nameutfl_crossbow_ballistic_01_tint01",
                PatchOp::Suffix(" [BP]".into()),
            )],
        );
        assert_eq!(
            lines[0],
            "item_Nameutfl_crossbow_ballistic_01_tint01,P=Novian \"Nighthunter\" Crossbow [BP]",
        );
    }

    // ── apply_renames ───────────────────────────────────────────────────

    #[test]
    fn rename_changes_key_keeps_value() {
        let mut lines = lines_of("old_key=MyValue\nother=Untouched\n");
        let n = apply_renames(
            &mut lines,
            &[KeyRename {
                from: "old_key".into(),
                to: "new_key".into(),
            }],
        );
        assert_eq!(n, 1);
        assert_eq!(value_of(&lines, "new_key").unwrap(), "MyValue");
        assert!(value_of(&lines, "old_key").is_none());
        assert_eq!(value_of(&lines, "other").unwrap(), "Untouched");
    }

    #[test]
    fn rename_missing_key_is_noop() {
        let mut lines = lines_of("existing=Value\n");
        let n = apply_renames(
            &mut lines,
            &[KeyRename {
                from: "nonexistent".into(),
                to: "new_key".into(),
            }],
        );
        assert_eq!(n, 0);
        assert_eq!(value_of(&lines, "existing").unwrap(), "Value");
        assert!(value_of(&lines, "new_key").is_none());
    }

    #[test]
    fn rename_preserves_line_count() {
        let mut lines = lines_of("a=1\nb=2\nc=3\n");
        apply_renames(
            &mut lines,
            &[KeyRename {
                from: "b".into(),
                to: "b_new".into(),
            }],
        );
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn rename_preserves_locale_metadata_suffix() {
        let mut lines = lines_of("old,P=Value\n");
        apply_renames(
            &mut lines,
            &[KeyRename {
                from: "old".into(),
                to: "new".into(),
            }],
        );
        assert_eq!(lines[0], "new,P=Value");
    }

    #[test]
    fn rename_then_patch_pipeline() {
        let mut lines = lines_of("item_NameSHLD_S01_CMP_YORM_Targa=Targa\nother=Foo\n");
        apply_renames(
            &mut lines,
            &[KeyRename {
                from: "item_NameSHLD_S01_CMP_YORM_Targa".into(),
                to: "item_NameSHLD_YORM_S01_Targa".into(),
            }],
        );
        apply(
            &mut lines,
            &[(
                "item_NameSHLD_YORM_S01_Targa",
                PatchOp::Replace("Targa Competition B".into()),
            )],
        );
        assert_eq!(
            value_of(&lines, "item_NameSHLD_YORM_S01_Targa").unwrap(),
            "Targa Competition B",
        );
        assert!(
            !lines
                .iter()
                .any(|l| l.contains("item_NameSHLD_S01_CMP_YORM_Targa"))
        );
    }

    // ── apply_language_pack ─────────────────────────────────────────────

    #[test]
    fn pack_replaces_matching_keys() {
        let mut lines = lines_of("item_NameABC=Bracer\nitem_DescABC=A cooler.\nother=untouched\n");
        let n = apply_language_pack(
            &mut lines,
            &lines_of("item_NameABC=Armreif\nitem_DescABC=Ein Kühler.\n"),
        );
        assert_eq!(n, 2);
        assert_eq!(value_of(&lines, "item_NameABC").unwrap(), "Armreif");
        assert_eq!(value_of(&lines, "item_DescABC").unwrap(), "Ein Kühler.");
        assert_eq!(value_of(&lines, "other").unwrap(), "untouched");
    }

    #[test]
    fn pack_appends_new_keys_not_in_base() {
        let mut lines = lines_of("existing=value\n");
        let n = apply_language_pack(
            &mut lines,
            &lines_of("existing=translated\nbrand_new=neuer eintrag\n"),
        );
        assert_eq!(n, 2);
        assert_eq!(value_of(&lines, "existing").unwrap(), "translated");
        assert_eq!(value_of(&lines, "brand_new").unwrap(), "neuer eintrag");
    }

    #[test]
    fn pack_preserves_base_line_order() {
        let mut lines = lines_of("zebra=Z\nalpha=A\nmiddle=M\n");
        apply_language_pack(&mut lines, &lines_of("middle=übersetzt\n"));
        let keys: Vec<&str> = lines.iter().filter_map(|l| l.split('=').next()).collect();
        assert_eq!(keys, vec!["zebra", "alpha", "middle"]);
    }

    #[test]
    fn pack_ignores_lines_without_equals() {
        let mut lines = lines_of("key=original\n");
        apply_language_pack(
            &mut lines,
            &lines_of("; comment line\n\nkey=translated\nrandom garbage line\n"),
        );
        assert_eq!(value_of(&lines, "key").unwrap(), "translated");
        assert!(!lines.iter().any(|l| l == "random garbage line"));
    }

    #[test]
    fn pack_preserves_values_with_embedded_equals() {
        let mut lines = lines_of("formula=a=b+c=d\n");
        apply_language_pack(&mut lines, &lines_of("formula=x=y+z=w\n"));
        assert_eq!(value_of(&lines, "formula").unwrap(), "x=y+z=w");
    }

    #[test]
    fn pack_matches_comma_suffixed_base_key() {
        let mut lines = lines_of("item_NameXYZ,P=Nighthunter\n");
        apply_language_pack(&mut lines, &lines_of("item_NameXYZ=Nachtjäger\n"));
        assert_eq!(lines[0], "item_NameXYZ,P=Nachtjäger");
        // Matched, not appended.
        assert_eq!(lines.len(), 1);
    }

    // ── apply_user_overrides ────────────────────────────────────────────

    fn overrides(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn user_override_replaces_existing_value() {
        let mut lines = lines_of("key_a=Patched\nkey_b=Untouched\n");
        let n = apply_user_overrides(&mut lines, &overrides(&[("key_a", "Mine")]));
        assert_eq!(n, 1);
        assert_eq!(value_of(&lines, "key_a").unwrap(), "Mine");
        assert_eq!(value_of(&lines, "key_b").unwrap(), "Untouched");
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn user_override_preserves_locale_metadata_suffix() {
        let mut lines = lines_of("item_NameXYZ,P=Novian\n");
        let n = apply_user_overrides(&mut lines, &overrides(&[("item_NameXYZ", "My Name")]));
        assert_eq!(n, 1);
        assert_eq!(lines[0], "item_NameXYZ,P=My Name");
    }

    #[test]
    fn user_override_appends_missing_key() {
        let mut lines = lines_of("existing=value\n");
        let n = apply_user_overrides(&mut lines, &overrides(&[("brand_new", "mine")]));
        assert_eq!(n, 1);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1], "brand_new=mine");
    }

    #[test]
    fn user_override_wins_over_previous_patch() {
        let mut lines = lines_of("title=Mining Contract\n");
        apply(&mut lines, &[("title", PatchOp::Suffix(" [BP]".into()))]);
        assert_eq!(value_of(&lines, "title").unwrap(), "Mining Contract [BP]");

        apply_user_overrides(&mut lines, &overrides(&[("title", "My Title")]));
        assert_eq!(value_of(&lines, "title").unwrap(), "My Title");
    }

    // ── install-dir I/O ─────────────────────────────────────────────────

    #[test]
    fn override_path_shape() {
        let p = override_path(Path::new("C:\\SC\\LIVE"));
        assert!(p.ends_with(Path::new("data/Localization/english/global.ini")));
    }

    #[test]
    fn write_patch_remove_patch_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let install = dir.path();
        std::fs::write(install.join("user.cfg"), "con_confirmQuit = 1\n").unwrap();

        let bytes = encode_utf8_bom(&lines_of("key=value"));
        write_patch(install, &bytes).unwrap();

        let ini_path = override_path(install);
        assert_eq!(std::fs::read(&ini_path).unwrap(), bytes);
        let cfg = std::fs::read_to_string(install.join("user.cfg")).unwrap();
        assert!(cfg.contains("con_confirmQuit = 1"));
        assert!(cfg.contains("g_language = english"));

        assert!(remove_patch(install).unwrap());
        assert!(!ini_path.exists());
        let cfg = std::fs::read_to_string(install.join("user.cfg")).unwrap();
        assert!(!cfg.contains("g_language"));
        assert!(cfg.contains("con_confirmQuit = 1"));

        // No override left → false, user.cfg untouched.
        assert!(!remove_patch(install).unwrap());
    }

    #[test]
    fn write_patch_updates_existing_g_language_line() {
        let dir = tempfile::tempdir().unwrap();
        let install = dir.path();
        std::fs::write(install.join("user.cfg"), "g_language = german\nother = 1\n").unwrap();

        write_patch(install, b"\xEF\xBB\xBF").unwrap();

        let cfg = std::fs::read_to_string(install.join("user.cfg")).unwrap();
        assert_eq!(cfg.matches("g_language").count(), 1);
        assert!(cfg.contains("g_language = english"));
        assert!(cfg.contains("other = 1"));
    }

    #[test]
    fn remove_patch_deletes_cfg_holding_only_g_language() {
        let dir = tempfile::tempdir().unwrap();
        let install = dir.path();

        write_patch(install, b"\xEF\xBB\xBF").unwrap();
        assert!(install.join("user.cfg").exists());

        assert!(remove_patch(install).unwrap());
        assert!(!install.join("user.cfg").exists());
    }
}
