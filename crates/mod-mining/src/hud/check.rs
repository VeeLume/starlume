//! Cross-checks: the panel's numbers have to agree with each other, which
//! catches most digit mix-ups (0 / 6 / 8 / 9 differ by a pixel or two).
//!
//! 1. **Composition.** The rows (inert included) add up to 100 % less a
//!    rounding hair (the game shows 99.97..100.00). If they don't, swapping
//!    the weakest digits for their runner-up is tried, smallest change
//!    first; a lone unread row (usually inert) is the remainder.
//! 2. **Model.** The scanner's resistance and instability follow from the
//!    mass and the composition (the model matches the panel to two
//!    decimals), bare or with the beam on. If the read values match
//!    neither, alternative readings of mass, resistance and instability are
//!    tried the same way.
//!
//! A fix is noted; what still disagrees is flagged unsure, never guessed.

use super::digits::Number;

/// The composition's sum as the game shows it.
pub const SUM_RANGE: (f32, f32) = (99.95, 100.01);

/// Two fixes whose costs (match margin given up) are closer than this are
/// equally plausible: neither is applied.
pub const AMBIGUOUS: f32 = 0.05;

/// The most read confidence (summed digit margins) a model correction may
/// overrule: a fix only swaps digits the reader was unsure of.
pub const MAX_FIX: f32 = 0.15;

/// A row's percent field: its read number (if any).
pub struct RowPct<'a> {
    pub number: Option<&'a Number>,
}

/// Outcome of the composition check: the percent per row (fixed where
/// needed), the rows changed, whether the sum agrees.
pub struct CompositionFix {
    pub percents: Vec<Option<f32>>,
    pub changed: Vec<usize>,
    pub derived: Option<usize>,
    pub agrees: bool,
}

pub fn composition(rows: &[RowPct<'_>]) -> CompositionFix {
    let read: Vec<Option<f32>> = rows
        .iter()
        .map(|r| r.number.and_then(Number::value))
        .collect();
    let missing: Vec<usize> = read
        .iter()
        .enumerate()
        .filter(|(_, v)| v.is_none())
        .map(|(i, _)| i)
        .collect();
    let sum: f32 = read.iter().flatten().sum();
    let ok = |s: f32| (SUM_RANGE.0..=SUM_RANGE.1).contains(&s);
    if missing.is_empty() && ok(sum) {
        return CompositionFix {
            percents: read,
            changed: vec![],
            derived: None,
            agrees: true,
        };
    }
    if missing.len() == 1 {
        // The lone gap is the remainder (rounded like the panel).
        let rest = ((100.0 - sum) * 100.0).round() / 100.0;
        if rest > 0.0 {
            let mut p = read.clone();
            p[missing[0]] = Some(rest);
            return CompositionFix {
                percents: p,
                changed: vec![],
                derived: Some(missing[0]),
                agrees: false,
            };
        }
    }
    if !missing.is_empty() {
        return CompositionFix {
            percents: read,
            changed: vec![],
            derived: None,
            agrees: false,
        };
    }
    // Every row read but the sum is off: try runner-up digits, cheapest
    // total change first.
    let alts: Vec<Vec<(f32, f32)>> = rows
        .iter()
        .map(|r| {
            r.number
                .map(|n| {
                    n.alternatives(2)
                        .into_iter()
                        .filter_map(|(t, c)| {
                            t.trim_end_matches('%').parse::<f32>().ok().map(|v| (v, c))
                        })
                        .take(6)
                        .collect()
                })
                .unwrap_or_default()
        })
        .collect();
    // Every fitting combination, to tell a clear fix from a guess.
    let mut fits: Vec<(f32, Vec<f32>)> = Vec::new();
    let mut idx = vec![0usize; alts.len()];
    loop {
        let vals: Vec<f32> = idx.iter().zip(&alts).map(|(&i, a)| a[i].0).collect();
        let cost: f32 = idx.iter().zip(&alts).map(|(&i, a)| a[i].1).sum();
        if ok(vals.iter().sum()) && cost <= MAX_FIX {
            fits.push((cost, vals));
        }
        // next combination
        let mut k = 0;
        loop {
            if k == idx.len() {
                break;
            }
            idx[k] += 1;
            if idx[k] < alts[k].len() {
                break;
            }
            idx[k] = 0;
            k += 1;
        }
        if k == idx.len() {
            break;
        }
    }
    fits.sort_by(|a, b| a.0.total_cmp(&b.0));
    // A fix only when one combination is clearly the cheapest: two that
    // are nearly as likely would be a guess.
    let clear = match fits.as_slice() {
        [only] => Some(only.clone()),
        [first, second, ..] if second.0 - first.0 >= AMBIGUOUS => Some(first.clone()),
        _ => None,
    };
    match clear {
        Some((_, vals)) => {
            let changed = vals
                .iter()
                .zip(&read)
                .enumerate()
                .filter(|(_, (v, r))| Some(**v) != **r)
                .map(|(i, _)| i)
                .collect();
            CompositionFix {
                percents: vals.into_iter().map(Some).collect(),
                changed,
                derived: None,
                agrees: true,
            }
        }
        None => CompositionFix {
            percents: read,
            changed: vec![],
            derived: None,
            agrees: false,
        },
    }
}

/// Which scanner values the panel showed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Basis {
    /// The rock on its own.
    Bare,
    /// With the calculator's loadout beaming it.
    Beam,
}

/// Whether (resistance %, instability) agree with the model's values, and
/// on which basis.
pub fn agrees(read: (f32, f32), v: &crate::ScannerValues) -> Option<Basis> {
    let close = |r: f32, i: f32, pr: f32, pi: f32| (r - pr).abs() < 0.5 && (i - pi).abs() <= 0.011;
    if close(read.0, read.1, v.bare_resistance_pct, v.bare_instability) {
        return Some(Basis::Bare);
    }
    if let (Some(pr), Some(pi)) = (v.beam_resistance_pct, v.beam_instability)
        && close(read.0, read.1, pr, pi)
    {
        return Some(Basis::Beam);
    }
    None
}
