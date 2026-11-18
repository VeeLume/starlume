//! What rank and mission plans read, gathered from the cooked mission
//! catalog and the cached standing read (`svc_dossier::PlayerStanding`).
//!
//! Joins as the Contracts pages do (`src/lib/modules/contracts/model.ts`):
//! a requirement names its ladder by the org's GEID and its reputation
//! scope, the standing reports a score per (GEID, scope); a mission's
//! completed contracts are the wallet's contract tokens. Only reads the
//! cache: no network call here, the read itself is gated in `dossier.rs`.

use std::collections::{BTreeSet, HashSet};

use mod_plans::contracts::{
    ContractInputs, MissionFacts, RankFacts, RankWindow, RepLadder, RepSource, ladder_key,
};
use mod_plans::steps::PlanPlace;
use mod_plans::{Goal, PlansState};
use svc_data::missions::{MissionEntry, MissionRegion, RepRequirement};
use svc_dossier::PlayerStanding;

/// Rank and mission plans' inputs; empty when none is planned.
pub(crate) fn gather(
    state: &PlansState,
    missions: &[MissionEntry],
    standing: Option<&PlayerStanding>,
    place: &dyn Fn(&MissionRegion) -> PlanPlace,
) -> ContractInputs {
    let empty = BTreeSet::new();
    let completed = standing.map_or(&empty, |s| &s.completed_contracts);
    let did = |contracts: &[String]| contracts.iter().any(|c| completed.contains(c));
    let mut out = ContractInputs::default();
    let mut ladders: HashSet<String> = HashSet::new();

    for plan in &state.plans {
        match &plan.goal {
            Goal::Rank(goal) => {
                ladders.insert(ladder_key(&goal.geid, &goal.scope));
            }
            Goal::Mission(goal) => {
                let Some(m) = missions.iter().find(|m| m.mission_id == goal.mission) else {
                    continue;
                };
                let window = |r: &RepRequirement| {
                    let key = key_of(r)?;
                    let (lo, hi) = window_of(r)?;
                    Some(RankWindow {
                        ladder: key,
                        lo,
                        hi,
                    })
                };
                let facts = MissionFacts {
                    title: m.title.clone().unwrap_or_else(|| m.debug_name.clone()),
                    faction: m
                        .faction
                        .as_ref()
                        .and_then(|f| f.name.clone())
                        .unwrap_or_default(),
                    once_only: m.once_only,
                    done: did(&m.contracts),
                    needs: m
                        .rep_required
                        .iter()
                        .filter(|r| !r.exclude)
                        .filter_map(window)
                        .collect(),
                    closed_at: m
                        .rep_required
                        .iter()
                        .filter(|r| r.exclude)
                        .filter_map(window)
                        .collect(),
                    chain: m
                        .chain_required
                        .iter()
                        .map(|c| {
                            (
                                c.title.clone().unwrap_or_else(|| c.mission_id.clone()),
                                did(&c.contracts),
                            )
                        })
                        .collect(),
                    minutes: Some(m.payout.time_to_complete).filter(|t| *t > 0.0),
                    at: m.locations.iter().map(place).collect(),
                    pay: m.payout.fixed.or(m.payout.estimate).map(f64::from),
                    rep: own_rep(m),
                    blueprint: blueprint_chance(m),
                };
                for w in facts.needs.iter().chain(&facts.closed_at) {
                    ladders.insert(w.ladder.clone());
                }
                out.missions.insert(goal.mission.clone(), facts);
            }
            _ => {}
        }
    }

    for key in ladders {
        // The ladder as any requirement on it carries it.
        let Some(req) = missions
            .iter()
            .flat_map(|m| &m.rep_required)
            .find(|r| key_of(r).as_deref() == Some(key.as_str()) && !r.ladder.is_empty())
        else {
            continue;
        };
        let geid = req.faction_geid.clone().unwrap_or_default();
        let ranks: Vec<RankFacts> = req
            .ladder
            .iter()
            .enumerate()
            .map(|(i, r)| RankFacts {
                id: r.id.clone(),
                name: r.name.clone().unwrap_or_else(|| format!("Rank {i}")),
                min: r.min_reputation,
            })
            .collect();
        let score = standing.and_then(|s| {
            s.reputations
                .iter()
                .find(|r| {
                    r.org_geid.as_deref() == Some(geid.as_str())
                        && req.scope.as_deref().is_none_or(|sc| r.scope_id == sc)
                })
                .map(|r| i64::from(r.score))
        });
        let last = ranks.len().saturating_sub(1);
        let sources = missions
            .iter()
            .filter(|m| !m.once_only)
            .filter(|m| m.faction.as_ref().and_then(|f| f.geid.as_deref()) == Some(geid.as_str()))
            // A mission behind one not done yet is not a source until then.
            .filter(|m| m.chain_required.iter().all(|c| did(&c.contracts)))
            .filter_map(|m| {
                let rep = own_rep(m).filter(|r| *r > 0)?;
                let own: Vec<&RepRequirement> = m
                    .rep_required
                    .iter()
                    .filter(|r| !r.exclude && r.faction_geid.as_deref() == Some(geid.as_str()))
                    .collect();
                let (lo, hi) = if own.is_empty() {
                    (0, last)
                } else {
                    // Gated on another of the org's ladders: not this one's.
                    let on = own
                        .iter()
                        .filter(|r| key_of(r).as_deref() == Some(key.as_str()))
                        .filter_map(|r| window_of(r))
                        .collect::<Vec<_>>();
                    if on.is_empty() {
                        return None;
                    }
                    // Several requirements all hold: their intersection.
                    let lo = on.iter().map(|w| w.0).max()?;
                    let hi = on.iter().map(|w| w.1).min()?;
                    if lo > hi {
                        return None;
                    }
                    (lo, hi)
                };
                Some(RepSource {
                    title: m.title.clone().unwrap_or_else(|| m.debug_name.clone()),
                    lo,
                    hi,
                    rep,
                    minutes: Some(m.payout.time_to_complete).filter(|t| *t > 0.0),
                    at: m.locations.iter().map(place).collect(),
                })
            })
            .collect();
        out.ladders.insert(
            key,
            RepLadder {
                faction: req.faction.clone().unwrap_or_default(),
                ranks,
                score,
                sources,
            },
        );
    }
    out
}

fn key_of(r: &RepRequirement) -> Option<String> {
    Some(ladder_key(r.faction_geid.as_deref()?, r.scope.as_deref()?))
}

/// The requirement's window on its ladder, inclusive: by rank name, else by
/// index, else the ladder's ends (the Contracts page's `ladderOf`).
fn window_of(r: &RepRequirement) -> Option<(usize, usize)> {
    if r.ladder.is_empty() {
        return None;
    }
    let last = r.ladder.len() - 1;
    let at = |name: &Option<String>, index: Option<i32>, fallback: usize| {
        name.as_ref()
            .and_then(|n| r.ladder.iter().position(|x| x.name.as_ref() == Some(n)))
            .or_else(|| {
                index
                    .and_then(|i| usize::try_from(i).ok())
                    .filter(|i| *i <= last)
            })
            .unwrap_or(fallback)
    };
    let lo = at(&r.min_rank, r.min_rank_index, 0);
    let hi = at(&r.max_rank, r.max_rank_index, last).max(lo);
    Some((lo, hi))
}

/// Rep with the mission's own faction one run gives; `None` when calculated.
fn own_rep(m: &MissionEntry) -> Option<i64> {
    let own = m.faction.as_ref()?;
    m.reputation
        .iter()
        .find(|r| r.faction_guid.as_deref() == Some(own.guid.as_str()))
        .and_then(|r| r.amount)
        .map(i64::from)
}

/// The chance a run gives at least one blueprint, over its pools.
fn blueprint_chance(m: &MissionEntry) -> Option<f64> {
    if m.blueprint_rewards.is_empty() {
        return None;
    }
    let none: f64 = m
        .blueprint_rewards
        .iter()
        .map(|p| 1.0 - f64::from(p.chance).clamp(0.0, 1.0))
        .product();
    Some(1.0 - none)
}

#[cfg(test)]
mod tests {
    use super::*;
    use svc_data::missions::RepRank;

    #[test]
    fn a_window_reads_by_name_then_index_then_the_ends() {
        let ladder: Vec<RepRank> = ["Neutral", "Jr.", "Contractor"]
            .iter()
            .enumerate()
            .map(|(i, n)| RepRank {
                id: format!("r{i}"),
                name: Some((*n).to_owned()),
                min_reputation: i as i64 * 1000,
            })
            .collect();
        let req = |min: Option<&str>, max_index: Option<i32>| RepRequirement {
            faction: Some("Headhunters".into()),
            min_rank: min.map(str::to_owned),
            max_rank: None,
            min_rank_index: None,
            max_rank_index: max_index,
            exclude: false,
            scope: Some("s".into()),
            ladder: ladder.clone(),
            faction_geid: Some("100".into()),
        };
        assert_eq!(window_of(&req(Some("Jr."), None)), Some((1, 2)));
        assert_eq!(window_of(&req(None, Some(1))), Some((0, 1)));
        assert_eq!(key_of(&req(None, None)).as_deref(), Some("100|s"));
    }
}
