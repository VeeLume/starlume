//! The wallet track: which balance samples are trusted, and which log event
//! explains a change.
//!
//! # A balance of 0
//!
//! The stream's balance is a plain integer with no "not loaded yet" state,
//! and what it reads while a character loads has not been observed against
//! a live game. So a 0 is **not trusted by itself**:
//!
//! - **0 before any balance is known** is held back. The opening balance is
//!   the first sample above 0, and [`Wallet::opened_after_zero`] says so.
//!   Cost: a wallet that really was empty opens on its first income, and
//!   `net` misses that income.
//! - **0 after a known balance** is held back too
//!   ([`Wallet::zero_since`] is set, the figures stay). When the balance
//!   comes back, the dip is forgotten: back to the same value records
//!   nothing, back to another value records one change from the old value
//!   to the new one. A drop to 0 and back is never a loss and a gain.
//! - The one way a 0 is trusted: a log event within
//!   [`ATTRIBUTION_WINDOW`] spends exactly the whole balance. Then the
//!   change to 0 is recorded with that event as its cause.
//!
//! `net` is right in every case but the first, because it is
//! `latest - opening` and both are trusted samples.
//!
//! # Attribution
//!
//! A change is explained by a log event at most [`ATTRIBUTION_WINDOW`]
//! away in time (either order: the stream is instant, the log is polled),
//! each event explaining one change. The pairing is worked out again
//! whenever an event or a change arrives, over everything still in the
//! window, so the order in which they arrive does not matter. Three passes,
//! each taking the pairs nearest in time first:
//!
//! 1. **The same amount.** An award, a fine, a purchase or a sale whose
//!    amount is the change.
//! 2. **Nearly the same amount**, same direction, within [`NEAR_PERCENT`]
//!    (a shop request carries the client's price, which the server may
//!    round).
//! 3. **An unknown amount**: a completed mission, which the log never
//!    prices. It gets only a *gain* that the first two passes left over,
//!    and only one seen no earlier than [`UNKNOWN_LEAD`] before it: a
//!    reward does not arrive before its mission ends. In the same pass, a
//!    vehicle terminal visit takes *every* spend left over from
//!    [`UNKNOWN_LEAD`] before it to [`TERMINAL_REACH`] after: what is paid
//!    there (repair, refuel, restock, claim) has no log line at all, and
//!    one visit often pays for several.
//!
//! Everything else is [`Cause::Untracked`]. Pass 3 is a guess by nature: a
//! mission that paid nothing can still be given an unrelated gain that
//! follows it within the window, and a terminal left open an unrelated
//! spend, if nothing with an amount explains it.

use std::collections::VecDeque;

use crate::model::{BalanceSample, Cause, MAX_WALLET_CHANGES, Wallet, WalletChange};
use crate::time::Millis;

/// How far apart a balance change and the log event explaining it may be.
pub const ATTRIBUTION_WINDOW: Millis = 30_000;
/// How far an event's amount may be off the change and still explain it.
pub const NEAR_PERCENT: i64 = 5;
/// How long before an event of unknown amount its gain may have been seen
/// (slack between the stream's arrival time and the log line's own).
pub const UNKNOWN_LEAD: Millis = 5_000;
/// How long after the vehicle terminal fetched your ships a spend may
/// still be paid there: the menu stays open while you pick services.
pub const TERMINAL_REACH: Millis = 120_000;
/// Most open explanations kept.
const MAX_EXPLANATIONS: usize = 64;
/// The attribution passes (see the module docs).
const PASSES: u8 = 3;

/// What a log event says about the money it moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Amount {
    /// Signed aUEC.
    Exact(i64),
    /// One gain of unknown size (a completed mission).
    Gain,
    /// Any number of spends of unknown size (a vehicle terminal visit).
    Spends,
}

/// A log event that moves money.
#[derive(Debug, Clone, Copy)]
struct Explanation {
    at: Millis,
    seq: u32,
    amount: Amount,
}

impl Explanation {
    /// How long after it a change may still be its.
    fn reach(&self) -> Millis {
        match self.amount {
            Amount::Spends => TERMINAL_REACH,
            _ => ATTRIBUTION_WINDOW,
        }
    }
}

/// The pass in which `e` may explain `change`; `None` = never.
fn pass_of(e: &Explanation, change: &WalletChange) -> Option<u8> {
    let after = change.at - e.at;
    if after > e.reach() || after < -ATTRIBUTION_WINDOW {
        return None;
    }
    let delta = change.delta;
    match e.amount {
        Amount::Exact(a) if a == delta => Some(0),
        Amount::Exact(a)
            if a.signum() == delta.signum()
                && (a - delta).abs() * 100 <= a.abs() * NEAR_PERCENT =>
        {
            Some(1)
        }
        Amount::Gain if delta > 0 && after >= -UNKNOWN_LEAD => Some(2),
        Amount::Spends if delta < 0 && after >= -UNKNOWN_LEAD => Some(2),
        _ => None,
    }
}

/// The log events that can still explain a balance change. Working state of
/// the reducer: rebuilt with the session, never stored.
#[derive(Debug, Default)]
pub(crate) struct Ledger {
    /// In arrival order, which is time order: a log is read front to back.
    explanations: VecDeque<Explanation>,
}

impl Ledger {
    fn knows(&self, seq: u32) -> bool {
        self.explanations.iter().any(|e| e.seq == seq)
    }

    /// Work the pairing out again for every change the open explanations
    /// can reach. Changes explained by an event that is no longer open keep
    /// their cause. Returns whether any cause changed.
    fn settle(&self, wallet: &mut Wallet) -> bool {
        let Some(oldest) = self.explanations.iter().map(|e| e.at).min() else {
            return false;
        };
        let horizon = oldest - ATTRIBUTION_WINDOW;
        let start = wallet
            .changes
            .iter()
            .rposition(|c| c.at < horizon)
            .map_or(0, |i| i + 1);
        let open = &mut wallet.changes[start..];
        let before: Vec<Cause> = open.iter().map(|c| c.cause).collect();

        for change in open.iter_mut() {
            if let Cause::Entry { seq } = change.cause
                && self.knows(seq)
            {
                change.cause = Cause::Untracked;
            }
        }
        let mut used = vec![false; self.explanations.len()];
        for pass in 0..PASSES {
            // (how far apart, explanation, change), nearest first.
            let mut pairs: Vec<(Millis, usize, usize)> = Vec::new();
            for (ei, e) in self.explanations.iter().enumerate() {
                if used[ei] {
                    continue;
                }
                for (ci, change) in open.iter().enumerate() {
                    if change.cause == Cause::Untracked && pass_of(e, change) == Some(pass) {
                        pairs.push(((change.at - e.at).abs(), ei, ci));
                    }
                }
            }
            pairs.sort_unstable();
            for (_, ei, ci) in pairs {
                if !used[ei] && open[ci].cause == Cause::Untracked {
                    let e = &self.explanations[ei];
                    open[ci].cause = Cause::Entry { seq: e.seq };
                    // A terminal visit pays for any number of services.
                    used[ei] = e.amount != Amount::Spends;
                }
            }
        }
        open.iter().map(|c| c.cause).ne(before)
    }

    /// An open event near `at` that spends exactly `balance` and explains
    /// no change yet.
    fn spends_all(&self, wallet: &Wallet, at: Millis, balance: i64) -> bool {
        self.explanations.iter().any(|e| {
            e.amount == Amount::Exact(-balance)
                && (e.at - at).abs() <= ATTRIBUTION_WINDOW
                && !wallet
                    .changes
                    .iter()
                    .any(|c| c.cause == (Cause::Entry { seq: e.seq }))
        })
    }

    /// A log event moved money: `amount` signed aUEC, `None` for a gain of
    /// unknown size. Returns whether the wallet changed.
    pub(crate) fn explain(
        &mut self,
        wallet: &mut Wallet,
        at: Millis,
        seq: u32,
        amount: Option<i64>,
    ) -> bool {
        let amount = amount.map_or(Amount::Gain, Amount::Exact);
        self.open(wallet, at, seq, amount)
    }

    /// The vehicle terminal fetched your ships (opened or refreshed):
    /// entry `seq` may explain the spends that follow. Returns whether the
    /// wallet changed.
    pub(crate) fn explain_spends(&mut self, wallet: &mut Wallet, at: Millis, seq: u32) -> bool {
        self.open(wallet, at, seq, Amount::Spends)
    }

    fn open(&mut self, wallet: &mut Wallet, at: Millis, seq: u32, amount: Amount) -> bool {
        // An event a window past its reach can explain no change that is
        // still to come: the stream is never behind the log.
        while self
            .explanations
            .front()
            .is_some_and(|e| e.at + e.reach() + ATTRIBUTION_WINDOW < at)
            || self.explanations.len() >= MAX_EXPLANATIONS
        {
            self.explanations.pop_front();
        }
        self.explanations.push_back(Explanation { at, seq, amount });

        // A held-back 0 that this event accounts for in full.
        let mut changed = false;
        if let (Some(zero_at), Some(latest)) = (wallet.zero_since, wallet.latest)
            && amount == Amount::Exact(-latest.balance)
            && (zero_at - at).abs() <= ATTRIBUTION_WINDOW
        {
            wallet.record(zero_at, 0);
            wallet.zero_since = None;
            changed = true;
        }
        self.settle(wallet) || changed
    }

    /// The request behind entry `seq` was rejected: it explains nothing.
    /// Returns whether the wallet changed.
    pub(crate) fn retract(&mut self, wallet: &mut Wallet, seq: u32) -> bool {
        self.explanations.retain(|e| e.seq != seq);
        let mut changed = false;
        for change in &mut wallet.changes {
            if change.cause == (Cause::Entry { seq }) {
                change.cause = Cause::Untracked;
                changed = true;
            }
        }
        // What it had taken may belong to another event.
        self.settle(wallet) || changed
    }
}

impl Wallet {
    /// Append an (as yet unexplained) change to `to` and move `latest`
    /// there.
    fn record(&mut self, at: Millis, to: i64) {
        let Some(latest) = self.latest else { return };
        self.changes.push(WalletChange {
            at,
            from: latest.balance,
            to,
            delta: to - latest.balance,
            cause: Cause::Untracked,
        });
        if self.changes.len() > MAX_WALLET_CHANGES {
            self.changes.remove(0);
            self.changes_dropped = self.changes_dropped.saturating_add(1);
        }
        self.latest = Some(BalanceSample { at, balance: to });
        self.net = to - self.opening.map_or(to, |o| o.balance);
    }

    /// Take a balance sample in (see the module docs for what is trusted).
    /// Returns whether the wallet changed.
    pub(crate) fn observe(&mut self, at: Millis, balance: i64, ledger: &mut Ledger) -> bool {
        let Some(latest) = self.latest else {
            if balance == 0 {
                // Not trusted as an opening balance.
                let first = self.zero_since.is_none();
                self.zero_since.get_or_insert(at);
                return first;
            }
            self.opened_after_zero = self.zero_since.take().is_some();
            let sample = BalanceSample { at, balance };
            self.opening = Some(sample);
            self.latest = Some(sample);
            return true;
        };

        if balance == latest.balance {
            // Unchanged, or back from a 0 that was never real.
            return self.zero_since.take().is_some();
        }
        if balance == 0 {
            if self.zero_since.is_some() {
                return false;
            }
            // Trusted only when a log event spent exactly everything.
            if ledger.spends_all(self, at, latest.balance) {
                self.record(at, 0);
                ledger.settle(self);
            } else {
                self.zero_since = Some(at);
            }
            return true;
        }

        self.zero_since = None;
        self.record(at, balance);
        ledger.settle(self);
        true
    }

    /// Continue this track with a later one of the same game run (the part
    /// kept from before the session was rebuilt from its log). Causes are
    /// dropped: they point at entries of a timeline that no longer exists,
    /// and the replayed log explains the changes again.
    pub(crate) fn append(&mut self, later: &Wallet) {
        let Some(their_opening) = later.opening else {
            self.zero_since = self.zero_since.or(later.zero_since);
            return;
        };
        if self.opening.is_none() {
            self.opening = Some(their_opening);
            self.latest = Some(their_opening);
            self.opened_after_zero = later.opened_after_zero;
        } else if self
            .latest
            .is_some_and(|l| l.balance != their_opening.balance)
        {
            // The balance moved while nothing was watching.
            self.record(their_opening.at, their_opening.balance);
        }
        for change in &later.changes {
            self.record(change.at, change.to);
        }
        self.changes_dropped = self.changes_dropped.saturating_add(later.changes_dropped);
        if let Some(latest) = later.latest {
            // Same balance as the last change; this keeps its time.
            self.latest = Some(latest);
        }
        self.zero_since = later.zero_since;
        self.net = match (self.opening, self.latest) {
            (Some(o), Some(l)) => l.balance - o.balance,
            _ => 0,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wallet_with(balance: i64) -> (Wallet, Ledger) {
        let mut wallet = Wallet::default();
        let mut ledger = Ledger::default();
        assert!(wallet.observe(1_000, balance, &mut ledger));
        (wallet, ledger)
    }

    fn causes(wallet: &Wallet) -> Vec<Option<u32>> {
        wallet
            .changes
            .iter()
            .map(|c| match c.cause {
                Cause::Entry { seq } => Some(seq),
                Cause::Untracked => None,
            })
            .collect()
    }

    #[test]
    fn the_first_sample_opens_the_wallet_and_a_repeat_changes_nothing() {
        let (mut wallet, mut ledger) = wallet_with(125_000);
        assert_eq!(
            wallet.opening,
            Some(BalanceSample {
                at: 1_000,
                balance: 125_000
            })
        );
        assert_eq!(wallet.latest, wallet.opening);
        assert!(!wallet.observe(2_000, 125_000, &mut ledger));
        assert!(wallet.changes.is_empty());
        assert_eq!(wallet.net, 0);
    }

    #[test]
    fn a_change_without_a_log_event_is_untracked() {
        let (mut wallet, mut ledger) = wallet_with(125_000);
        assert!(wallet.observe(5_000, 118_500, &mut ledger));
        assert_eq!(
            wallet.changes,
            [WalletChange {
                at: 5_000,
                from: 125_000,
                to: 118_500,
                delta: -6_500,
                cause: Cause::Untracked,
            }]
        );
        assert_eq!(wallet.net, -6_500);
        assert_eq!(wallet.latest.unwrap().balance, 118_500);
    }

    #[test]
    fn a_zero_before_any_balance_is_not_the_opening_balance() {
        let mut wallet = Wallet::default();
        let mut ledger = Ledger::default();
        assert!(wallet.observe(1_000, 0, &mut ledger));
        assert!(!wallet.observe(2_000, 0, &mut ledger));
        assert_eq!(wallet.opening, None);
        assert_eq!(wallet.zero_since, Some(1_000));

        assert!(wallet.observe(9_000, 40_000, &mut ledger));
        assert_eq!(wallet.opening.unwrap().balance, 40_000);
        assert!(wallet.opened_after_zero);
        assert_eq!(wallet.zero_since, None);
        assert!(wallet.changes.is_empty(), "loading is not income");
        assert_eq!(wallet.net, 0);
    }

    #[test]
    fn a_drop_to_zero_and_back_is_not_a_loss_and_a_gain() {
        let (mut wallet, mut ledger) = wallet_with(125_000);
        assert!(wallet.observe(5_000, 0, &mut ledger));
        assert_eq!(wallet.zero_since, Some(5_000));
        assert_eq!(wallet.latest.unwrap().balance, 125_000, "the figures stay");
        assert!(wallet.changes.is_empty());

        assert!(wallet.observe(8_000, 125_000, &mut ledger));
        assert_eq!(wallet.zero_since, None);
        assert!(wallet.changes.is_empty());
        assert_eq!(wallet.net, 0);
    }

    #[test]
    fn back_from_zero_at_another_value_is_one_change_from_the_old_one() {
        let (mut wallet, mut ledger) = wallet_with(125_000);
        wallet.observe(5_000, 0, &mut ledger);
        wallet.observe(8_000, 130_000, &mut ledger);
        assert_eq!(wallet.changes.len(), 1);
        assert_eq!(
            (wallet.changes[0].from, wallet.changes[0].to),
            (125_000, 130_000)
        );
        assert_eq!(wallet.net, 5_000);
    }

    #[test]
    fn a_zero_is_trusted_when_a_log_event_spent_everything() {
        // The purchase is logged first.
        let (mut wallet, mut ledger) = wallet_with(19_175);
        ledger.explain(&mut wallet, 4_000, 7, Some(-19_175));
        assert!(wallet.observe(5_000, 0, &mut ledger));
        assert_eq!(wallet.zero_since, None);
        assert_eq!(causes(&wallet), [Some(7)]);
        assert_eq!(wallet.latest.unwrap().balance, 0);
        assert_eq!(wallet.net, -19_175);
        // From a trusted 0, income is a change like any other.
        assert!(wallet.observe(60_000, 500, &mut ledger));
        assert_eq!(wallet.changes[1].delta, 500);

        // The stream is first, the log line follows.
        let (mut wallet, mut ledger) = wallet_with(19_175);
        wallet.observe(5_000, 0, &mut ledger);
        assert_eq!(wallet.zero_since, Some(5_000));
        assert!(ledger.explain(&mut wallet, 6_500, 3, Some(-19_175)));
        assert_eq!(wallet.zero_since, None);
        assert_eq!(wallet.changes[0].at, 5_000);
        assert_eq!(causes(&wallet), [Some(3)]);
    }

    #[test]
    fn an_event_explains_a_change_in_either_order_within_the_window() {
        // Log first.
        let (mut wallet, mut ledger) = wallet_with(125_000);
        assert!(!ledger.explain(&mut wallet, 10_000, 1, Some(-6_500)));
        wallet.observe(12_000, 118_500, &mut ledger);
        assert_eq!(causes(&wallet), [Some(1)]);

        // Stream first.
        wallet.observe(50_000, 119_000, &mut ledger);
        assert_eq!(causes(&wallet), [Some(1), None]);
        assert!(ledger.explain(&mut wallet, 51_500, 2, Some(500)));
        assert_eq!(causes(&wallet), [Some(1), Some(2)]);

        // Too far apart.
        wallet.observe(200_000, 119_500, &mut ledger);
        assert!(!ledger.explain(&mut wallet, 200_000 + ATTRIBUTION_WINDOW + 1, 3, Some(500)));
        assert_eq!(causes(&wallet), [Some(1), Some(2), None]);
    }

    #[test]
    fn one_event_explains_one_change() {
        let (mut wallet, mut ledger) = wallet_with(10_000);
        ledger.explain(&mut wallet, 2_000, 1, Some(500));
        wallet.observe(3_000, 10_500, &mut ledger);
        wallet.observe(4_000, 11_000, &mut ledger);
        assert_eq!(causes(&wallet), [Some(1), None], "the nearer one");
    }

    #[test]
    fn the_exact_amount_wins_whatever_the_order_in_the_batch() {
        // One poll's batch: a mission completes (unknown reward), then a
        // sale of exactly the gain. The gain is the sale's.
        let (mut wallet, mut ledger) = wallet_with(10_000);
        wallet.observe(20_000, 58_000, &mut ledger);
        ledger.explain(&mut wallet, 19_000, 1, None);
        assert_eq!(causes(&wallet), [Some(1)], "all there is so far");
        ledger.explain(&mut wallet, 19_500, 2, Some(48_000));
        assert_eq!(causes(&wallet), [Some(2)]);

        // The mission's own reward, when it comes, is the mission's.
        wallet.observe(21_000, 70_000, &mut ledger);
        assert_eq!(causes(&wallet), [Some(2), Some(1)]);
    }

    #[test]
    fn the_passes_are_exact_then_near_then_unknown() {
        let (mut wallet, mut ledger) = wallet_with(10_000);
        ledger.explain(&mut wallet, 2_000, 1, None);
        ledger.explain(&mut wallet, 2_100, 2, Some(49_000));
        ledger.explain(&mut wallet, 9_000, 3, Some(48_000));
        // Three gains of 48 000.
        wallet.observe(3_000, 58_000, &mut ledger);
        wallet.observe(3_500, 106_000, &mut ledger);
        wallet.observe(8_000, 154_000, &mut ledger);
        // Exact: event 3 takes the gain nearest to it. Near: event 2 takes
        // the nearest of the rest. Unknown: event 1 gets what is left.
        assert_eq!(causes(&wallet), [Some(2), Some(1), Some(3)]);
    }

    #[test]
    fn an_unknown_amount_explains_only_a_gain_that_follows_it() {
        let (mut wallet, mut ledger) = wallet_with(10_000);
        // A loss, and a gain well before the mission ended.
        wallet.observe(2_000, 9_000, &mut ledger);
        wallet.observe(3_000, 9_500, &mut ledger);
        assert!(!ledger.explain(&mut wallet, 20_000, 1, None));
        assert_eq!(causes(&wallet), [None, None]);
        // A gain the stream showed a moment before the log line's time.
        wallet.observe(19_000, 12_000, &mut ledger);
        assert_eq!(causes(&wallet), [None, None, Some(1)]);
    }

    #[test]
    fn direction_and_size_must_fit() {
        let change = |delta| WalletChange {
            at: 1_000,
            from: 0,
            to: delta,
            delta,
            cause: Cause::Untracked,
        };
        let event = |amount| Explanation {
            at: 1_000,
            seq: 0,
            amount,
        };
        use Amount::{Exact, Gain, Spends};
        assert_eq!(pass_of(&event(Exact(-500)), &change(500)), None);
        assert_eq!(pass_of(&event(Gain), &change(-500)), None);
        assert_eq!(pass_of(&event(Exact(1_000)), &change(1_051)), None);
        assert_eq!(pass_of(&event(Exact(1_000)), &change(1_050)), Some(1));
        assert_eq!(pass_of(&event(Exact(-19_175)), &change(-19_175)), Some(0));
        assert_eq!(pass_of(&event(Gain), &change(500)), Some(2));
        assert_eq!(pass_of(&event(Spends), &change(-500)), Some(2));
        assert_eq!(pass_of(&event(Spends), &change(500)), None);
    }

    #[test]
    fn a_rejected_request_explains_nothing() {
        let (mut wallet, mut ledger) = wallet_with(10_000);
        ledger.explain(&mut wallet, 2_000, 1, Some(-500));
        wallet.observe(3_000, 9_500, &mut ledger);
        assert_eq!(causes(&wallet), [Some(1)]);
        assert!(ledger.retract(&mut wallet, 1));
        assert_eq!(causes(&wallet), [None]);

        // Rejected before any change: the next change finds nothing.
        ledger.explain(&mut wallet, 100_000, 2, Some(-500));
        assert!(!ledger.retract(&mut wallet, 2));
        wallet.observe(101_000, 9_000, &mut ledger);
        assert_eq!(causes(&wallet), [None, None]);
    }

    #[test]
    fn a_cause_outlives_its_event_leaving_the_ledger() {
        let (mut wallet, mut ledger) = wallet_with(10_000);
        ledger.explain(&mut wallet, 2_000, 1, Some(500));
        wallet.observe(3_000, 10_500, &mut ledger);
        // Much later events push the first one out.
        for i in 0..(MAX_EXPLANATIONS as i64 + 5) {
            ledger.explain(&mut wallet, 1_000_000 + i, 100 + i as u32, Some(-1));
        }
        assert!(!ledger.knows(1));
        assert_eq!(causes(&wallet), [Some(1)]);
    }

    #[test]
    fn the_changes_are_capped_and_the_totals_stay_right() {
        let (mut wallet, mut ledger) = wallet_with(1);
        for i in 0..(MAX_WALLET_CHANGES as i64 + 10) {
            wallet.observe(2_000 + i, 2 + i, &mut ledger);
        }
        assert_eq!(wallet.changes.len(), MAX_WALLET_CHANGES);
        assert_eq!(wallet.changes_dropped, 10);
        assert_eq!(wallet.opening.unwrap().balance, 1);
        assert_eq!(wallet.net, MAX_WALLET_CHANGES as i64 + 10);
    }

    #[test]
    fn a_later_track_continues_an_earlier_one() {
        let (mut early, mut ledger) = wallet_with(10_000);
        ledger.explain(&mut early, 2_000, 1, Some(500));
        early.observe(2_500, 10_500, &mut ledger);

        let mut late = Wallet::default();
        late.observe(90_000, 12_000, &mut ledger);
        late.observe(95_000, 11_000, &mut ledger);

        let mut whole = Wallet::default();
        whole.append(&early);
        whole.append(&late);
        assert_eq!(whole.opening.unwrap().balance, 10_000);
        assert_eq!(whole.latest.unwrap(), late.latest.unwrap());
        assert_eq!(whole.net, 1_000);
        let steps: Vec<_> = whole.changes.iter().map(|c| (c.from, c.to)).collect();
        assert_eq!(
            steps,
            [(10_000, 10_500), (10_500, 12_000), (12_000, 11_000)],
            "the gap between the two tracks is a change of its own"
        );
        assert!(whole.changes.iter().all(|c| c.cause == Cause::Untracked));
    }
}
