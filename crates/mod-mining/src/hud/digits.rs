//! Reading a number from a field band (brightness ink, reference scale),
//! with svc-glyph's matcher: the field's [`Format`] is its grammar, the
//! mean glyph templates place the glyphs, and the digit exemplars name
//! them (the HUD font tells 0, 6, 8, 9 apart by one- or two-pixel holes,
//! which mean templates blur away). The runner-up is kept for the
//! cross-checks.

use svc_capture::vision::Gray;
use svc_glyph::{Grammar, Params, Slot};

use super::data::data;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Format {
    Int,
    IntPct,
    Dec,
    DecPct,
}

impl Format {
    fn grammar(self) -> Grammar {
        const D: &str = "0123456789";
        Grammar(match self {
            Format::Int => vec![Slot::new(D, 1, 6)],
            Format::IntPct => vec![Slot::new(D, 1, 3), Slot::new("%", 1, 1)],
            Format::Dec => vec![Slot::new(D, 1, 3), Slot::new(".", 1, 1), Slot::new(D, 2, 2)],
            Format::DecPct => vec![
                Slot::new(D, 1, 3),
                Slot::new(".", 1, 1),
                Slot::new(D, 2, 2),
                Slot::new("%", 1, 1),
            ],
        })
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

/// Read the band as `fmt`; `None` when no string of the format fits.
pub fn read(band: &Gray, fmt: Format) -> Option<Number> {
    let read = svc_glyph::read(band, &data().digits, &fmt.grammar(), &Params::default())?;
    let glyphs = read
        .iter()
        .map(|p| {
            let alt = p.runner_up();
            Glyph {
                ch: p.ch(),
                alt: alt.map(|a| a.0),
                margin: alt.map_or(1.0, |a| a.1),
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
