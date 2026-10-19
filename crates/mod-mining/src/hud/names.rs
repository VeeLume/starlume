//! Names on the panel (the rock, the composition rows, the difficulty),
//! matched against the names the game can show, rendered in the HUD's own
//! font.
//!
//! The candidates come from the localisation the game actually loads (a
//! user's language pack or Starlume's text patching included): every ore
//! the ship ruleset offers, the inert row and the difficulty labels, each
//! uppercased as the HUD draws them. The font (Blender Pro Bold) is read
//! from the user's own `fonts_en.gfx`. Each candidate is rendered once at
//! the reference scale and slid over the name band; the best match names
//! it, and the runner-up stays for the cross-checks.

use svc_capture::gfxfont::Font;
use svc_capture::vision::Gray;
use svc_data::LocaleMap;

use crate::catalog::Catalog;

/// Rendering that matches the captures: em size (reference pixels) and
/// glow blur, calibrated on normal captures.
const PX: f32 = 17.0;
const BLUR: f32 = 0.6;

/// The localisation keys of the panel's fixed words.
const INERT_KEY: &str = "hud_mining_empty_elements";
const DIFFICULTY_KEYS: [&str; 5] = [
    "ui_Mining_MiningLaser_DifficultyForecast_Easy",
    "ui_Mining_MiningLaser_DifficultyForecast_Fair",
    "ui_Mining_MiningLaser_DifficultyForecast_Challenging",
    "ui_Mining_MiningLaser_DifficultyForecast_Extreme",
    "ui_Mining_MiningLaser_DifficultyForecast_Impossible",
];

#[derive(Debug, Clone, PartialEq)]
pub enum NameKind {
    /// An ore (catalog element id).
    Element(String),
    Inert,
    Difficulty,
}

pub struct NameEntry {
    pub kind: NameKind,
    /// As the HUD shows it (uppercased).
    pub text: String,
    t: Gray,
}

/// The names the panel can show, rendered.
pub struct NameSet {
    entries: Vec<NameEntry>,
}

/// A band's best name and its runner-up (indices into the set), with the
/// score gap between them.
#[derive(Debug, Clone, Copy)]
pub struct NameMatch {
    pub best: usize,
    pub score: f32,
    pub alt: Option<usize>,
    pub margin: f32,
}

impl NameSet {
    /// Render every candidate: the catalog's ores (by their localisation
    /// key, through `locale`), the inert row and the difficulty labels.
    pub fn build(font: &Font, catalog: &Catalog, locale: &LocaleMap) -> Self {
        let mut entries = Vec::new();
        let mut push = |kind: NameKind, text: String| {
            let up = text.trim().to_uppercase();
            if up.is_empty() {
                return;
            }
            let t = font.render(&up, PX, 1.0).blur(BLUR).padded(1);
            entries.push(NameEntry { kind, text: up, t });
        };
        for (id, key) in &catalog.element_keys {
            let shown = key
                .as_deref()
                .and_then(|k| locale.resolve_name(k))
                .filter(|t| !t.starts_with('@'))
                .map(str::to_string)
                .or_else(|| {
                    catalog
                        .element_names
                        .iter()
                        .find(|e| &e.id == id)
                        .map(|e| e.name.clone())
                });
            if let Some(text) = shown {
                push(NameKind::Element(id.clone()), text);
            }
        }
        push(
            NameKind::Inert,
            locale
                .resolve_name(INERT_KEY)
                .unwrap_or("Inert Materials")
                .to_string(),
        );
        for key in DIFFICULTY_KEYS {
            if let Some(t) = locale.resolve_name(key) {
                push(NameKind::Difficulty, t.to_string());
            }
        }
        Self { entries }
    }

    pub fn get(&self, i: usize) -> &NameEntry {
        &self.entries[i]
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The best candidate (of those `want` accepts) for a name band
    /// (brightness ink at the reference scale).
    pub fn best(&self, band: &Gray, want: impl Fn(&NameKind) -> bool) -> Option<NameMatch> {
        let mut scored: Vec<(usize, f32)> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| want(&e.kind))
            .map(|(i, e)| (i, slide(band, &e.t)))
            .collect();
        scored.sort_by(|a, b| b.1.total_cmp(&a.1));
        let (best, score) = *scored.first()?;
        let alt = scored.get(1).copied();
        Some(NameMatch {
            best,
            score,
            alt: alt.map(|a| a.0),
            margin: alt.map_or(1.0, |a| score - a.1),
        })
    }
}

/// Best NCC of `t` anywhere in `band`; a template wider or taller than the
/// band is compared by the part that fits.
fn slide(band: &Gray, t: &Gray) -> f32 {
    let (tw, th) = (t.w.min(band.w), t.h.min(band.h));
    let n = (tw * th) as f32;
    let (mut st, mut stt) = (0.0f32, 0.0f32);
    for y in 0..th {
        for x in 0..tw {
            let v = t.get(x, y);
            st += v;
            stt += v * v;
        }
    }
    let vt = stt - st * st / n;
    if vt <= 1e-6 {
        return 0.0;
    }
    let mut best = f32::MIN;
    for oy in 0..=band.h - th {
        for ox in 0..=band.w - tw {
            let (mut si, mut sii, mut sit) = (0.0f32, 0.0f32, 0.0f32);
            for y in 0..th {
                let row = (oy + y) * band.w + ox;
                for x in 0..tw {
                    let a = band.v[row + x];
                    si += a;
                    sii += a * a;
                    sit += a * t.v[y * t.w + x];
                }
            }
            let vi = sii - si * si / n;
            if vi > 1e-6 {
                best = best.max((sit - si * st / n) / (vi * vt).sqrt());
            }
        }
    }
    best.max(0.0)
}
