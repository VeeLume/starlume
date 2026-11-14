//! One materials ledger across every plan.
//!
//! Each stack you hold is counted once. Demands are served in plan order, so
//! what the first plan reserves is gone for the second. A demand takes the
//! lowest quality that still qualifies first: the better stock stays free
//! for a later demand that needs it.

use serde::Serialize;

/// Amounts closer than this are the same amount (SCU sums of f32 recipe
/// figures do not land exactly).
const EPS: f64 = 1e-6;

/// One stack of a material you hold.
#[derive(Debug, Clone, PartialEq)]
pub struct Stack {
    pub material: String,
    /// 0..=1000.
    pub quality: u32,
    /// SCU for bulk, pieces else.
    pub amount: f64,
    pub bulk: bool,
    pub place: String,
}

/// What one plan needs of one material. Demands arrive in plan order.
#[derive(Debug, Clone, PartialEq)]
pub struct Demand {
    pub plan: String,
    pub material: String,
    pub min_quality: u32,
    pub amount: f64,
    pub bulk: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Ledger {
    /// One row per material and wanted quality, in the order first asked for.
    pub rows: Vec<LedgerRow>,
    /// One entry per demand, in the order given.
    pub fills: Vec<Fill>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LedgerRow {
    pub material: String,
    pub min_quality: u32,
    pub bulk: bool,
    /// What every plan together needs at this quality.
    pub need: f64,
    /// What of your stock is reserved for it; never above `need`.
    pub held: f64,
    /// The plans that ask for it, in plan order.
    pub plans: Vec<String>,
}

impl LedgerRow {
    pub fn short(&self) -> f64 {
        (self.need - self.held).max(0.0)
    }
}

/// How far one demand is covered, and from where.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Fill {
    pub reserved: f64,
    pub short: f64,
    /// Where the reserved stock sits, largest share first.
    pub places: Vec<(String, f64)>,
}

impl Fill {
    pub fn covered(&self) -> bool {
        self.short <= EPS
    }
}

pub fn reserve(stock: &[Stack], demands: &[Demand]) -> Ledger {
    // Remaining amounts, lowest quality first per material.
    let mut left: Vec<(usize, f64)> = stock
        .iter()
        .enumerate()
        .filter(|(_, s)| s.amount > EPS)
        .map(|(i, s)| (i, s.amount))
        .collect();
    left.sort_by_key(|(i, _)| stock[*i].quality);

    let mut ledger = Ledger::default();
    for d in demands {
        let mut want = d.amount.max(0.0);
        let mut fill = Fill::default();
        for (i, rest) in &mut left {
            if want <= EPS {
                break;
            }
            let s = &stock[*i];
            if *rest <= EPS
                || s.material != d.material
                || s.bulk != d.bulk
                || s.quality < d.min_quality
            {
                continue;
            }
            let take = rest.min(want);
            *rest -= take;
            want -= take;
            fill.reserved += take;
            match fill.places.iter_mut().find(|(p, _)| *p == s.place) {
                Some((_, a)) => *a += take,
                None => fill.places.push((s.place.clone(), take)),
            }
        }
        fill.short = if want <= EPS { 0.0 } else { want };
        fill.places.sort_by(|a, b| b.1.total_cmp(&a.1));

        let row = match ledger.rows.iter_mut().find(|r| {
            r.material == d.material && r.min_quality == d.min_quality && r.bulk == d.bulk
        }) {
            Some(row) => row,
            None => {
                ledger.rows.push(LedgerRow {
                    material: d.material.clone(),
                    min_quality: d.min_quality,
                    bulk: d.bulk,
                    need: 0.0,
                    held: 0.0,
                    plans: Vec::new(),
                });
                ledger.rows.last_mut().expect("just pushed")
            }
        };
        row.need += d.amount.max(0.0);
        row.held += fill.reserved;
        if !row.plans.contains(&d.plan) {
            row.plans.push(d.plan.clone());
        }
        ledger.fills.push(fill);
    }
    ledger
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(material: &str, quality: u32, amount: f64, place: &str) -> Stack {
        Stack {
            material: material.into(),
            quality,
            amount,
            bulk: true,
            place: place.into(),
        }
    }

    fn demand(plan: &str, material: &str, min_quality: u32, amount: f64) -> Demand {
        Demand {
            plan: plan.into(),
            material: material.into(),
            min_quality,
            amount,
            bulk: true,
        }
    }

    #[test]
    fn stock_is_counted_once_and_reserved_in_plan_order() {
        let stock = [stack("Ore A", 800, 6.0, "Depot One")];
        let ledger = reserve(
            &stock,
            &[
                demand("p1", "Ore A", 700, 4.0),
                demand("p2", "Ore A", 700, 4.0),
            ],
        );
        assert_eq!(ledger.fills[0].reserved, 4.0);
        assert!(ledger.fills[0].covered());
        assert_eq!(ledger.fills[1].reserved, 2.0);
        assert_eq!(ledger.fills[1].short, 2.0);
        // Both plans ask at the same quality: one row, 6 of 8 held.
        assert_eq!(ledger.rows.len(), 1);
        assert_eq!((ledger.rows[0].need, ledger.rows[0].held), (8.0, 6.0));
        assert_eq!(ledger.rows[0].plans, ["p1", "p2"]);
        assert_eq!(ledger.rows[0].short(), 2.0);
    }

    #[test]
    fn stock_below_the_wanted_quality_does_not_count() {
        let stock = [stack("Ore A", 650, 10.0, "Depot One")];
        let ledger = reserve(&stock, &[demand("p1", "Ore A", 700, 1.0)]);
        assert_eq!(ledger.fills[0].reserved, 0.0);
        assert_eq!(ledger.fills[0].short, 1.0);
    }

    #[test]
    fn the_lowest_quality_that_qualifies_goes_first() {
        // The first plan is happy with Q500; the Q900 stack must stay free
        // for the second, which needs Q850.
        let stock = [
            stack("Ore A", 900, 2.0, "Depot One"),
            stack("Ore A", 550, 2.0, "Depot Two"),
        ];
        let ledger = reserve(
            &stock,
            &[
                demand("p1", "Ore A", 500, 2.0),
                demand("p2", "Ore A", 850, 2.0),
            ],
        );
        assert!(ledger.fills[0].covered());
        assert_eq!(ledger.fills[0].places, [("Depot Two".to_owned(), 2.0)]);
        assert!(ledger.fills[1].covered());
        assert_eq!(ledger.rows.len(), 2);
    }

    #[test]
    fn a_demand_can_be_filled_from_several_places() {
        let stock = [
            stack("Ore A", 700, 1.0, "Depot One"),
            stack("Ore A", 720, 3.0, "Depot Two"),
        ];
        let ledger = reserve(&stock, &[demand("p1", "Ore A", 700, 3.5)]);
        assert!(ledger.fills[0].covered());
        assert_eq!(
            ledger.fills[0].places,
            [("Depot Two".to_owned(), 2.5), ("Depot One".to_owned(), 1.0)]
        );
    }

    #[test]
    fn pieces_and_bulk_of_one_name_are_kept_apart() {
        let mut gem = stack("Gem B", 600, 3.0, "Depot One");
        gem.bulk = false;
        let ledger = reserve(&[gem], &[demand("p1", "Gem B", 0, 1.0)]);
        assert_eq!(ledger.fills[0].reserved, 0.0);
    }

    #[test]
    fn recipe_fractions_that_sum_to_the_stock_are_covered() {
        let stock = [stack("Ore A", 500, 0.06, "Depot One")];
        let ledger = reserve(
            &stock,
            &[
                demand("p1", "Ore A", 500, f64::from(0.02_f32)),
                demand("p1", "Ore A", 500, f64::from(0.04_f32)),
            ],
        );
        assert!(ledger.fills.iter().all(Fill::covered));
    }
}
