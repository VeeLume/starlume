//! Image tools for reading HUD panels: grey images, the two feature images
//! the readers match on, resizing and rotation, and normalised
//! cross-correlation (NCC) over a whole image via FFT.
//!
//! Feature images, both from the brightest channel v of each pixel:
//! - [`Gray::ramp`]: brightness ink. HUD text is near-saturated (250+)
//!   with a dark outline; a ramp high up keeps the glow out of the glyphs'
//!   holes.
//! - [`Gray::blackhat`]: shadow ink, the grey closing minus v: thin dark
//!   lines (the glyphs' drop shadow and their holes). Under glare,
//!   everything else saturates and these stay dark.

use rustfft::FftPlanner;
use rustfft::num_complex::Complex;

use crate::Frame;

/// The grey image the readers work on; it lives in svc-glyph, the pure
/// glyph matcher, so a reader can build ink without a capture.
pub use svc_glyph::Gray;

/// The brightest of R, G, B per pixel, 0..255.
pub fn brightest(frame: &Frame) -> Gray {
    let v = frame
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| p[0].max(p[1]).max(p[2]) as f32)
        .collect();
    Gray {
        w: frame.width as usize,
        h: frame.height as usize,
        v,
    }
}

/// `frame` rotated by `degrees` (counter-clockwise) about its centre,
/// bilinear, black outside. A panel tilted by head tracking reads upright
/// in the frame rotated the other way.
pub fn rotate_frame(frame: &Frame, degrees: f32) -> Frame {
    let (w, h) = (frame.width as usize, frame.height as usize);
    let (s, c) = degrees.to_radians().sin_cos();
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let mut rgba = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = (x as f32 - cx, y as f32 - cy);
            // inverse map: where this output pixel comes from
            let sx = c * dx - s * dy + cx;
            let sy = s * dx + c * dy + cy;
            if sx < 0.0 || sy < 0.0 || sx >= (w - 1) as f32 || sy >= (h - 1) as f32 {
                continue;
            }
            let (x0, y0) = (sx as usize, sy as usize);
            let (tx, ty) = (sx - x0 as f32, sy - y0 as f32);
            for ch in 0..3 {
                let p = |xx: usize, yy: usize| frame.rgba[(yy * w + xx) * 4 + ch] as f32;
                let v = p(x0, y0) * (1.0 - tx) * (1.0 - ty)
                    + p(x0 + 1, y0) * tx * (1.0 - ty)
                    + p(x0, y0 + 1) * (1.0 - tx) * ty
                    + p(x0 + 1, y0 + 1) * tx * ty;
                rgba[(y * w + x) * 4 + ch] = v.round() as u8;
            }
            rgba[(y * w + x) * 4 + 3] = 255;
        }
    }
    Frame {
        width: frame.width,
        height: frame.height,
        rgba,
    }
}

// ── NCC via FFT ─────────────────────────────────────────────────────────────

/// The smallest n ≥ `min` whose prime factors are 2, 3 and 5 (fast FFTs).
fn fast_len(min: usize) -> usize {
    let mut n = min.max(1);
    loop {
        let mut m = n;
        for p in [2, 3, 5] {
            while m.is_multiple_of(p) {
                m /= p;
            }
        }
        if m == 1 {
            return n;
        }
        n += 1;
    }
}

/// An image prepared for correlating many templates against it: its
/// spectrum and integral images (sum, sum of squares).
pub struct Correlator {
    w: usize,
    h: usize,
    fw: usize,
    fh: usize,
    spectrum: Vec<Complex<f32>>,
    sum: Vec<f64>,
    sq: Vec<f64>,
    planner: FftPlanner<f32>,
}

fn fft2(
    planner: &mut FftPlanner<f32>,
    data: &mut [Complex<f32>],
    fw: usize,
    fh: usize,
    inverse: bool,
) {
    let row = if inverse {
        planner.plan_fft_inverse(fw)
    } else {
        planner.plan_fft_forward(fw)
    };
    for r in data.chunks_exact_mut(fw) {
        row.process(r);
    }
    let col = if inverse {
        planner.plan_fft_inverse(fh)
    } else {
        planner.plan_fft_forward(fh)
    };
    let mut buf = vec![Complex::new(0.0, 0.0); fh];
    for x in 0..fw {
        for y in 0..fh {
            buf[y] = data[y * fw + x];
        }
        col.process(&mut buf);
        for y in 0..fh {
            data[y * fw + x] = buf[y];
        }
    }
}

impl Correlator {
    pub fn new(img: &Gray) -> Self {
        let (fw, fh) = (fast_len(img.w), fast_len(img.h));
        let mut planner = FftPlanner::new();
        let mut spectrum = vec![Complex::new(0.0, 0.0); fw * fh];
        for y in 0..img.h {
            for x in 0..img.w {
                spectrum[y * fw + x].re = img.v[y * img.w + x];
            }
        }
        fft2(&mut planner, &mut spectrum, fw, fh, false);
        let (iw, ih) = (img.w + 1, img.h + 1);
        let mut sum = vec![0.0f64; iw * ih];
        let mut sq = vec![0.0f64; iw * ih];
        for y in 0..img.h {
            let (mut rs, mut rq) = (0.0f64, 0.0f64);
            for x in 0..img.w {
                let v = img.v[y * img.w + x] as f64;
                rs += v;
                rq += v * v;
                sum[(y + 1) * iw + x + 1] = sum[y * iw + x + 1] + rs;
                sq[(y + 1) * iw + x + 1] = sq[y * iw + x + 1] + rq;
            }
        }
        Self {
            w: img.w,
            h: img.h,
            fw,
            fh,
            spectrum,
            sum,
            sq,
            planner,
        }
    }

    /// NCC of `t` at every position where it fits whole: a (w − tw + 1) x
    /// (h − th + 1) map in −1..1. Nearly flat windows (empty sky,
    /// saturated glare) have no shape to match and score 0.
    pub fn ncc(&mut self, t: &Gray) -> Gray {
        if t.w > self.w || t.h > self.h {
            return Gray::new(0, 0);
        }
        let n = (t.w * t.h) as f64;
        let mean = t.v.iter().map(|&x| x as f64).sum::<f64>() / n;
        let tz: Vec<f32> = t.v.iter().map(|&x| (x as f64 - mean) as f32).collect();
        let tn = tz.iter().map(|&x| (x as f64) * (x as f64)).sum::<f64>();
        let mut ts = vec![Complex::new(0.0, 0.0); self.fw * self.fh];
        for y in 0..t.h {
            for x in 0..t.w {
                ts[y * self.fw + x].re = tz[y * t.w + x];
            }
        }
        fft2(&mut self.planner, &mut ts, self.fw, self.fh, false);
        for (a, b) in ts.iter_mut().zip(&self.spectrum) {
            *a = b * a.conj();
        }
        fft2(&mut self.planner, &mut ts, self.fw, self.fh, true);
        let scale = 1.0 / (self.fw * self.fh) as f64;
        let (ow, oh) = (self.w - t.w + 1, self.h - t.h + 1);
        let iw = self.w + 1;
        let floor = 0.02 * tn;
        let mut out = Vec::with_capacity(ow * oh);
        for y in 0..oh {
            for x in 0..ow {
                let bx = |a: &[f64]| {
                    a[(y + t.h) * iw + x + t.w] - a[y * iw + x + t.w] - a[(y + t.h) * iw + x]
                        + a[y * iw + x]
                };
                let s1 = bx(&self.sum);
                let s2 = bx(&self.sq);
                let var = s2 - s1 * s1 / n;
                let num = ts[y * self.fw + x].re as f64 * scale;
                out.push(if var > floor && tn > 0.0 {
                    (num / (var.sqrt() * tn.sqrt())).clamp(-1.0, 1.0) as f32
                } else {
                    0.0
                });
            }
        }
        Gray {
            w: ow,
            h: oh,
            v: out,
        }
    }
}

/// The `k` best local maxima of a score map above `min`, at least `sep`
/// pixels apart: (score, x, y).
pub fn peaks(map: &Gray, k: usize, sep: usize, min: f32) -> Vec<(f32, usize, usize)> {
    let mut m = map.v.clone();
    let mut out = Vec::new();
    for _ in 0..k {
        let Some((i, &v)) = m.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)) else {
            break;
        };
        if v <= min {
            break;
        }
        let (x, y) = (i % map.w, i / map.w);
        out.push((v, x, y));
        for yy in y.saturating_sub(sep)..(y + sep + 1).min(map.h) {
            for xx in x.saturating_sub(sep)..(x + sep + 1).min(map.w) {
                m[yy * map.w + xx] = f32::MIN;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ncc_finds_a_planted_patch() {
        let mut img = Gray::new(64, 48);
        let mut seed = 12345u32;
        for v in img.v.iter_mut() {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            *v = (seed >> 16) as f32 / 65_536.0;
        }
        let mut t = Gray::new(9, 7);
        for y in 0..7 {
            for x in 0..9 {
                t.v[y * 9 + x] = img.v[(20 + y) * 64 + 30 + x];
            }
        }
        let mut c = Correlator::new(&img);
        let map = c.ncc(&t);
        let (v, x, y) = peaks(&map, 1, 2, 0.0)[0];
        assert_eq!((x, y), (30, 20));
        assert!((v - 1.0).abs() < 1e-3, "{v}");
    }

    #[test]
    fn flat_windows_score_zero() {
        let img = Gray::new(32, 32);
        let mut t = Gray::new(5, 5);
        t.v[12] = 1.0;
        let map = Correlator::new(&img).ncc(&t);
        assert!(map.v.iter().all(|&v| v == 0.0));
    }
}
