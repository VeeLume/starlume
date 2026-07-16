//! Weapon enhancer — size/tracking prefixes on names, combat-stat blocks on
//! descriptions, pool-first so weapon families sharing one loc key collapse
//! to a single patch.
//!
//! Port of sc-langpatch's `weapon_enhancer.rs` decisions (incl. its two
//! collision refinements: size-matching by `_S{n}` loc-key suffix, and
//! honest range rendering when matched stats still diverge). The entity
//! materialization moved into svc-data's weapons cook; pooling by loc key
//! happens here — cheap plain code over the flat entries.

use std::collections::BTreeMap;

use svc_data::{CookedData, MissileEntry, ShipWeaponEntry};

use crate::format::{NEWLINE, PARAGRAPH_BREAK, header};
use crate::ops::{OpSet, OptionKind, PatchOp, PatcherConfig, PatcherOption};

pub struct WeaponEnhancer;

impl crate::Patcher for WeaponEnhancer {
    fn id(&self) -> &'static str {
        "weapon_enhancer"
    }

    fn name(&self) -> &'static str {
        "Weapon Enhancer"
    }

    fn description(&self) -> &'static str {
        "Add size prefixes, missile tracking type, and combat stats to weapon descriptions"
    }

    fn options(&self) -> Vec<PatcherOption> {
        let toggle = |id: &str, label: &str, description: &str| PatcherOption {
            id: id.into(),
            label: label.into(),
            description: description.into(),
            kind: OptionKind::Bool,
            default: "true".into(),
        };
        vec![
            toggle(
                "size_prefix",
                "Size prefix",
                "Add weapon size prefix to names (e.g. S3 Attrition)",
            ),
            toggle(
                "missile_type_prefix",
                "Missile/torpedo type prefix",
                "Add tracking type prefix to missile names (e.g. [IR] Ignite)",
            ),
            toggle(
                "weapon_stats",
                "Weapon stats",
                "Append damage, penetration, speed, and ammo stats to weapon descriptions",
            ),
            toggle(
                "missile_stats",
                "Missile/torpedo stats",
                "Append damage, speed, arm time, and lock stats to missile descriptions",
            ),
        ]
    }

    fn derive(&self, cooked: &CookedData, config: &PatcherConfig) -> anyhow::Result<OpSet> {
        let opt_size_prefix = config.get_bool("size_prefix", true);
        let opt_missile_type = config.get_bool("missile_type_prefix", true);
        let opt_weapon_stats = config.get_bool("weapon_stats", true);
        let opt_missile_stats = config.get_bool("missile_stats", true);

        let w = &cooked.weapons;

        // ── Pool by stripped loc key ────────────────────────────────────
        let mut name_pools: BTreeMap<&str, Pool<'_>> = BTreeMap::new();
        let mut desc_pools: BTreeMap<&str, Pool<'_>> = BTreeMap::new();
        for s in &w.ship_weapons {
            if let Some(k) = stripped(&s.name_key) {
                name_pools.entry(k).or_default().ships.push(s);
            }
            if let Some(k) = stripped(&s.desc_key) {
                desc_pools.entry(k).or_default().ships.push(s);
            }
        }
        for m in &w.missiles {
            if let Some(k) = stripped(&m.name_key) {
                name_pools.entry(k).or_default().missiles.push(m);
            }
            if let Some(k) = stripped(&m.desc_key) {
                desc_pools.entry(k).or_default().missiles.push(m);
            }
        }

        let mut patches = Vec::new();

        // ── Name pool pass ──────────────────────────────────────────────
        for (key, pool) in &name_pools {
            if cooked
                .locale
                .resolve(key)
                .filter(|v| !v.is_empty())
                .is_none()
            {
                continue;
            }
            let target_size = parse_size_from_key(key);
            let ships = matched(&pool.ships, target_size, |s| s.size);
            let missiles = matched(&pool.missiles, target_size, |m| m.size);

            let prefix = if !ships.is_empty() {
                ship_name_prefix(&ships, opt_size_prefix)
            } else if !missiles.is_empty() {
                missile_name_prefix(&missiles, opt_size_prefix, opt_missile_type)
            } else {
                continue;
            };
            if !prefix.is_empty() {
                patches.push((key.to_string(), PatchOp::Prefix(prefix)));
            }
        }

        // ── Description pool pass ───────────────────────────────────────
        for (key, pool) in &desc_pools {
            if cooked
                .locale
                .resolve(key)
                .filter(|v| !v.is_empty())
                .is_none()
            {
                continue;
            }
            // CIG data quirk: some weapons have no dedicated `item_Desc*`
            // entry, so the entity's Description falls back to its Name
            // key. Patching such a key with a stats block corrupts the
            // name field — skip it.
            if name_pools.contains_key(key) {
                continue;
            }
            let target_size = parse_size_from_key(key);
            let ships = matched(&pool.ships, target_size, |s| s.size);
            let missiles = matched(&pool.missiles, target_size, |m| m.size);

            let suffix = if !ships.is_empty() && opt_weapon_stats {
                ship_stats_suffix(&ships)
            } else if !missiles.is_empty() && opt_missile_stats {
                missile_stats_suffix(&missiles)
            } else {
                String::new()
            };
            if !suffix.is_empty() {
                patches.push((key.to_string(), PatchOp::Suffix(suffix)));
            }
        }

        Ok(OpSet {
            renames: Vec::new(),
            patches,
        })
    }
}

#[derive(Default)]
struct Pool<'a> {
    ships: Vec<&'a ShipWeaponEntry>,
    missiles: Vec<&'a MissileEntry>,
}

fn stripped(key: &Option<String>) -> Option<&str> {
    key.as_deref()
        .map(|k| k.strip_prefix('@').unwrap_or(k))
        .filter(|k| !k.is_empty())
}

/// Filter pool members by size-match against the loc-key suffix; fall back
/// to all members when nothing matches (or the key carries no size).
fn matched<'a, T>(all: &[&'a T], target_size: Option<i32>, size: impl Fn(&T) -> i32) -> Vec<&'a T> {
    match target_size {
        Some(s) => {
            let m: Vec<&T> = all.iter().copied().filter(|x| size(x) == s).collect();
            if m.is_empty() { all.to_vec() } else { m }
        }
        None => all.to_vec(),
    }
}

/// Extract a size hint from a loc-key suffix: first `_S{digits}` bounded by
/// `_` or end-of-string (`_Stanton4_` must not match).
fn parse_size_from_key(key: &str) -> Option<i32> {
    let bytes = key.as_bytes();
    let mut i = 0;
    while i + 2 < bytes.len() {
        if bytes[i] == b'_' && bytes[i + 1] == b'S' && bytes[i + 2].is_ascii_digit() {
            let start = i + 2;
            let mut end = start;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            if (end == bytes.len() || bytes[end] == b'_')
                && let Ok(n) = key[start..end].parse::<i32>()
            {
                return Some(n);
            }
        }
        i += 1;
    }
    None
}

// ── Prefix builders ─────────────────────────────────────────────────────────

fn ship_name_prefix(ships: &[&ShipWeaponEntry], with_size: bool) -> String {
    if !with_size {
        return String::new();
    }
    let sizes: Vec<i32> = ships.iter().map(|w| w.size).filter(|s| *s > 0).collect();
    match (sizes.iter().copied().min(), sizes.iter().copied().max()) {
        (Some(lo), Some(hi)) if lo == hi => format!("S{lo} "),
        (Some(lo), Some(hi)) => format!("S{lo}-S{hi} "),
        _ => String::new(),
    }
}

fn missile_name_prefix(missiles: &[&MissileEntry], with_size: bool, with_tracking: bool) -> String {
    let mut parts: Vec<String> = Vec::new();
    if with_size {
        let sizes: Vec<i32> = missiles.iter().map(|m| m.size).filter(|s| *s > 0).collect();
        if let (Some(lo), Some(hi)) = (sizes.iter().copied().min(), sizes.iter().copied().max()) {
            if lo == hi {
                parts.push(format!("S{lo}"));
            } else {
                parts.push(format!("S{lo}-S{hi}"));
            }
        }
    }
    if with_tracking {
        let tags: std::collections::BTreeSet<&str> = missiles
            .iter()
            .filter_map(|m| m.tracking.as_ref().and_then(|t| tracking_tag(&t.signal)))
            .collect();
        // Multiple distinct tags → omit (honest about the disagreement).
        if tags.len() == 1 {
            parts.push(format!("[{}]", tags.into_iter().next().unwrap()));
        }
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("{} ", parts.join(" "))
    }
}

fn tracking_tag(signal: &str) -> Option<&'static str> {
    match signal {
        "Infrared" => Some("IR"),
        "Electromagnetic" => Some("EM"),
        "CrossSection" => Some("CS"),
        _ => None,
    }
}

// ── Suffix builders ─────────────────────────────────────────────────────────

fn ship_stats_suffix(ships: &[&ShipWeaponEntry]) -> String {
    let mut lines: Vec<String> = Vec::new();

    // Alpha damage: per-type breakdown when the matched set agrees, bare
    // range when it diverges (combining mismatched breakdowns would lie).
    let alphas: Vec<f32> = ships
        .iter()
        .filter_map(|w| w.damage.as_ref().map(|d| d.total()))
        .filter(|a| *a > 0.0)
        .collect();
    if let Some((lo, hi)) = min_max(&alphas) {
        if approx_eq(lo, hi, 0.5) {
            let head = ships
                .iter()
                .find_map(|w| w.damage.as_ref())
                .expect("alphas non-empty");
            lines.push(format!(
                "Alpha: {:.0} ({})",
                head.total(),
                damage_breakdown(head)
            ));
        } else {
            lines.push(format!("Alpha: {}", range(lo, hi)));
        }
    }

    if let Some((lo, hi)) = min_max_by(ships, |w| w.penetration_m, |v| v > 0.0) {
        if approx_eq(lo, hi, 0.01) {
            lines.push(format!("Penetration: {lo:.2}m"));
        } else {
            lines.push(format!("Penetration: {lo:.2}-{hi:.2}m"));
        }
    }

    if let Some((lo, hi)) = min_max_by(ships, |w| w.ammo_speed, |v| v > 0.0) {
        if approx_eq(lo, hi, 0.5) {
            lines.push(format!("Projectile Speed: {lo:.0} m/s"));
        } else {
            lines.push(format!("Projectile Speed: {} m/s", range(lo, hi)));
        }
    }

    let ammos: Vec<i32> = ships
        .iter()
        .filter_map(|w| w.total_ammo)
        .filter(|m| *m > 0)
        .collect();
    if let (Some(lo), Some(hi)) = (ammos.iter().min(), ammos.iter().max()) {
        lines.push(format!(
            "Ammo: {}",
            if lo == hi {
                format!("{lo}")
            } else {
                format!("{lo}-{hi}")
            }
        ));
    }

    if let Some((lo, hi)) = min_max_by(ships, |w| w.capacitor, |v| v > 0.0) {
        if approx_eq(lo, hi, 0.5) {
            lines.push(format!("Capacitor: {lo:.0}"));
        } else {
            lines.push(format!("Capacitor: {}", range(lo, hi)));
        }
    }

    render_block("Weapon Stats", &lines)
}

fn missile_stats_suffix(missiles: &[&MissileEntry]) -> String {
    let mut lines: Vec<String> = Vec::new();

    let totals: Vec<f32> = missiles
        .iter()
        .filter_map(|m| m.damage.as_ref().map(|d| d.total()))
        .filter(|a| *a > 0.0)
        .collect();
    if let Some((lo, hi)) = min_max(&totals) {
        if approx_eq(lo, hi, 0.5) {
            let head = missiles
                .iter()
                .find_map(|m| m.damage.as_ref())
                .expect("totals non-empty");
            lines.push(format!(
                "Damage: {:.0} ({})",
                head.total(),
                damage_breakdown(head)
            ));
        } else {
            lines.push(format!("Damage: {}", range(lo, hi)));
        }
    }

    if let Some((lo, hi)) = min_max_by(missiles, |m| m.speed, |v| v > 0.0) {
        if approx_eq(lo, hi, 0.5) {
            lines.push(format!("Speed: {lo:.0} m/s"));
        } else {
            lines.push(format!("Speed: {} m/s", range(lo, hi)));
        }
    }

    let arms: Vec<f32> = missiles
        .iter()
        .map(|m| m.arm_time)
        .filter(|t| *t > 0.0)
        .collect();
    if let Some((lo, hi)) = min_max(&arms) {
        if approx_eq(lo, hi, 0.01) {
            lines.push(format!("Arm Time: {lo:.2}s"));
        } else {
            lines.push(format!("Arm Time: {lo:.2}-{hi:.2}s"));
        }
    }

    // Tracking — only when every matched missile has a profile; partial
    // numbers would mislead.
    let trackings: Vec<_> = missiles
        .iter()
        .filter_map(|m| m.tracking.as_ref())
        .collect();
    if trackings.len() == missiles.len() && !trackings.is_empty() {
        let locks: Vec<f32> = trackings
            .iter()
            .map(|t| t.lock_time)
            .filter(|v| *v > 0.0)
            .collect();
        if let Some((lo, hi)) = min_max(&locks) {
            if approx_eq(lo, hi, 0.05) {
                lines.push(format!("Lock Time: {lo:.1}s"));
            } else {
                lines.push(format!("Lock Time: {lo:.1}-{hi:.1}s"));
            }
        }
        let angles: Vec<f32> = trackings
            .iter()
            .map(|t| t.lock_angle_deg)
            .filter(|v| *v > 0.0)
            .collect();
        if let Some((lo, hi)) = min_max(&angles) {
            if approx_eq(lo, hi, 0.5) {
                lines.push(format!("Lock Angle: {lo:.0}°"));
            } else {
                lines.push(format!("Lock Angle: {}°", range(lo, hi)));
            }
        }
        let mins: Vec<f32> = trackings.iter().map(|t| t.lock_range_min_m).collect();
        let maxes: Vec<f32> = trackings.iter().map(|t| t.lock_range_max_m).collect();
        if mins.iter().any(|v| *v > 0.0) || maxes.iter().any(|v| *v > 0.0) {
            let fmt = |vals: &[f32]| {
                let (lo, hi) = min_max(vals).unwrap();
                if approx_eq(lo, hi, 0.5) {
                    format!("{lo:.0}")
                } else {
                    range(lo, hi)
                }
            };
            lines.push(format!("Lock Range: {}m - {}m", fmt(&mins), fmt(&maxes)));
        }
    }

    let label = if missiles.iter().any(|m| m.is_torpedo) {
        "Torpedo Stats"
    } else {
        "Missile Stats"
    };
    render_block(label, &lines)
}

fn render_block(label: &str, lines: &[String]) -> String {
    if lines.is_empty() {
        return String::new();
    }
    let body: String = lines.iter().map(|l| format!("{NEWLINE}{l}")).collect();
    format!("{PARAGRAPH_BREAK}{}{body}", header(label))
}

fn damage_breakdown(d: &svc_data::DamageBreakdown) -> String {
    let mut parts = Vec::new();
    let mut push = |v: f32, label: &str| {
        if v > 0.0 {
            parts.push(format!("{v:.0} {label}"));
        }
    };
    push(d.physical, "phys");
    push(d.energy, "energy");
    push(d.distortion, "dist");
    push(d.thermal, "therm");
    push(d.biochemical, "bio");
    push(d.stun, "stun");
    parts.join(", ")
}

// ── Range helpers ───────────────────────────────────────────────────────────

fn min_max(values: &[f32]) -> Option<(f32, f32)> {
    if values.is_empty() {
        return None;
    }
    let lo = values.iter().copied().fold(f32::INFINITY, f32::min);
    let hi = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    Some((lo, hi))
}

fn min_max_by<T>(
    items: &[&T],
    extract: impl Fn(&T) -> Option<f32>,
    keep: impl Fn(f32) -> bool,
) -> Option<(f32, f32)> {
    let vs: Vec<f32> = items
        .iter()
        .filter_map(|x| extract(x))
        .filter(|v| keep(*v))
        .collect();
    min_max(&vs)
}

fn approx_eq(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol
}

fn range(lo: f32, hi: f32) -> String {
    format!("{}-{}", lo.round() as i64, hi.round() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_size_handles_known_key_shapes() {
        assert_eq!(parse_size_from_key("item_NameBEHR_LaserCannon_S7"), Some(7));
        assert_eq!(
            parse_size_from_key("item_NameBEHR_LaserCannon_VNG_S2"),
            Some(2)
        );
        assert_eq!(
            parse_size_from_key("item_NameAPAR_BallisticScatterGun_S1_Shark"),
            Some(1)
        );
        assert_eq!(
            parse_size_from_key("item_NameMISL_S02_CS_FSKI_Tempest"),
            Some(2)
        );
        assert_eq!(
            parse_size_from_key("item_NameGMISL_S05_IR_TALN_Valkyrie"),
            Some(5)
        );
        // `_Stanton4_` must not be confused for a size token.
        assert_eq!(parse_size_from_key("item_NameFoo_Stanton4_Bar"), None);
        assert_eq!(parse_size_from_key("item_NameNoSizeHere"), None);
        assert_eq!(parse_size_from_key("item_NameFoo_Special"), None);
    }

    #[test]
    fn tracking_tags_map_signal_strings() {
        assert_eq!(tracking_tag("Infrared"), Some("IR"));
        assert_eq!(tracking_tag("Electromagnetic"), Some("EM"));
        assert_eq!(tracking_tag("CrossSection"), Some("CS"));
        assert_eq!(tracking_tag("SomethingNew"), None);
    }

    #[test]
    fn approx_and_range_helpers() {
        assert!(approx_eq(1.0, 1.4, 0.5));
        assert!(!approx_eq(1.0, 2.0, 0.5));
        assert_eq!(range(2306.4, 6750.2), "2306-6750");
    }
}
