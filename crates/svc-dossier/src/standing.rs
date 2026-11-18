//! The player's standing for Contracts: reputation per faction and scope,
//! and which contracts they have completed (the mission token wallet).
//!
//! Joined Starlume-side against the cooked mission catalog: a mission's rank
//! requirement names its faction by `FactionReputation.GEID` and its ladder
//! by scope and standing GUIDs, the same ids a [`RepRow`] carries; a chain
//! step names its contracts by record name, the same names a completed
//! contract token carries.
//!
//! **No row is not a rank.** The service returns rows only for orgs the
//! player has a score with; a faction without a row is unknown, never
//! assumed Neutral (the 2026-08-27 probe: 68 of 500 live contracts gated on
//! an org with no row). Consumers must show such missions, not hide them.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// One reputation row: the player's score with one org on one scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepRow {
    /// `FactionReputation.GEID`, decimal; `None` when the entity urn has
    /// another shape.
    pub org_geid: Option<String>,
    /// Scope name (`"FactionReputation"`, `"Hauling"`, …).
    pub scope: String,
    /// The scope's ladder, an `SReputationScopeParams` GUID.
    pub scope_id: String,
    /// Current score.
    pub score: i32,
    /// The rank the score falls in, an `SReputationStandingParams` GUID.
    pub standing_id: Option<String>,
    /// That rank's lowest score.
    pub standing_min: Option<i32>,
}

/// The cached snapshot.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlayerStanding {
    pub reputations: Vec<RepRow>,
    /// `ContractDefinition` record names the wallet holds a completion token
    /// for. Not a full history: plain repeatables leave no token; intros,
    /// once-only contracts and chain steps do.
    pub completed_contracts: BTreeSet<String>,
    /// When fetched, Unix seconds; `0` = never.
    pub fetched_at: i64,
}

impl PlayerStanding {
    /// Map the raw reads into the snapshot.
    pub(crate) fn from_reads(
        reputations: Vec<sc_dossier::Reputation>,
        tokens: Vec<sc_dossier::MissionToken>,
        fetched_at: i64,
    ) -> Self {
        let reputations = reputations
            .into_iter()
            .map(|r| RepRow {
                org_geid: r.org_geid.map(|g| g.to_string()),
                scope: r.scope,
                scope_id: r.scope_id,
                score: r.score,
                standing_id: r.standing.as_ref().map(|s| s.id.clone()),
                standing_min: r.standing.as_ref().map(|s| s.min),
            })
            .collect();
        let completed_contracts = tokens
            .into_iter()
            .filter_map(|t| match t.kind {
                sc_dossier::TokenKind::Contract { name } => Some(name),
                _ => None,
            })
            .collect();
        Self {
            reputations,
            completed_contracts,
            fetched_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rep(geid: Option<u64>, score: i32, standing: Option<&str>) -> sc_dossier::Reputation {
        sc_dossier::Reputation {
            id: "row".into(),
            org_geid: geid,
            entity: format!("urn:sc:reputations:org:geid:{}", geid.unwrap_or_default()),
            scope: "FactionReputation".into(),
            score,
            scope_id: "d31bd373-35ab-4ea4-b0a7-0ccc867ef082".into(),
            standing: standing.map(|id| sc_dossier::Standing {
                id: id.into(),
                name: "FactionRep_Rank1".into(),
                min: 800,
                drift_amount: 0,
                drift_every_secs: 0,
                gated: false,
            }),
            updated_at_ms: 0,
        }
    }

    fn token(reason: &str) -> sc_dossier::MissionToken {
        sc_dossier::MissionToken {
            id: "t".into(),
            kind: sc_dossier::TokenKind::parse(reason),
            count: 1,
            created_at_ms: 0,
            expires_at: None,
            reason: reason.into(),
        }
    }

    #[test]
    fn maps_rows_and_keeps_only_contract_tokens() {
        let s = PlayerStanding::from_reads(
            vec![
                rep(Some(199_998_949_491), 1000, Some("rank1")),
                rep(None, 5, None),
            ],
            vec![
                token("Contract: HeadHunters_ShipAmbush_Pyro_Intro_0"),
                token("Contract: Wikelo_Collect Applied Tag: Tag.0000 - Count: 3"),
                token("somewhere else"),
            ],
            42,
        );
        assert_eq!(s.reputations[0].org_geid.as_deref(), Some("199998949491"));
        assert_eq!(s.reputations[0].standing_id.as_deref(), Some("rank1"));
        assert_eq!(s.reputations[0].standing_min, Some(800));
        assert_eq!(s.reputations[1].org_geid, None);
        assert_eq!(
            s.completed_contracts.iter().collect::<Vec<_>>(),
            vec!["HeadHunters_ShipAmbush_Pyro_Intro_0"]
        );
        assert_eq!(s.fetched_at, 42);
    }
}
