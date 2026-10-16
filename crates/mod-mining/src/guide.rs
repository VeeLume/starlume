//! The throttle guide: what the instability wave means for how to fly the
//! laser on this rock, as points on the throttle scale.
//!
//! The tick (client code, LIVE 4.10; Firefly notes on the fracture model
//! and the instability wave): the rock drains each laser's queue of hits,
//! then subtracts the decay from the same accumulator, and only if that net
//! is positive multiplies it by `1 + f` (or divides by `1 - f`), `f =
//! instability * w * damping`, `w = sin(2π t / T) * a` with a fresh period
//! `T` and height `a ∈ 0..1` each cycle. A negative net is never multiplied:
//! the wave never speeds up the drain.
//!
//! The queue is drained at [`HIT_DRAIN`] times the tick time, so one tick
//! can carry up to 1.2× the mean gain (and others less). That is why the
//! wave still acts at the throttle where the *mean* gain equals the decay,
//! and why the charge can be pushed up even a little below it. Which ticks
//! run strong depends on hit timing that is not traced, so the guide takes
//! the strong tick as the worst case: its jumps are upper bounds.
//!
//! One positive half-wave at height 1 adds `net * (T/2 + A * T/π)` with
//! `A = instability * damping`. The guide uses the longest period
//! (`period + variance`) and the damping steady aim reaches after the
//! hit-consistency history window; the first seconds of firing wobble more.
//!
//! sc-mining's `simulate` meters the gain evenly (no queue), so its
//! difficulty reads high-instability rocks as calmer at balance than this.

use serde::Serialize;
use svc_data::mining::{Combined, ModifiedRock, Ruleset, SimConfig, hit_consistency};

/// How a rock and loadout fly, by the wave's peaks at the hold point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    /// Even a maximum peak cannot jump the window: hold and climb freely.
    Safe,
    /// An average peak stays within the window; the big ones can overshoot.
    Okay,
    /// Average peaks already jump the window and can overcharge hard.
    Dangerous,
    /// The laser is too strong for the rock: even at its minimum throttle
    /// the charge keeps climbing, so it cannot be held, only pulsed.
    TooStrong,
}

/// One step of the climb: this throttle is safe up to this charge (a
/// maximum peak from there lands at the window's top).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct ClimbStep {
    pub until_charge: f32,
    pub throttle: f32,
}

/// How much faster than real time the rock drains a laser's hit queue
/// (`DAT_147f070b0` = 1.2, LIVE 4.10.196.36804).
pub const HIT_DRAIN: f32 = 1.2;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ThrottleGuide {
    /// The charge bar's optimal window (0..1 of the bar).
    pub window_low: f32,
    pub window_high: f32,
    /// The lowest throttle the lasers go to (the highest laser minimum).
    pub throttle_minimum: f32,
    /// Below this throttle no tick gains charge, so the wave cannot lift it:
    /// the charge only drains. `None` when even the minimum throttle can
    /// still be lifted.
    pub decline: Option<f32>,
    /// The throttle at which the mean gain equals the decay: the charge
    /// holds on average, jittering with the wave. `None` when even full
    /// throttle cannot raise the charge.
    pub balance: Option<f32>,
    /// The highest throttle at which one maximum wave peak cannot carry the
    /// charge from the window's bottom past its top. `None` with `balance`.
    pub safe_climb: Option<f32>,
    /// The largest multiplier a wave peak puts on a tick's net gain (`1 + A`).
    pub wave_max: f32,
    /// The wave period's mean and longest length, s.
    pub wave_period_s: f32,
    pub wave_period_max_s: f32,
    /// Share of the wave steady aim removes (hit consistency).
    pub damping: f32,
    /// Charge (share of the bar) one maximum wave peak can add at the
    /// balance throttle — how far the held charge can jump.
    pub peak_jump_at_balance: Option<f32>,
    /// Charge one maximum wave peak can add at full throttle.
    pub peak_jump_full: f32,
    /// Mean charge lost per second at the minimum throttle; `None` when the
    /// charge rises there on average (it cannot be held: it overcharges).
    pub drain_at_minimum: Option<f32>,
    /// Seconds from empty to the window at full throttle, wave left out (it
    /// only shortens the climb).
    pub time_to_window_s: Option<f32>,
    /// Seconds in the window that crack the rock cleanly.
    pub time_in_window_s: Option<f32>,
    /// `None` when the rock cannot be cracked.
    pub category: Option<Category>,
    /// Charge an average peak (half height, mean period) can add at the
    /// balance throttle.
    pub peak_jump_avg_at_balance: Option<f32>,
    // ── When a jump overcharges ──
    /// Seconds above the window until the danger meter fills (an overcharge
    /// fracture), just above the window and at the top of the bar. The meter
    /// keeps what it gathered and only drains back in the window.
    pub danger_fill_edge_s: Option<f32>,
    pub danger_fill_top_s: Option<f32>,
    /// Seconds a full danger meter takes to drain once the charge is back in
    /// (or below) the window.
    pub danger_drain_s: Option<f32>,
    /// Seconds back inside the window that pay off one second above it
    /// (danger fill over danger drain), just above the window and at the top.
    pub payback_edge: Option<f32>,
    pub payback_top: Option<f32>,
    /// Seconds above the window that crack the rock anyway (the controlled
    /// meter keeps filling there, slower).
    pub crack_above_s: Option<f32>,
    /// The window's top with the beam off (the laser's modifiers dropped).
    pub window_high_off: f32,
    /// Seconds from the top of the bar back into the window, beam off (the
    /// fastest way down: the wave never speeds up a fall).
    pub back_from_top_off_s: Option<f32>,
    /// The same at the minimum throttle; `None` when it does not fall there.
    pub back_from_top_min_s: Option<f32>,
    /// The highest charge a jump can reach and still be saved, danger meter
    /// empty: falling back from there fills the meter just to the brim.
    /// Beam off at once, and at the minimum throttle. `1.0` when even the
    /// top of the bar is saved; `None` when the charge does not fall.
    pub no_return_off: Option<f32>,
    pub no_return_min: Option<f32>,
    /// After a jump over the window: below this charge, dropping to the
    /// minimum throttle gathers less danger than cutting the beam (the laser
    /// keeps its window; a cut drops it to `window_high_off` and the charge
    /// has further to fall), above it cutting does (the faster fall wins).
    /// `1.0`: keep the laser on at any height. `None`: always cut (the charge
    /// does not fall at the minimum throttle, or cutting is never worse).
    pub keep_on_below: Option<f32>,
    // ── Advice along the charge bar ──
    /// Below this charge full throttle is safe: even a maximum peak lands no
    /// higher than the window's top. `None` when it is never safe.
    pub full_until: Option<f32>,
    /// From the full-throttle zone to the window: the highest safe throttle
    /// falls as the charge rises, in steps of 10 %.
    pub climb: Vec<ClimbStep>,
    /// The throttle to run in the window: the balance when a maximum peak
    /// from the window's middle stays inside, else the highest throttle
    /// whose peak does (below balance: the charge drains slowly and the
    /// peaks lift it — riding the waves).
    pub hold_throttle: Option<f32>,
    pub riding: bool,
    /// The charge rises on average even at the minimum throttle (the
    /// balance lies below the laser's minimum): it cannot be held.
    pub too_strong: bool,
    /// How fast it climbs there, share of the bar per second.
    pub climb_at_minimum: Option<f32>,
    /// Pulsing the beam to hold it: about this share of the time on at the
    /// minimum throttle (the wave makes the real share a little lower).
    pub pulse_on_share: Option<f32>,
    /// Seconds the charge takes to cross the window at the minimum throttle,
    /// wave left out (against `time_in_window_s` for a clean crack).
    pub window_cross_s: Option<f32>,
    /// From this charge on a jump is lost: the better way down (minimum
    /// throttle below `keep_on_below`, beam cut above) still fills the
    /// danger meter. `None` when every height can be saved.
    pub lost_from: Option<f32>,
    /// A beam off longer than this drops the laser's and modules' modifiers
    /// from the rock (resistance, instability and window change back).
    pub modifier_persistence_s: f32,
}

/// The guide for `rock` (the setup applied); `beam_off` is the rock with only
/// the gadgets on (what it is while the beam is off).
pub fn throttle_guide(
    rock: &ModifiedRock,
    beam_off: &ModifiedRock,
    ruleset: &Ruleset,
) -> ThrottleGuide {
    throttle_guide_with(rock, beam_off, ruleset, HIT_DRAIN)
}

/// [`throttle_guide`] with the strongest tick at `hit_drain` × the mean gain.
/// `1.0` is sc-mining's even metering, which the tests check the wave
/// arithmetic against.
pub fn throttle_guide_with(
    rock: &ModifiedRock,
    beam_off: &ModifiedRock,
    ruleset: &Ruleset,
    hit_drain: f32,
) -> ThrottleGuide {
    let gain = |t: f32| rock.gain_rate(ruleset, t);
    let decay = gain(1.0) - rock.net_rate(ruleset, 1.0);
    // The strongest tick's net, per second.
    let strong = |t: f32| hit_drain * gain(t) - decay;

    let hc = &ruleset.hit_consistency;
    let steady = hit_consistency(hc, hc.history_window, SimConfig::default().aim_scatter_m);
    let damping = steady.min(hc.max_effect_on_instability);
    let amplitude = rock.instability * (1.0 - damping);
    let period_max = ruleset.instability_wave_period + ruleset.instability_wave_variance;
    // Charge per unit of net gain over one positive half-wave at height 1.
    let half_wave = period_max / 2.0 + amplitude * period_max / std::f32::consts::PI;
    let jump = |t: f32| strong(t).max(0.0) * half_wave;
    // An average peak: half the height, the mean period.
    let period = ruleset.instability_wave_period;
    let half_wave_avg = period / 2.0 + amplitude * 0.5 * period / std::f32::consts::PI;
    let jump_avg = |t: f32| strong(t).max(0.0) * half_wave_avg;

    // The breaking meters (client code): ship rocks all sit at the 0.1 floor
    // of the mass scale. The catastrophic additive term comes per tick.
    let scale = if ruleset.default_mass > 0.0 {
        ((ruleset.default_mass - rock.mass) / ruleset.default_mass + 1.0).max(0.1)
    } else {
        0.1
    };
    let ticks_per_s = 1.0 / SimConfig::default().tick_s;
    let danger_rate = |over: f32, cat: &Combined| {
        (over + 1.0).powf(ruleset.danger_breaking_fill_rate_exponent)
            * ruleset.danger_breaking_fill_rate
            * scale
            * cat.factor
            + cat.add * ticks_per_s
    };
    let danger_drain = ruleset.danger_breaking_decay_rate * scale;
    let seconds = |rate: f32| (rate > 0.0).then(|| 1.0 / rate);
    let fall = |gain_now: f32, high: f32| {
        let rate = decay - gain_now;
        (rate > 0.0 && high < 1.0).then(|| (1.0 - high) / rate)
    };
    // Danger gathered falling from charge `p` to `high` at `rate` per second.
    let gathered = |p: f32, high: f32, rate: f32, cat: &Combined| {
        const STEPS: usize = 200;
        let dp = (p - high) / STEPS as f32;
        (0..STEPS)
            .map(|k| {
                let charge = high + (k as f32 + 0.5) * dp;
                danger_rate((charge - high) / (1.0 - high), cat) * dp / rate
            })
            .sum::<f32>()
    };
    // Danger gathered falling back from charge `p`: beam off, and at the
    // minimum throttle (`None` where that does not fall).
    let gathered_off = |p: f32| {
        let high = beam_off.window.high;
        if p <= high || decay <= 0.0 {
            0.0
        } else {
            gathered(p, high, decay, &beam_off.catastrophic_rate)
        }
    };
    let gathered_min = |p: f32| {
        let (high, rate) = (rock.window.high, decay - gain(0.0));
        if p <= high {
            Some(0.0)
        } else {
            (rate > 0.0).then(|| gathered(p, high, rate, &rock.catastrophic_rate))
        }
    };
    let keep_on_below = {
        let from = rock.window.high.min(beam_off.window.high);
        const STEPS: usize = 400;
        let mut last_on = None;
        for k in 1..=STEPS {
            let p = from + (1.0 - from) * k as f32 / STEPS as f32;
            match gathered_min(p) {
                Some(on) if on <= gathered_off(p) => last_on = Some(p),
                _ => break,
            }
        }
        last_on.filter(|p| *p > rock.window.high)
    };
    let no_return = |gain_now: f32, high: f32, cat: &Combined| {
        let rate = decay - gain_now;
        if rate <= 0.0 || high >= 1.0 {
            return None;
        }
        if gathered(1.0, high, rate, cat) < 1.0 {
            return Some(1.0);
        }
        let (mut lo, mut hi) = (high, 1.0f32);
        for _ in 0..40 {
            let mid = (lo + hi) / 2.0;
            if gathered(mid, high, rate, cat) < 1.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        Some(lo)
    };

    // `gain` grows with the throttle: bisect for `f(t) = target` on lo..hi.
    let solve = |f: &dyn Fn(f32) -> f32, target: f32, mut lo: f32, mut hi: f32| {
        for _ in 0..40 {
            let mid = (lo + hi) / 2.0;
            if f(mid) <= target {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo
    };

    let balance = rock.balance_throttle(ruleset);
    let window = rock.window.size();
    let safe_climb = balance.map(|b| {
        if jump(1.0) <= window {
            1.0
        } else {
            // No tick gains below b / hit_drain, so the jump is 0 there.
            solve(&jump, window, b / hit_drain, 1.0)
        }
    });
    let decline = (strong(0.0) < 0.0).then(|| solve(&strong, 0.0, 0.0, balance.unwrap_or(1.0)));

    // The highest throttle whose maximum peak from charge `c` lands no higher
    // than the window's top (`0.0`: not even the minimum is).
    let high = rock.window.high;
    let safe_from = |c: f32| {
        let room = high - c;
        if jump(1.0) <= room {
            1.0
        } else if jump(0.0) > room {
            0.0
        } else {
            solve(&jump, room, 0.0, 1.0)
        }
    };
    let low = rock.window.low;
    let full_until =
        (balance.is_some() && high - jump(1.0) > 0.0).then(|| (high - jump(1.0)).min(low));
    let at_window = safe_from(low);
    let climb: Vec<ClimbStep> = if balance.is_some() {
        (1..10)
            .rev()
            .map(|n| n as f32 / 10.0)
            .filter(|t| *t > at_window)
            .map(|t| ClimbStep {
                until_charge: high - jump(t),
                throttle: t,
            })
            .filter(|s| s.until_charge > full_until.unwrap_or(0.0) && s.until_charge < low)
            .collect()
    } else {
        vec![]
    };
    let ride = safe_from(rock.window.centre());
    // The throttle cannot go below the lasers' minimum: when the charge
    // still rises there, there is no hold, only pulsing the beam.
    let climb_min = rock.net_rate(ruleset, 0.0);
    let too_strong = balance.is_some() && climb_min > 0.0;
    let hold_throttle = balance.filter(|_| !too_strong).map(|b| b.min(ride));
    // Above the window: which way down is better, and where both fail.
    let lost_from = {
        const STEPS: usize = 400;
        (1..=STEPS)
            .map(|k| high + (1.0 - high) * k as f32 / STEPS as f32)
            .find(|&p| {
                let on = keep_on_below
                    .filter(|b| p <= *b)
                    .and_then(|_| gathered_min(p));
                on.unwrap_or_else(|| gathered_off(p)) >= 1.0
            })
    };

    ThrottleGuide {
        window_low: rock.window.low,
        window_high: rock.window.high,
        throttle_minimum: rock
            .lasers
            .iter()
            .map(|l| l.throttle_minimum)
            .fold(0.0, f32::max),
        decline,
        balance,
        safe_climb,
        wave_max: 1.0 + amplitude,
        wave_period_s: ruleset.instability_wave_period,
        wave_period_max_s: period_max,
        damping,
        peak_jump_at_balance: balance.map(jump),
        peak_jump_full: jump(1.0),
        drain_at_minimum: (gain(0.0) < decay).then(|| decay - gain(0.0)),
        time_to_window_s: rock.time_to_window_s(ruleset),
        time_in_window_s: rock.time_in_window_s(ruleset),
        category: balance.map(|b| {
            if too_strong {
                Category::TooStrong
            } else if jump(b) <= window {
                Category::Safe
            } else if jump_avg(b) <= window {
                Category::Okay
            } else {
                Category::Dangerous
            }
        }),
        peak_jump_avg_at_balance: balance.map(jump_avg),
        danger_fill_edge_s: seconds(danger_rate(0.0, &rock.catastrophic_rate)),
        danger_fill_top_s: seconds(danger_rate(1.0, &rock.catastrophic_rate)),
        danger_drain_s: seconds(danger_drain),
        payback_edge: (danger_drain > 0.0)
            .then(|| danger_rate(0.0, &rock.catastrophic_rate) / danger_drain),
        payback_top: (danger_drain > 0.0)
            .then(|| danger_rate(1.0, &rock.catastrophic_rate) / danger_drain),
        crack_above_s: seconds(
            rock.catastrophic_rate
                .apply(scale * ruleset.controlled_breaking_fill_rate_danger),
        ),
        window_high_off: beam_off.window.high,
        back_from_top_off_s: fall(0.0, beam_off.window.high),
        back_from_top_min_s: fall(gain(0.0), rock.window.high),
        no_return_off: no_return(0.0, beam_off.window.high, &beam_off.catastrophic_rate),
        no_return_min: no_return(gain(0.0), rock.window.high, &rock.catastrophic_rate),
        keep_on_below,
        full_until,
        climb,
        hold_throttle,
        riding: !too_strong && balance.is_some_and(|b| ride < b),
        too_strong,
        climb_at_minimum: (climb_min > 0.0).then_some(climb_min),
        pulse_on_share: (too_strong && gain(0.0) > 0.0).then(|| decay / gain(0.0)),
        window_cross_s: (climb_min > 0.0).then(|| window / climb_min),
        lost_from,
        modifier_persistence_s: ruleset.modifier_persistence_time,
    }
}
