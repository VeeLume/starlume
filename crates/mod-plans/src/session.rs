//! The next session: one trip through the open steps, four ways to choose
//! it.
//!
//! A session is computed from the steps that can be done now and whose place
//! the map knows, starting from the last known place. Quantum legs are timed
//! with the drive the shell hands in; what a stop itself takes is only known
//! for crafting, so no other time is stated.

use serde::Serialize;

use crate::steps::{PlanStep, StepKind};

/// A trip visits at most this many places: the card it is shown on keeps
/// its height.
pub const MAX_STOPS: usize = 3;

/// A place quantum travel can go.
#[derive(Debug, Clone, PartialEq)]
pub struct Site {
    pub id: String,
    pub name: String,
    pub system: String,
    /// System-global position, m.
    pub position: [f64; 3],
    /// Where travel drops out, m from the centre.
    pub arrival: f64,
}

/// A quantum drive as sampled travel times: (distance Gm, seconds with the
/// spool-up), ascending.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Drive {
    pub times: Vec<(f64, f64)>,
}

impl Drive {
    /// Seconds for one jump, or `None` beyond what was sampled.
    pub fn seconds(&self, gm: f64) -> Option<f64> {
        let at = self.times.iter().position(|(d, _)| *d >= gm)?;
        let (d1, t1) = self.times[at];
        if at == 0 {
            return Some(t1);
        }
        let (d0, t0) = self.times[at - 1];
        Some(if d1 > d0 {
            t0 + (t1 - t0) * (gm - d0) / (d1 - d0)
        } else {
            t1
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum TripChoice {
    /// The most steps in one trip.
    Most,
    /// The nearest place with something to do.
    Nearest,
    /// The step that is hardest to come by.
    Rarest,
    /// The place that moves the most plans.
    Plans,
}

impl TripChoice {
    pub const ALL: [TripChoice; 4] = [
        TripChoice::Most,
        TripChoice::Nearest,
        TripChoice::Rarest,
        TripChoice::Plans,
    ];
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Trip {
    pub choice: TripChoice,
    pub stops: Vec<TripStop>,
    /// One sentence on why this trip.
    pub why: String,
    /// All quantum legs together; `None` when one of them could not be
    /// timed (no drive, or out of its range).
    pub travel_seconds: Option<f64>,
    /// The crafting among the steps.
    pub work_seconds: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TripStop {
    pub site: String,
    pub place: String,
    /// The jump that gets you here; `None` for a stop where you already
    /// stand, or a first stop with no known start.
    pub leg: Option<TripLeg>,
    pub steps: Vec<TripStep>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TripLeg {
    pub gm: f64,
    pub seconds: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TripStep {
    pub id: String,
    pub kind: StepKind,
    pub title: String,
    pub seconds: Option<f64>,
}

struct Group<'a> {
    site: &'a Site,
    steps: Vec<&'a PlanStep>,
}

impl Group<'_> {
    fn plans(&self) -> usize {
        let mut plans: Vec<&str> = self
            .steps
            .iter()
            .flat_map(|s| &s.plans)
            .map(String::as_str)
            .collect();
        plans.sort_unstable();
        plans.dedup();
        plans.len()
    }
}

/// One jump from `a` to `b`, Gm.
fn jump(a: &Site, b: &Site) -> f64 {
    let d: f64 = (0..3)
        .map(|i| (a.position[i] - b.position[i]).powi(2))
        .sum::<f64>()
        .sqrt();
    (d - b.arrival).max(0.0) / 1e9
}

/// All four sessions for the open steps. `start` is the site id of the last
/// known place.
pub fn sessions(
    steps: &[PlanStep],
    sites: &[Site],
    start: Option<&str>,
    drive: Option<&Drive>,
) -> Vec<Trip> {
    let start = start.and_then(|id| sites.iter().find(|s| s.id == id));
    let ready: Vec<&PlanStep> = steps.iter().filter(|s| !s.blocked).collect();
    let mut groups: Vec<Group> = Vec::new();
    for step in &ready {
        let Some(site) = step
            .at
            .as_ref()
            .and_then(|w| w.site.as_ref())
            .and_then(|id| sites.iter().find(|s| s.id == *id))
        else {
            continue;
        };
        match groups.iter_mut().find(|g| g.site.id == site.id) {
            Some(g) => g.steps.push(step),
            None => groups.push(Group {
                site,
                steps: vec![step],
            }),
        }
    }
    // One trip stays in one system: positions of two systems do not share a
    // frame. Yours when the start is known, else the one with the most to do.
    let system = match start {
        Some(s) => Some(s.system.clone()),
        None => {
            let mut by: Vec<(&str, usize)> = Vec::new();
            for g in &groups {
                match by.iter_mut().find(|(s, _)| *s == g.site.system) {
                    Some((_, n)) => *n += g.steps.len(),
                    None => by.push((&g.site.system, g.steps.len())),
                }
            }
            by.iter()
                .max_by_key(|(_, n)| *n)
                .map(|(s, _)| (*s).to_owned())
        }
    };
    let placed = !groups.is_empty();
    groups.retain(|g| Some(&g.site.system) == system.as_ref());

    // Distance from the start; 0 when it is unknown, so ties fall back to
    // the order the steps came in.
    let far = |g: &Group| {
        start.map_or(0.0, |s| {
            if s.id == g.site.id {
                0.0
            } else {
                jump(s, g.site)
            }
        })
    };
    let open = ready.len();

    TripChoice::ALL
        .iter()
        .map(|&choice| {
            if groups.is_empty() {
                let why = if open == 0 {
                    "Nothing is open that you could do right now.".to_owned()
                } else if placed {
                    "The open steps with a known place are all in another system.".to_owned()
                } else {
                    "None of the open steps has a place the map knows.".to_owned()
                };
                return Trip { choice, stops: Vec::new(), why, travel_seconds: Some(0.0), work_seconds: 0.0 };
            }
            let (picked, why): (Vec<&Group>, String) = match choice {
                TripChoice::Most => {
                    let mut by: Vec<&Group> = groups.iter().collect();
                    by.sort_by(|a, b| b.steps.len().cmp(&a.steps.len()).then(far(a).total_cmp(&far(b))));
                    by.truncate(MAX_STOPS);
                    let n: usize = by.iter().map(|g| g.steps.len()).sum();
                    let why = format!(
                        "Covers {n} of your {open} open {} in {}.",
                        plural(open, "step"),
                        count(by.len(), "stop")
                    );
                    (by, why)
                }
                TripChoice::Nearest => {
                    let g = groups.iter().min_by(|a, b| far(a).total_cmp(&far(b))).expect("not empty");
                    let first = lower_first(&g.steps[0].title);
                    let why = match start {
                        None => format!("Your last place is not known, so this starts at {}: {first}.", g.site.name),
                        Some(s) if s.id == g.site.id => {
                            format!("The closest thing you still need is where you stand: {first}.")
                        }
                        Some(_) => format!(
                            "The closest thing you still need is at {}, {} away: {first}.",
                            g.site.name,
                            gm_text(far(g))
                        ),
                    };
                    (vec![g], why)
                }
                TripChoice::Rarest => {
                    let all = || groups.iter().flat_map(|g| g.steps.iter().map(move |s| (g, *s)));
                    let mined = all()
                        .filter_map(|(g, s)| s.odds.map(|o| (g, s, o)))
                        .min_by(|a, b| a.2.total_cmp(&b.2));
                    match mined {
                        Some((g, step, odds)) => (
                            vec![g],
                            format!(
                                "{} is the hardest to find of what you need: {} of the rocks at {} hold it.",
                                subject(&step.title),
                                percent(odds),
                                g.site.name
                            ),
                        ),
                        None => {
                            let (g, step) = all()
                                .filter(|(_, s)| s.sources > 0)
                                .min_by_key(|(_, s)| s.sources)
                                .or_else(|| all().next())
                                .expect("not empty");
                            let why = format!(
                                "Nothing open is mined, so this takes the step with the fewest places to do it: {}.",
                                lower_first(&step.title)
                            );
                            (vec![g], why)
                        }
                    }
                }
                TripChoice::Plans => {
                    let g = groups
                        .iter()
                        .min_by(|a, b| {
                            b.plans()
                                .cmp(&a.plans())
                                .then(b.steps.len().cmp(&a.steps.len()))
                                .then(far(a).total_cmp(&far(b)))
                        })
                        .expect("not empty");
                    let why = match g.plans() {
                        1 => format!(
                            "No stop moves more than one plan; {} has the most to do, {}.",
                            g.site.name,
                            count(g.steps.len(), "step")
                        ),
                        n => format!("One stop at {} moves {n} plans.", g.site.name),
                    };
                    (vec![g], why)
                }
            };
            route(choice, picked, start, drive, why)
        })
        .collect()
}

/// Order the stops nearest first from where you are, and time the legs.
fn route(
    choice: TripChoice,
    mut left: Vec<&Group>,
    start: Option<&Site>,
    drive: Option<&Drive>,
    why: String,
) -> Trip {
    let mut stops = Vec::new();
    let mut here = start;
    let mut travel = Some(0.0);
    let mut work = 0.0;
    while !left.is_empty() {
        let next = match here {
            Some(h) => {
                let dist = |g: &Group| {
                    if g.site.id == h.id {
                        0.0
                    } else {
                        jump(h, g.site)
                    }
                };
                (0..left.len())
                    .min_by(|a, b| dist(left[*a]).total_cmp(&dist(left[*b])))
                    .expect("not empty")
            }
            None => 0,
        };
        let g = left.remove(next);
        let leg = here.filter(|h| h.id != g.site.id).map(|h| {
            let gm = jump(h, g.site);
            TripLeg {
                gm,
                seconds: drive.and_then(|d| d.seconds(gm)),
            }
        });
        if let Some(leg) = &leg {
            travel = travel.zip(leg.seconds).map(|(a, b)| a + b);
        }
        work += g.steps.iter().filter_map(|s| s.seconds).sum::<f64>();
        stops.push(TripStop {
            site: g.site.id.clone(),
            place: g.site.name.clone(),
            leg,
            steps: g
                .steps
                .iter()
                .map(|s| TripStep {
                    id: s.id.clone(),
                    kind: s.kind,
                    title: s.title.clone(),
                    seconds: s.seconds,
                })
                .collect(),
        });
        here = Some(g.site);
    }
    Trip {
        choice,
        stops,
        why,
        travel_seconds: travel,
        work_seconds: work,
    }
}

fn plural(n: usize, one: &str) -> String {
    if n == 1 {
        one.to_owned()
    } else {
        format!("{one}s")
    }
}

fn count(n: usize, one: &str) -> String {
    format!("{n} {}", plural(n, one))
}

fn gm_text(gm: f64) -> String {
    if gm >= 10.0 {
        format!("{gm:.0} Gm")
    } else {
        format!("{gm:.1} Gm")
    }
}

/// A sampled share, worded as the estimate it is.
fn percent(share: f32) -> String {
    let pct = share * 100.0;
    if pct > 0.0 && pct < 1.0 {
        "under 1%".to_owned()
    } else {
        format!("about {pct:.0}%")
    }
}

/// "Mine 2 SCU Ore A, Q700+" → "Ore A, Q700+": the material a mining step
/// names, for a sentence about it.
fn subject(title: &str) -> String {
    title
        .split_once(" SCU ")
        .or_else(|| title.split_once(" \u{d7} "))
        .map_or_else(|| title.to_owned(), |(_, rest)| rest.to_owned())
}

fn lower_first(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::steps::PlanPlace;

    /// Sites on a line, 1 Gm apart per unit, in one system.
    fn site(id: &str, x: f64) -> Site {
        Site {
            id: id.into(),
            name: format!("Place {id}"),
            system: "Sys".into(),
            position: [x * 1e9, 0.0, 0.0],
            arrival: 0.0,
        }
    }

    fn step(id: &str, kind: StepKind, site: Option<&str>, plans: &[&str]) -> PlanStep {
        PlanStep {
            id: id.into(),
            kind,
            title: format!("Do {id}"),
            detail: String::new(),
            plans: plans.iter().map(|p| (*p).to_owned()).collect(),
            blocked: false,
            at: site.map(|s| PlanPlace {
                name: format!("Place {s}"),
                site: Some(s.into()),
            }),
            seconds: None,
            cost: None,
            odds: None,
            sources: 1,
        }
    }

    /// 60 s spool, then 10 s a Gm, sampled to 100 Gm.
    fn drive() -> Drive {
        Drive {
            times: vec![(0.0, 60.0), (100.0, 1060.0)],
        }
    }

    fn world() -> (Vec<PlanStep>, Vec<Site>) {
        let sites = vec![
            site("home", 0.0),
            site("near", 2.0),
            site("mid", 10.0),
            site("far", 30.0),
        ];
        let mut rare = step("ore-rare", StepKind::Mine, Some("far"), &["p1"]);
        rare.title = "Mine 2 SCU Ore R, Q700+".into();
        rare.odds = Some(0.04);
        let mut common = step("ore-common", StepKind::Mine, Some("mid"), &["p1", "p2"]);
        common.odds = Some(0.5);
        let steps = vec![
            step("fit", StepKind::Fit, Some("near"), &["p1"]),
            common,
            step("buy", StepKind::Buy, Some("mid"), &["p3"]),
            rare,
            step("nowhere", StepKind::Find, None, &["p1"]),
        ];
        (steps, sites)
    }

    fn pick(sessions: &[Trip], choice: TripChoice) -> &Trip {
        sessions.iter().find(|s| s.choice == choice).unwrap()
    }

    fn places(session: &Trip) -> Vec<&str> {
        session.stops.iter().map(|s| s.site.as_str()).collect()
    }

    #[test]
    fn most_takes_the_fullest_places_and_flies_them_nearest_first() {
        let (steps, sites) = world();
        let all = sessions(&steps, &sites, Some("home"), Some(&drive()));
        let most = pick(&all, TripChoice::Most);
        assert_eq!(places(most), ["near", "mid", "far"]);
        assert_eq!(most.why, "Covers 4 of your 5 open steps in 3 stops.");
        // 2 + 8 + 20 Gm at 10 s a Gm, three spools of 60 s.
        assert_eq!(most.travel_seconds, Some(480.0));
        assert_eq!(
            most.stops[1].leg,
            Some(TripLeg {
                gm: 8.0,
                seconds: Some(140.0)
            })
        );
        assert_eq!(most.stops[1].steps.len(), 2);
    }

    #[test]
    fn nearest_is_one_stop_and_none_when_you_stand_there() {
        let (steps, sites) = world();
        let all = sessions(&steps, &sites, Some("near"), Some(&drive()));
        let near = pick(&all, TripChoice::Nearest);
        assert_eq!(places(near), ["near"]);
        assert_eq!(near.stops[0].leg, None);
        assert_eq!(near.travel_seconds, Some(0.0));
        assert_eq!(
            near.why,
            "The closest thing you still need is where you stand: do fit."
        );
    }

    #[test]
    fn rarest_goes_for_the_ore_fewest_rocks_hold() {
        let (steps, sites) = world();
        let all = sessions(&steps, &sites, Some("home"), Some(&drive()));
        let rare = pick(&all, TripChoice::Rarest);
        assert_eq!(places(rare), ["far"]);
        assert_eq!(
            rare.why,
            "Ore R, Q700+ is the hardest to find of what you need: about 4% of the rocks at Place far hold it."
        );
    }

    #[test]
    fn plans_goes_where_one_stop_moves_the_most_plans() {
        let (steps, sites) = world();
        let all = sessions(&steps, &sites, Some("home"), Some(&drive()));
        let plans = pick(&all, TripChoice::Plans);
        assert_eq!(places(plans), ["mid"]);
        assert_eq!(plans.why, "One stop at Place mid moves 3 plans.");
    }

    #[test]
    fn blocked_steps_and_other_systems_stay_out_of_the_trip() {
        let (mut steps, mut sites) = world();
        steps[0].blocked = true;
        sites[3].system = "Elsewhere".into();
        let all = sessions(&steps, &sites, Some("home"), Some(&drive()));
        assert_eq!(places(pick(&all, TripChoice::Most)), ["mid"]);
        // The rare ore is in the other system: the common one is what is left.
        assert_eq!(places(pick(&all, TripChoice::Rarest)), ["mid"]);
    }

    #[test]
    fn steps_that_are_all_in_another_system_are_named_as_that() {
        let (steps, mut sites) = world();
        for site in sites.iter_mut().skip(1) {
            site.system = "Elsewhere".into();
        }
        let all = sessions(&steps, &sites, Some("home"), Some(&drive()));
        assert!(all.iter().all(|t| t.stops.is_empty()));
        assert_eq!(
            all[0].why,
            "The open steps with a known place are all in another system."
        );
    }

    #[test]
    fn without_a_start_or_a_drive_no_time_is_made_up() {
        let (steps, sites) = world();
        let all = sessions(&steps, &sites, None, None);
        let most = pick(&all, TripChoice::Most);
        assert_eq!(most.stops[0].leg, None);
        assert!(most.stops[1].leg.is_some_and(|l| l.seconds.is_none()));
        assert_eq!(most.travel_seconds, None);
        assert!(
            pick(&all, TripChoice::Nearest)
                .why
                .starts_with("Your last place is not known")
        );
    }

    #[test]
    fn a_jump_beyond_the_drives_samples_has_no_time() {
        let (steps, sites) = world();
        let short = Drive {
            times: vec![(0.0, 60.0), (5.0, 110.0)],
        };
        let all = sessions(&steps, &sites, Some("home"), Some(&short));
        let most = pick(&all, TripChoice::Most);
        assert_eq!(most.stops[0].leg.unwrap().seconds, Some(80.0));
        assert_eq!(most.stops[1].leg.unwrap().seconds, None);
        assert_eq!(most.travel_seconds, None);
    }

    #[test]
    fn craft_time_is_the_only_work_time_counted() {
        let (mut steps, sites) = world();
        let mut craft = step("craft", StepKind::Craft, Some("near"), &["p1"]);
        craft.seconds = Some(510.0);
        steps.push(craft);
        let all = sessions(&steps, &sites, Some("home"), Some(&drive()));
        assert_eq!(pick(&all, TripChoice::Nearest).work_seconds, 510.0);
    }

    #[test]
    fn nothing_placed_gives_four_empty_sessions_that_say_why() {
        let steps = vec![step("nowhere", StepKind::Find, None, &["p1"])];
        let all = sessions(&steps, &[], Some("home"), None);
        assert_eq!(all.len(), 4);
        assert!(all.iter().all(|s| s.stops.is_empty()));
        assert_eq!(
            all[0].why,
            "None of the open steps has a place the map knows."
        );
        let none = sessions(&[], &[], None, None);
        assert_eq!(none[0].why, "Nothing is open that you could do right now.");
    }
}
