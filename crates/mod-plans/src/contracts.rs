//! Rank and mission plans: the path to a rank with a faction, or to a
//! mission, as steps.
//!
//! A rank is reached by running missions that give its faction rep: one
//! step a rank on the way, each with the runs it takes of the repeatable
//! mission open at that rank that gives the most rep. A mission is opened by
//! the ranks it needs and the missions before it, then run.
//!
//! Your rank comes from the standing read (Contracts, Standing). A ladder the
//! read has no row for is unknown, never Neutral: its plan waits on the read.

use std::collections::HashMap;

use crate::model::{MissionGoal, Plan, RankGoal};
use crate::steps::{
    Grant, PlanBoard, PlanKind, PlanPlace, PlanStage, PlanStep, RunTime, StageState, StepKind,
    auec_figure, missing, percent, tile,
};

/// Everything rank and mission plans read, gathered by the shell; empty when
/// none is planned.
#[derive(Debug, Clone, Default)]
pub struct ContractInputs {
    /// By [`ladder_key`].
    pub ladders: HashMap<String, RepLadder>,
    /// By mission id: the missions planned.
    pub missions: HashMap<String, MissionFacts>,
}

/// A ladder is one org's rep on one scope.
pub fn ladder_key(geid: &str, scope: &str) -> String {
    format!("{geid}|{scope}")
}

/// One faction's ladder, your score on it and the missions that move it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RepLadder {
    pub faction: String,
    /// Lowest first.
    pub ranks: Vec<RankFacts>,
    /// Your score; `None` when the standing was not read or has no row for
    /// it (no row is not Neutral).
    pub score: Option<i64>,
    /// Repeatable missions that give rep on it.
    pub sources: Vec<RepSource>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RankFacts {
    /// The standing GUID.
    pub id: String,
    pub name: String,
    /// The rep it begins at.
    pub min: i64,
}

/// A repeatable mission that gives rep on a ladder, open from rank `lo` to
/// rank `hi` (inclusive).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RepSource {
    pub title: String,
    pub lo: usize,
    pub hi: usize,
    /// Rep one run gives.
    pub rep: i64,
    /// Its time from the best source.
    pub time: Option<RunTime>,
    pub at: Vec<PlanPlace>,
}

/// A rank window on a ladder (inclusive).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RankWindow {
    pub ladder: String,
    pub lo: usize,
    pub hi: usize,
}

/// A mission as a plan reads it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MissionFacts {
    pub title: String,
    pub faction: String,
    pub once_only: bool,
    /// You completed one of its contracts (the token wallet).
    pub done: bool,
    /// The ranks it needs.
    pub needs: Vec<RankWindow>,
    /// The ranks that close it.
    pub closed_at: Vec<RankWindow>,
    /// The missions before it, by title, and whether you did each.
    pub chain: Vec<(String, bool)>,
    /// Its time from the best source.
    pub time: Option<RunTime>,
    pub at: Vec<PlanPlace>,
    /// aUEC a run pays.
    pub pay: Option<f64>,
    /// Rep with its own faction a run gives.
    pub rep: Option<i64>,
    /// The chance a run gives a blueprint, 0..=1.
    pub blueprint: Option<f64>,
}

impl RepLadder {
    /// Your rank: the highest whose start your score reaches (the lowest
    /// below them all).
    pub fn rank_of(&self, score: i64) -> usize {
        self.ranks.iter().rposition(|r| score >= r.min).unwrap_or(0)
    }

    fn you(&self) -> Option<usize> {
        self.score.map(|s| self.rank_of(s))
    }
}

/// The steps of every rank and mission plan not parked, in plan order, then
/// `extra` (the paths to missions that grant a planned blueprint). A step
/// two plans share (the same rank on the way) is one step for both.
pub fn steps(plans: &[Plan], inputs: &ContractInputs, extra: Vec<PlanStep>) -> Vec<PlanStep> {
    let mut out: Vec<PlanStep> = Vec::new();
    let mut add = |step: PlanStep| match out.iter_mut().find(|s| s.id == step.id) {
        Some(have) => {
            for p in step.plans {
                if !have.plans.contains(&p) {
                    have.plans.push(p);
                }
            }
        }
        None => out.push(step),
    };
    for plan in plans.iter().filter(|p| !p.parked) {
        match &plan.goal {
            crate::Goal::Rank(goal) => {
                let key = ladder_key(&goal.geid, &goal.scope);
                if let Some(ladder) = inputs.ladders.get(&key)
                    && let Some(to) = ladder.ranks.iter().position(|r| r.id == goal.standing)
                {
                    rep_legs(&key, ladder, to, &plan.id)
                        .into_iter()
                        .for_each(&mut add);
                }
            }
            crate::Goal::Mission(goal) => {
                mission_steps(goal, inputs, &plan.id)
                    .into_iter()
                    .for_each(&mut add);
            }
            _ => {}
        }
    }
    extra.into_iter().for_each(&mut add);
    out
}

/// The granting mission a blueprint is earned from, and the steps before it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EarnRoute {
    /// Its index in the grants; `None` when every granting mission is
    /// closed to you, outgrown or done once and for all.
    pub grant: Option<usize>,
    pub before: Vec<PlanStep>,
    /// The other granting missions open to you now, and those a rank or a
    /// mission before them still keeps closed (the chosen one not counted).
    pub open: usize,
    pub later: usize,
}

/// The nearest granting mission: the one with the fewest steps before it,
/// then the best odds, then the shorter run. `None` when no grant's mission
/// is known (the earn step then says only who grants it).
pub fn earn_route(grants: &[Grant], inputs: &ContractInputs, plan: &str) -> Option<EarnRoute> {
    let known: Vec<(usize, &Grant, &MissionFacts)> = grants
        .iter()
        .enumerate()
        .filter_map(|(i, g)| inputs.missions.get(&g.mission).map(|m| (i, g, m)))
        .collect();
    if known.is_empty() {
        return None;
    }
    let reachable: Vec<(usize, &Grant, Vec<PlanStep>)> = known
        .into_iter()
        .filter(|(_, _, m)| !(m.once_only && m.done))
        .filter_map(|(i, g, m)| {
            path(&g.mission, m, inputs, plan)
                .ok()
                .map(|before| (i, g, before))
        })
        .collect();
    let open_now = reachable.iter().filter(|(.., b)| b.is_empty()).count();
    let later_on = reachable.len() - open_now;
    let best = reachable.into_iter().min_by(|(_, a, pa), (_, b, pb)| {
        let minutes = |g: &Grant| g.time.map_or(f32::MAX, |t| t.minutes);
        pa.len()
            .cmp(&pb.len())
            .then_with(|| b.chance.unwrap_or(0.0).total_cmp(&a.chance.unwrap_or(0.0)))
            .then_with(|| minutes(a).total_cmp(&minutes(b)))
    });
    Some(match best {
        Some((i, _, before)) => {
            let (open, later) = if before.is_empty() {
                (open_now - 1, later_on)
            } else {
                (open_now, later_on - 1)
            };
            EarnRoute {
                grant: Some(i),
                before,
                open,
                later,
            }
        }
        None => EarnRoute::default(),
    })
}

/// The legs from your rank up to rank `to`: one step a rank, each after the
/// one before.
fn rep_legs(key: &str, ladder: &RepLadder, to: usize, plan: &str) -> Vec<PlanStep> {
    let Some(score) = ladder.score else {
        return vec![PlanStep {
            title: format!("Read your rank with {}", ladder.faction),
            detail: "Waits: Contracts, Standing reads it from the game".to_owned(),
            blocked: true,
            ..step(format!("standing:{key}"), StepKind::Rep, plan)
        }];
    };
    let you = ladder.rank_of(score);
    let mut out: Vec<PlanStep> = Vec::new();
    for r in you..to {
        let next = &ladder.ranks[r + 1];
        let need = (next.min - score.max(ladder.ranks[r].min)).max(1);
        let open: Vec<&RepSource> = ladder
            .sources
            .iter()
            .filter(|s| s.lo <= r && r <= s.hi && s.rep > 0)
            .collect();
        let best = open.iter().copied().max_by(|a, b| {
            a.rep.cmp(&b.rep).then_with(|| {
                // The shorter run wins a tie.
                let t = |s: &RepSource| s.time.map_or(f32::MAX, |t| t.minutes);
                t(b).total_cmp(&t(a))
            })
        });
        let runs = best.map(|b| (need + b.rep - 1) / b.rep);
        let id = format!("rep:{key}:{}", r + 1);
        let mut s = PlanStep {
            title: format!("Reach {} with {}", next.name, ladder.faction),
            detail: match (best, runs) {
                (Some(b), Some(n)) => format!(
                    "{} rep: {} {} of {}, +{} each",
                    auec_figure(need as f64),
                    n,
                    if n == 1 { "run" } else { "runs" },
                    b.title,
                    auec_figure(b.rep as f64)
                ),
                _ => format!(
                    "No repeatable mission at {} gives {} rep",
                    ladder.ranks[r].name, ladder.faction
                ),
            },
            blocked: best.is_none(),
            at: best.and_then(|b| first_place(&b.at)),
            seconds: best
                .and_then(|b| b.time)
                .zip(runs)
                .map(|(t, n)| t.seconds() * n as f64),
            seconds_from: best.and_then(|b| b.time).map(|t| t.from),
            sources: open.len() as u32,
            after: out.last().map(|p| vec![p.id.clone()]).unwrap_or_default(),
            ..step(id, StepKind::Rep, plan)
        };
        if r > you && !s.blocked {
            // Done only once the rank before it is reached.
            s.blocked = true;
            s.detail = format!("Waits on {}: {}", ladder.ranks[r].name, s.detail);
        }
        out.push(s);
    }
    out
}

/// A mission's steps: closed or outgrown says so and stops; else the ranks
/// it needs, the missions before it, then the mission.
fn mission_steps(goal: &MissionGoal, inputs: &ContractInputs, plan: &str) -> Vec<PlanStep> {
    let Some(m) = inputs.missions.get(&goal.mission) else {
        return Vec::new();
    };
    if m.once_only && m.done {
        return Vec::new();
    }
    let mut out = match path(&goal.mission, m, inputs, plan) {
        Ok(before) => before,
        Err(stop) => return vec![*stop],
    };
    let mut figures: Vec<String> = Vec::new();
    if let Some(pay) = m.pay {
        figures.push(format!("{} aUEC", auec_figure(pay)));
    }
    if let Some(rep) = m.rep {
        figures.push(format!("+{} rep", auec_figure(rep as f64)));
    }
    if let Some(chance) = m.blueprint {
        figures.push(format!("a blueprint {} of runs", percent(chance)));
    }
    let before: Vec<String> = out.iter().map(|s| s.id.clone()).collect();
    out.push(PlanStep {
        title: format!("Run {}", m.title),
        detail: if figures.is_empty() {
            m.faction.clone()
        } else {
            figures.join(", ")
        },
        blocked: !before.is_empty(),
        at: first_place(&m.at),
        seconds: m.time.map(RunTime::seconds),
        seconds_from: m.time.map(|t| t.from),
        sources: 1,
        after: before,
        ..step(format!("run:{}", goal.mission), StepKind::Run, plan)
    });
    out
}

/// What opens mission `id`: the ranks it needs and the missions before it,
/// as steps (none when it is open to you now). `Err` is the one step that
/// says your rank closes it or you outgrew it.
fn path(
    id: &str,
    m: &MissionFacts,
    inputs: &ContractInputs,
    plan: &str,
) -> Result<Vec<PlanStep>, Box<PlanStep>> {
    for w in &m.closed_at {
        if let Some(ladder) = inputs.ladders.get(&w.ladder)
            && let Some(you) = ladder.you()
            && w.lo <= you
            && you <= w.hi
        {
            return Err(Box::new(PlanStep {
                title: format!("{} is closed to you", m.title),
                detail: format!(
                    "Waits: {} with {} closes it",
                    ladder.ranks[you].name, ladder.faction
                ),
                blocked: true,
                ..step(format!("closed:{id}"), StepKind::Run, plan)
            }));
        }
    }
    let mut out: Vec<PlanStep> = Vec::new();
    for w in &m.needs {
        let Some(ladder) = inputs.ladders.get(&w.ladder) else {
            continue;
        };
        match ladder.you() {
            Some(you) if you > w.hi => {
                return Err(Box::new(PlanStep {
                    title: format!("You outgrew {}", m.title),
                    detail: format!(
                        "Waits: it is for {} up to {} with {}",
                        ladder.ranks[w.lo].name, ladder.ranks[w.hi].name, ladder.faction
                    ),
                    blocked: true,
                    ..step(format!("outgrown:{id}"), StepKind::Run, plan)
                }));
            }
            Some(you) if you >= w.lo => {}
            _ => out.extend(rep_legs(&w.ladder, ladder, w.lo, plan)),
        }
    }
    for (i, (title, done)) in m.chain.iter().enumerate() {
        if *done {
            continue;
        }
        out.push(PlanStep {
            title: format!("Run {title} first"),
            detail: format!("{} follows it", m.title),
            ..step(format!("chain:{id}:{i}"), StepKind::Run, plan)
        });
    }
    Ok(out)
}

fn step(id: String, kind: StepKind, plan: &str) -> PlanStep {
    PlanStep {
        id,
        kind,
        title: String::new(),
        detail: String::new(),
        plans: vec![plan.to_owned()],
        blocked: false,
        at: None,
        seconds: None,
        seconds_from: None,
        cost: None,
        odds: None,
        sources: 0,
        after: Vec::new(),
        parts: 0,
        rocks: None,
        scu: 0.0,
    }
}

/// A place on the map first, else the first named one.
fn first_place(at: &[PlanPlace]) -> Option<PlanPlace> {
    at.iter()
        .find(|p| p.site.is_some())
        .or_else(|| at.first())
        .cloned()
}

/// A rank plan's card: the ranks from yours to the goal as stages, the rep
/// still to go.
pub fn rank_board(goal: &RankGoal, inputs: &ContractInputs) -> PlanBoard {
    let ladder = inputs.ladders.get(&ladder_key(&goal.geid, &goal.scope));
    let Some((ladder, to)) = ladder.and_then(|l| {
        l.ranks
            .iter()
            .position(|r| r.id == goal.standing)
            .map(|to| (l, to))
    }) else {
        let mut b = missing(
            PlanKind::Rank,
            &goal.rank,
            "This build has no such rank with the faction",
        );
        b.subtitle = goal.faction.clone();
        return b;
    };
    let target = &ladder.ranks[to];
    let base = ladder.ranks[0].min;
    let you = ladder.you();
    let from = you.unwrap_or(0).min(to);
    let mut b = missing(PlanKind::Rank, &target.name, "");
    b.missing = false;
    b.waiting = false;
    b.subtitle = ladder.faction.clone();
    b.name = format!("{}, {}", target.name, ladder.faction);
    b.progress = match ladder.score {
        Some(s) if target.min > base => {
            ((s - base) as f32 / (target.min - base) as f32).clamp(0.0, 1.0)
        }
        Some(s) if s >= target.min => 1.0,
        _ => 0.0,
    };
    b.stages = (from..=to)
        .map(|i| PlanStage {
            name: ladder.ranks[i].name.clone(),
            sub: auec_figure(ladder.ranks[i].min.max(0) as f64),
            state: match you {
                Some(y) if i < y || (i == y && y >= to) => StageState::Done,
                Some(y) if i == y => StageState::Current,
                _ => StageState::Ahead,
            },
        })
        .collect();
    if let Some(s) = ladder.score {
        b.tiles.push(tile(
            auec_figure((target.min - s).max(0) as f64),
            "rep to go",
        ));
        b.tiles.push(tile(auec_figure(s.max(0) as f64), "your rep"));
    }
    b
}

/// A mission plan's card: its pay, rep and blueprint odds.
pub fn mission_board(goal: &MissionGoal, inputs: &ContractInputs) -> PlanBoard {
    let Some(m) = inputs.missions.get(&goal.mission) else {
        return missing(
            PlanKind::Mission,
            &goal.title,
            "This build has no such mission",
        );
    };
    let mut b = missing(PlanKind::Mission, &m.title, "");
    b.missing = false;
    b.waiting = false;
    b.subtitle = m.faction.clone();
    b.name = m.title.clone();
    if m.once_only && m.done {
        b.progress = 1.0;
    }
    if let Some(pay) = m.pay {
        b.tiles.push(tile(auec_figure(pay), "aUEC a run"));
    }
    if let Some(rep) = m.rep {
        b.tiles
            .push(tile(format!("+{}", auec_figure(rep as f64)), "rep a run"));
    }
    if let Some(chance) = m.blueprint {
        b.tiles.push(tile(percent(chance), "blueprint a run"));
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Goal, PlansState};
    use crate::steps::TimeSource;

    const KEY: &str = "100|scope";

    fn ladder(score: Option<i64>) -> RepLadder {
        let ranks = [
            ("Neutral", 0),
            ("Jr.", 800),
            ("Contractor", 2200),
            ("Sr.", 5800),
        ]
        .iter()
        .enumerate()
        .map(|(i, (n, m))| RankFacts {
            id: format!("r{i}"),
            name: (*n).to_owned(),
            min: *m,
        })
        .collect();
        RepLadder {
            faction: "Headhunters".to_owned(),
            ranks,
            score,
            sources: vec![
                RepSource {
                    title: "Small Hit".to_owned(),
                    lo: 0,
                    hi: 3,
                    rep: 100,
                    // Your runs take 12 minutes; CIG says 10.
                    time: RunTime::best(Some(12.0), None, Some(10.0)),
                    at: Vec::new(),
                },
                RepSource {
                    title: "Big Hit".to_owned(),
                    lo: 1,
                    hi: 3,
                    rep: 400,
                    time: RunTime::best(None, None, Some(20.0)),
                    at: Vec::new(),
                },
            ],
        }
    }

    fn inputs(score: Option<i64>) -> ContractInputs {
        ContractInputs {
            ladders: HashMap::from([(KEY.to_owned(), ladder(score))]),
            missions: HashMap::new(),
        }
    }

    fn rank_plan(state: &mut PlansState, to: &str) -> String {
        state.add(
            "",
            Goal::Rank(RankGoal {
                faction: "Headhunters".into(),
                geid: "100".into(),
                scope: "scope".into(),
                standing: to.into(),
                rank: "Contractor".into(),
            }),
        )
    }

    #[test]
    fn a_rank_is_reached_rank_by_rank_with_the_best_rep_open_at_each() {
        let mut state = PlansState::default();
        rank_plan(&mut state, "r2");
        let steps = steps(&state.plans, &inputs(Some(500)), Vec::new());
        assert_eq!(steps.len(), 2);
        // Neutral: only Small Hit is open, 300 rep to go.
        assert_eq!(steps[0].title, "Reach Jr. with Headhunters");
        assert_eq!(steps[0].detail, "300 rep: 3 runs of Small Hit, +100 each");
        assert_eq!(steps[0].seconds, Some(3.0 * 12.0 * 60.0), "your time first");
        assert_eq!(steps[0].seconds_from, Some(TimeSource::Yours));
        assert!(!steps[0].blocked);
        // Jr.: Big Hit opens, from the rank's start.
        assert_eq!(
            steps[1].detail,
            "Waits on Jr.: 1,400 rep: 4 runs of Big Hit, +400 each"
        );
        assert!(steps[1].blocked);
        assert_eq!(steps[1].after, vec![steps[0].id.clone()]);
    }

    #[test]
    fn no_row_waits_on_the_read_and_a_reached_rank_is_done() {
        let mut state = PlansState::default();
        rank_plan(&mut state, "r2");
        let unread = steps(&state.plans, &inputs(None), Vec::new());
        assert_eq!(unread.len(), 1);
        assert!(unread[0].blocked);
        assert_eq!(unread[0].title, "Read your rank with Headhunters");
        assert!(steps(&state.plans, &inputs(Some(3000)), Vec::new()).is_empty());
        let board = rank_board(
            match &state.plans[0].goal {
                Goal::Rank(g) => g,
                _ => unreachable!(),
            },
            &inputs(Some(1100)),
        );
        assert_eq!(board.tiles[0].value, "1,100");
        assert_eq!(board.stages.len(), 2);
        assert_eq!(board.stages[0].state, StageState::Current);
    }

    #[test]
    fn a_mission_needs_its_rank_and_the_missions_before_it() {
        let mut ins = inputs(Some(500));
        ins.missions.insert(
            "m1".into(),
            MissionFacts {
                title: "Kill the Boss".into(),
                faction: "Headhunters".into(),
                needs: vec![RankWindow {
                    ladder: KEY.into(),
                    lo: 1,
                    hi: 3,
                }],
                chain: vec![
                    ("Setting a Trap".into(), false),
                    ("Done Before".into(), true),
                ],
                pay: Some(80_000.0),
                rep: Some(500),
                blueprint: Some(0.25),
                ..MissionFacts::default()
            },
        );
        let mut state = PlansState::default();
        state.add(
            "",
            Goal::Mission(MissionGoal {
                mission: "m1".into(),
                title: "Kill the Boss".into(),
            }),
        );
        let s = steps(&state.plans, &ins, Vec::new());
        let titles: Vec<&str> = s.iter().map(|x| x.title.as_str()).collect();
        assert_eq!(
            titles,
            [
                "Reach Jr. with Headhunters",
                "Run Setting a Trap first",
                "Run Kill the Boss"
            ]
        );
        let run = s.last().unwrap();
        assert!(run.blocked);
        assert_eq!(run.detail, "80,000 aUEC, +500 rep, a blueprint 25% of runs");
        assert_eq!(run.after.len(), 2);
    }

    #[test]
    fn a_mission_your_rank_closes_or_outgrew_says_so_and_a_parked_one_has_no_steps() {
        let mut ins = inputs(Some(6000));
        ins.missions.insert(
            "m1".into(),
            MissionFacts {
                title: "Rookie Job".into(),
                needs: vec![RankWindow {
                    ladder: KEY.into(),
                    lo: 0,
                    hi: 1,
                }],
                ..MissionFacts::default()
            },
        );
        let mut state = PlansState::default();
        let id = state.add(
            "",
            Goal::Mission(MissionGoal {
                mission: "m1".into(),
                title: "Rookie Job".into(),
            }),
        );
        let s = steps(&state.plans, &ins, Vec::new());
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].title, "You outgrew Rookie Job");
        assert!(s[0].blocked);
        state.set_parked(&id, true);
        assert!(steps(&state.plans, &ins, Vec::new()).is_empty());
    }
}
