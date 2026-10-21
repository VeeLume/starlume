//! The player the calculator rates rocks for: a person at the throttle,
//! not a controller.
//!
//! sc-mining's `Player` knows the exact balance throttle and the exact
//! charge and corrects smoothly after 0.3 s; it holds a 1 % window forever,
//! so a rock it cracks says little about whether a person can. This one
//! flies like the recorded cracks (2026-10-17, Golem; charge and throttle
//! read off the HUD) and the way Valerie keeps the window (2026-10-18):
//!
//! - it **decides only now and then**: every [`DECIDE_S`] while climbing,
//!   every [`WINDOW_DECIDE_S`] inside the window (watching closely), and
//!   keeps the throttle constant in between;
//! - it **reacts late**: it judges the charge it saw [`REACTION_S`] ago;
//! - it **misjudges the charge**: read by eye, off by up to
//!   [`CHARGE_UNCERTAINTY`] of the bar; the error drifts (re-judged every
//!   [`REJUDGE_S`]) rather than averaging out tick by tick;
//! - it **sees the bar move**: it judges where the charge will be
//!   [`LOOK_AHEAD_S`] on, so it steps down before the window;
//! - it **follows the calculator's plan**: below the window the guide's
//!   climb ladder, above it the minimum, inside it a slight rise in the
//!   lower half and a slight fall in the upper half (the guide's two keep
//!   settings; with too much power it pulses: fires at the minimum in the
//!   lower half and cuts the beam in the upper half, the laser's modifiers
//!   leaving the rock 0.25 s into a cut as in the game). Settings land
//!   exactly on the game's 1 % steps ([`STEP`]):
//!   the HUD shows the throttle as a number. In a crew the supports keep
//!   their own setting while the lead follows the plan;
//! - it **drifts**: the beam's distance moves about a metre either way (an
//!   attentive player correcting): on a surface the hovering ship wanders,
//!   in space the turning rock swings it ([`Environment`]). That matters
//!   past the lasers' optimal range (a too-powerful laser backed off).
//!
//! The guide reads the reaction, decision interval and misjudgement from
//! here, so the plan and the rating are one model. The values are estimates
//! from two recordings; calibrating them from overlay captures is open.

use std::collections::VecDeque;

use rayon::prelude::*;

use svc_data::mining::{
    Difficulty, LaserControl, ModifiedRock, Outcome, Pool, Rock, Ruleset, Setup, Shape, SimConfig,
    Throttle, Timeline, View, simulate_timeline,
};

/// Seconds between seeing the charge and acting on it.
pub const REACTION_S: f32 = 1.0;
/// Seconds between decisions while climbing, drawn in this range each time.
pub const DECIDE_S: (f32, f32) = (2.0, 5.0);
/// Seconds between decisions inside the window, where the player watches
/// the bar closely (an estimate; not measured yet).
pub const WINDOW_DECIDE_S: (f32, f32) = (1.0, 2.5);
/// Seconds between decisions while pulsing (too much power): the player
/// watches the bar and taps the beam on and off (an estimate).
pub const PULSE_DECIDE_S: (f32, f32) = (0.3, 0.8);
/// How far off the bar's reading by eye can be, share of the bar (±).
pub const CHARGE_UNCERTAINTY: f32 = 0.03;
/// How often the player re-judges where the charge is.
pub const REJUDGE_S: f32 = 2.0;
/// Throttle settings come in the game's steps (`throttleChangePerAction`).
pub const STEP: f32 = 0.01;
/// The player sees the bar move: it judges where the charge will be this
/// many seconds on (the recorded cracks step down well before the window).
pub const LOOK_AHEAD_S: f32 = 3.0;
/// A crack only counts as clean when the danger pool is at most this at the
/// crack: the client keeps `round(N * (1 - danger))` of a rock's N pieces
/// and the ore of the dropped ones is lost, so a crack loses nothing only
/// while danger <= 0.5 / N (0.083 for 6 pieces, 0.071 for 7; client
/// 4.10.197, sc-holotable 2026-10-18). Meshes have 6 or 7 pieces.
pub const CLEAN_DANGER: f32 = 0.5 / 7.0;
/// The margin the distance advice keeps either way (m): wide enough for a
/// player who doesn't watch the gauge all the time (Valerie, 2026-10-18:
/// about ±1 m when active, worse when not watching).
pub const SIT_MARGIN_M: f32 = 3.0;
/// On a surface the rock doesn't turn but the hovering ship is less
/// steady: the distance wanders within ±this (m), heading somewhere new
/// every [`SURFACE_RETARGET_S`] at [`SURFACE_SPEED`] m/s. Estimates.
pub const SURFACE_DRIFT_M: f32 = 1.0;
pub const SURFACE_RETARGET_S: f32 = 2.0;
pub const SURFACE_SPEED: f32 = 0.8;
/// In space the ship holds still but the rock turns: the beam's spot swings
/// the distance by ±this (m) once per rotation, a rotation taking
/// [`ROTATION_S`] (drawn per attempt), plus a little ship drift. Estimates
/// until the spawn spin is read.
pub const SPACE_SWING_M: f32 = 0.8;
pub const ROTATION_S: (f32, f32) = (20.0, 60.0);
pub const SPACE_SHIP_DRIFT_M: f32 = 0.2;
/// A window narrower than this (share of the bar) is never rated easy,
/// whatever the simulation says: it is inside the misjudgement.
pub const MIN_EASY_WINDOW: f32 = CHARGE_UNCERTAINTY;

/// Whether `rock`'s window is too narrow to rate it easy.
pub fn narrow_window(rock: &ModifiedRock) -> bool {
    rock.window.size() < MIN_EASY_WINDOW
}

/// Where the rock is, for how the beam's distance moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Environment {
    /// In space: the ship holds still, the rock turns.
    #[default]
    Space,
    /// On a planet or moon: the rock doesn't turn, the ship hovers.
    Surface,
}

/// The calculator's plan a [`Human`] follows, from the throttle guide.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// The climb ladder, highest charge first: up to that charge, at most
    /// that throttle (the window's bottom first, at the climb limit).
    pub climb: Vec<(f32, f32)>,
    /// Inside the window: the rise setting (lower half) and the fall
    /// setting (upper half).
    pub keep_low: f32,
    pub keep_high: f32,
    /// Above the window: the lasers' minimum.
    pub minimum: f32,
    /// Per laser, a crew support's own setting (it keeps it through the
    /// climb and the window); `None` for the lead and a solo laser, which
    /// follow the plan. Empty: every laser follows it.
    pub holds: Vec<Option<f32>>,
    pub environment: Environment,
    /// Too much power: even the minimum keeps the charge climbing, so the
    /// window is kept by pulsing: fire at the minimum in the lower half of
    /// `pulse_window`, cut the beam in its upper half (everyone at once, in
    /// a crew).
    pub pulse: bool,
    /// Where a pulse keeps the charge: the window with the beam on and the
    /// one with it off overlap here (0.25 s into a cut the lasers'
    /// modifiers leave the rock, a Focus module's wider window with them;
    /// Valerie, 2026-10-18). The beam-on window when they don't overlap.
    pub pulse_window: (f32, f32),
}

impl Plan {
    /// The guide's plan for `rock` (`beam_off`: the same rock with the
    /// lasers off, see [`beam_off`]). A window that can't be kept is still
    /// tried, with the gentlest rise.
    pub fn for_rock(rock: &ModifiedRock, beam_off: &ModifiedRock, ruleset: &Ruleset) -> Self {
        Self::from_guide(&crate::guide::throttle_guide(rock, beam_off, ruleset))
    }

    pub fn from_guide(g: &crate::guide::ThrottleGuide) -> Self {
        let mut climb: Vec<(f32, f32)> = vec![(g.window_low, g.safe_climb.unwrap_or(1.0).min(1.0))];
        let mut steps: Vec<(f32, f32)> = g
            .climb
            .iter()
            .map(|c| (c.until_charge, c.throttle))
            .collect();
        steps.sort_by(|a, b| b.0.total_cmp(&a.0));
        climb.extend(steps);
        if let Some(f) = g.full_until {
            climb.push((f, 1.0));
        }
        let gentlest = g
            .balance
            .map_or(1.0, |b| (((b + STEP / 2.0) / STEP).ceil() * STEP).min(1.0));
        let minimum = g.throttle_minimum;
        Self {
            climb,
            keep_low: if g.too_much_power {
                minimum
            } else {
                g.keep_low.unwrap_or(gentlest)
            },
            keep_high: if g.too_much_power {
                minimum
            } else {
                g.keep_high.unwrap_or(minimum)
            },
            minimum,
            holds: vec![],
            environment: Environment::default(),
            pulse: g.too_much_power,
            pulse_window: {
                let (lo, hi) = (
                    g.window_low.max(g.window_low_off),
                    g.window_high.min(g.window_high_off),
                );
                if lo < hi {
                    (lo, hi)
                } else {
                    (g.window_low, g.window_high)
                }
            },
        }
    }

    /// The plan where the rock is.
    pub fn in_environment(mut self, environment: Environment) -> Self {
        self.environment = environment;
        self
    }

    /// The plan with the supports' settings (a crew).
    pub fn with_holds(mut self, holds: Vec<Option<f32>>) -> Self {
        self.holds = holds;
        self
    }
}

/// A person at the throttle (see the module docs).
#[derive(Debug, Clone)]
pub struct Human {
    seen: VecDeque<(f32, f32)>,
    history: VecDeque<(f32, f32)>,
    rng: u64,
    /// Misjudgement now and the one it drifts towards, and when.
    error: f32,
    target: f32,
    next_judge: f32,
    /// The setting held, and when the next decision comes.
    throttle: f32,
    next_decide: f32,
    plan: Plan,
    /// The ship's offset from the setup's distance, heading to `drift_to`;
    /// in space the rock's rotation (period and phase) on top.
    drift: f32,
    drift_to: f32,
    next_drift: f32,
    rotation_s: f32,
    phase: f32,
    /// The setup's distances, read on the first tick.
    base_m: Vec<f32>,
}

impl Human {
    /// A player following `plan`.
    pub fn following(seed: u64, plan: Plan) -> Self {
        let mut h = Self {
            seen: VecDeque::new(),
            history: VecDeque::new(),
            rng: seed ^ 0x5EED_1A55,
            error: 0.0,
            target: 0.0,
            next_judge: 0.0,
            throttle: 1.0,
            next_decide: 0.0,
            plan,
            drift: 0.0,
            drift_to: 0.0,
            next_drift: 0.0,
            rotation_s: ROTATION_S.0,
            phase: 0.0,
            base_m: vec![],
        };
        h.error = h.uniform() * CHARGE_UNCERTAINTY;
        h.target = h.error;
        h.rotation_s = h.within(ROTATION_S);
        h.phase = h.uniform() * std::f32::consts::PI;
        h
    }

    /// Uniform in -1..1.
    fn uniform(&mut self) -> f32 {
        self.rng = self.rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        let u = (z >> 11) as f32 / (1u64 << 53) as f32;
        u * 2.0 - 1.0
    }

    /// Uniform in ±CHARGE_UNCERTAINTY.
    fn draw(&mut self) -> f32 {
        self.uniform() * CHARGE_UNCERTAINTY
    }

    /// Uniform in `range`.
    fn within(&mut self, range: (f32, f32)) -> f32 {
        range.0 + (range.1 - range.0) * (self.uniform() * 0.5 + 0.5)
    }
}

impl Human {
    /// The one setting the plan gives for what the player sees now; `None`
    /// between decisions (the setting stays).
    fn decide(&mut self, view: &View<'_>) -> Option<(f32, bool)> {
        // the charge as seen a reaction time ago (and a few seconds of it)
        self.seen.push_back((view.time_s, view.power));
        self.history.push_back((view.time_s, view.power));
        while self
            .history
            .front()
            .is_some_and(|&(t, _)| t < view.time_s - REACTION_S - 2.5)
        {
            self.history.pop_front();
        }
        while self
            .seen
            .get(1)
            .is_some_and(|&(t, _)| t <= view.time_s - REACTION_S)
        {
            self.seen.pop_front();
        }
        // the misjudgement drifts towards a fresh one every REJUDGE_S
        if view.time_s >= self.next_judge {
            self.error = self.target;
            self.target = self.draw();
            self.next_judge = view.time_s + REJUDGE_S;
        }
        if view.time_s < self.next_decide {
            return None;
        }
        let seen = self.seen.front().map_or(0.0, |&(_, p)| p);
        let k = 1.0 - (self.next_judge - view.time_s) / REJUDGE_S;
        let judged_now = seen + self.error + (self.target - self.error) * k;
        // how fast it moves, over the last two seconds it saw
        let earlier = self
            .history
            .iter()
            .find(|&&(t, _)| t >= view.time_s - REACTION_S - 2.0)
            .map_or(seen, |&(_, p)| p);
        let rate = (seen - earlier) / 2.0;
        let judged = judged_now + rate.max(0.0) * LOOK_AHEAD_S;

        let w = view.rock.window;
        let inside = (w.low..=w.high).contains(&judged);
        let wait = self.within(if inside && self.plan.pulse {
            PULSE_DECIDE_S
        } else if inside {
            WINDOW_DECIDE_S
        } else {
            DECIDE_S
        });
        self.next_decide = view.time_s + wait;

        let plan = &self.plan;
        let rung = || {
            plan.climb
                .iter()
                .filter(|(until, _)| judged < *until)
                .map(|(_, t)| *t)
                .next_back()
        };
        let next = if plan.pulse {
            // too much power: climb to the overlap of the beam-on and
            // beam-off windows, then fire at the minimum in its lower half
            // and cut the beam from its upper half on
            let (lo, hi) = plan.pulse_window;
            if judged < lo {
                // the ladder below the window; inside the beam-on window but
                // under the overlap the minimum already climbs
                (rung().unwrap_or(plan.minimum).max(plan.minimum), true)
            } else {
                (plan.minimum, judged < (lo + hi) / 2.0)
            }
        } else if judged < w.low {
            // the ladder's step for this charge (full power below the first),
            // never gentler than the rise setting
            (rung().unwrap_or(1.0).max(plan.keep_low), true)
        } else if judged > w.high {
            (plan.minimum, true)
        } else if judged < w.centre() {
            // lower half: a slight rise
            (plan.keep_low, true)
        } else {
            // upper half: a slight fall
            (plan.keep_high, true)
        };
        Some(next)
    }

    /// The beam's distance moving around the setup's: on a surface the
    /// hovering ship wanders (a fresh target every few seconds); in space
    /// the turning rock swings it once per rotation and the ship barely
    /// drifts.
    fn drift(&mut self, view: &View<'_>, lasers: &mut [LaserControl]) {
        if self.base_m.is_empty() {
            self.base_m = lasers.iter().map(|l| l.distance_m).collect();
        }
        let (reach, retarget, speed) = match self.plan.environment {
            Environment::Surface => (SURFACE_DRIFT_M, SURFACE_RETARGET_S, SURFACE_SPEED),
            Environment::Space => (
                SPACE_SHIP_DRIFT_M,
                SURFACE_RETARGET_S * 2.0,
                SURFACE_SPEED / 4.0,
            ),
        };
        if view.time_s >= self.next_drift {
            self.drift_to = self.uniform() * reach;
            self.next_drift = view.time_s + retarget;
        }
        let step = speed * SimConfig::default().tick_s;
        self.drift += (self.drift_to - self.drift).clamp(-step, step);
        let swing = match self.plan.environment {
            Environment::Space => {
                SPACE_SWING_M
                    * (std::f32::consts::TAU * view.time_s / self.rotation_s + self.phase).sin()
            }
            Environment::Surface => 0.0,
        };
        for (l, base) in lasers.iter_mut().zip(&self.base_m) {
            l.distance_m = (base + self.drift + swing).max(0.0);
        }
    }
}

fn stepped(t: f32) -> f32 {
    (t / STEP).round() * STEP
}

impl Throttle for Human {
    fn throttle(&mut self, view: &View<'_>, lasers: &mut [LaserControl]) {
        self.drift(view, lasers);
        let Some((setting, firing)) = self.decide(view) else {
            return;
        };
        self.throttle = stepped(setting).clamp(0.0, 1.0);
        for (i, l) in lasers.iter_mut().enumerate() {
            l.throttle = match self.plan.holds.get(i).copied().flatten() {
                Some(hold) => stepped(hold).clamp(0.0, 1.0),
                None => self.throttle,
            };
            l.firing = firing;
        }
    }
}

/// The share of a rock's ore a fracture at `danger` leaves, over the
/// meshes it may have (ship rocks: asteroid_01..03, mining_rock_01). The
/// client keeps the first `round(N * (1 - danger))` pieces; the rest and
/// their ore are lost. `None` without meshes.
pub fn ore_kept(shapes: &[Shape], danger: f32) -> Option<f32> {
    let with: Vec<f32> = shapes
        .iter()
        .filter(|s| !s.pieces.is_empty())
        .map(|s| s.yield_fraction(danger))
        .collect();
    (!with.is_empty()).then(|| with.iter().sum::<f32>() / with.len() as f32)
}

/// How the attempts went: sc-mining's summary, plus the ore the cracks
/// kept on average (over the attempts that cracked).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rating {
    pub difficulty: Difficulty,
    pub ore_kept: Option<f32>,
}

/// [`Human`] over `runs` attempts (different waves and misjudgements),
/// following `plan` on the rock as `timeline` changes it (active modules
/// switching off). Summarised like sc-mining's `difficulty`, except that a
/// crack with the danger pool above [`CLEAN_DANGER`] (a piece and its ore
/// lost) counts as overcharged; `shapes` give the ore kept.
pub fn rate(
    timeline: &Timeline,
    ruleset: &Ruleset,
    plan: &Plan,
    shapes: &[Shape],
    config: &SimConfig,
    runs: u32,
) -> Rating {
    let mut times = Vec::new();
    let (mut controlled, mut over, mut timed_out) = (0u32, 0u32, 0u32);
    let (mut danger_sum, mut in_window_sum) = (0.0f32, 0.0f32);
    let (mut kept_sum, mut kept_n) = (0.0f32, 0u32);
    let attempt = |i: u32| {
        let config = SimConfig {
            seed: config.seed.wrapping_add(u64::from(i)),
            record: false,
            ..*config
        };
        let mut player = Human::following(config.seed, plan.clone());
        let trace = simulate_timeline(timeline, ruleset, &mut player, &config);
        (trace.outcome, trace.time_in_window_s)
    };
    // The tries run side by side on the job's pool (`crate::par`: a grade
    // inside a comfort solve shares its threads), summed in seed order.
    let attempts: Vec<(Outcome, f32)> =
        crate::par::job(|| (0..runs).into_par_iter().map(attempt).collect());
    for (outcome, time_in_window_s) in attempts {
        let duration = match outcome {
            Outcome::Fractured {
                time_s,
                pool,
                danger,
            } => {
                match pool {
                    Pool::Controlled if danger <= CLEAN_DANGER => controlled += 1,
                    _ => over += 1,
                }
                times.push(time_s);
                danger_sum += danger;
                // a danger-pool fracture is a full overcharge: nothing spawns
                let d = if pool == Pool::Danger { 1.0 } else { danger };
                if let Some(k) = ore_kept(shapes, d) {
                    kept_sum += k;
                    kept_n += 1;
                }
                time_s
            }
            Outcome::TimedOut => {
                timed_out += 1;
                config.max_time_s
            }
        };
        if duration > 0.0 {
            in_window_sum += time_in_window_s / duration;
        }
    }
    times.sort_by(f32::total_cmp);
    let share = |n: u32| {
        if runs > 0 {
            n as f32 / runs as f32
        } else {
            0.0
        }
    };
    Rating {
        difficulty: Difficulty {
            runs,
            controlled: share(controlled),
            overcharged: share(over),
            timed_out: share(timed_out),
            median_time_s: times.get(times.len() / 2).copied(),
            mean_danger: if times.is_empty() {
                0.0
            } else {
                danger_sum / times.len() as f32
            },
            in_window: if runs > 0 {
                in_window_sum / runs as f32
            } else {
                0.0
            },
        },
        ore_kept: (kept_n > 0).then(|| kept_sum / kept_n as f32),
    }
}

/// `rock` as it is while every beam is off: the gadgets stay, the lasers'
/// modifiers leave (0.25 s into a cut). A pulse keeps the charge where this
/// window and the beam-on one overlap.
pub fn beam_off(ruleset: &Ruleset, rock: &Rock, setup: &Setup) -> ModifiedRock {
    ModifiedRock::new(
        ruleset,
        rock,
        &Setup {
            lasers: vec![],
            gadgets: setup.gadgets.clone(),
            activations: vec![],
        },
    )
}

/// [`rate`] for one laser (or every laser following the plan) on a rock
/// that doesn't change: the rock finder's grades.
pub fn difficulty(
    rock: &ModifiedRock,
    beam_off: &ModifiedRock,
    ruleset: &Ruleset,
    config: &SimConfig,
    runs: u32,
) -> Difficulty {
    let plan = Plan::for_rock(rock, beam_off, ruleset);
    rate(
        &Timeline::from(rock.clone()),
        ruleset,
        &plan,
        &[],
        config,
        runs,
    )
    .difficulty
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn misjudgement_stays_within_the_uncertainty() {
        let plan = Plan {
            climb: vec![],
            keep_low: 0.5,
            keep_high: 0.48,
            minimum: 0.0,
            holds: vec![],
            environment: Environment::Surface,
            pulse: false,
            pulse_window: (0.4, 0.5),
        };
        let mut h = Human::following(7, plan);
        for _ in 0..1000 {
            let e = h.draw();
            assert!(e.abs() <= CHARGE_UNCERTAINTY + 1e-6);
        }
    }

    #[test]
    fn settings_come_in_steps() {
        assert!((stepped(0.574) - 0.57).abs() < 1e-6);
        assert!((stepped(0.936) - 0.94).abs() < 1e-6);
    }
}
