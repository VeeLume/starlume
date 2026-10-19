//! Reading the ship's SCAN RESULTS panel from a captured frame.
//!
//! The HUD is drawn in the world, so the panel sits at a slightly different
//! place, size and tilt in every shot (head position, head tracking, ship
//! drift), over any background (snow, glare, space). The reader:
//!
//! 1. **Features.** From the brightest channel: brightness ink (a ramp high
//!    up, 238..252: the glow stays out of the glyphs' holes) and shadow ink
//!    (grey closing minus the channel: the glyphs' dark outline and holes,
//!    which survive glare).
//! 2. **Locate** ([`locate`]): four panel parts vote for the panel's
//!    position and scale; if no three agree, the frame is tried tilted
//!    ±8° and ±16° (head tracking). The MASS labels then pin the origin and
//!    COMPOSITION the scale at full resolution.
//! 3. **Fields.** Every field sits at a fixed offset from the MASS label,
//!    in reference pixels (2560x1440); bands are resampled to the reference
//!    scale. The first composition row is found from the ink under the
//!    bar, the rest follow at the row pitch.
//! 4. **Numbers** ([`digits`]): templates place the glyphs within the
//!    field's format; the nearest labelled example names each digit.
//! 5. **Names** ([`names`]): the names the game can show (its active
//!    localisation), rendered in its own HUD font, matched to the bands.
//! 6. **Checks** ([`check`]): the composition adds up to 100 %, and the
//!    resistance and instability match what the model predicts from mass
//!    and composition; failing that, runner-up digits are tried.
//!
//! Every field is optional: what can't be read is left out and noted;
//! what no check confirmed is listed as unsure.

mod check;
mod data;
mod digits;
mod locate;
mod names;

use serde::Serialize;
use svc_capture::vision::{Gray, rotate_frame};
use svc_capture::{Frame, Rect};

pub use check::Basis;
pub use data::{load as load_scan_data, loaded as scan_data_loaded};
use digits::{Format, Number};
pub use names::{NameKind, NameSet};

use crate::catalog::Catalog;
use crate::solve::{EntryInput, LoadoutInput, RockInput, scanner_values};

/// What the panel showed. Percentages are 0..100.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ScanReading {
    /// Where the panel was found.
    pub panel: Option<PanelSpot>,
    pub rock_name: Option<String>,
    pub mass: Option<f32>,
    pub resistance: Option<f32>,
    pub instability: Option<f32>,
    pub difficulty: Option<String>,
    pub rows: Vec<ScanRow>,
    /// Which scanner values the resistance and instability are (the bare
    /// rock, or under the calculator's loadout's beam), when the model
    /// check placed them.
    pub basis: Option<Basis>,
    /// Checks that passed, in words.
    pub verified: Vec<String>,
    /// Fields no check confirmed ("mass", "row 2 %", …).
    pub unsure: Vec<String>,
    /// What could not be read or was corrected, in words.
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct PanelSpot {
    /// The MASS label's top-left, in the (tilted) frame's pixels.
    pub x: f32,
    pub y: f32,
    /// HUD scale against the reference captures (2560x1440).
    pub scale: f32,
    /// The vote: summed match of the agreeing parts, and how many agreed.
    pub score: f32,
    pub parts: u8,
    /// The frame was tilted by this much (degrees) to read the panel
    /// upright; 0 without head tracking.
    pub angle: f32,
}

/// A composition row.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ScanRow {
    pub percent: Option<f32>,
    /// The name as read.
    pub name: Option<String>,
    /// The catalog element it matched (id); `None` for inert material or
    /// an unmatched name.
    pub element: Option<String>,
    /// A close second match for the name (the model check may pick it),
    /// and how much worse it matched.
    #[serde(skip)]
    pub element_alt: Option<String>,
    #[serde(skip)]
    pub element_margin: f32,
    pub inert: bool,
    pub quality: Option<u16>,
}

// ── Layout, in reference pixels from the MASS label's top-left ─────────────

/// (left, top, right, bottom)
type Band = (f32, f32, f32, f32);
const MASS: Band = (125.0, -4.0, 215.0, 22.0);
const RESIST: Band = (125.0, 24.0, 190.0, 50.0);
const INSTAB: Band = (125.0, 52.0, 200.0, 78.0);
const ROCK_NAME: Band = (-6.0, -32.0, 230.0, -8.0);
const DIFFICULTY: Band = (20.0, 78.0, 230.0, 100.0);
/// Where the first composition row's text usually starts, and the pitch.
const ROW0: f32 = 193.0;
const ROW_PITCH: f32 = 22.3;
const MAX_ROWS: usize = 7;
const ROW_PCT: Band = (-8.0, 0.0, 75.0, 20.0);
const ROW_NAME: Band = (64.0, -2.0, 205.0, 20.0);
const ROW_QUAL: Band = (205.0, 0.0, 262.0, 20.0);
/// Head-tracking tilts tried when the upright frame has no panel.
const TILTS: [f32; 4] = [-8.0, 8.0, -16.0, 16.0];
/// A name band matches nothing below this score.
const NAME_MIN: f32 = 0.35;
/// A name whose runner-up is within this score gap is ambiguous.
const NAME_CLEAR: f32 = 0.03;

/// A band of `img` at `spot`, one sample per reference pixel.
fn patch(img: &Gray, spot: &PanelSpot, band: Band) -> Gray {
    let (l, t, r, b) = band;
    let (w, h) = ((r - l) as usize, (b - t) as usize);
    let mut v = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            v.push(img.at(
                spot.x + (l + x as f32) * spot.scale,
                spot.y + (t + y as f32) * spot.scale,
            ));
        }
    }
    Gray { w, h, v }
}

/// The frame rectangle of `band` (for OCR crops).
fn frame_rect(spot: &PanelSpot, band: Band, frame: &Frame) -> Rect {
    let (l, t, r, b) = band;
    let x0 = (spot.x + l * spot.scale).max(0.0) as u32;
    let y0 = (spot.y + t * spot.scale).max(0.0) as u32;
    let x1 = ((spot.x + r * spot.scale) as u32).min(frame.width);
    let y1 = ((spot.y + b * spot.scale) as u32).min(frame.height);
    Rect {
        x: x0,
        y: y0,
        w: x1.saturating_sub(x0),
        h: y1.saturating_sub(y0),
    }
}

/// The panel, cut out of `frame` (tilted as the reader read it): previews.
pub fn panel_crop(frame: &Frame, spot: &PanelSpot) -> Frame {
    let bottom = ROW0 + MAX_ROWS as f32 * ROW_PITCH;
    let band = (-24.0, -80.0, 268.0, bottom);
    if spot.angle == 0.0 {
        frame.crop(frame_rect(spot, band, frame))
    } else {
        let tilted = rotate_frame(frame, spot.angle);
        tilted.crop(frame_rect(spot, band, &tilted))
    }
}

/// The first composition row's text top (reference px from the MASS
/// label), from the ink row profile of the percent column under the bar.
fn first_row(ink: &Gray, spot: &PanelSpot) -> Option<f32> {
    let p = patch(ink, spot, (-8.0, 180.0, 75.0, 215.0));
    let thr = (0.08 * p.w as f32).max(2.0);
    let mut start = None;
    for y in 0..=p.h {
        let sum: f32 = if y < p.h {
            p.v[y * p.w..(y + 1) * p.w].iter().sum()
        } else {
            0.0
        };
        match (sum > thr, start) {
            (true, None) => start = Some(y),
            (false, Some(s0)) => {
                if y - s0 >= 7 {
                    let top = 180.0 + s0 as f32;
                    return (top < 205.0).then_some(top);
                }
                start = None;
            }
            _ => {}
        }
    }
    None
}

/// Find the panel: upright first, then tilted for head tracking. Returns
/// the frame it was found in (tilted or not) with its features.
fn find(frame: &Frame) -> Option<(Frame, locate::Features, PanelSpot)> {
    let f = locate::Features::of(frame);
    if let Some(spot) = locate::vote(&f, frame.height) {
        let spot = locate::refine(&f, spot);
        return Some((frame.clone(), f, spot));
    }
    for angle in TILTS {
        let tilted = rotate_frame(frame, angle);
        let f = locate::Features::of(&tilted);
        if let Some(spot) = locate::vote(&f, tilted.height) {
            let spot = PanelSpot {
                angle,
                ..locate::refine(&f, spot)
            };
            return Some((tilted, f, spot));
        }
    }
    None
}

/// Read the SCAN RESULTS panel in `frame`. `names` are the names the game
/// can show, rendered in its font ([`NameSet::build`]). With the
/// calculator's loadout, the model check also recognises beam-on values.
/// Blocking (template search): call it off the UI thread.
pub fn read_scan(
    frame: &Frame,
    catalog: &Catalog,
    names: &NameSet,
    loadout: Option<&LoadoutInput>,
) -> ScanReading {
    let mut out = ScanReading::default();
    if !data::loaded() {
        out.notes
            .push("the HUD reader's data isn't on this machine yet".into());
        return out;
    }
    let Some((_tilted, f, spot)) = find(frame) else {
        out.notes
            .push("no SCAN RESULTS panel found (mining mode, panel in view?)".into());
        return out;
    };
    out.panel = Some(spot);
    let num = |band: Band, fmt: Format| digits::read(&patch(&f.ink, &spot, band), fmt);

    let mass = num(MASS, Format::Int);
    let resist = num(RESIST, Format::IntPct);
    let instab = num(INSTAB, Format::Dec);
    out.mass = mass.as_ref().and_then(Number::value);
    out.resistance = resist.as_ref().and_then(Number::value);
    out.instability = instab.as_ref().and_then(Number::value);
    // The rock is named after its main ore; the difficulty is one of the
    // forecast labels.
    out.rock_name = names
        .best(&patch(&f.ink, &spot, ROCK_NAME), |k| {
            matches!(k, NameKind::Element(_))
        })
        .filter(|m| m.score >= NAME_MIN)
        .map(|m| names.get(m.best).text.clone());
    out.difficulty = names
        .best(&patch(&f.ink, &spot, DIFFICULTY), |k| {
            *k == NameKind::Difficulty
        })
        .filter(|m| m.score >= NAME_MIN)
        .map(|m| names.get(m.best).text.clone());
    for (field, v) in [
        ("mass", out.mass),
        ("resistance", out.resistance),
        ("instability", out.instability),
    ] {
        if v.is_none() {
            out.notes.push(format!("couldn't read the {field}"));
        }
    }

    // Composition rows.
    let row0 = first_row(&f.ink, &spot).unwrap_or(ROW0);
    let mut pct_numbers: Vec<Option<Number>> = Vec::new();
    let mut total = 0.0;
    for i in 0..MAX_ROWS {
        let dy = row0 - 6.0 + i as f32 * ROW_PITCH;
        let shift = |b: Band| (b.0, b.1 + dy, b.2, b.3 + dy);
        let pct = num(shift(ROW_PCT), Format::DecPct);
        let percent = pct.as_ref().and_then(Number::value);
        // Every row has a percentage; what follows the list (a world
        // marker's distance, the LOCK line) doesn't.
        if percent.is_none() {
            break;
        }
        // The name: the best rendered ore name (or the inert row); a close
        // runner-up stays for the model check to decide.
        let m = names
            .best(&patch(&f.ink, &spot, shift(ROW_NAME)), |k| {
                !matches!(k, NameKind::Difficulty)
            })
            .filter(|m| m.score >= NAME_MIN);
        let quality = num(shift(ROW_QUAL), Format::Int)
            .and_then(|n| n.value())
            .map(|q| q as u16);
        let kind = m.map(|m| names.get(m.best).kind.clone());
        // Inert material always shows quality 0.
        let inert = kind == Some(NameKind::Inert) || quality == Some(0);
        let element = match (&kind, inert) {
            (Some(NameKind::Element(id)), false) => Some(id.clone()),
            _ => None,
        };
        let element_alt = m
            .filter(|m| m.margin < NAME_CLEAR && !inert)
            .and_then(|m| m.alt)
            .and_then(|a| match &names.get(a).kind {
                NameKind::Element(id) => Some(id.clone()),
                _ => None,
            });
        let element_margin = m.map_or(1.0, |m| m.margin);
        let name = m.map(|m| names.get(m.best).text.clone());
        let quality = if inert { None } else { quality };
        if !inert && element.is_none() {
            out.notes.push(format!(
                "row {}: the ore name didn't match any the game shows",
                i + 1
            ));
        }
        let has_ore = out.rows.iter().any(|r| !r.inert);
        out.rows.push(ScanRow {
            percent,
            name,
            element,
            element_alt,
            element_margin,
            inert,
            quality,
        });
        pct_numbers.push(pct);
        // The inert row closes the list once ores were listed (while a scan
        // is still filling it comes first).
        total += percent.unwrap_or(0.0);
        if (inert && has_ore) || total >= 101.0 {
            break;
        }
    }
    if out.rows.is_empty() {
        out.notes.push("no composition rows read".into());
    }

    // Check 1: the composition adds up.
    let fix = check::composition(
        &pct_numbers
            .iter()
            .map(|n| check::RowPct { number: n.as_ref() })
            .collect::<Vec<_>>(),
    );
    for (i, p) in fix.percents.iter().enumerate() {
        if let Some(row) = out.rows.get_mut(i)
            && row.percent != *p
        {
            if fix.changed.contains(&i) {
                out.notes.push(format!(
                    "row {} read {} and corrected to {} so the composition adds up to 100 %",
                    i + 1,
                    row.percent.map_or("?".into(), |v| format!("{v:.2} %")),
                    p.map_or("?".into(), |v| format!("{v:.2} %"))
                ));
            }
            row.percent = *p;
        }
    }
    if let Some(i) = fix.derived {
        out.notes
            .push(format!("row {} is the remainder to 100 %", i + 1));
    }
    if fix.agrees && !out.rows.is_empty() {
        out.verified.push("the composition adds up to 100 %".into());
    } else {
        for i in 0..out.rows.len() {
            out.unsure.push(format!("row {} %", i + 1));
        }
    }

    // Check 2: resistance and instability follow from mass and composition.
    model_check(
        &mut out,
        catalog,
        loadout,
        mass.as_ref(),
        resist.as_ref(),
        instab.as_ref(),
    );
    out
}

fn rock_input(mass: f32, rows: &[ScanRow]) -> Option<RockInput> {
    let entries: Vec<EntryInput> = rows
        .iter()
        .filter(|r| !r.inert)
        .map(|r| {
            Some(EntryInput {
                element: r.element.clone()?,
                percent: r.percent?,
                quality: r.quality,
            })
        })
        .collect::<Option<_>>()?;
    (!entries.is_empty() && mass > 0.0).then_some(RockInput {
        mass,
        entries,
        surface: None,
    })
}

fn model_check(
    out: &mut ScanReading,
    catalog: &Catalog,
    loadout: Option<&LoadoutInput>,
    mass: Option<&Number>,
    resist: Option<&Number>,
    instab: Option<&Number>,
) {
    let (Some(mass), Some(resist), Some(instab)) = (mass, resist, instab) else {
        out.unsure
            .extend(["mass", "resistance", "instability"].map(String::from));
        return;
    };
    let alts = |n: &Number, depth: usize| -> Vec<(f32, f32)> {
        n.alternatives(depth)
            .into_iter()
            .filter_map(|(t, c)| t.trim_end_matches('%').parse::<f32>().ok().map(|v| (v, c)))
            .take(8)
            .collect()
    };
    // Rows whose ore didn't read: every catalog element is tried there (up
    // to two such rows); an element is only taken when it alone makes the
    // numbers agree.
    // Rows whose ore is open: unread (every catalog element is tried) or
    // ambiguous (the best name and its runner-up). An element is only
    // taken when it alone makes the numbers agree.
    let ids: Vec<String> = catalog.element_names.iter().map(|e| e.id.clone()).collect();
    // (row, choices with their cost: the runner-up name costs its margin)
    let open: Vec<(usize, Vec<(String, f32)>)> = out
        .rows
        .iter()
        .enumerate()
        .filter(|(_, r)| !r.inert)
        .filter_map(|(i, r)| match (&r.element, &r.element_alt) {
            (None, _) => Some((i, ids.iter().map(|id| (id.clone(), 0.0)).collect())),
            (Some(a), Some(b)) => Some((i, vec![(a.clone(), 0.0), (b.clone(), r.element_margin)])),
            _ => None,
        })
        .collect();
    let unread = out
        .rows
        .iter()
        .filter(|r| !r.inert && r.element.is_none())
        .count();
    if unread > 2 {
        out.notes
            .push("too many ores unread to check resistance and instability".into());
        out.unsure
            .extend(["mass", "resistance", "instability"].map(String::from));
        return;
    }
    let unknown: Vec<usize> = open.iter().map(|(i, _)| *i).collect();
    let mut combos: Vec<(Vec<String>, f32)> = vec![(vec![], 0.0)];
    for (_, choices) in &open {
        combos = combos
            .into_iter()
            .flat_map(|(c, cost)| {
                choices.iter().map(move |(e, ec)| {
                    let mut c = c.clone();
                    c.push(e.clone());
                    (c, cost + ec)
                })
            })
            .collect();
    }
    // The reading as it is wins when it already agrees: no correction then.
    if unread == 0
        && let (Some(m), Some(r), Some(i)) = (out.mass, out.resistance, out.instability)
        && let Some(rock) = rock_input(m, &out.rows)
        && let Ok(v) = scanner_values(catalog, loadout, &rock)
        && let Some(basis) = check::agrees((r, i), &v)
    {
        out.basis = Some(basis);
        out.verified.push(match basis {
            Basis::Bare => "resistance and instability match the composition (bare rock)".into(),
            Basis::Beam => {
                "resistance and instability match the composition (your beam on it)".into()
            }
        });
        return;
    }
    let mut best: Option<(f32, f32, f32, f32, Basis, Vec<String>)> = None;
    // how many different readings fit nearly as cheaply as the best
    let mut ties = 0usize;
    for (fill, fill_cost) in &combos {
        let mut rows = out.rows.clone();
        for (k, &i) in unknown.iter().enumerate() {
            rows[i].element = Some(fill[k].clone());
        }
        for (m, cm) in alts(mass, 2) {
            let Some(rock) = rock_input(m, &rows) else {
                continue;
            };
            let Ok(v) = scanner_values(catalog, loadout, &rock) else {
                continue;
            };
            for (r, cr) in alts(resist, 1) {
                for (i, ci) in alts(instab, 2) {
                    let cost = cm + cr + ci + fill_cost;
                    // Overruling confident digits isn't a fix, it's a guess.
                    if cm + cr + ci > check::MAX_FIX {
                        continue;
                    }
                    if let Some(basis) = check::agrees((r, i), &v) {
                        let same = |b: &(f32, f32, f32, f32, Basis, Vec<String>)| {
                            b.1 == m && b.2 == r && b.3 == i && b.5 == *fill
                        };
                        match &best {
                            Some(b) if same(b) => {}
                            Some(b) if cost < b.0 - check::AMBIGUOUS => {
                                best = Some((cost, m, r, i, basis, fill.clone()));
                                ties = 0;
                            }
                            Some(b) if cost < b.0 + check::AMBIGUOUS => {
                                ties += 1;
                                if cost < b.0 {
                                    best = Some((cost, m, r, i, basis, fill.clone()));
                                }
                            }
                            Some(_) => {}
                            None => best = Some((cost, m, r, i, basis, fill.clone())),
                        }
                    }
                }
            }
        }
    }
    if ties > 0 {
        out.notes.push(
            "several readings fit the numbers equally well: check mass, resistance and instability"
                .into(),
        );
        out.unsure
            .extend(["mass", "resistance", "instability"].map(String::from));
        return;
    }
    if !unknown.is_empty() {
        match &best {
            Some(b) => {
                for (k, &i) in unknown.iter().enumerate() {
                    out.rows[i].element = Some(b.5[k].clone());
                    if out.rows[i].element.as_ref() != Some(&b.5[k]) {
                        out.notes.push(format!(
                            "row {}: {} is the ore that fits the numbers",
                            i + 1,
                            catalog
                                .element_names
                                .iter()
                                .find(|e| e.id == b.5[k])
                                .map_or("?", |e| e.name.as_str())
                        ));
                    }
                }
            }
            _ => {
                out.notes
                    .push("an ore name didn't read and the numbers don't single one out".into());
                out.unsure
                    .extend(["mass", "resistance", "instability"].map(String::from));
                return;
            }
        }
    }
    let best = best.map(|b| (b.0, b.1, b.2, b.3, b.4));
    match best {
        Some((_, m, r, i, basis)) => {
            for (field, read, now) in [
                ("mass", out.mass, m),
                ("resistance", out.resistance, r),
                ("instability", out.instability, i),
            ] {
                if read != Some(now) {
                    out.notes.push(format!(
                        "{field} read {} and corrected to {now} to match the composition",
                        read.map_or("?".into(), |v| v.to_string())
                    ));
                }
            }
            out.mass = Some(m);
            out.resistance = Some(r);
            out.instability = Some(i);
            out.basis = Some(basis);
            out.verified.push(match basis {
                Basis::Bare => {
                    "resistance and instability match the composition (bare rock)".into()
                }
                Basis::Beam => {
                    "resistance and instability match the composition (your beam on it)".into()
                }
            });
        }
        None => {
            out.notes.push("resistance and instability don't match the composition: check the mass and the rows".into());
            out.unsure
                .extend(["mass", "resistance", "instability"].map(String::from));
        }
    }
}
