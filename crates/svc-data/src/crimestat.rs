//! Crimestat-risk detection — ported from sc-langpatch's
//! `mission_enhancer/crimestat.rs`, retargeted for the 4.8 DCB shape.
//!
//! A mission carries crimestat risk when it sets a `DontHarm*` mission
//! variable ("don't harm allies/civilians" — killing them incurs a
//! crimestat). In the current DCB those live in **`ContractTemplate.
//! contractProperties`** (langpatch's old contract-record `paramOverrides.
//! propertyOverrides` path is gone: contracts are no longer top-level
//! records, and sc-missions v2 mission ids aren't record GUIDs — verified
//! against 4.8 LIVE, see the `diag` test below). The typed sc-missions
//! surface doesn't model mission variables, so this walks the template
//! record raw.
//!
//! Risk classification:
//! - [`CrimestatRisk::High`] — `DontHarm*` set without any allied-marker
//!   NPC spawn: friendlies present but indistinguishable from foes.
//! - [`CrimestatRisk::Moderate`] — `DontHarm*` set AND at least one NPC
//!   slot carries `mission_allied_marker = true`: friendlies have HUD
//!   markers.
//! - [`CrimestatRisk::None`] — neither signal present.

use sc_holotable::asset::{DataCoreDatabase, Instance, Value};
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

/// Classify the crimestat risk for one mission.
///
/// Walks the mission's `ContractTemplate.contractProperties` for
/// `DontHarm*` bool flags. Any NPC encounter with
/// `mission_allied_marker = true` downgrades the risk from `High` to
/// `Moderate`.
pub(crate) fn classify(db: &DataCoreDatabase, mission: &Mission) -> CrimestatRisk {
    if !template_has_dont_harm_flag(db, mission) {
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

fn template_has_dont_harm_flag(db: &DataCoreDatabase, mission: &Mission) -> bool {
    let Some(template_id) = mission.template_id else {
        return false;
    };
    let Some(record) = db.record(&template_id) else {
        return false;
    };
    let Some(props) = record.as_instance().get_array("contractProperties") else {
        return false;
    };
    properties_have_dont_harm(db, props)
}

fn properties_have_dont_harm<'a>(
    db: &'a DataCoreDatabase,
    props: impl Iterator<Item = Value<'a>>,
) -> bool {
    for pv in props {
        let Some(prop) = to_instance(db, &pv) else {
            continue;
        };
        let var_name = prop.get_str("missionVariableName").unwrap_or("");
        if !is_dont_harm_var(var_name) {
            continue;
        }
        let Some(val) = prop.get_instance("value") else {
            continue;
        };
        // Two shapes: `value.options[].value == 1` or `value.value == 1`.
        if val.get_i32("value") == Some(1) {
            return true;
        }
        if let Some(opts) = val.get_array("options") {
            for ov in opts {
                if let Some(oi) = to_instance(db, &ov)
                    && oi.get_i32("value") == Some(1)
                {
                    return true;
                }
            }
        }
    }
    false
}

fn is_dont_harm_var(name: &str) -> bool {
    matches!(
        name,
        "DontHarmAllies_BP" | "BP_DontHarmAllies" | "DontHarmCivs_BP" | "BP_DontHarmCivs"
    )
}

fn to_instance<'a>(db: &'a DataCoreDatabase, val: &Value<'a>) -> Option<Instance<'a>> {
    match val {
        Value::Class { struct_index, data } => {
            Some(Instance::from_inline_data(db, *struct_index, data))
        }
        Value::StrongPointer(Some(r)) | Value::ClassRef(r) => {
            Some(db.instance(r.struct_index, r.instance_index))
        }
        _ => None,
    }
}
