//! Reading a number from a field band (brightness ink, reference scale).
//!
//! Two steps, kept apart because they fail differently:
//! 1. **Place.** The mean glyph templates slide over the band; a dynamic
//!    program picks the best string of the field's format (an integer,
//!    `12%`, `12.34`, `12.34%`) with each glyph starting just after the
//!    previous one's ink. This decides where the glyphs are and which are
//!    `.` and `%`.
//! 2. **Name.** Each placed digit is matched against every labelled digit
//!    cut from the training captures (nearest example, ±2 px): the HUD
//!    font tells 0, 6, 8, 9 apart by one- or two-pixel holes, which mean
//!    templates blur away. The runner-up is kept for the cross-checks.

use std::collections::HashMap;

use svc_capture::vision::Gray;

use super::data::data;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Format {
    Int,
    IntPct,
    Dec,
    DecPct,
}

impl Format {
    /// Slots: (allowed chars, min, max).
    fn slots(self) -> &'static [(&'static str, usize, usize)] {
        const D: &str = "0123456789";
        match self {
            Format::Int => &[(D, 1, 6)],
            Format::IntPct => &[(D, 1, 3), ("%", 1, 1)],
            Format::Dec => &[(D, 1, 3), (".", 1, 1), (D, 2, 2)],
            Format::DecPct => &[(D, 1, 3), (".", 1, 1), (D, 2, 2), ("%", 1, 1)],
        }
    }
}

/// One read glyph: the char, and for a digit its runner-up and how much
/// worse that matched (0: a toss-up).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glyph {
    pub ch: char,
    pub alt: Option<char>,
    pub margin: f32,
}

/// A read field: its glyphs left to right.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Number {
    pub glyphs: Vec<Glyph>,
}

impl Number {
    pub fn text(&self) -> String {
        self.glyphs.iter().map(|g| g.ch).collect()
    }

    pub fn value(&self) -> Option<f32> {
        self.text().trim_end_matches('%').parse().ok()
    }

    /// Readings with up to `depth` low-margin digits swapped for their
    /// runner-up, best first, as (text, total margin given up).
    pub fn alternatives(&self, depth: usize) -> Vec<(String, f32)> {
        let mut weak: Vec<(usize, f32)> = self
            .glyphs
            .iter()
            .enumerate()
            .filter(|(_, g)| g.alt.is_some())
            .map(|(i, g)| (i, g.margin))
            .collect();
        weak.sort_by(|a, b| a.1.total_cmp(&b.1));
        weak.truncate(4);
        let mut out = vec![(self.text(), 0.0)];
        let n = weak.len();
        for mask in 1u32..(1 << n) {
            if mask.count_ones() as usize > depth {
                continue;
            }
            let mut g: Vec<char> = self.glyphs.iter().map(|g| g.ch).collect();
            let mut cost = 0.0;
            for (k, &(i, m)) in weak.iter().enumerate() {
                if mask & (1 << k) != 0 {
                    g[i] = self.glyphs[i].alt.unwrap();
                    cost += m;
                }
            }
            out.push((g.into_iter().collect(), cost));
        }
        out.sort_by(|a, b| a.1.total_cmp(&b.1));
        out
    }
}

/// NCC of `t` at (x, y) in `img` (both small); 0 outside or flat.
fn ncc_at(img: &Gray, x: i64, y: i64, t: &Gray) -> f32 {
    if x < 0 || y < 0 || x as usize + t.w > img.w || y as usize + t.h > img.h {
        return 0.0;
    }
    let n = (t.w * t.h) as f32;
    let (mut si, mut sii, mut st, mut stt, mut sit) = (0.0f32, 0.0, 0.0, 0.0, 0.0);
    for ty in 0..t.h {
        let row = (y as usize + ty) * img.w + x as usize;
        for tx in 0..t.w {
            let a = img.v[row + tx];
            let b = t.v[ty * t.w + tx];
            si += a;
            sii += a * a;
            st += b;
            stt += b * b;
            sit += a * b;
        }
    }
    let vi = sii - si * si / n;
    let vt = stt - st * st / n;
    if vi <= 1e-6 || vt <= 1e-6 {
        return 0.0;
    }
    (sit - si * st / n) / (vi * vt).sqrt()
}

struct Cand {
    /// Ink left edge and width in the band.
    x: usize,
    w: usize,
    ch: char,
    score: f32,
}

fn candidates(band: &Gray) -> Vec<Cand> {
    let mut out = Vec::new();
    for (ch, t) in &data().glyphs {
        if t.w > band.w || t.h > band.h + 2 {
            continue;
        }
        let cols: Vec<usize> = (0..t.w)
            .filter(|&x| (0..t.h).any(|y| t.get(x, y) > 0.5))
            .collect();
        let (Some(&il), Some(&ir)) = (cols.first(), cols.last()) else {
            continue;
        };
        let top = band.h as i64 - t.h as i64 + 1;
        let scores: Vec<f32> = (0..=band.w - t.w)
            .map(|x| {
                (-1..=top)
                    .map(|y| ncc_at(band, x as i64, y, t))
                    .fold(f32::MIN, f32::max)
            })
            .collect();
        for x in 0..scores.len() {
            let v = scores[x];
            let (lo, hi) = (x.saturating_sub(2), (x + 3).min(scores.len()));
            if v >= 0.25 && scores[lo..hi].iter().all(|&o| o <= v) {
                out.push(Cand {
                    x: x + il,
                    w: ir - il + 1,
                    ch: *ch,
                    score: v,
                });
            }
        }
    }
    out.sort_by_key(|c| c.x);
    out
}

/// Step 1: (ink left, ink width, char) per glyph of the best string.
fn place(band: &Gray, fmt: Format) -> Vec<(usize, usize, char)> {
    const BIAS: f32 = 0.45;
    let cands = candidates(band);
    let slots = fmt.slots();
    type Key = (usize, usize, usize);
    let mut best: HashMap<Key, (f32, Option<Key>)> = HashMap::new();
    for (i, c) in cands.iter().enumerate() {
        for (si, &(chars, _, hi)) in slots.iter().enumerate() {
            if !chars.contains(c.ch) {
                continue;
            }
            for cnt in 1..=hi {
                let mut val: Option<(f32, Option<Key>)> =
                    (si == 0 && cnt == 1).then_some((c.score - BIAS, None));
                for (j, p) in cands[..i].iter().enumerate() {
                    if !(p.x + p.w <= c.x + 1 && c.x <= p.x + p.w + 5) {
                        continue;
                    }
                    let prev = if cnt > 1 {
                        Some((j, si, cnt - 1))
                    } else if si > 0 {
                        let (_, plo, phi) = slots[si - 1];
                        (plo..=phi)
                            .map(|k| (j, si - 1, k))
                            .filter(|k| best.contains_key(k))
                            .max_by(|a, b| best[a].0.total_cmp(&best[b].0))
                    } else {
                        None
                    };
                    if let Some(prev) = prev
                        && let Some(&(pv, _)) = best.get(&prev)
                    {
                        let v = pv + c.score - BIAS;
                        if val.is_none_or(|(cur, _)| v > cur) {
                            val = Some((v, Some(prev)));
                        }
                    }
                }
                if let Some(v) = val {
                    best.insert((i, si, cnt), v);
                }
            }
        }
    }
    let last = slots.len() - 1;
    let (lo, hi) = (slots[last].1, slots[last].2);
    let Some(mut k) = best
        .iter()
        .filter(|(k, _)| k.1 == last && (lo..=hi).contains(&k.2))
        .max_by(|a, b| a.1.0.total_cmp(&b.1.0))
        .map(|(k, _)| *k)
    else {
        return vec![];
    };
    let mut out = Vec::new();
    loop {
        let c = &cands[k.0];
        out.push((c.x, c.w, c.ch));
        match best[&k].1 {
            Some(prev) => k = prev,
            None => break,
        }
    }
    out.reverse();
    out
}

/// Step 2: the nearest example around the glyph whose ink starts at `x`:
/// (best char, runner-up char, score gap).
fn name(band: &Gray, x: usize, w: usize) -> Option<(char, Option<char>, f32)> {
    let ex = &data().exemplars;
    let (tw, th) = (ex.first()?.1.w, ex.first()?.1.h);
    // the exemplar window centres the glyph's ink in its cell
    let x0 = x as i64 - 1 - (tw as i64 - 2 - w as i64) / 2;
    let mut per: HashMap<char, f32> = HashMap::new();
    let top = band.h as i64 - th as i64;
    for (ch, g) in ex {
        let mut b = f32::MIN;
        for dx in -2..=2 {
            for y in -1..=top + 1 {
                b = b.max(ncc_at(band, x0 + dx, y, g));
            }
        }
        let e = per.entry(*ch).or_insert(f32::MIN);
        *e = e.max(b);
    }
    let mut ranked: Vec<(char, f32)> = per.into_iter().collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
    let (c1, s1) = *ranked.first()?;
    let alt = ranked.get(1).copied();
    Some((c1, alt.map(|a| a.0), alt.map_or(1.0, |a| s1 - a.1)))
}

/// Read the band as `fmt`; `None` when no string of the format fits.
pub fn read(band: &Gray, fmt: Format) -> Option<Number> {
    let placed = place(band, fmt);
    if placed.is_empty() {
        return None;
    }
    let glyphs = placed
        .into_iter()
        .map(|(x, w, ch)| {
            if ch.is_ascii_digit()
                && let Some((c, alt, margin)) = name(band, x, w)
            {
                return Glyph { ch: c, alt, margin };
            }
            Glyph {
                ch,
                alt: None,
                margin: 1.0,
            }
        })
        .collect();
    Some(Number { glyphs })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alternatives_swap_the_weakest_digits_first() {
        let n = Number {
            glyphs: vec![
                Glyph {
                    ch: '8',
                    alt: Some('0'),
                    margin: 0.01,
                },
                Glyph {
                    ch: '.',
                    alt: None,
                    margin: 1.0,
                },
                Glyph {
                    ch: '5',
                    alt: Some('6'),
                    margin: 0.2,
                },
            ],
        };
        let alts = n.alternatives(1);
        assert_eq!(alts[0].0, "8.5");
        assert_eq!(alts[1].0, "0.5");
        assert_eq!(alts[2].0, "8.6");
        assert_eq!(alts.len(), 3);
    }
}
