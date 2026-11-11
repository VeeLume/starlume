//! Glyph matching: read a line of game text from an ink image of a known
//! region, against glyph templates passed in as data.
//!
//! Two steps, kept apart because they fail differently:
//! 1. **Place** ([`place`]). Every mean template slides over the line;
//!    a dynamic program picks the best string the [`Grammar`] allows, each
//!    glyph starting just after the previous one's ink (a pixel of overlap
//!    and a few of gap tolerated, so fractional glyph positions still
//!    chain). This decides where the glyphs are.
//! 2. **Name** ([`read`]). Each placed glyph is ranked again: against the
//!    labelled exemplars when the set has some for its slot (nearest
//!    example, a few pixels of slack; tells apart glyphs that differ by a
//!    one-pixel hole, which mean templates blur away), else by the template
//!    hits at the same spot.
//!
//! The result keeps **ranked candidates per position**, not one answer:
//! callers apply what they know (checksums, sums, a model) to pick among
//! them. Building templates (rendered from the game's font, or learned
//! from verified reads), capturing and locating the region stay with the
//! callers.
//!
//! Pure: no capture, no IO, no platform code.

mod gray;

use std::collections::{BTreeMap, HashMap};

pub use gray::Gray;

/// The templates a line is read against.
#[derive(Clone, Debug, Default)]
pub struct TemplateSet {
    /// Mean templates, one or more per char: they place the glyphs.
    pub glyphs: Vec<(char, Gray)>,
    /// Labelled examples (every one kept, not averaged): they name the
    /// placed glyphs whose slot allows a char that has examples.
    pub exemplars: Vec<(char, Gray)>,
}

/// One run of glyphs in a line: which chars, how many.
#[derive(Clone, Debug, PartialEq)]
pub struct Slot {
    pub chars: String,
    pub min: usize,
    pub max: usize,
}

impl Slot {
    pub fn new(chars: &str, min: usize, max: usize) -> Self {
        Self {
            chars: chars.into(),
            min,
            max,
        }
    }
}

/// The shape of a line: its slots left to right, e.g. digits 1..3, `.`,
/// digits 2..2, `%`.
#[derive(Clone, Debug, PartialEq)]
pub struct Grammar(pub Vec<Slot>);

/// The matcher's tolerances. [`Default`] is what the mining scan panel
/// was tuned with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    /// The lowest template score that counts as a hit.
    pub min_score: f32,
    /// A hit must be the best within this many px left and right.
    pub peak_radius: usize,
    /// Subtracted per placed glyph: keeps the program from padding a
    /// string with weak hits.
    pub bias: f32,
    /// How far a glyph's ink may start before the previous one's ends.
    pub overlap: usize,
    /// How far after the previous glyph's ink a glyph may start.
    pub gap: usize,
    /// Naming searches the exemplars this many px left and right.
    pub reach: i64,
    /// Template hits within this many px of a placed glyph rank it when
    /// there are no exemplars.
    pub same_spot: usize,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            min_score: 0.25,
            peak_radius: 2,
            bias: 0.45,
            overlap: 1,
            gap: 5,
            reach: 2,
            same_spot: 1,
        }
    }
}

/// A template hit: ink left edge and width in the line, the char, the
/// score (NCC, up to 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub x: usize,
    pub w: usize,
    pub ch: char,
    pub score: f32,
}

/// One read glyph: where its ink is, the slot it fills, and the chars it
/// could be, best first, with their scores.
#[derive(Clone, Debug, PartialEq)]
pub struct Position {
    pub x: usize,
    pub w: usize,
    pub slot: usize,
    pub ranked: Vec<(char, f32)>,
}

impl Position {
    /// The best char.
    pub fn ch(&self) -> char {
        self.ranked[0].0
    }

    /// The runner-up and how much worse it scored (0: a toss-up); `None`
    /// when nothing else fits here.
    pub fn runner_up(&self) -> Option<(char, f32)> {
        self.ranked.get(1).map(|&(c, s)| (c, self.ranked[0].1 - s))
    }
}

/// NCC of `t` with its top-left at (x, y) in `img`; 0 outside or for a
/// flat window.
pub fn ncc_at(img: &Gray, x: i64, y: i64, t: &Gray) -> f32 {
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

/// Every template's local score peaks along the line, by ink left edge.
/// A template may sit a pixel above or below the line's own top/bottom.
pub fn hits(line: &Gray, set: &TemplateSet, p: &Params) -> Vec<Hit> {
    let mut out = Vec::new();
    for (ch, t) in &set.glyphs {
        if t.w > line.w || t.h > line.h + 2 {
            continue;
        }
        let cols: Vec<usize> = (0..t.w)
            .filter(|&x| (0..t.h).any(|y| t.get(x, y) > 0.5))
            .collect();
        let (Some(&il), Some(&ir)) = (cols.first(), cols.last()) else {
            continue;
        };
        let top = line.h as i64 - t.h as i64 + 1;
        let scores: Vec<f32> = (0..=line.w - t.w)
            .map(|x| {
                (-1..=top)
                    .map(|y| ncc_at(line, x as i64, y, t))
                    .fold(f32::MIN, f32::max)
            })
            .collect();
        for x in 0..scores.len() {
            let v = scores[x];
            let lo = x.saturating_sub(p.peak_radius);
            let hi = (x + p.peak_radius + 1).min(scores.len());
            if v >= p.min_score && scores[lo..hi].iter().all(|&o| o <= v) {
                out.push(Hit {
                    x: x + il,
                    w: ir - il + 1,
                    ch: *ch,
                    score: v,
                });
            }
        }
    }
    out.sort_by_key(|h| h.x);
    out
}

/// The best string of `grammar` along the line: (slot, hit) per glyph,
/// left to right; empty when no string of the grammar fits.
pub fn place(line: &Gray, set: &TemplateSet, grammar: &Grammar, p: &Params) -> Vec<(usize, Hit)> {
    place_hits(&hits(line, set, p), grammar, p)
}

fn place_hits(cands: &[Hit], grammar: &Grammar, p: &Params) -> Vec<(usize, Hit)> {
    let slots = &grammar.0;
    if slots.is_empty() {
        return vec![];
    }
    // (hit, slot, count in slot) → (best total, back pointer)
    type Key = (usize, usize, usize);
    let mut best: HashMap<Key, (f32, Option<Key>)> = HashMap::new();
    for (i, c) in cands.iter().enumerate() {
        for (si, slot) in slots.iter().enumerate() {
            if !slot.chars.contains(c.ch) {
                continue;
            }
            for cnt in 1..=slot.max {
                let mut val: Option<(f32, Option<Key>)> =
                    (si == 0 && cnt == 1).then_some((c.score - p.bias, None));
                for (j, q) in cands[..i].iter().enumerate() {
                    if !(q.x + q.w <= c.x + p.overlap && c.x <= q.x + q.w + p.gap) {
                        continue;
                    }
                    let prev = if cnt > 1 {
                        Some((j, si, cnt - 1))
                    } else if si > 0 {
                        let before = &slots[si - 1];
                        (before.min..=before.max)
                            .map(|k| (j, si - 1, k))
                            .filter(|k| best.contains_key(k))
                            .max_by(|a, b| best[a].0.total_cmp(&best[b].0))
                    } else {
                        None
                    };
                    if let Some(prev) = prev
                        && let Some(&(pv, _)) = best.get(&prev)
                    {
                        let v = pv + c.score - p.bias;
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
    let (lo, hi) = (slots[last].min, slots[last].max);
    let Some(mut k) = best
        .iter()
        .filter(|(k, _)| k.1 == last && (lo..=hi).contains(&k.2))
        // ties go to the smallest key, whatever order the map yields
        .max_by(|a, b| a.1.0.total_cmp(&b.1.0).then(b.0.cmp(a.0)))
        .map(|(k, _)| *k)
    else {
        return vec![];
    };
    let mut out = Vec::new();
    loop {
        out.push((k.1, cands[k.0]));
        match best[&k].1 {
            Some(prev) => k = prev,
            None => break,
        }
    }
    out.reverse();
    out
}

/// The exemplars of `allowed` chars around the glyph whose ink starts at
/// `x` (width `w`), best first: one score per char, its nearest example.
fn name(
    line: &Gray,
    set: &TemplateSet,
    x: usize,
    w: usize,
    allowed: &str,
    p: &Params,
) -> Vec<(char, f32)> {
    let mut per: BTreeMap<char, f32> = BTreeMap::new();
    for (ch, g) in &set.exemplars {
        if !allowed.contains(*ch) {
            continue;
        }
        // the exemplar window centres the glyph's ink in its cell
        let x0 = x as i64 - 1 - (g.w as i64 - 2 - w as i64) / 2;
        let top = line.h as i64 - g.h as i64;
        let mut b = f32::MIN;
        for dx in -p.reach..=p.reach {
            for y in -1..=top + 1 {
                b = b.max(ncc_at(line, x0 + dx, y, g));
            }
        }
        let e = per.entry(*ch).or_insert(f32::MIN);
        *e = e.max(b);
    }
    let mut ranked: Vec<(char, f32)> = per.into_iter().collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
    ranked
}

/// Read the line as `grammar`: each placed glyph with its ranked
/// candidates. `None` when no string of the grammar fits.
pub fn read(
    line: &Gray,
    set: &TemplateSet,
    grammar: &Grammar,
    p: &Params,
) -> Option<Vec<Position>> {
    let cands = hits(line, set, p);
    let placed = place_hits(&cands, grammar, p);
    if placed.is_empty() {
        return None;
    }
    let named: String = set.exemplars.iter().map(|(c, _)| *c).collect();
    let out = placed
        .into_iter()
        .map(|(slot, h)| {
            let allowed = &grammar.0[slot].chars;
            let mut ranked = if named.contains(h.ch) {
                name(line, set, h.x, h.w, allowed, p)
            } else {
                vec![]
            };
            if ranked.is_empty() {
                ranked = spot_ranking(&cands, &h, allowed, p);
            }
            Position {
                x: h.x,
                w: h.w,
                slot,
                ranked,
            }
        })
        .collect();
    Some(out)
}

/// The placed hit first, then the other allowed chars' best hits at the
/// same spot.
fn spot_ranking(cands: &[Hit], h: &Hit, allowed: &str, p: &Params) -> Vec<(char, f32)> {
    let mut per: BTreeMap<char, f32> = BTreeMap::new();
    for c in cands {
        if c.ch != h.ch && allowed.contains(c.ch) && c.x.abs_diff(h.x) <= p.same_spot {
            let e = per.entry(c.ch).or_insert(f32::MIN);
            *e = e.max(c.score);
        }
    }
    let mut rest: Vec<(char, f32)> = per.into_iter().collect();
    rest.sort_by(|a, b| b.1.total_cmp(&a.1));
    let mut ranked = vec![(h.ch, h.score)];
    ranked.extend(rest);
    ranked
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 5x7 glyph from rows of '#'.
    fn glyph(rows: &[&str]) -> Gray {
        let (w, h) = (rows[0].len(), rows.len());
        let mut g = Gray::new(w, h);
        for (y, r) in rows.iter().enumerate() {
            for (x, c) in r.chars().enumerate() {
                if c == '#' {
                    g.v[y * w + x] = 1.0;
                }
            }
        }
        g
    }

    fn font() -> Vec<(char, Gray)> {
        vec![
            (
                '1',
                glyph(&[
                    "..#..", ".##..", "..#..", "..#..", "..#..", "..#..", ".###.",
                ]),
            ),
            (
                '7',
                glyph(&[
                    "#####", "....#", "...#.", "..#..", "..#..", "..#..", "..#..",
                ]),
            ),
            (
                '0',
                glyph(&[
                    ".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###.",
                ]),
            ),
            (
                '8',
                glyph(&[
                    ".###.", "#...#", "#...#", ".###.", "#...#", "#...#", ".###.",
                ]),
            ),
            (
                '.',
                glyph(&[
                    ".....", ".....", ".....", ".....", ".....", ".##..", ".##..",
                ]),
            ),
        ]
    }

    /// The glyphs of `text` side by side with a pixel between, on a line
    /// a pixel taller above and below.
    fn line(text: &str) -> Gray {
        let f = font();
        let mut out = Gray::new(text.len() * 6 + 2, 9);
        for (i, ch) in text.chars().enumerate() {
            let g = &f.iter().find(|(c, _)| *c == ch).unwrap().1;
            for y in 0..g.h {
                for x in 0..g.w {
                    out.v[(y + 1) * out.w + 1 + i * 6 + x] = g.get(x, y);
                }
            }
        }
        out
    }

    fn set() -> TemplateSet {
        TemplateSet {
            glyphs: font(),
            exemplars: vec![],
        }
    }

    #[test]
    fn reads_a_line_of_its_grammar() {
        let g = Grammar(vec![
            Slot::new("0178", 1, 3),
            Slot::new(".", 1, 1),
            Slot::new("0178", 1, 1),
        ]);
        let read = read(&line("10.7"), &set(), &g, &Params::default()).unwrap();
        let text: String = read.iter().map(Position::ch).collect();
        assert_eq!(text, "10.7");
        assert_eq!(
            read.iter().map(|p| p.slot).collect::<Vec<_>>(),
            vec![0, 0, 1, 2]
        );
    }

    #[test]
    fn a_line_outside_the_grammar_reads_nothing() {
        // three glyphs wanted, two on the line
        let g = Grammar(vec![Slot::new("0178", 3, 3)]);
        assert_eq!(read(&line("17"), &set(), &g, &Params::default()), None);
    }

    #[test]
    fn close_glyphs_keep_a_runner_up() {
        // 0 and 8 differ by the middle bar: each is the other's runner-up.
        let g = Grammar(vec![Slot::new("0178", 1, 2)]);
        let read = read(&line("80"), &set(), &g, &Params::default()).unwrap();
        assert_eq!(read[0].ch(), '8');
        assert_eq!(read[0].runner_up().map(|r| r.0), Some('0'));
        assert_eq!(read[1].ch(), '0');
        assert_eq!(read[1].runner_up().map(|r| r.0), Some('8'));
    }

    #[test]
    fn exemplars_rank_within_the_slot() {
        let mut s = set();
        s.exemplars = font()
            .into_iter()
            .filter(|(c, _)| c.is_ascii_digit())
            .collect();
        let g = Grammar(vec![
            Slot::new("018", 1, 1),
            Slot::new(".", 1, 1),
            Slot::new("18", 1, 1),
        ]);
        let read = read(&line("0.8"), &s, &g, &Params::default()).unwrap();
        assert_eq!(read[0].ch(), '0');
        assert!(read[0].ranked.iter().all(|(c, _)| "018".contains(*c)));
        // the '.' has no exemplars: ranked by its own hit alone
        assert_eq!(read[1].ranked.len(), 1);
        // the last slot leaves out 0 and 7
        assert!(read[2].ranked.iter().all(|(c, _)| "18".contains(*c)));
        assert_eq!(read[2].ch(), '8');
    }
}
