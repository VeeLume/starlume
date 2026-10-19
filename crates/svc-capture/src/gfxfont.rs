//! The game's HUD fonts, read from its own `Data/UI/fonts/Shared/fonts_en.gfx`
//! (a Scaleform movie: an uncompressed SWF with a `GFX` signature) and
//! rendered to grey images, so readers can match HUD text against exact
//! glyphs. Nothing of the fonts ships with Starlume: they are commercial,
//! and the user's own game files hold them.
//!
//! Supported: `DefineFont3` (tag 75), the only font tag the HUD movie
//! uses: glyph outlines as SWF shape records (straight and quadratic
//! edges, 20 units per twip, 1024-twip em), the code table, and the
//! layout block (advances, kerning).

use std::collections::HashMap;

use crate::vision::Gray;

/// Units per em in DefineFont3 outlines and layout (1024 twips x 20).
const EM: f32 = 20480.0;

#[derive(Debug, thiserror::Error)]
pub enum FontError {
    #[error("not a GFX/SWF movie")]
    NotSwf,
    #[error("compressed movies aren't supported (use the .gfx, not the .swf)")]
    Compressed,
    #[error("font data ends early")]
    Truncated,
    #[error("no font named {0:?} in the movie")]
    Missing(String),
}

/// A point on a glyph outline, in font units (y down, baseline at 0).
type Pt = (f32, f32);

#[derive(Debug, Clone, Default)]
pub struct Glyph {
    /// Closed polylines (curves flattened).
    pub contours: Vec<Vec<Pt>>,
    /// Pen advance, font units.
    pub advance: f32,
}

#[derive(Debug, Clone)]
pub struct Font {
    pub name: String,
    pub ascent: f32,
    pub descent: f32,
    pub glyphs: HashMap<char, Glyph>,
    pub kerning: HashMap<(char, char), f32>,
}

// ── SWF reading ─────────────────────────────────────────────────────────────

struct Bits<'a> {
    b: &'a [u8],
    pos: usize, // bit position
}

impl<'a> Bits<'a> {
    fn new(b: &'a [u8]) -> Self {
        Self { b, pos: 0 }
    }
    fn ub(&mut self, n: u32) -> Result<u32, FontError> {
        let mut v = 0u32;
        for _ in 0..n {
            let byte = *self.b.get(self.pos / 8).ok_or(FontError::Truncated)?;
            let bit = (byte >> (7 - (self.pos % 8))) & 1;
            v = (v << 1) | bit as u32;
            self.pos += 1;
        }
        Ok(v)
    }
    fn sb(&mut self, n: u32) -> Result<i32, FontError> {
        if n == 0 {
            return Ok(0);
        }
        let v = self.ub(n)?;
        let shift = 32 - n;
        Ok(((v << shift) as i32) >> shift)
    }
    fn align(&mut self) {
        self.pos = self.pos.div_ceil(8) * 8;
    }
}

fn u16_at(b: &[u8], i: usize) -> Result<u16, FontError> {
    Ok(u16::from_le_bytes([
        *b.get(i).ok_or(FontError::Truncated)?,
        *b.get(i + 1).ok_or(FontError::Truncated)?,
    ]))
}

fn u32_at(b: &[u8], i: usize) -> Result<u32, FontError> {
    let s = b.get(i..i + 4).ok_or(FontError::Truncated)?;
    Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn i16_at(b: &[u8], i: usize) -> Result<i16, FontError> {
    Ok(u16_at(b, i)? as i16)
}

/// Flatten one glyph's shape records into closed contours.
fn shape(b: &[u8]) -> Result<Vec<Vec<Pt>>, FontError> {
    let mut r = Bits::new(b);
    let fill_bits = r.ub(4)?;
    let line_bits = r.ub(4)?;
    let mut contours: Vec<Vec<Pt>> = Vec::new();
    let mut cur: Vec<Pt> = Vec::new();
    let (mut x, mut y) = (0.0f32, 0.0f32);
    loop {
        let edge = r.ub(1)?;
        if edge == 0 {
            let flags = r.ub(5)?;
            if flags == 0 {
                break;
            }
            let (new_styles, line, fill1, fill0, move_to) = (
                flags & 16 != 0,
                flags & 8 != 0,
                flags & 4 != 0,
                flags & 2 != 0,
                flags & 1 != 0,
            );
            if move_to {
                let n = r.ub(5)?;
                x = r.sb(n)? as f32;
                y = r.sb(n)? as f32;
                if cur.len() > 1 {
                    contours.push(std::mem::take(&mut cur));
                }
                cur.clear();
                cur.push((x, y));
            }
            if fill0 {
                r.ub(fill_bits)?;
            }
            if fill1 {
                r.ub(fill_bits)?;
            }
            if line {
                r.ub(line_bits)?;
            }
            if new_styles {
                // never in font glyphs
                return Err(FontError::Truncated);
            }
        } else if r.ub(1)? == 1 {
            // straight
            let n = r.ub(4)? + 2;
            let general = r.ub(1)? == 1;
            let (dx, dy) = if general {
                (r.sb(n)?, r.sb(n)?)
            } else if r.ub(1)? == 1 {
                (0, r.sb(n)?)
            } else {
                (r.sb(n)?, 0)
            };
            if cur.is_empty() {
                cur.push((x, y));
            }
            x += dx as f32;
            y += dy as f32;
            cur.push((x, y));
        } else {
            // quadratic curve
            let n = r.ub(4)? + 2;
            let (cx, cy) = (x + r.sb(n)? as f32, y + r.sb(n)? as f32);
            let (ax, ay) = (cx + r.sb(n)? as f32, cy + r.sb(n)? as f32);
            if cur.is_empty() {
                cur.push((x, y));
            }
            for k in 1..=8 {
                let t = k as f32 / 8.0;
                let u = 1.0 - t;
                cur.push((
                    u * u * x + 2.0 * u * t * cx + t * t * ax,
                    u * u * y + 2.0 * u * t * cy + t * t * ay,
                ));
            }
            x = ax;
            y = ay;
        }
    }
    if cur.len() > 1 {
        contours.push(cur);
    }
    r.align();
    Ok(contours)
}

fn define_font3(t: &[u8]) -> Result<Font, FontError> {
    let flags = *t.get(2).ok_or(FontError::Truncated)?;
    let (has_layout, wide_offsets, wide_codes) =
        (flags & 0x80 != 0, flags & 0x08 != 0, flags & 0x04 != 0);
    let name_len = *t.get(4).ok_or(FontError::Truncated)? as usize;
    let name = String::from_utf8_lossy(t.get(5..5 + name_len).ok_or(FontError::Truncated)?)
        .trim_end_matches('\0')
        .to_string();
    let mut p = 5 + name_len;
    let n = u16_at(t, p)? as usize;
    p += 2;
    let table = p;
    let off = |i: usize| -> Result<usize, FontError> {
        Ok(if wide_offsets {
            u32_at(t, table + 4 * i)? as usize
        } else {
            u16_at(t, table + 2 * i)? as usize
        })
    };
    let code_table = off(n)?; // the entry after the last glyph offset
    let mut shapes = Vec::with_capacity(n);
    for i in 0..n {
        let (a, b) = (
            table + off(i)?,
            if i + 1 < n {
                table + off(i + 1)?
            } else {
                table + code_table
            },
        );
        shapes.push(shape(t.get(a..b).ok_or(FontError::Truncated)?)?);
    }
    let mut q = table + code_table;
    let mut codes = Vec::with_capacity(n);
    for _ in 0..n {
        if wide_codes {
            codes.push(u16_at(t, q)? as u32);
            q += 2;
        } else {
            codes.push(*t.get(q).ok_or(FontError::Truncated)? as u32);
            q += 1;
        }
    }
    let (mut ascent, mut descent) = (EM * 0.8, EM * 0.2);
    let mut advances = vec![0.0f32; n];
    let mut kerning = HashMap::new();
    if has_layout {
        ascent = u16_at(t, q)? as f32;
        descent = u16_at(t, q + 2)? as f32;
        q += 6; // ascent, descent, leading
        for a in advances.iter_mut() {
            *a = i16_at(t, q)? as f32;
            q += 2;
        }
        // bounds table: n RECTs (bit-packed, byte-aligned each)
        for _ in 0..n {
            let nbits = (*t.get(q).ok_or(FontError::Truncated)? >> 3) as usize;
            q += (5 + 4 * nbits).div_ceil(8);
        }
        let count = u16_at(t, q)? as usize;
        q += 2;
        for _ in 0..count {
            let (c1, c2) = if wide_codes {
                let v = (u16_at(t, q)? as u32, u16_at(t, q + 2)? as u32);
                q += 4;
                v
            } else {
                let v = (t[q] as u32, t[q + 1] as u32);
                q += 2;
                v
            };
            let adj = i16_at(t, q)? as f32;
            q += 2;
            if let (Some(a), Some(b)) = (char::from_u32(c1), char::from_u32(c2)) {
                kerning.insert((a, b), adj);
            }
        }
    }
    let glyphs = codes
        .into_iter()
        .zip(shapes)
        .zip(advances)
        .filter_map(|((c, contours), advance)| {
            char::from_u32(c).map(|ch| (ch, Glyph { contours, advance }))
        })
        .collect();
    Ok(Font {
        name,
        ascent,
        descent,
        glyphs,
        kerning,
    })
}

/// Every DefineFont3 font in a GFX (or uncompressed SWF) movie.
pub fn parse_fonts(movie: &[u8]) -> Result<Vec<Font>, FontError> {
    match movie.get(..3) {
        Some(b"GFX") | Some(b"FWS") => {}
        Some(b"CWS") | Some(b"CFX") | Some(b"ZWS") => return Err(FontError::Compressed),
        _ => return Err(FontError::NotSwf),
    }
    let nbits = (*movie.get(8).ok_or(FontError::Truncated)? >> 3) as usize;
    let mut p = 8 + (5 + 4 * nbits).div_ceil(8) + 4;
    let mut fonts = Vec::new();
    while p + 2 <= movie.len() {
        let cl = u16_at(movie, p)?;
        p += 2;
        let code = cl >> 6;
        let mut len = (cl & 0x3f) as usize;
        if len == 0x3f {
            len = u32_at(movie, p)? as usize;
            p += 4;
        }
        if code == 0 {
            break;
        }
        let body = movie.get(p..p + len).ok_or(FontError::Truncated)?;
        if code == 75 {
            fonts.push(define_font3(body)?);
        }
        p += len;
    }
    Ok(fonts)
}

/// The font named `name` (DefineFont3's font name, e.g. "Blender Pro Bold").
pub fn find_font(movie: &[u8], name: &str) -> Result<Font, FontError> {
    parse_fonts(movie)?
        .into_iter()
        .find(|f| f.name == name)
        .ok_or_else(|| FontError::Missing(name.into()))
}

// ── Rendering ───────────────────────────────────────────────────────────────

impl Font {
    /// `text` rendered white on black: coverage 0..1, cropped to its ink,
    /// at `px` pixels per em, widened by `stretch`. Unknown characters are
    /// skipped (their advance too).
    pub fn render(&self, text: &str, px: f32, stretch: f32) -> Gray {
        let s = px / EM;
        let mut edges: Vec<(Pt, Pt)> = Vec::new();
        let mut pen = 0.0f32;
        let mut prev: Option<char> = None;
        for ch in text.chars() {
            let Some(g) = self.glyphs.get(&ch) else {
                continue;
            };
            if let Some(p) = prev {
                pen += self.kerning.get(&(p, ch)).copied().unwrap_or(0.0);
            }
            for c in &g.contours {
                for k in 0..c.len() {
                    let a = c[k];
                    let b = c[(k + 1) % c.len()];
                    edges.push((
                        ((a.0 + pen) * s * stretch, (a.1 + self.ascent) * s),
                        ((b.0 + pen) * s * stretch, (b.1 + self.ascent) * s),
                    ));
                }
            }
            pen += g.advance;
            prev = Some(ch);
        }
        let w = ((pen * s * stretch).ceil() as usize + 4).max(1);
        let h = (((self.ascent + self.descent) * s).ceil() as usize + 4).max(1);
        let img = fill(&edges, w, h);
        crop_ink(&img)
    }
}

/// Even-odd fill with 4x4 supersampling: coverage per pixel.
fn fill(edges: &[(Pt, Pt)], w: usize, h: usize) -> Gray {
    const SS: usize = 4;
    let mut acc = vec![0.0f32; w * h];
    let mut xs: Vec<f32> = Vec::new();
    for sy in 0..h * SS {
        let y = (sy as f32 + 0.5) / SS as f32;
        xs.clear();
        for &((x0, y0), (x1, y1)) in edges {
            if (y0 <= y && y < y1) || (y1 <= y && y < y0) {
                xs.push(x0 + (y - y0) * (x1 - x0) / (y1 - y0));
            }
        }
        xs.sort_by(|a, b| a.total_cmp(b));
        let row = sy / SS;
        for pair in xs.as_chunks::<2>().0 {
            // span [a, b) in pixels; add coverage per sub-row
            let (a, b) = (pair[0].max(0.0), pair[1].min(w as f32));
            if b <= a {
                continue;
            }
            let (ia, ib) = (a.floor() as usize, (b.ceil() as usize).min(w));
            for x in ia..ib {
                let lo = a.max(x as f32);
                let hi = b.min(x as f32 + 1.0);
                if hi > lo {
                    acc[row * w + x] += (hi - lo) / SS as f32;
                }
            }
        }
    }
    Gray {
        w,
        h,
        v: acc.into_iter().map(|v| v.min(1.0)).collect(),
    }
}

fn crop_ink(img: &Gray) -> Gray {
    let (mut x0, mut y0, mut x1, mut y1) = (img.w, img.h, 0, 0);
    for y in 0..img.h {
        for x in 0..img.w {
            if img.get(x, y) > 0.02 {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    if x1 <= x0 {
        return Gray::new(1, 1);
    }
    let mut v = Vec::with_capacity((x1 - x0) * (y1 - y0));
    for y in y0..y1 {
        for x in x0..x1 {
            v.push(img.get(x, y));
        }
    }
    Gray {
        w: x1 - x0,
        h: y1 - y0,
        v,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bits_read_signed_values() {
        let mut b = Bits::new(&[0b1110_0000]);
        assert_eq!(b.sb(3).unwrap(), -1);
        let mut b = Bits::new(&[0b0110_0000]);
        assert_eq!(b.sb(3).unwrap(), 3);
    }

    #[test]
    fn a_square_fills_whole() {
        let edges = [
            ((1.0, 1.0), (3.0, 1.0)),
            ((3.0, 1.0), (3.0, 3.0)),
            ((3.0, 3.0), (1.0, 3.0)),
            ((1.0, 3.0), (1.0, 1.0)),
        ];
        let g = fill(&edges, 4, 4);
        assert!((g.get(1, 1) - 1.0).abs() < 1e-4 && (g.get(2, 2) - 1.0).abs() < 1e-4);
        assert_eq!(g.get(0, 0), 0.0);
    }

    /// Against the real movie: `STARLUME_FONTS_GFX` = path to fonts_en.gfx.
    #[test]
    #[ignore = "needs fonts_en.gfx — set STARLUME_FONTS_GFX"]
    fn reads_blender_pro_bold() {
        let path = std::env::var("STARLUME_FONTS_GFX").expect("STARLUME_FONTS_GFX");
        let movie = std::fs::read(path).unwrap();
        let fonts = parse_fonts(&movie).unwrap();
        assert_eq!(fonts.len(), 29);
        let f = find_font(&movie, "Blender Pro Bold").unwrap();
        for c in "0123456789.%ABCDEFGHIJKLMNOPQRSTUVWXYZ()".chars() {
            assert!(f.glyphs.contains_key(&c), "missing {c:?}");
        }
        let g = f.render("6393", 17.0, 1.0);
        assert!(g.w > 20 && g.h > 8, "{g:?}");
        let png: Vec<u8> = g.v.iter().map(|v| (v * 255.0) as u8).collect();
        let img = image::GrayImage::from_raw(g.w as u32, g.h as u32, png).unwrap();
        img.save(std::env::temp_dir().join("starlume-font-6393.png"))
            .unwrap();
    }
}
