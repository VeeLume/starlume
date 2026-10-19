//! Finding the SCAN RESULTS panel: anchor voting.
//!
//! Four static parts of the panel (the header, the MASS / RESIST / INSTAB
//! labels, COMPOSITION, the composition bar's frame) are each searched
//! over the whole frame at half resolution and a few scales. Each part has
//! a small bank of templates: from a normal capture under brightness and
//! shadow ink, and from glare captures under shadow ink; a position scores
//! its bank's best match. Every part's best few matches predict where the
//! panel is (the MASS label's top-left) and at what scale; predictions that
//! agree form a cluster, scored by the sum of each part's best member. The
//! best cluster wins if at least three parts agree.
//!
//! This is what tells the panel from look-alikes: the MINING MODULES panel
//! on the other side of the HUD uses the same font, so a single part can
//! match there, but not all four in this panel's arrangement.

use svc_capture::vision::{Correlator, Gray, peaks};

use super::PanelSpot;
use super::data::{ANCHOR_OFFSETS, data};

/// Relative HUD scales searched around the frame's size (the reference is
/// 1440 px tall).
const SCALES: [f32; 5] = [0.88, 0.94, 1.0, 1.06, 1.12];
/// Matches kept per part and scale.
const TOP: usize = 8;
/// Agreement needed: parts and summed score.
const MIN_PARTS: usize = 3;
const MIN_SCORE: f32 = 1.6;

/// The two feature images of a frame, at full resolution.
pub struct Features {
    /// Brightness ink: the brightest channel ramped over 238..252.
    pub ink: Gray,
    /// Shadow ink: the grey closing (5x5) minus the brightest channel, /60.
    pub shadow: Gray,
}

impl Features {
    pub fn of(frame: &svc_capture::Frame) -> Self {
        let v = Gray::brightest(frame);
        Self {
            ink: v.ramp(238.0, 252.0),
            shadow: v.blackhat(5, 60.0),
        }
    }

    fn get(&self, feature: u8) -> &Gray {
        if feature == 0 {
            &self.ink
        } else {
            &self.shadow
        }
    }
}

/// One part's prediction: (score, origin x, origin y, scale).
type Cand = (f32, f32, f32, f32);

fn part_candidates(anchor: usize, half: &[Gray; 2], base: f32) -> Vec<Cand> {
    let bank: Vec<_> = data()
        .anchors
        .iter()
        .filter(|a| a.anchor == anchor)
        .collect();
    let mut corr = [Correlator::new(&half[0]), Correlator::new(&half[1])];
    let (dx, dy) = ANCHOR_OFFSETS[anchor];
    let mut out = Vec::new();
    for rel in SCALES {
        let sc = base * rel;
        let mut best: Option<Gray> = None;
        for a in &bank {
            let m = corr[a.feature as usize].ncc(&a.t.resized(sc / 2.0));
            best = Some(match best {
                None => m,
                Some(b) => {
                    let (w, h) = (b.w.min(m.w), b.h.min(m.h));
                    let mut v = Vec::with_capacity(w * h);
                    for y in 0..h {
                        for x in 0..w {
                            v.push(b.get(x, y).max(m.get(x, y)));
                        }
                    }
                    Gray { w, h, v }
                }
            });
        }
        let Some(map) = best else { continue };
        for (v, x, y) in peaks(&map, TOP, 10, 0.2) {
            out.push((v, x as f32 * 2.0 - dx * sc, y as f32 * 2.0 - dy * sc, sc));
        }
    }
    out.sort_by(|a, b| b.0.total_cmp(&a.0));
    out.truncate(TOP * 2);
    out
}

/// The panel's position by vote, or `None` when fewer than three parts
/// agree.
pub fn vote(f: &Features, frame_h: u32) -> Option<PanelSpot> {
    let base = frame_h as f32 / 1440.0;
    let half = [f.ink.half(), f.shadow.half()];
    let parts: Vec<Vec<Cand>> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..ANCHOR_OFFSETS.len())
            .map(|a| {
                let half = &half;
                s.spawn(move || part_candidates(a, half, base))
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("locator thread"))
            .collect()
    });
    let near = 12.0 * base;
    let mut best: Option<(f32, usize, f32, f32, f32)> = None;
    for seed in parts.iter().flatten() {
        let mut members: Vec<Option<Cand>> = vec![None; parts.len()];
        for (pi, cs) in parts.iter().enumerate() {
            for c in cs {
                if (c.1 - seed.1).abs() <= near
                    && (c.2 - seed.2).abs() <= near
                    && (c.3 - seed.3).abs() <= 0.04 * base
                    && members[pi].is_none_or(|m| c.0 > m.0)
                {
                    members[pi] = Some(*c);
                }
            }
        }
        let got: Vec<Cand> = members.into_iter().flatten().collect();
        let total: f32 = got.iter().map(|c| c.0).sum();
        if best.is_none_or(|b| total > b.0) {
            let n = got.len() as f32;
            best = Some((
                total,
                got.len(),
                got.iter().map(|c| c.1).sum::<f32>() / n,
                got.iter().map(|c| c.2).sum::<f32>() / n,
                got.iter().map(|c| c.3).sum::<f32>() / n,
            ));
        }
    }
    let (score, n, x, y, scale) = best?;
    (n >= MIN_PARTS && score >= MIN_SCORE).then_some(PanelSpot {
        x,
        y,
        scale,
        score,
        parts: n as u8,
        angle: 0.0,
    })
}

/// NCC of `t` with its top-left at (x, y) in `img`; 0 for a flat window
/// or outside.
fn ncc_at(img: &Gray, x: i64, y: i64, t: &Gray) -> f32 {
    if x < 0 || y < 0 || x as usize + t.w > img.w || y as usize + t.h > img.h {
        return 0.0;
    }
    let n = (t.w * t.h) as f64;
    let (mut si, mut sii, mut st, mut stt, mut sit) = (0.0f64, 0.0, 0.0, 0.0, 0.0);
    for ty in 0..t.h {
        for tx in 0..t.w {
            let a = img.get(x as usize + tx, y as usize + ty) as f64;
            let b = t.get(tx, ty) as f64;
            si += a;
            sii += a * a;
            st += b;
            stt += b * b;
            sit += a * b;
        }
    }
    let vt = stt - st * st / n;
    let vi = sii - si * si / n;
    if vt <= 0.0 || vi <= 0.02 * vt {
        return 0.0;
    }
    ((sit - si * st / n) / (vi * vt).sqrt()) as f32
}

/// The best (score, x, y, scale) of `anchor`'s bank around (x, y) within
/// ±`reach` px and the given scales.
fn best_near(
    f: &Features,
    anchor: usize,
    x: f32,
    y: f32,
    reach: i64,
    scales: &[f32],
) -> Option<(f32, f32, f32, f32)> {
    let mut best: Option<(f32, f32, f32, f32)> = None;
    for &sc in scales {
        for a in data().anchors.iter().filter(|a| a.anchor == anchor) {
            let t = a.t.resized(sc);
            let img = f.get(a.feature);
            for dy in -reach..=reach {
                for dx in -reach..=reach {
                    let (px, py) = (x.round() as i64 + dx, y.round() as i64 + dy);
                    let v = ncc_at(img, px, py, &t);
                    if best.is_none_or(|b| v > b.0) {
                        best = Some((v, px as f32, py as f32, sc));
                    }
                }
            }
        }
    }
    best
}

/// Full-resolution refinement: the MASS labels pin the origin (±4 px),
/// then COMPOSITION, 130 reference px further down, pins the scale (±4 %).
pub fn refine(f: &Features, spot: PanelSpot) -> PanelSpot {
    let mut s = spot;
    if let Some((_, x, y, _)) = best_near(f, 1, s.x, s.y, 4, &[s.scale]) {
        s.x = x;
        s.y = y;
    }
    let (cdx, cdy) = ANCHOR_OFFSETS[2];
    let scales: Vec<f32> = (-4..=4)
        .map(|k| s.scale * (1.0 + k as f32 * 0.01))
        .collect();
    if let Some((v, _, cy, _)) =
        best_near(f, 2, s.x + cdx * s.scale, s.y + cdy * s.scale, 6, &scales)
        && v >= 0.4
    {
        s.scale = (cy - s.y) / cdy;
    }
    s
}
