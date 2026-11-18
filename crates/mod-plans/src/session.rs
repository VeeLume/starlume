//! The next sessions as parallel tracks: one per kind of work, each with the
//! trip it would fly next (decision 'Plans show parallel tracks by kind of
//! work, each with its next trip, ordered by sc-travel', 2026-11-16).
//!
//! Kinds of work differ in length and run side by side: a shopping run fits
//! one session, mining and missions take several, a refinery works for hours
//! or days while you do something else, crafting follows refining. So every
//! kind gets its own [`Track`], and a step that follows another (refining the
//! ore a mining step gives) waits in its track until that step is done.
//!
//! This crate is pure: it decides which stops a trip has and what each
//! stop's work is, and asks the shell's [`Router`] for the order. The shell
//! answers with sc-travel (holotable's travel graph, crossing systems
//! through jump points). Without a router (no ship with a drive) a trip
//! keeps its stops in step order, without legs.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::steps::{PlanPlace, PlanStep, REFINE, StepKind, place_refine};

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

/// One jump from `a` to `b`, Gm. Only meaningful within one system.
fn jump(a: &Site, b: &Site) -> f64 {
    let d: f64 = (0..3)
        .map(|i| (a.position[i] - b.position[i]).powi(2))
        .sum::<f64>()
        .sqrt();
    (d - b.arrival).max(0.0) / 1e9
}

/// The one of `among` (site ids) that is the shortest jump from `from`, in
/// `from`'s system; `from` itself when it is among them. `None` when `from`
/// is not on the map or none of them is in its system (two systems share no
/// frame, so there is no distance between them).
pub fn nearest<'a>(
    sites: &[Site],
    from: &str,
    among: impl IntoIterator<Item = &'a str>,
) -> Option<&'a str> {
    let from = sites.iter().find(|s| s.id == from)?;
    among
        .into_iter()
        .filter_map(|id| {
            let site = sites.iter().find(|s| s.id == id)?;
            (site.system == from.system).then(|| {
                let gm = if site.id == from.id {
                    0.0
                } else {
                    jump(from, site)
                };
                (id, gm)
            })
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(id, _)| id)
}

/// A kind of work, one track each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum TrackKind {
    /// Fit parts you hold, buy parts: one session.
    Shopping,
    /// Earn blueprints from missions.
    Missions,
    /// Mine (or otherwise gather) materials: hours or sessions.
    Mining,
    /// Take ore to a refinery and wait for it.
    Refining,
    /// Loot parts from containers in the world.
    Looting,
    /// Craft from what you hold.
    Crafting,
}

impl TrackKind {
    /// Track order on the page, and the order the next session is picked
    /// in: the order the work flows. A shopping run first (it fits one
    /// session and unblocks fitting), then mining, the refinery the ore
    /// goes to, missions, looting, crafting.
    pub const ALL: [TrackKind; 6] = [
        TrackKind::Shopping,
        TrackKind::Mining,
        TrackKind::Refining,
        TrackKind::Missions,
        TrackKind::Looting,
        TrackKind::Crafting,
    ];

    /// The track a step belongs to; `None` for a step no work gets done
    /// (a part nobody sells or makes).
    pub fn of(kind: StepKind) -> Option<Self> {
        match kind {
            StepKind::Fit | StepKind::Buy => Some(TrackKind::Shopping),
            StepKind::Earn | StepKind::Rep | StepKind::Run => Some(TrackKind::Missions),
            StepKind::Mine | StepKind::Gather => Some(TrackKind::Mining),
            StepKind::Refine => Some(TrackKind::Refining),
            StepKind::Loot => Some(TrackKind::Looting),
            StepKind::Craft => Some(TrackKind::Crafting),
            StepKind::Find => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub kind: TrackKind,
    /// Open steps of this kind, ready or not.
    pub open: u32,
    /// Steps that can be done now.
    pub ready: u32,
    /// The track every waiting step waits on, when nothing here is ready
    /// because another track comes first (refining waits on mining).
    pub waits_on: Option<TrackKind>,
    /// The next trip over the ready steps with a known place.
    pub trip: Trip,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Trip {
    pub stops: Vec<TripStop>,
    /// Arrival at the last stop, seconds from the start: travel, arrivals
    /// and the work timed at the stops before it. `None` when the trip was
    /// not ordered (no drive).
    pub travel_seconds: Option<f64>,
    /// The timed work among the ready steps (crafting).
    pub work_seconds: f64,
    /// A leg is an estimate (a place on a rotating body, a detour, a spline).
    pub estimate: bool,
    /// Ready steps whose place the map does not know (crafting has none).
    pub unplaced: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TripStop {
    pub site: String,
    pub place: String,
    /// Seconds from the start on arrival; `None` when not ordered.
    pub at: Option<f64>,
    /// The travel that gets you here; `None` for a stop where you already
    /// stand, a first stop with no known start, or an unordered trip.
    pub leg: Option<TripLeg>,
    pub steps: Vec<TripStep>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TripLeg {
    pub gm: f64,
    pub seconds: Option<f64>,
    /// The leg goes through a jump point into another system.
    pub crosses: bool,
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

/// One stop the router orders: any one of `sites`, `dwell` seconds of work
/// there, after the stops `after` (indexes into the same list).
#[derive(Debug, Clone, PartialEq)]
pub struct StopSpec {
    pub sites: Vec<String>,
    pub dwell: f64,
    pub after: Vec<usize>,
}

/// The router's answer: the stops in the order to fly them.
#[derive(Debug, Clone, PartialEq)]
pub struct Ordered {
    pub visits: Vec<OrderedVisit>,
    pub estimate: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OrderedVisit {
    /// Index into the stops asked about.
    pub stop: usize,
    /// The site chosen among the stop's.
    pub site: String,
    /// Seconds from the start on arrival.
    pub at: f64,
    pub leg: Option<TripLeg>,
}

/// Orders a trip: the shell's travel graph and drive. `start` is the site
/// the player stands at, when known.
pub trait Router {
    fn order(&self, start: Option<&str>, stops: &[StopSpec]) -> Option<Ordered>;
}

/// Every track, and the one the next session suggests.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Tracks {
    pub tracks: Vec<Track>,
    /// The first track (in [`TrackKind::ALL`] order) with something ready
    /// to do; `None` when nothing is.
    pub next: Option<TrackKind>,
}

/// The tracks for the open `steps`. `start` is the site id of the last
/// known place.
pub fn tracks(
    steps: &[PlanStep],
    sites: &[Site],
    start: Option<&str>,
    router: Option<&dyn Router>,
) -> Tracks {
    let open: HashMap<&str, &PlanStep> = steps.iter().map(|s| (s.id.as_str(), s)).collect();
    let site = |s: &PlanStep| {
        s.at.as_ref()
            .and_then(|w| w.site.as_deref())
            .and_then(|id| sites.iter().find(|x| x.id == id))
    };
    let start = start.filter(|id| sites.iter().any(|s| s.id == *id));

    let mut out = Vec::new();
    for kind in TrackKind::ALL {
        let mine: Vec<&PlanStep> = steps
            .iter()
            .filter(|s| TrackKind::of(s.kind) == Some(kind))
            .collect();
        if mine.is_empty() {
            continue;
        }
        // A step waits while a step it follows is still open.
        let waiting_on = |s: &PlanStep| {
            s.after
                .iter()
                .filter_map(|a| open.get(a.as_str()))
                .find_map(|a| TrackKind::of(a.kind))
        };
        let ready: Vec<&PlanStep> = mine
            .iter()
            .copied()
            .filter(|s| !s.blocked && waiting_on(s).is_none())
            .collect();
        let waits_on = if ready.is_empty() {
            mine.iter().find_map(|s| waiting_on(s))
        } else {
            None
        };

        // One stop per place: two steps at one shop are one arrival.
        let mut groups: Vec<(&Site, Vec<&PlanStep>)> = Vec::new();
        let mut unplaced = 0;
        for s in &ready {
            match site(s) {
                Some(at) => match groups.iter_mut().find(|(g, _)| g.id == at.id) {
                    Some((_, list)) => list.push(s),
                    None => groups.push((at, vec![s])),
                },
                None => unplaced += 1,
            }
        }
        let specs: Vec<StopSpec> = groups
            .iter()
            .map(|(at, list)| StopSpec {
                sites: vec![at.id.clone()],
                dwell: list.iter().filter_map(|s| s.seconds).sum(),
                after: Vec::new(),
            })
            .collect();
        let ordered = if specs.is_empty() {
            None
        } else {
            router.and_then(|r| r.order(start, &specs))
        };
        let trip_step = |s: &PlanStep| TripStep {
            id: s.id.clone(),
            kind: s.kind,
            title: s.title.clone(),
            seconds: s.seconds,
        };
        let stop = |i: usize, at: Option<f64>, leg: Option<TripLeg>| TripStop {
            site: groups[i].0.id.clone(),
            place: groups[i].0.name.clone(),
            at,
            leg,
            steps: groups[i].1.iter().map(|s| trip_step(s)).collect(),
        };
        let (stops, travel_seconds, estimate) = match ordered {
            Some(o) => {
                let last = o.visits.last().map(|v| v.at);
                let stops = o
                    .visits
                    .iter()
                    .map(|v| stop(v.stop, Some(v.at), v.leg))
                    .collect();
                (stops, last, o.estimate)
            }
            None => (
                (0..groups.len()).map(|i| stop(i, None, None)).collect(),
                None,
                false,
            ),
        };
        out.push(Track {
            kind,
            open: mine.len() as u32,
            ready: ready.len() as u32,
            waits_on,
            trip: Trip {
                stops,
                travel_seconds,
                work_seconds: ready.iter().filter_map(|s| s.seconds).sum(),
                estimate,
                unplaced,
            },
        });
    }
    let next = out
        .iter()
        .find(|t| !t.trip.stops.is_empty() || (t.ready > 0 && t.kind == TrackKind::Crafting))
        .map(|t| t.kind);
    Tracks { tracks: out, next }
}

/// A plan's steps, for its own trips: its own, and the other plans' mining
/// that feeds a refine step of its (one load mines it anyway, and the
/// refining waits on it). Board order.
pub fn plan_steps(steps: &[PlanStep], plan: &str) -> Vec<PlanStep> {
    let own = |s: &PlanStep| s.plans.iter().any(|p| p == plan);
    let feeds: HashSet<&str> = steps
        .iter()
        .filter(|s| own(s) && s.kind == StepKind::Refine)
        .flat_map(|s| s.after.iter().map(String::as_str))
        .collect();
    steps
        .iter()
        .filter(|s| own(s) || feeds.contains(s.id.as_str()))
        .cloned()
        .collect()
}

/// Move each load's refine step beside the last stop the mining trip makes
/// for that load: its ore is flown from there to the refinery `near` that
/// stop (the shell's nearest, in its system). A load whose mining has no
/// stop on the trip, or whose last stop `near` knows no refinery for,
/// keeps its area's.
pub fn refine_after_mining(
    steps: &mut [PlanStep],
    tracks: &Tracks,
    near: &dyn Fn(&str) -> Option<PlanPlace>,
) {
    let Some(trip) = tracks
        .tracks
        .iter()
        .find(|t| t.kind == TrackKind::Mining)
        .map(|t| &t.trip)
    else {
        return;
    };
    for step in steps.iter_mut().filter(|s| s.id.starts_with(REFINE)) {
        let last = trip
            .stops
            .iter()
            .rev()
            .find(|stop| stop.steps.iter().any(|s| step.after.contains(&s.id)));
        if let Some(at) = last.and_then(|stop| near(&stop.site)) {
            place_refine(step, Some(at));
        }
    }
}

/// Sort `steps`: what can be done now before what waits (blocked, or
/// following a step still open), each by track, and within a track in the
/// order its trip flies them; steps outside a trip keep their order after
/// it.
pub fn sort_steps(steps: &mut [PlanStep], tracks: &Tracks) {
    let open: HashSet<&str> = steps.iter().map(|s| s.id.as_str()).collect();
    let waits = |s: &PlanStep| s.blocked || s.after.iter().any(|a| open.contains(a.as_str()));
    let mut place: HashMap<&str, usize> = HashMap::new();
    for t in &tracks.tracks {
        for (i, stop) in t.trip.stops.iter().enumerate() {
            for s in &stop.steps {
                place.entry(s.id.as_str()).or_insert(i);
            }
        }
    }
    let rank = |k: StepKind| {
        TrackKind::of(k)
            .and_then(|t| TrackKind::ALL.iter().position(|x| *x == t))
            .unwrap_or(TrackKind::ALL.len())
    };
    let keys: HashMap<String, (bool, usize, usize)> = steps
        .iter()
        .map(|s| {
            (
                s.id.clone(),
                (
                    waits(s),
                    rank(s.kind),
                    place.get(s.id.as_str()).copied().unwrap_or(usize::MAX),
                ),
            )
        })
        .collect();
    steps.sort_by_key(|s| keys[&s.id]);
    // A load's refining reads right after its mining, waiting or not: the
    // ore goes from the last rock to the refinery.
    let refines: Vec<String> = steps
        .iter()
        .filter(|s| s.kind == StepKind::Refine && !s.after.is_empty())
        .map(|s| s.id.clone())
        .collect();
    for id in refines {
        let Some(at) = steps.iter().position(|s| s.id == id) else {
            continue;
        };
        let after = &steps[at].after;
        let Some(last) = steps.iter().rposition(|s| after.contains(&s.id)) else {
            continue;
        };
        if last > at {
            steps[at..=last].rotate_left(1);
        } else {
            steps[last + 1..=at].rotate_right(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn step(id: &str, kind: StepKind, site: Option<&str>) -> PlanStep {
        PlanStep {
            id: id.into(),
            kind,
            title: format!("Do {id}"),
            detail: String::new(),
            plans: vec!["p1".into()],
            blocked: false,
            at: site.map(|s| PlanPlace {
                name: format!("Place {s}"),
                site: Some(s.into()),
            }),
            seconds: None,
            cost: None,
            odds: None,
            sources: 1,
            after: Vec::new(),
            parts: 0,
            rocks: None,
            scu: 0.0,
        }
    }

    /// Orders stops farthest first, so a test can tell routed from not.
    struct Backwards;

    impl Router for Backwards {
        fn order(&self, _: Option<&str>, stops: &[StopSpec]) -> Option<Ordered> {
            Some(Ordered {
                visits: (0..stops.len())
                    .rev()
                    .enumerate()
                    .map(|(n, i)| OrderedVisit {
                        stop: i,
                        site: stops[i].sites[0].clone(),
                        at: 100.0 * (n + 1) as f64,
                        leg: Some(TripLeg {
                            gm: 1.0,
                            seconds: Some(100.0),
                            crosses: false,
                        }),
                    })
                    .collect(),
                estimate: true,
            })
        }
    }

    fn sites() -> Vec<Site> {
        vec![site("a", 1.0), site("b", 2.0), site("c", 3.0)]
    }

    #[test]
    fn each_kind_of_work_is_its_own_track() {
        let steps = vec![
            step("fit", StepKind::Fit, Some("a")),
            step("buy", StepKind::Buy, Some("b")),
            step("mine", StepKind::Mine, Some("c")),
            step("earn", StepKind::Earn, None),
        ];
        let t = tracks(&steps, &sites(), None, Some(&Backwards));
        let kinds: Vec<TrackKind> = t.tracks.iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            [TrackKind::Shopping, TrackKind::Mining, TrackKind::Missions]
        );
        let shop = &t.tracks[0];
        assert_eq!(shop.trip.stops.len(), 2);
        assert_eq!(t.next, Some(TrackKind::Shopping));
        // A mission without a place is counted, not a stop.
        assert_eq!(t.tracks[2].trip.unplaced, 1);
    }

    #[test]
    fn the_router_orders_the_trip_and_times_it() {
        let steps = vec![
            step("fit", StepKind::Fit, Some("a")),
            step("buy", StepKind::Buy, Some("b")),
        ];
        let t = tracks(&steps, &sites(), Some("a"), Some(&Backwards));
        let trip = &t.tracks[0].trip;
        let order: Vec<&str> = trip.stops.iter().map(|s| s.site.as_str()).collect();
        assert_eq!(order, ["b", "a"]);
        assert_eq!(trip.travel_seconds, Some(200.0));
        assert!(trip.estimate);
    }

    #[test]
    fn two_steps_at_one_place_are_one_stop() {
        let steps = vec![
            step("fit", StepKind::Fit, Some("a")),
            step("buy", StepKind::Buy, Some("a")),
        ];
        let t = tracks(&steps, &sites(), None, Some(&Backwards));
        assert_eq!(t.tracks[0].trip.stops.len(), 1);
        assert_eq!(t.tracks[0].trip.stops[0].steps.len(), 2);
    }

    #[test]
    fn without_a_router_the_stops_keep_step_order_and_no_times() {
        let steps = vec![
            step("fit", StepKind::Fit, Some("a")),
            step("buy", StepKind::Buy, Some("b")),
        ];
        let t = tracks(&steps, &sites(), None, None);
        let trip = &t.tracks[0].trip;
        let order: Vec<&str> = trip.stops.iter().map(|s| s.site.as_str()).collect();
        assert_eq!(order, ["a", "b"]);
        assert_eq!(trip.travel_seconds, None);
        assert!(trip.stops.iter().all(|s| s.leg.is_none() && s.at.is_none()));
    }

    #[test]
    fn refining_waits_on_the_mining_it_follows() {
        let mine = step("mine", StepKind::Mine, Some("c"));
        let mut refine = step("refine", StepKind::Refine, Some("b"));
        refine.after = vec!["mine".into()];
        let t = tracks(&[mine, refine], &sites(), None, Some(&Backwards));
        let refining = t
            .tracks
            .iter()
            .find(|t| t.kind == TrackKind::Refining)
            .unwrap();
        assert_eq!(refining.ready, 0);
        assert_eq!(refining.waits_on, Some(TrackKind::Mining));
        assert!(refining.trip.stops.is_empty());
        // Refining comes before mining in the pick order, but it waits.
        assert_eq!(t.next, Some(TrackKind::Mining));
    }

    #[test]
    fn a_blocked_step_is_open_but_not_ready() {
        let mut fit = step("fit", StepKind::Fit, Some("a"));
        fit.blocked = true;
        let t = tracks(&[fit], &sites(), None, Some(&Backwards));
        assert_eq!((t.tracks[0].open, t.tracks[0].ready), (1, 0));
        assert_eq!(t.next, None);
    }

    #[test]
    fn crafting_has_no_place_but_can_be_next() {
        let mut craft = step("craft", StepKind::Craft, None);
        craft.seconds = Some(90.0);
        let t = tracks(&[craft], &sites(), None, Some(&Backwards));
        assert_eq!(t.tracks[0].trip.work_seconds, 90.0);
        assert_eq!(t.next, Some(TrackKind::Crafting));
    }

    #[test]
    fn steps_sort_by_track_then_trip_order() {
        let mut steps = vec![
            step("mine", StepKind::Mine, Some("c")),
            step("fit", StepKind::Fit, Some("a")),
            step("buy", StepKind::Buy, Some("b")),
        ];
        let t = tracks(&steps, &sites(), None, Some(&Backwards));
        sort_steps(&mut steps, &t);
        let ids: Vec<&str> = steps.iter().map(|s| s.id.as_str()).collect();
        // Shopping first, flown b then a; mining after.
        assert_eq!(ids, ["buy", "fit", "mine"]);
    }

    /// The playtest's plan read backwards: the refinery came before the
    /// mining that gives it the ore.
    #[test]
    fn mining_comes_before_the_refinery_it_feeds() {
        let mine = step("mine", StepKind::Mine, Some("a"));
        let mut refine = step("refine", StepKind::Refine, Some("b"));
        refine.after = vec!["mine".into()];
        let mut craft = step("craft", StepKind::Craft, None);
        craft.blocked = true;
        let mut steps = vec![craft, refine, mine];
        let t = tracks(&steps, &sites(), None, Some(&Backwards));
        sort_steps(&mut steps, &t);
        let ids: Vec<&str> = steps.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["mine", "refine", "craft"]);
        let kinds: Vec<TrackKind> = t.tracks.iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            [TrackKind::Mining, TrackKind::Refining, TrackKind::Crafting]
        );
    }

    /// A plan's steps take the other plans' mining its refining waits on,
    /// and nothing else of theirs.
    #[test]
    fn a_plan_takes_the_mining_its_refinery_waits_on() {
        let mut mine_a = step("m-a", StepKind::Mine, Some("a"));
        mine_a.plans = vec!["p1".into()];
        let mut mine_b = step("m-b", StepKind::Mine, Some("b"));
        mine_b.plans = vec!["p2".into()];
        let mut far = step("m-far", StepKind::Mine, Some("c"));
        far.plans = vec!["p2".into()];
        let mut refine = step("refine:x", StepKind::Refine, Some("a"));
        refine.plans = vec!["p1".into(), "p2".into()];
        refine.after = vec!["m-a".into(), "m-b".into()];
        let steps = vec![mine_a, mine_b, far, refine];
        let ids: Vec<String> = plan_steps(&steps, "p1").into_iter().map(|s| s.id).collect();
        assert_eq!(ids, ["m-a", "m-b", "refine:x"]);
    }

    /// Each load's refining reads right after its own mining, not after
    /// every ready step (the playtest list put both refineries below a
    /// mission).
    #[test]
    fn a_load_refines_right_after_its_mining() {
        let mut refine_x = step("refine:x", StepKind::Refine, Some("a"));
        refine_x.after = vec!["m1".into()];
        let mut refine_y = step("refine:y", StepKind::Refine, Some("c"));
        refine_y.after = vec!["m2".into(), "m3".into()];
        let mut steps = vec![
            step("earn", StepKind::Earn, Some("b")),
            refine_y,
            refine_x,
            step("m3", StepKind::Mine, Some("c")),
            step("m2", StepKind::Mine, Some("b")),
            step("m1", StepKind::Mine, Some("a")),
        ];
        let t = tracks(&steps, &sites(), None, Some(&Backwards));
        sort_steps(&mut steps, &t);
        let ids: Vec<&str> = steps.iter().map(|s| s.id.as_str()).collect();
        // The stops listed c, b, a, flown backwards: m1, then m2 and m3.
        assert_eq!(ids, ["m1", "refine:x", "m2", "m3", "refine:y", "earn"]);
    }

    /// The refinery moves beside the mining trip's last stop: flown
    /// backwards, c then b then a, so the one nearest a.
    #[test]
    fn the_refinery_follows_the_last_mining_stop() {
        let mut refine = step(&format!("{REFINE}c"), StepKind::Refine, Some("c"));
        refine.title = "Refine the ore at Place c".into();
        refine.after = vec!["m1".into(), "m2".into(), "m3".into()];
        let mut steps = vec![
            step("m1", StepKind::Mine, Some("a")),
            step("m2", StepKind::Mine, Some("b")),
            step("m3", StepKind::Mine, Some("c")),
            refine,
        ];
        let t = tracks(&steps, &sites(), None, Some(&Backwards));
        let near = |site: &str| {
            Some(PlanPlace {
                name: format!("Refinery by {site}"),
                site: Some(format!("r-{site}")),
            })
        };
        refine_after_mining(&mut steps, &t, &near);
        let refine = steps.iter().find(|s| s.id.starts_with(REFINE)).unwrap();
        assert_eq!(refine.title, "Refine the ore at Refinery by a");
        assert_eq!(
            refine.at.as_ref().and_then(|p| p.site.as_deref()),
            Some("r-a")
        );
    }

    /// Two ready steps (ore for the refinery already in hand) keep the
    /// work order among themselves.
    #[test]
    fn ready_steps_keep_the_work_order() {
        let mine = step("mine", StepKind::Mine, Some("a"));
        let refine = step("refine", StepKind::Refine, Some("b"));
        let mut steps = vec![refine, mine];
        let t = tracks(&steps, &sites(), None, Some(&Backwards));
        sort_steps(&mut steps, &t);
        let ids: Vec<&str> = steps.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["mine", "refine"]);
    }

    #[test]
    fn nearest_stays_in_its_system() {
        let mut other = site("x", 0.5);
        other.system = "Other".into();
        let s = [site("a", 1.0), site("b", 5.0), other];
        assert_eq!(nearest(&s, "a", ["b", "x"]), Some("b"));
        assert_eq!(nearest(&s, "x", ["a", "b"]), None);
    }
}
