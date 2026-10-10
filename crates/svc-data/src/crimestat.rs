//! Crimestat-risk detection — ported from sc-langpatch's
//! `mission_enhancer/crimestat.rs`.
//!
//! A mission carries crimestat risk when it sets a `DontHarm*` mission
//! variable ("don't harm allies/civilians" — killing them incurs a
//! crimestat). sc-holotable resolves those across the template → handler →
//! contract → sub-contract layers into [`Mission::integer_properties`]; the
//! override layers matter, because contracts regularly set their template's
//! flag back to 0 (172 on 4.10 LIVE).
//!
//! Risk classification:
//! - [`CrimestatRisk::High`] — `DontHarm*` set without any allied-marker
//!   NPC spawn: friendlies present but indistinguishable from foes.
//! - [`CrimestatRisk::Moderate`] — `DontHarm*` set AND at least one NPC
//!   slot carries `mission_allied_marker = true`: friendlies have HUD
//!   markers.
//! - [`CrimestatRisk::None`] — no `DontHarm*` flag set.

use sc_holotable::missions::{Encounter, Mission};
use serde::{Deserialize, Serialize};

/// How likely a mission is to hand the player a crimestat by accident.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CrimestatRisk {
    #[default]
    None,
    /// Friendlies present WITH HUD markers.
    Moderate,
    /// Friendlies present WITHOUT HUD markers — cannot distinguish friend
    /// from foe.
    High,
}

/// The `DontHarm*` mission variables, both spellings the generators use.
const DONT_HARM_VARS: [&str; 4] = [
    "DontHarmAllies_BP",
    "BP_DontHarmAllies",
    "DontHarmCivs_BP",
    "BP_DontHarmCivs",
];

/// Classify the crimestat risk for one mission. A flag counts as set when
/// any option the engine may draw is 1. Any NPC encounter with
/// `mission_allied_marker = true` downgrades the risk from `High` to
/// `Moderate`.
pub(crate) fn classify(mission: &Mission) -> CrimestatRisk {
    let dont_harm = DONT_HARM_VARS.iter().any(|var| {
        mission
            .integer_properties
            .get(*var)
            .is_some_and(|options| options.contains(&1))
    });
    if !dont_harm {
        return CrimestatRisk::None;
    }

    let has_allied_marker = mission.encounters.iter().any(|e| match e {
        Encounter::Npcs(npc) => npc
            .phases
            .iter()
            .flat_map(|p| p.all_options())
            .any(|slot| slot.mission_allied_marker),
        _ => false,
    });

    if has_allied_marker {
        CrimestatRisk::Moderate
    } else {
        CrimestatRisk::High
    }
}
