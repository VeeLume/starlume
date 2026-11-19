//! Pure parsers for Star Citizen `Game.log` records.
//!
//! Carved from Hearth's `sensors::parse` and widened. The log format is
//! CIG-internal and churns across patches, so these stay small, isolated,
//! pure functions: a format break is a local fix here, never a rewrite of
//! the tailing machinery. Every fixture in the tests is a verbatim line
//! from a real log (LIVE 4.8 to 4.10, 2026), with only the account and
//! player ids replaced; the doc comment on each parser quotes the shape it
//! matches.
//!
//! **HUD notifications are matched by their English text**, so those
//! events are locale-dependent: see the `event` module docs for which, and
//! for what the proper fix would be.
//!
//! A **record** is one log line, or a HUD notification whose text spans
//! several lines, joined with `\n` by [`crate::tailer::RecordJoiner`].
//!
//! Rules that hold for the whole file (measured over 1,085 logs, see the
//! "Game.log across versions" note in the maintainer's records):
//! - 4.x lines read `<ts> [Notice] <Event> msg [Team][Tag]`; older builds
//!   put the team tags first. [`tag`] skips any leading bracket groups, so
//!   both parse.
//! - Every HUD notification is logged three ways: the
//!   `<SHUDEvent_OnNotification> Added notification` line, an indented echo
//!   of the queue, and `<UpdateNotificationItem>` lifecycle lines. Only the
//!   first is an event; matching on the full prefix keeps the others out.
//! - Starlume's own text patch adds tags to contract titles
//!   (`<EM4>[BP]</EM4>`); [`clean_title`] removes them.

use crate::event::{
    BodyPart, ContractChange, LogEvent, LogEventKind, MoneyChange, ObjectiveChange, TradeSide,
};

/// Recognise one record. `None` for the ~99.9% that carry nothing tracked.
pub fn parse_record(record: &str) -> Option<LogEvent> {
    let (ts, rest) = split_ts(record)?;
    let event = parse_body(rest)?;
    Some(LogEvent {
        ts: ts.to_owned(),
        event,
    })
}

/// `<2026-11-13T16:13:05.561Z> rest` → (`2026-11-13T16:13:05.561Z`, ` rest`).
pub(crate) fn split_ts(line: &str) -> Option<(&str, &str)> {
    let inner = line.strip_prefix('<')?;
    let end = inner.find('>')?;
    let ts = &inner[..end];
    // A timestamp, not some other `<…>` at the start of a torn line.
    if ts.len() < 20 || !ts.as_bytes()[0].is_ascii_digit() || !ts.ends_with('Z') {
        return None;
    }
    Some((ts, &inner[end + 1..]))
}

fn parse_body(rest: &str) -> Option<LogEventKind> {
    let body = rest.trim_start();
    if let Some(tag) = tag(body) {
        return match tag {
            "SHUDEvent_OnNotification" => notification(body),
            "RequestLocationInventory" => location_inventory(body),
            "ObjectiveUpserted" => objective_upserted(body),
            "CLocalMissionPhaseMarker::CreateMarker" => mission_marker(body),
            "EndMission" => mission_ended(body),
            "PlayerLeft" => mission_left(body),
            "Join PU" => shard_joined(body),
            "CEntityComponentCommodityUIProvider::SendCommodityBuyRequest" => {
                commodity_trade(body, TradeSide::Buy)
            }
            "CEntityComponentCommodityUIProvider::SendCommoditySellRequest" => {
                commodity_trade(body, TradeSide::Sell)
            }
            "CEntityComponentShopUIProvider::SendShopBuyRequest"
            | "CEntityComponentShoppingProvider::SendStandardItemBuyRequest" => shop_buy(body),
            "CEntityComponentShoppingProvider::SendRentalRequest" => shop_rental(body),
            "CEntityComponentShopUIProvider::RmShopFlowResponse"
            | "CEntityComponentShoppingProvider::RmShopFlowResponse" => shop_flow_response(body),
            "Legacy login response" => session_handle(body),
            "AccountLoginCharacterStatus_Character" => session_account_id(body),
            "SystemQuit" => session_ended(body),
            "Player Selected Quantum Target - Local" => quantum_target_selected(body),
            "Calculate Route" => quantum_route(body),
            "Quantum Drive Arrived - Arrived at Final Destination" => quantum_arrived(body),
            "AttachmentReceived" => held_item_received(body),
            "EquipItem" => attachment_requested(body),
            "Vehicle Destroyed" => vehicle_destroyed(body),
            "MED BED HEAL" => med_bed_surgery(body),
            // Its other lines (`Querying … inventory`) are the same request.
            "OnRequestFetchVehicles" => body
                .contains("> Fetching player vehicle list")
                .then_some(LogEventKind::VehicleListFetched),
            _ => None,
        };
    }
    header(body)
}

/// The event tag of a line: `[Notice] <Join PU> address[…` → `Join PU`.
/// Skips leading `[…]` groups (severity; team tags on pre-4.x builds).
fn tag(body: &str) -> Option<&str> {
    let mut s = body;
    while let Some(after) = s.strip_prefix('[') {
        s = after[after.find(']')? + 1..].trim_start();
    }
    let inner = s.strip_prefix('<')?;
    // A tag can contain `>` (`…::<lambda_1>::operator ()>`); it ends at the
    // first `> `.
    let end = inner.find("> ")?;
    Some(&inner[..end])
}

/// The value of `key[value]`, with `key` given including its `[`.
fn bracket<'a>(s: &'a str, key: &str) -> Option<&'a str> {
    let start = s.find(key)? + key.len();
    let rest = &s[start..];
    Some(&rest[..rest.find(']')?])
}

fn owned(s: &str) -> Option<String> {
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_owned())
}

// ── Session header ──────────────────────────────────────────────────────

/// The untagged lines at the top of every log:
///
/// ```text
/// <…> Log started on Fri Nov 13 16:10:48 2026
/// <…>    [Cmdline* ] --envtag='PUB'
/// <…> Branch: sc-alpha-4.10.0
/// <…> Changelist: 13165946
/// ```
fn header(body: &str) -> Option<LogEventKind> {
    if body.starts_with("Log started on ") {
        return Some(LogEventKind::SessionStarted);
    }
    if let Some(name) = body.strip_prefix("Branch: ") {
        return Some(LogEventKind::Branch { name: owned(name)? });
    }
    if let Some(number) = body.strip_prefix("Changelist: ") {
        return Some(LogEventKind::Changelist {
            number: owned(number)?,
        });
    }
    const ENVTAG: &str = "--envtag='";
    if body.starts_with('[')
        && let Some(at) = body.find(ENVTAG)
    {
        let value = &body[at + ENVTAG.len()..];
        return Some(LogEventKind::SessionPlatform {
            env_tag: owned(&value[..value.find('\'')?])?,
        });
    }
    None
}

/// ```text
/// <…> [Notice] <Legacy login response> [CIG-net] User Login Success - Handle[VeeLume] - Time[375324561] [Team_GameServices][Login]
/// ```
fn session_handle(body: &str) -> Option<LogEventKind> {
    Some(LogEventKind::SessionHandle {
        handle: owned(bracket(body, "Handle[")?)?,
    })
}

/// ```text
/// <…> [Notice] <AccountLoginCharacterStatus_Character> Character: createdAt … - geid 200000000001 - accountId 1234567 - name VeeLume - state STATE_CURRENT [Team_GameServices][Login]
/// ```
///
/// Only the `STATE_CURRENT` character, so defunct character rows don't
/// leak their ids in.
fn session_account_id(body: &str) -> Option<LogEventKind> {
    if !body.contains("STATE_CURRENT") {
        return None;
    }
    const KEY: &str = "accountId ";
    let rest = &body[body.find(KEY)? + KEY.len()..];
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    let geid = body.find("geid ").and_then(|i| {
        let rest = &body[i + "geid ".len()..];
        let end = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        owned(&rest[..end])
    });
    Some(LogEventKind::SessionAccountId {
        account_id: owned(&rest[..end])?,
        geid,
    })
}

/// ```text
/// <…> [Notice] <Join PU> address[34.156.32.60] port[64385] shard[pub_euw1b_13165946_100] locationId[-281470681677823] [Team_GameServices][GIM][Matchmaking]
/// ```
fn shard_joined(body: &str) -> Option<LogEventKind> {
    Some(LogEventKind::ShardJoined {
        shard: owned(bracket(body, "shard[")?)?,
    })
}

/// ```text
/// <…> [Notice] <SystemQuit> CSystem::Quit invoked with - cause=30016, reason=Quit via console command, exitCode=0, thread id=39820, main thread id=39820 [Team_Unknown][System]
/// ```
fn session_ended(body: &str) -> Option<LogEventKind> {
    const KEY: &str = "reason=";
    let rest = &body[body.find(KEY)? + KEY.len()..];
    let reason = &rest[..rest.find(", exitCode").unwrap_or(rest.len())];
    Some(LogEventKind::SessionEnded {
        reason: reason.trim().trim_matches('"').to_owned(),
    })
}

// ── Where ───────────────────────────────────────────────────────────────

/// ```text
/// <…> [Notice] <RequestLocationInventory> Player[VeeLume] requested inventory for Location[Nyx_Levski] [Team_CoreGameplayFeatures][Inventory]
/// ```
fn location_inventory(body: &str) -> Option<LogEventKind> {
    Some(LogEventKind::LocationInventoryRequested {
        location: owned(bracket(body, "Location[")?)?,
    })
}

// ── Quantum travel ──────────────────────────────────────────────────────

/// The parts of a navigation line, which are `|`-separated:
///
/// ```text
/// <…> [Notice] <Tag> [ItemNavigation][CL][41200] | NOT AUTH | RSI_Constellation_Mk5_Centaurus_930000000001[930000000001]|CSCItemNavigation::Function|Message [Team_CGP4][QuantumTravel]
/// ```
///
/// → (the entity's class, the message). The class is the entity name
/// without its `_<entity id>`; `None` for `NULL ENTITY`.
fn navigation(body: &str) -> Option<(Option<String>, &str)> {
    let mut parts = body.rsplit('|');
    let message = parts.next()?;
    let _function = parts.next()?;
    let entity = parts.next()?.trim();
    let message = message[..message.rfind(" [Team_").unwrap_or(message.len())].trim();
    let name = &entity[..entity.find('[').unwrap_or(entity.len())];
    let class = match name.rsplit_once('_') {
        Some((class, id)) if !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()) => class,
        _ => name,
    };
    let class = (!class.contains(' ')).then(|| owned(class)).flatten();
    Some((class, message))
}

/// ```text
/// <…> [Notice] <Player Selected Quantum Target - Local> [ItemNavigation][CL][41200] | NOT AUTH | RSI_Constellation_Mk5_Centaurus_930000000001[930000000001]|CSCItemNavigation::OnPlayerSelectedQuantumTarget|Player has selected point Orison_LOC as their destination, routing locally [Team_CGP4][QuantumTravel]
/// ```
///
/// The `<Player Requested Fuel to Quantum Target - …>` lines beside it are
/// the star map asking what a destination would cost while the player
/// browses: not a selection, not an event.
fn quantum_target_selected(body: &str) -> Option<LogEventKind> {
    let (vehicle_class, message) = navigation(body)?;
    let destination = message
        .strip_prefix("Player has selected point ")?
        .split(" as their destination")
        .next()?;
    Some(LogEventKind::QuantumTargetSelected {
        vehicle_class,
        destination: owned(destination)?,
    })
}

/// ```text
/// <…> [Notice] <Calculate Route> [ItemNavigation][CL][41200] | NOT AUTH | RSI_…[…]|CSCItemNavigation::CalculateRoute|Projected Start Location is Nyx Gateway for route to destination Orison_LOC [Team_CGP4][QuantumTravel]
/// ```
///
/// The same tag's `Successfully calculated route to … fuel estimate …`
/// line, and `<Calculate Route!>` (`Adding surface location …`), add
/// nothing to it and are not events.
fn quantum_route(body: &str) -> Option<LogEventKind> {
    let (_, message) = navigation(body)?;
    let rest = message.strip_prefix("Projected Start Location is ")?;
    // The origin is a display name and may hold anything; the destination
    // is an internal name without spaces, so split at the last separator.
    let (origin, destination) = rest.rsplit_once(" for route to destination ")?;
    Some(LogEventKind::QuantumRouteCalculated {
        origin: owned(origin)?,
        destination: owned(destination)?,
    })
}

/// ```text
/// <…> [Notice] <Quantum Drive Arrived - Arrived at Final Destination> [ItemNavigation][CL][41200] | NOT AUTH | RSI_…[…]|CSCItemNavigation::OnQuantumDriveArrived|Quantum Drive has arrived at final destination [Team_CGP4][QuantumTravel]
/// ```
fn quantum_arrived(body: &str) -> Option<LogEventKind> {
    let (vehicle_class, _) = navigation(body)?;
    Some(LogEventKind::QuantumArrived { vehicle_class })
}

// ── Gear ────────────────────────────────────────────────────────────────

/// A slot of the player that holds a weapon or a tool: what an attachment
/// port hangs off (`wep_stocked_2`, `wep_sidearm`,
/// `weapon_attach_hand_right`, `utility_attach_1`).
fn holds_a_weapon(port: &str) -> bool {
    ["wep_", "weapon_attach_", "utility_attach_"]
        .iter()
        .any(|prefix| port.starts_with(prefix))
}

/// ```text
/// <…> [Notice] <AttachmentReceived> Player[VeeLume] Attachment[lbco_rifle_energy_01_mr01_930000000002, lbco_rifle_energy_01_mr01, 930000000002] Status[persistent] Port[wep_stocked_2] Elapsed[0.000056] [Team_CoreGameplayFeatures][Inventory]
/// ```
///
/// Logged for every item on the player (hundreds per session); only the
/// ones landing in a weapon or tool slot are events. `Status[local]` is the
/// client's placeholder before the server's answer and is skipped.
fn held_item_received(body: &str) -> Option<LogEventKind> {
    if bracket(body, "Status[")? != "persistent" {
        return None;
    }
    let port = bracket(body, " Port[")?;
    if !holds_a_weapon(port) {
        return None;
    }
    // `Attachment[<entity name>, <class>, <entity id>]`.
    let class = bracket(body, "Attachment[")?.split(',').nth(1)?;
    Some(LogEventKind::HeldItemReceived {
        item_class: owned(class)?,
        port: port.to_owned(),
    })
}

/// ```text
/// <…> [Notice] <EquipItem> Request[25] equip from Inventory[200000000001:Location:3700000001] Class[arma_barrel_supp_s2] Rank[amsabcdefghin] Port[Body_ItemPort:Armor_Undersuit:Armor_Torso:backpack:wep_stocked_2:barrel_attach] DependentRequest[4294967295] PostAction[None] [Team_CoreGameplayFeatures][Inventory]
/// ```
///
/// Every equip request has this shape (armor, magazines into the armor's
/// pouches, medpens); an attachment is the one whose port hangs off a
/// weapon or tool slot. The tag's other shapes (`Equip looting entity[…] to
/// port[…]`, the `cannot store` refusals) carry no port path and are not
/// events.
fn attachment_requested(body: &str) -> Option<LogEventKind> {
    let path = bracket(body, " Port[")?;
    let mut ports = path.rsplit(':');
    let port = ports.next()?;
    let holder_port = ports.next()?;
    if !holds_a_weapon(holder_port) {
        return None;
    }
    Some(LogEventKind::AttachmentRequested {
        item_class: owned(bracket(body, " Class[")?)?,
        holder_port: holder_port.to_owned(),
        port: owned(port)?,
    })
}

// ── HUD notifications ───────────────────────────────────────────────────

/// The parts of one HUD notification record:
///
/// ```text
/// <…> [Notice] <SHUDEvent_OnNotification> Added notification "New Objective: Defeat Hostile Ships: " [49] to queue. New queue size: 1, MissionId: [1d80…], ObjectiveId: [6d44…] [Team_CoreGameplayFeatures][Missions][Comms]
/// ```
///
/// The text always ends in `: ` (after a line break on multi-line ones);
/// that is trimmed. A zero mission id and an empty objective id are `None`.
struct Notification<'a> {
    text: &'a str,
    mission_id: Option<String>,
    objective_id: Option<String>,
}

fn split_notification(body: &str) -> Option<Notification<'_>> {
    const OPEN: &str = "Added notification \"";
    let rest = &body[body.find(OPEN)? + OPEN.len()..];
    // The text holds no `"`; the closing quote is followed by ` [n] to queue`.
    let close = rest.find("\" [")?;
    let text = rest[..close]
        .trim_end()
        .trim_end_matches(':')
        .trim_end()
        .trim_start();
    let tail = &rest[close..];
    const ZERO: &str = "00000000-0000-0000-0000-000000000000";
    Some(Notification {
        text,
        mission_id: bracket(tail, "MissionId: [")
            .filter(|id| *id != ZERO)
            .and_then(owned),
        objective_id: bracket(tail, "ObjectiveId: [").and_then(owned),
    })
}

fn notification(body: &str) -> Option<LogEventKind> {
    let n = split_notification(body)?;
    let text = n.text;

    const CONTRACTS: [(&str, ContractChange); 6] = [
        ("Contract Available: ", ContractChange::Available),
        ("Contract Accepted: ", ContractChange::Accepted),
        ("Contract Shared: ", ContractChange::Shared),
        ("Contract Complete: ", ContractChange::Completed),
        ("Contract Failed: ", ContractChange::Failed),
        ("Contract Withdrawn: ", ContractChange::Withdrawn),
    ];
    for (prefix, change) in CONTRACTS {
        if let Some(title) = text.strip_prefix(prefix) {
            return Some(LogEventKind::Contract {
                change,
                mission_id: n.mission_id,
                title: clean_title(title),
            });
        }
    }

    const OBJECTIVES: [(&str, ObjectiveChange); 3] = [
        ("New Objective: ", ObjectiveChange::New),
        ("Objective Complete: ", ObjectiveChange::Completed),
        ("Objective Withdrawn: ", ObjectiveChange::Withdrawn),
    ];
    for (prefix, change) in OBJECTIVES {
        if let Some(objective) = text.strip_prefix(prefix) {
            return Some(LogEventKind::Objective {
                change,
                mission_id: n.mission_id,
                objective_id: n.objective_id,
                text: clean_title(objective),
            });
        }
    }

    if let Some(injury) = injury(text) {
        return Some(injury);
    }
    if let Some(name) = text.strip_prefix("Received Blueprint: ") {
        return Some(LogEventKind::BlueprintReceived { name: owned(name)? });
    }
    if let Some(location) = text.strip_prefix("A Refinery Work Order has been Completed at ") {
        return Some(LogEventKind::RefineryOrderCompleted {
            location: owned(location)?,
        });
    }
    if let Some(jurisdiction) = text
        .strip_prefix("Entered ")
        .and_then(|t| t.strip_suffix(" Jurisdiction"))
    {
        return Some(LogEventKind::JurisdictionEntered {
            jurisdiction: owned(jurisdiction)?,
        });
    }
    money(text)
}

/// `Minor Injury Detected - Left arm - Tier 3 Treatment Required`.
fn injury(text: &str) -> Option<LogEventKind> {
    let (_, rest) = text.split_once(" Injury Detected - ")?;
    let (part, rest) = rest.split_once(" - ")?;
    let part = match part {
        "Head" => BodyPart::Head,
        "Torso" => BodyPart::Torso,
        "Left arm" => BodyPart::LeftArm,
        "Right arm" => BodyPart::RightArm,
        "Left leg" => BodyPart::LeftLeg,
        "Right leg" => BodyPart::RightLeg,
        _ => return None,
    };
    let tier = rest
        .strip_prefix("Tier ")?
        .split(' ')
        .next()?
        .parse()
        .ok()?;
    Some(LogEventKind::Injured { part, tier })
}

/// `Actor: VeeLume (…) | […] | -> Perform surgery event Success, med bed
/// name: …, vehicle name: none, head: true torso: false leftArm: true …`.
fn med_bed_surgery(body: &str) -> Option<LogEventKind> {
    let actor = body.split_once("Actor: ")?.1;
    let actor = owned(&actor[..actor.find(" (").or_else(|| actor.find(" |"))?])?;
    let outcome = body.split_once("surgery event ")?.1;
    let outcome = owned(outcome.split([',', ' ']).next()?)?;
    let vehicle = body.split_once("vehicle name: ")?.1;
    let vehicle = vehicle.split(',').next()?.trim();
    // A ship's bed names it by its display-name key: `@vehicle_NameRSI_Polaris`.
    const KEY: &str = "@vehicle_name";
    let vehicle = match vehicle.get(..KEY.len()) {
        Some(prefix) if prefix.eq_ignore_ascii_case(KEY) => owned(&vehicle[KEY.len()..]),
        _ => None,
    };
    const PARTS: [(&str, BodyPart); 6] = [
        (" head: true", BodyPart::Head),
        (" torso: true", BodyPart::Torso),
        (" leftArm: true", BodyPart::LeftArm),
        (" rightArm: true", BodyPart::RightArm),
        (" leftLeg: true", BodyPart::LeftLeg),
        (" rightLeg: true", BodyPart::RightLeg),
    ];
    let parts = PARTS
        .iter()
        .filter(|(key, _)| body.contains(key))
        .map(|(_, part)| *part)
        .collect();
    Some(LogEventKind::MedBedSurgery {
        actor,
        outcome,
        vehicle,
        parts,
    })
}

/// `Awarded 500 aUEC` / `Fined 72000 UEC` (the game prints both units).
fn money(text: &str) -> Option<LogEventKind> {
    let mut words = text.split(' ');
    let change = match words.next()? {
        "Awarded" => MoneyChange::Awarded,
        "Fined" => MoneyChange::Fined,
        _ => return None,
    };
    let amount: f64 = words.next()?.parse().ok()?;
    if !matches!(words.next()?, "aUEC" | "UEC") || words.next().is_some() {
        return None;
    }
    Some(LogEventKind::Money { change, amount })
}

/// A contract title or objective text as the game shows it, minus what
/// Starlume's text patch added: `<EM4>[BP]</EM4>` spans go, whitespace
/// collapses.
pub(crate) fn clean_title(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(open) = rest.find("<EM") {
        let Some(close) = rest[open..].find("</EM") else {
            break;
        };
        let after_close = &rest[open + close..];
        let Some(end) = after_close.find('>') else {
            break;
        };
        out.push_str(&rest[..open]);
        rest = &after_close[end + 1..];
    }
    out.push_str(rest);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ── Missions ────────────────────────────────────────────────────────────

/// ```text
/// <…> [Notice] <ObjectiveUpserted> Received ObjectiveUpserted push message for: mission_id 1d80… - objective_id d745… - state MISSION_OBJECTIVE_STATE_INPROGRESS - created 1 - flags=RespectInheritedVisibility| [Team_GameServices][Missions]
/// ```
fn objective_upserted(body: &str) -> Option<LogEventKind> {
    let field = |key: &str| -> Option<String> {
        let rest = &body[body.find(key)? + key.len()..];
        owned(&rest[..rest.find(" - ").unwrap_or(rest.len())])
    };
    Some(LogEventKind::ObjectiveUpserted {
        mission_id: field("mission_id ")?,
        objective_id: field("objective_id ")?,
        state: field("state ")?,
    })
}

/// ```text
/// <…> [Notice] <CLocalMissionPhaseMarker::CreateMarker> Creating objective marker: missionId [1d80…], generator name [Intersec_ShipWaveAttack], contract [Intersec_3Way_Nyx_VeryHard_Van], contractDefinitionId[7974…], objectiveId [fb51…], markerEntityId [73919], zoneHostId [926420132861], position [x: …] [Team_MissionFeatures][Missions]
/// ```
fn mission_marker(body: &str) -> Option<LogEventKind> {
    Some(LogEventKind::MissionMarkerCreated {
        mission_id: owned(bracket(body, "missionId [")?)?,
        generator: owned(bracket(body, "generator name [")?)?,
        contract: owned(bracket(body, ", contract [")?)?,
        contract_definition_id: owned(bracket(body, "contractDefinitionId[")?)?,
        objective_id: owned(bracket(body, "objectiveId [")?)?,
    })
}

/// ```text
/// <…> [Notice] <EndMission> Ending mission for player. MissionId[0432…] Player[VeeLume] PlayerId[200000000001] CompletionType[Complete] Reason[Mission Ended] [Team_MissionFeatures][Missions]
/// ```
///
/// The `<MissionEnded>` push that usually sits beside it (same timestamp)
/// is deliberately not an event: it repeats this line and is missing for
/// abandoned missions.
fn mission_ended(body: &str) -> Option<LogEventKind> {
    Some(LogEventKind::MissionEnded {
        mission_id: owned(bracket(body, "MissionId[")?)?,
        completion: owned(bracket(body, "CompletionType[")?)?,
        reason: bracket(body, "Reason[").unwrap_or_default().to_owned(),
    })
}

/// ```text
/// <2026-11-17T22:09:47.665Z> [Notice] <PlayerLeft> Received PlayerLeft push message for: mission_id de74…-6dfee9d92d6c - player_id 204717100112 [Team_GameServices][Missions]
/// ```
fn mission_left(body: &str) -> Option<LogEventKind> {
    let after = |key: &str| {
        let rest = &body[body.find(key)? + key.len()..];
        owned(rest.split_whitespace().next()?)
    };
    Some(LogEventKind::MissionLeft {
        mission_id: after("mission_id ")?,
        player_id: after("player_id ")?,
    })
}

// ── Combat ──────────────────────────────────────────────────────────────

/// ```text
/// <2026-11-18T19:26:26.880Z> [Notice] <Vehicle Destroyed> Vehicle 'MISC_Razor_LX_PU_AI_CRIM_939606349453' [939606349453] class 'MISC_Razor_LX_PU_AI_CRIM' (owner player [0], driver [0]) destroyed: reason [Combat] level [0] -> [2] flags [4066] killer 'VeeLume' [204717100112] authority [0] [Team_CGP4][Vehicle]
/// ```
fn vehicle_destroyed(body: &str) -> Option<LogEventKind> {
    let vehicle = body.find("Vehicle '")?;
    let rest = &body[vehicle..];
    let id_start = rest.find("' [")? + 3;
    let vehicle_id = owned(&rest[id_start..id_start + rest[id_start..].find(']')?])?;
    let class = owned(quoted(rest, "class '")?)?;
    let owner = bracket(rest, "(owner player [")?;
    let killer_at = rest.find(" killer '")?;
    let killer_rest = &rest[killer_at..];
    let killer = owned(quoted(killer_rest, " killer '")?)?;
    let killer_id = owned(bracket(killer_rest, "' [")?)?;
    Some(LogEventKind::VehicleDestroyed {
        vehicle_id,
        class,
        owner: (owner != "0").then(|| owner.to_owned()),
        reason: owned(bracket(rest, " reason [")?)?,
        level: bracket(rest, " -> [")?.parse().ok()?,
        killer,
        killer_id,
    })
}

/// The text between `key` and the next `'`.
fn quoted<'a>(s: &'a str, key: &str) -> Option<&'a str> {
    let start = s.find(key)? + key.len();
    let rest = &s[start..];
    Some(&rest[..rest.find('\'')?])
}

// ── Economy ─────────────────────────────────────────────────────────────

/// ```text
/// <…> [Notice] <CEntityComponentCommodityUIProvider::SendCommodityBuyRequest> Sending SShopCommodityBuyRequest - playerId[…] shopId[…] shopName[SCShop_Levski_CargoOffice_Commodities] kioskId[…] price[738720.000000] shopPricePerCentiSCU[153.899994] resourceGUID[93c8…] autoLoading[0] quantity[4800.000000 cSCU] Cargo Box Data: boxSize[16.000000] | unitAmount[3] […]
/// <…> [Notice] <CEntityComponentCommodityUIProvider::SendCommoditySellRequest> Sending SShopCommoditySellRequest - playerId[…] shopId[…] shopName[SCShop_CommEx_TDD_Orison] kioskId[…] amount[204119.000000] resourceGUID[392b…] autoLoading[0] quantity[4] transactionMode[ResourceContainer] Cargo Box Data:  [boxSize[4] | unitAmount[1]] […]
/// ```
///
/// A buy names its total `price[…]`, a sell `amount[…]`.
fn commodity_trade(body: &str, side: TradeSide) -> Option<LogEventKind> {
    let price = match side {
        TradeSide::Buy => bracket(body, " price[")?,
        TradeSide::Sell => bracket(body, " amount[")?,
    };
    let mut quantity = bracket(body, " quantity[")?.split_whitespace();
    Some(LogEventKind::CommodityTradeRequested {
        side,
        shop: owned(bracket(body, "shopName[")?)?,
        resource_guid: owned(bracket(body, "resourceGUID[")?)?,
        price: price.parse().ok()?,
        quantity: quantity.next()?.parse().ok()?,
        quantity_unit: quantity.next().map(str::to_owned),
    })
}

/// ```text
/// <…> [Notice] <CEntityComponentShopUIProvider::SendShopBuyRequest> Sending SShopBuyRequest - playerId[…] shopId[…] shopName[SCShop_RefineryAdmin_ITEM] kioskId[…] client_price[19175.000000] itemClassGUID[27ad…] itemName[grin_tractor_01] quantity[1]  […]
/// <…> [Notice] <CEntityComponentShoppingProvider::SendStandardItemBuyRequest> Sending SShopBuyRequest - playerId[…] shopId[…] shopName[SCShop_NewDeal_Interior_Lorville] kioskId[…] client_price[8483738.000000] itemClassGUID[aaa1…] itemName[ARGO_MOTH] quantity[1] currencyType[UEC] […]
/// ```
fn shop_buy(body: &str) -> Option<LogEventKind> {
    Some(LogEventKind::ShopBuyRequested {
        shop: owned(bracket(body, "shopName[")?)?,
        item_guid: owned(bracket(body, "itemClassGUID[")?)?,
        item_name: owned(bracket(body, "itemName[")?)?,
        price: bracket(body, "client_price[")?.parse().ok()?,
        quantity: bracket(body, " quantity[")?.parse().ok()?,
    })
}

/// ```text
/// <…> [Notice] <CEntityComponentShoppingProvider::SendRentalRequest> Sending SShopRentalRequest - playerId[…] shopId[…] shopName[SCShop_IAE_Rentals] kioskId[…] client_price[0.000000] itemClassGUID[9f4e…] itemName[RSI_Constellation_Andromeda_GS] offering[0] currencyType[UEC] […]
/// ```
fn shop_rental(body: &str) -> Option<LogEventKind> {
    Some(LogEventKind::ShopRentalRequested {
        shop: owned(bracket(body, "shopName[")?)?,
        item_guid: owned(bracket(body, "itemClassGUID[")?)?,
        item_name: owned(bracket(body, "itemName[")?)?,
        price: bracket(body, "client_price[")?.parse().ok()?,
    })
}

/// ```text
/// <…> [Notice] <CEntityComponentShopUIProvider::RmShopFlowResponse> Received ShopFlowResponse - playerId[…] shopId[…] shopName[…] kioskId[…] kioskState[BuyRequestProcessing] result[Success] type[Buying] […]
/// <…> [Notice] <CEntityComponentShoppingProvider::RmShopFlowResponse> Shop Flow Response - playerId[…] result[Success] […]
/// ```
fn shop_flow_response(body: &str) -> Option<LogEventKind> {
    let result = owned(bracket(body, "result[")?)?;
    Some(LogEventKind::ShopFlowResponse {
        success: result == "Success",
        result,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Every fixture is a line from a real LIVE Game.log (the logbackups of
    // 2026, builds 4.8 to 4.10.3), verbatim except for the account id and
    // the player's entity id (geid / playerId), which are made up: this
    // repository is public.

    fn kind(line: &str) -> Option<LogEventKind> {
        parse_record(line).map(|e| e.event)
    }

    #[test]
    fn an_event_carries_its_lines_timestamp() {
        let line = r#"<2026-11-13T16:13:05.561Z> [Notice] <Join PU> address[34.156.32.60] port[64385] shard[pub_euw1b_13165946_100] locationId[-281470681677823] [Team_GameServices][GIM][Matchmaking]"#;
        assert_eq!(
            parse_record(line),
            Some(LogEvent {
                ts: "2026-11-13T16:13:05.561Z".into(),
                event: LogEventKind::ShardJoined {
                    shard: "pub_euw1b_13165946_100".into()
                },
            })
        );
    }

    #[test]
    fn the_join_pu_broker_line_is_not_a_shard_join() {
        // Logged beside <Join PU>, without a shard.
        let line = r#"<2026-11-13T16:13:05.561Z> [+] [CIG] {Join PU} [0] id[c7305ac1-1d91-43f4-83a4-115954ef2352] status[1] port[64385]"#;
        assert_eq!(kind(line), None);
    }

    #[test]
    fn session_header_lines() {
        assert_eq!(
            kind("<2026-11-13T16:10:48.019Z> Log started on Fri Nov 13 16:10:48 2026"),
            Some(LogEventKind::SessionStarted)
        );
        assert_eq!(
            kind("<2026-11-13T16:10:48.734Z> Branch: sc-alpha-4.10.0"),
            Some(LogEventKind::Branch {
                name: "sc-alpha-4.10.0".into()
            })
        );
        assert_eq!(
            kind("<2026-11-13T16:10:48.734Z> Changelist: 13165946"),
            Some(LogEventKind::Changelist {
                number: "13165946".into()
            })
        );
        assert_eq!(
            kind("<2026-11-13T16:10:48.023Z>    [Cmdline* ] --envtag='PUB'"),
            Some(LogEventKind::SessionPlatform {
                env_tag: "PUB".into()
            })
        );
    }

    #[test]
    fn other_mentions_of_the_env_tag_are_ignored() {
        // The deprecation hint and the full command line both name it.
        assert_eq!(
            kind(
                "<2026-11-13T16:10:48.023Z>    -envtag (Please use '--system-startup-env-tag' instead)"
            ),
            None
        );
        let cmdline = r#"<2026-11-13T16:10:48.732Z> [CIG] startupParams.szSystemCmdLine["C:\Games\StarCitizen\LIVE\Bin64/StarCitizen.exe" -no_login_dialog -envtag PUB --client-login-show-dialog 0 --services-config-enabled 1"#;
        assert_eq!(kind(cmdline), None);
    }

    #[test]
    fn session_handle_and_account() {
        let login = r#"<2026-11-13T16:11:16.243Z> [Notice] <Legacy login response> [CIG-net] User Login Success - Handle[VeeLume] - Time[375324561] [Team_GameServices][Login]"#;
        assert_eq!(
            kind(login),
            Some(LogEventKind::SessionHandle {
                handle: "VeeLume".into()
            })
        );
        let character = r#"<2026-11-13T16:11:13.101Z> [Notice] <AccountLoginCharacterStatus_Character> Character: createdAt 1700000000000 - updatedAt 1700000000001 - geid 200000000001 - accountId 1234567 - name VeeLume - state STATE_CURRENT [Team_GameServices][Login]"#;
        assert_eq!(
            kind(character),
            Some(LogEventKind::SessionAccountId {
                account_id: "1234567".into(),
                geid: Some("200000000001".into()),
            })
        );
    }

    #[test]
    fn a_player_leaving_a_mission() {
        let line = r#"<2026-11-17T22:09:47.665Z> [Notice] <PlayerLeft> Received PlayerLeft push message for: mission_id de7491a1-121d-4b5e-a26d-6dfee9d92d6c - player_id 204717100112 [Team_GameServices][Missions]"#;
        assert_eq!(
            kind(line),
            Some(LogEventKind::MissionLeft {
                mission_id: "de7491a1-121d-4b5e-a26d-6dfee9d92d6c".into(),
                player_id: "204717100112".into(),
            })
        );
    }

    #[test]
    fn an_injury_on_the_hud() {
        let line = r#"<2026-11-18T20:38:33.983Z> [Notice] <SHUDEvent_OnNotification> Added notification "Moderate Injury Detected - Left arm - Tier 2 Treatment Required : " [76] to queue. New queue size: 1, MissionId: [00000000-0000-0000-0000-000000000000], ObjectiveId: [] [Team_CoreGameplayFeatures][Missions][Comms]"#;
        assert_eq!(
            kind(line),
            Some(LogEventKind::Injured {
                part: BodyPart::LeftArm,
                tier: 2
            })
        );
    }

    #[test]
    fn surgery_in_a_hospital_and_in_a_ship() {
        let hospital = r#"<2026-11-18T21:02:36.564Z> [Notice] <MED BED HEAL> Actor: VeeLume (Non-Authoritative CLIENT: VeeLume) | [CEntityComponentMedBed::HandleComponentEvent:1101] | -> Perform surgery event Success, med bed name: bed_hospital_2_a-005, vehicle name: none, head: true torso: false leftArm: true rightArm: false leftLeg: false rightLeg: false [Team_ActorFeatures][Actor]"#;
        assert_eq!(
            kind(hospital),
            Some(LogEventKind::MedBedSurgery {
                actor: "VeeLume".into(),
                outcome: "Success".into(),
                vehicle: None,
                parts: vec![BodyPart::Head, BodyPart::LeftArm],
            })
        );
        let ship = r#"<2024-12-02T22:07:59.522Z> [Notice] <MED BED HEAL> Actor: VeeLume (Non-Authoritative CLIENT: VeeLume) | [CEntityComponentMedBed::HandleComponentEvent:817] | -> Perform surgery event Success, med bed name: Bed_Single_Medical_Carrack-001, vehicle name: @vehicle_nameANVL_Carrack, head: false torso: true leftArm: false rightArm: false leftLeg: false rightLeg: false [Team_ActorFeatures][Actor]"#;
        assert_eq!(
            kind(ship),
            Some(LogEventKind::MedBedSurgery {
                actor: "VeeLume".into(),
                outcome: "Success".into(),
                vehicle: Some("ANVL_Carrack".into()),
                parts: vec![BodyPart::Torso],
            })
        );
    }

    #[test]
    fn the_vehicle_terminal_fetches_your_ships() {
        let line = r#"<2026-11-17T22:57:02.640Z> [Notice] <OnRequestFetchVehicles> Fetching player vehicle list [Team_GameServices][ASOP][Entitlement][Insurance]"#;
        assert_eq!(kind(line), Some(LogEventKind::VehicleListFetched));
        let line = r#"<2026-11-17T22:57:02.877Z> [Notice] <OnRequestFetchVehicles> Querying location inventory for player [204717100112] at location [848868938] [Team_GameServices][ASOP][Entitlement][Insurance]"#;
        assert_eq!(kind(line), None);
    }

    #[test]
    fn a_ship_you_shot_down() {
        let line = r#"<2026-11-18T19:26:26.880Z> [Notice] <Vehicle Destroyed> Vehicle 'MISC_Razor_LX_PU_AI_CRIM_939606349453' [939606349453] class 'MISC_Razor_LX_PU_AI_CRIM' (owner player [0], driver [0]) destroyed: reason [Combat] level [0] -> [2] flags [4066] killer 'VeeLume' [204717100112] authority [0] [Team_CGP4][Vehicle]"#;
        assert_eq!(
            kind(line),
            Some(LogEventKind::VehicleDestroyed {
                vehicle_id: "939606349453".into(),
                class: "MISC_Razor_LX_PU_AI_CRIM".into(),
                owner: None,
                reason: "Combat".into(),
                level: 2,
                killer: "VeeLume".into(),
                killer_id: "204717100112".into(),
            })
        );
    }

    #[test]
    fn your_ship_lost_to_an_npc_and_to_itself() {
        let rammed = r#"<2026-11-17T23:31:05.927Z> [Notice] <Vehicle Destroyed> Vehicle 'DRAK_Marauder_939405947782' [939405947782] class 'DRAK_Marauder' (owner player [204717100112], driver [0]) destroyed: reason [Collision] level [0] -> [2] flags [4066] killer 'PU_Pilots-Human-Criminal-Pilot_Light_939479930047' [939479930047] authority [0] [Team_CGP4][Vehicle]"#;
        assert_eq!(
            kind(rammed),
            Some(LogEventKind::VehicleDestroyed {
                vehicle_id: "939405947782".into(),
                class: "DRAK_Marauder".into(),
                owner: Some("204717100112".into()),
                reason: "Collision".into(),
                level: 2,
                killer: "PU_Pilots-Human-Criminal-Pilot_Light_939479930047".into(),
                killer_id: "939479930047".into(),
            })
        );
        let self_destruct = r#"<2026-11-17T21:58:50.410Z> [Notice] <Vehicle Destroyed> Vehicle 'DRAK_Marauder_939283554698' [939283554698] class 'DRAK_Marauder' (owner player [204717100112], driver [0]) destroyed: reason [SelfDestruct] level [0] -> [2] flags [16482] killer 'DRAK_Marauder_939283554698' [939283554698] authority [0] [Team_CGP4][Vehicle]"#;
        let Some(LogEventKind::VehicleDestroyed {
            reason,
            killer_id,
            vehicle_id,
            ..
        }) = kind(self_destruct)
        else {
            panic!("not a destroyed vehicle");
        };
        assert_eq!(reason, "SelfDestruct");
        assert_eq!(killer_id, vehicle_id);
    }

    #[test]
    fn a_character_that_is_not_current_gives_no_account() {
        // Hearth's fixture: a defunct character row.
        let line = r#"<2026-05-30T13:19:26.325Z> [Notice] <AccountLoginCharacterStatus_Character> Character: … - accountId 1234567 - name OldChar - state STATE_DEFUNCT [Team_GameServices][Login]"#;
        assert_eq!(kind(line), None);
    }

    #[test]
    fn quitting_ends_the_session() {
        let line = r#"<2026-11-12T00:42:08.671Z> [Notice] <SystemQuit> CSystem::Quit invoked with - cause=30016, reason=Quit via console command, exitCode=0, thread id=39820, main thread id=39820 [Team_Unknown][System]"#;
        assert_eq!(
            kind(line),
            Some(LogEventKind::SessionEnded {
                reason: "Quit via console command".into()
            })
        );
    }

    #[test]
    fn location_inventory_request() {
        let line = r#"<2026-11-13T16:13:38.798Z> [Notice] <RequestLocationInventory> Player[VeeLume] requested inventory for Location[Nyx_Levski] [Team_CoreGameplayFeatures][Inventory]"#;
        assert_eq!(
            kind(line),
            Some(LogEventKind::LocationInventoryRequested {
                location: "Nyx_Levski".into()
            })
        );
    }

    #[test]
    fn jurisdiction_entered() {
        let line = r#"<2026-11-13T16:34:11.637Z> [Notice] <SHUDEvent_OnNotification> Added notification "Entered People's Alliance Jurisdiction: " [5] to queue. New queue size: 2, MissionId: [00000000-0000-0000-0000-000000000000], ObjectiveId: [] [Team_CoreGameplayFeatures][Missions][Comms]"#;
        assert_eq!(
            kind(line),
            Some(LogEventKind::JurisdictionEntered {
                jurisdiction: "People's Alliance".into()
            })
        );
        // "Entered Monitored Space" is a notification, but not a jurisdiction.
        let monitored = r#"<2026-11-12T00:08:26.976Z> [Notice] <SHUDEvent_OnNotification> Added notification "Entered Monitored Space: " [0] to queue. New queue size: 1, MissionId: [00000000-0000-0000-0000-000000000000], ObjectiveId: [] [Team_CoreGameplayFeatures][Missions][Comms]"#;
        assert_eq!(kind(monitored), None);
    }

    #[test]
    fn contract_accepted_drops_the_text_patch_tag() {
        let line = r#"<2026-07-16T15:51:01.258Z> [Notice] <SHUDEvent_OnNotification> Added notification "Contract Accepted:  Moraine Data Retrieval <EM4>[BP]</EM4>: " [98] to queue. New queue size: 1, MissionId: [816f97d2-9cbb-410d-89d7-73bb4ffbf548], ObjectiveId: [] [Team_CoreGameplayFeatures][Missions][Comms]"#;
        assert_eq!(
            kind(line),
            Some(LogEventKind::Contract {
                change: ContractChange::Accepted,
                mission_id: Some("816f97d2-9cbb-410d-89d7-73bb4ffbf548".into()),
                title: "Moraine Data Retrieval".into(),
            })
        );
    }

    #[test]
    fn contract_changes() {
        let complete = r#"<2026-07-16T16:28:12.643Z> [Notice] <SHUDEvent_OnNotification> Added notification "Contract Complete: Moraine Data Retrieval <EM4>[BP]</EM4>: " [123] to queue. New queue size: 1, MissionId: [816f97d2-9cbb-410d-89d7-73bb4ffbf548], ObjectiveId: [] [Team_CoreGameplayFeatures][Missions][Comms]"#;
        let failed = r#"<2026-10-20T03:10:57.929Z> [Notice] <SHUDEvent_OnNotification> Added notification "Contract Failed: RSI Disc. Month: Orange Lvl. - RSI Ship Under Attack: " [80] to queue. New queue size: 3, MissionId: [05a09aa4-1be3-4730-9896-acca9571732a], ObjectiveId: [] [Team_CoreGameplayFeatures][Missions][Comms]"#;
        let shared = r#"<2026-10-20T03:06:57.957Z> [Notice] <SHUDEvent_OnNotification> Added notification "Contract Shared: RSI Disc. Month: Yellow Lvl. - Support Ship: " [59] to queue. New queue size: 1, MissionId: [00000000-0000-0000-0000-000000000000], ObjectiveId: [] [Team_CoreGameplayFeatures][Missions][Comms]"#;
        let available = r#"<2026-10-09T18:01:44.915Z> [Notice] <SHUDEvent_OnNotification> Added notification "Contract Available: RSI Disc. Month: Constellation Rental Offer [Solo] [Uniq]: " [4] to queue. New queue size: 1, MissionId: [040b4f07-d7a8-481e-89a9-fbc8b642d580], ObjectiveId: [] [Team_CoreGameplayFeatures][Missions][Comms]"#;
        let withdrawn = r#"<2026-09-05T04:50:23.136Z> [Notice] <SHUDEvent_OnNotification> Added notification "Contract Withdrawn:  Scan And Mining Needed <EM4>[BP]</EM4> <EM3>[✓]</EM3> [Solo]: " [46] to queue. New queue size: 1, MissionId: [f90a3a8f-a9bb-4c24-9316-dd3eccbf7f04], ObjectiveId: [] [Team_CoreGameplayFeatures][Missions][Comms]"#;

        assert_eq!(
            kind(complete),
            Some(LogEventKind::Contract {
                change: ContractChange::Completed,
                mission_id: Some("816f97d2-9cbb-410d-89d7-73bb4ffbf548".into()),
                title: "Moraine Data Retrieval".into(),
            })
        );
        assert_eq!(
            kind(failed),
            Some(LogEventKind::Contract {
                change: ContractChange::Failed,
                mission_id: Some("05a09aa4-1be3-4730-9896-acca9571732a".into()),
                // A title with colons of its own stays whole.
                title: "RSI Disc. Month: Orange Lvl. - RSI Ship Under Attack".into(),
            })
        );
        assert_eq!(
            kind(shared),
            Some(LogEventKind::Contract {
                change: ContractChange::Shared,
                mission_id: None,
                title: "RSI Disc. Month: Yellow Lvl. - Support Ship".into(),
            })
        );
        assert_eq!(
            kind(available),
            Some(LogEventKind::Contract {
                change: ContractChange::Available,
                mission_id: Some("040b4f07-d7a8-481e-89a9-fbc8b642d580".into()),
                title: "RSI Disc. Month: Constellation Rental Offer [Solo] [Uniq]".into(),
            })
        );
        assert_eq!(
            kind(withdrawn),
            Some(LogEventKind::Contract {
                change: ContractChange::Withdrawn,
                mission_id: Some("f90a3a8f-a9bb-4c24-9316-dd3eccbf7f04".into()),
                title: "Scan And Mining Needed [Solo]".into(),
            })
        );
    }

    #[test]
    fn objective_changes() {
        let new = r#"<2026-11-13T17:36:52.723Z> [Notice] <SHUDEvent_OnNotification> Added notification "New Objective: Defeat Hostile Ships: " [49] to queue. New queue size: 1, MissionId: [1d803b44-738f-4f97-bcec-2641e38e93b9], ObjectiveId: [6d44421f-50ce-c78f-17a8-201d53377194] [Team_CoreGameplayFeatures][Missions][Comms]"#;
        assert_eq!(
            kind(new),
            Some(LogEventKind::Objective {
                change: ObjectiveChange::New,
                mission_id: Some("1d803b44-738f-4f97-bcec-2641e38e93b9".into()),
                objective_id: Some("6d44421f-50ce-c78f-17a8-201d53377194".into()),
                text: "Defeat Hostile Ships".into(),
            })
        );
        let withdrawn = r#"<2026-10-20T03:09:06.513Z> [Notice] <SHUDEvent_OnNotification> Added notification "Objective Withdrawn: Go to Constellation wreck site: " [69] to queue. New queue size: 1, MissionId: [9efa4553-3b8d-4a6e-b7fc-b4619565d8e5], ObjectiveId: [29eee4b1-e415-1541-bc48-300b32b57ef0] [Team_CoreGameplayFeatures][Missions][Comms]"#;
        assert_eq!(
            kind(withdrawn),
            Some(LogEventKind::Objective {
                change: ObjectiveChange::Withdrawn,
                mission_id: Some("9efa4553-3b8d-4a6e-b7fc-b4619565d8e5".into()),
                objective_id: Some("29eee4b1-e415-1541-bc48-300b32b57ef0".into()),
                text: "Go to Constellation wreck site".into(),
            })
        );
    }

    #[test]
    fn a_multi_line_notification_parses_once_joined() {
        // Two log lines; the tailer joins the continuation with `\n`.
        let record = concat!(
            r#"<2026-11-13T17:36:45.782Z> [Notice] <SHUDEvent_OnNotification> Added notification "Objective Complete: Go to Glaciem Ring Cluster"#,
            "\n",
            r#": " [48] to queue. New queue size: 1, MissionId: [1d803b44-738f-4f97-bcec-2641e38e93b9], ObjectiveId: [fb517a56-add8-9741-ac83-855782ca88dd] [Team_CoreGameplayFeatures][Missions][Comms]"#,
        );
        assert_eq!(
            kind(record),
            Some(LogEventKind::Objective {
                change: ObjectiveChange::Completed,
                mission_id: Some("1d803b44-738f-4f97-bcec-2641e38e93b9".into()),
                objective_id: Some("fb517a56-add8-9741-ac83-855782ca88dd".into()),
                text: "Go to Glaciem Ring Cluster".into(),
            })
        );
    }

    #[test]
    fn notification_echoes_and_lifecycle_lines_are_not_events() {
        let echo = r#"<2026-09-26T11:37:58.233Z>    "Awarded 500 aUEC: " [16]"#;
        let next = r#"<2026-09-26T11:38:00.473Z> [Notice] <UpdateNotificationItem> Notification "Awarded 500 aUEC: " [16], Action: Next [Team_CoreGameplayFeatures][Missions][Comms]"#;
        let remove = r#"<2026-11-12T00:09:10.105Z> [Notice] <UpdateNotificationItem> Notification "A Refinery Work Order has been Completed at Stanton Gateway: " [2], Action: Remove [Team_CoreGameplayFeatures][Missions][Comms]"#;
        assert_eq!(kind(echo), None);
        assert_eq!(kind(next), None);
        assert_eq!(kind(remove), None);
    }

    #[test]
    fn money_awarded_and_fined() {
        let awarded = r#"<2026-09-26T11:37:58.233Z> [Notice] <SHUDEvent_OnNotification> Added notification "Awarded 500 aUEC: " [16] to queue. New queue size: 1, MissionId: [00000000-0000-0000-0000-000000000000], ObjectiveId: [] [Team_CoreGameplayFeatures][Missions][Comms]"#;
        assert_eq!(
            kind(awarded),
            Some(LogEventKind::Money {
                change: MoneyChange::Awarded,
                amount: 500.0
            })
        );
        // Fines are printed in "UEC", awards in "aUEC".
        let fined = r#"<2026-09-23T21:26:58.779Z> [Notice] <SHUDEvent_OnNotification> Added notification "Fined 72000 UEC: " [9] to queue. New queue size: 1, MissionId: [00000000-0000-0000-0000-000000000000], ObjectiveId: [] [Team_CoreGameplayFeatures][Missions][Comms]"#;
        assert_eq!(
            kind(fined),
            Some(LogEventKind::Money {
                change: MoneyChange::Fined,
                amount: 72000.0
            })
        );
    }

    #[test]
    fn refinery_order_completed() {
        let line = r#"<2026-11-12T00:08:53.096Z> [Notice] <SHUDEvent_OnNotification> Added notification "A Refinery Work Order has been Completed at Stanton Gateway: " [2] to queue. New queue size: 1, MissionId: [00000000-0000-0000-0000-000000000000], ObjectiveId: [] [Team_CoreGameplayFeatures][Missions][Comms]"#;
        assert_eq!(
            kind(line),
            Some(LogEventKind::RefineryOrderCompleted {
                location: "Stanton Gateway".into()
            })
        );
    }

    #[test]
    fn blueprint_received() {
        // Hearth's fixture (4.8, 2026-05-30).
        let line = r#"<2026-05-30T14:43:35.628Z> [Notice] <SHUDEvent_OnNotification> Added notification "Received Blueprint: S3 Attrition-3 Repeater: " [61] to queue. New queue size: 3, MissionId: [00000000-0000-0000-0000-000000000000], ObjectiveId: [] [Team_CoreGameplayFeatures][Missions][Comms]"#;
        assert_eq!(
            kind(line),
            Some(LogEventKind::BlueprintReceived {
                name: "S3 Attrition-3 Repeater".into()
            })
        );
    }

    #[test]
    fn objective_upserted() {
        let line = r#"<2026-11-13T17:36:45.999Z> [Notice] <ObjectiveUpserted> Received ObjectiveUpserted push message for: mission_id 1d803b44-738f-4f97-bcec-2641e38e93b9 - objective_id d745b504-88c9-3fce-a594-58080872e0ab - state MISSION_OBJECTIVE_STATE_INPROGRESS - created 1 - flags=RespectInheritedVisibility| [Team_GameServices][Missions]"#;
        assert_eq!(
            kind(line),
            Some(LogEventKind::ObjectiveUpserted {
                mission_id: "1d803b44-738f-4f97-bcec-2641e38e93b9".into(),
                objective_id: "d745b504-88c9-3fce-a594-58080872e0ab".into(),
                state: "MISSION_OBJECTIVE_STATE_INPROGRESS".into(),
            })
        );
        // Hauling objectives carry a composite id, not a bare guid.
        let hauling = r#"<2026-08-23T01:24:41.310Z> [Notice] <ObjectiveUpserted> Received ObjectiveUpserted push message for: mission_id 75942893-5d24-4bdc-a21c-9e4db85a80a8 - objective_id pickup_2234f2b0-c67f-45c2-8f47-12d2086fa19e_0 - state MISSION_OBJECTIVE_STATE_COMPLETED - created 0 - flags=ShowInLog| [Team_GameServices][Missions]"#;
        assert_eq!(
            kind(hauling),
            Some(LogEventKind::ObjectiveUpserted {
                mission_id: "75942893-5d24-4bdc-a21c-9e4db85a80a8".into(),
                objective_id: "pickup_2234f2b0-c67f-45c2-8f47-12d2086fa19e_0".into(),
                state: "MISSION_OBJECTIVE_STATE_COMPLETED".into(),
            })
        );
    }

    #[test]
    fn mission_marker_ties_a_mission_to_its_contract() {
        let line = r#"<2026-11-13T17:26:29.657Z> [Notice] <CLocalMissionPhaseMarker::CreateMarker> Creating objective marker: missionId [1d803b44-738f-4f97-bcec-2641e38e93b9], generator name [Intersec_ShipWaveAttack], contract [Intersec_3Way_Nyx_VeryHard_Van], contractDefinitionId[79741482-555e-4e17-b6cf-c08268cd89d7], objectiveId [fb517a56-add8-9741-ac83-855782ca88dd], markerEntityId [73919], zoneHostId [926420132861], position [x: 30527.994473, y: -19228.953341, z: -38144.475943] [Team_MissionFeatures][Missions]"#;
        assert_eq!(
            kind(line),
            Some(LogEventKind::MissionMarkerCreated {
                mission_id: "1d803b44-738f-4f97-bcec-2641e38e93b9".into(),
                generator: "Intersec_ShipWaveAttack".into(),
                contract: "Intersec_3Way_Nyx_VeryHard_Van".into(),
                contract_definition_id: "79741482-555e-4e17-b6cf-c08268cd89d7".into(),
                objective_id: "fb517a56-add8-9741-ac83-855782ca88dd".into(),
            })
        );
    }

    #[test]
    fn mission_ended() {
        let complete = r#"<2026-10-20T02:38:23.242Z> [Notice] <EndMission> Ending mission for player. MissionId[0432d4d8-80c8-4308-9422-306527878bbc] Player[VeeLume] PlayerId[200000000001] CompletionType[Complete] Reason[Mission Ended] [Team_MissionFeatures][Missions]"#;
        assert_eq!(
            kind(complete),
            Some(LogEventKind::MissionEnded {
                mission_id: "0432d4d8-80c8-4308-9422-306527878bbc".into(),
                completion: "Complete".into(),
                reason: "Mission Ended".into(),
            })
        );
        let abandoned = r#"<2026-10-20T03:12:21.761Z> [Notice] <EndMission> Ending mission for player. MissionId[60f7a038-9d29-471c-8a01-3083d15bee6d] Player[VeeLume] PlayerId[200000000001] CompletionType[Abandon] Reason[Player left] [Team_MissionFeatures][Missions]"#;
        assert_eq!(
            kind(abandoned),
            Some(LogEventKind::MissionEnded {
                mission_id: "60f7a038-9d29-471c-8a01-3083d15bee6d".into(),
                completion: "Abandon".into(),
                reason: "Player left".into(),
            })
        );
        // The push beside it repeats the same ending: not a second event.
        let push = r#"<2026-10-20T02:38:23.242Z> [Notice] <MissionEnded> Received MissionEnded push message for: mission_id 0432d4d8-80c8-4308-9422-306527878bbc - mission_state MISSION_STATE_COMPLETED [Team_GameServices][Missions]"#;
        assert_eq!(kind(push), None);
    }

    #[test]
    fn commodity_buy_request() {
        let line = r#"<2026-07-23T22:54:25.143Z> [Notice] <CEntityComponentCommodityUIProvider::SendCommodityBuyRequest> Sending SShopCommodityBuyRequest - playerId[200000000001] shopId[730150087848] shopName[SCShop_Levski_CargoOffice_Commodities] kioskId[730150087845] price[738720.000000] shopPricePerCentiSCU[153.899994] resourceGUID[93c8b7df-d6ac-4b4f-a115-b0e3afc238b8] autoLoading[0] quantity[4800.000000 cSCU] Cargo Box Data: boxSize[16.000000] | unitAmount[3] [Team_CoreGameplayFeatures][Shops][UI]"#;
        assert_eq!(
            kind(line),
            Some(LogEventKind::CommodityTradeRequested {
                side: TradeSide::Buy,
                shop: "SCShop_Levski_CargoOffice_Commodities".into(),
                resource_guid: "93c8b7df-d6ac-4b4f-a115-b0e3afc238b8".into(),
                price: 738720.0,
                quantity: 4800.0,
                quantity_unit: Some("cSCU".into()),
            })
        );
    }

    #[test]
    fn commodity_sell_request() {
        let line = r#"<2026-06-14T20:57:48.695Z> [Notice] <CEntityComponentCommodityUIProvider::SendCommoditySellRequest> Sending SShopCommoditySellRequest - playerId[200000000001] shopId[432753933748] shopName[SCShop_CommEx_TDD_Orison] kioskId[432753933765] amount[204119.000000] resourceGUID[392b4dca-449a-4d4d-8fef-beab024d9ee7] autoLoading[0] quantity[4] transactionMode[ResourceContainer] Cargo Box Data:  [boxSize[4] | unitAmount[1]] [Team_CoreGameplayFeatures][Shops][UI]"#;
        assert_eq!(
            kind(line),
            Some(LogEventKind::CommodityTradeRequested {
                side: TradeSide::Sell,
                shop: "SCShop_CommEx_TDD_Orison".into(),
                resource_guid: "392b4dca-449a-4d4d-8fef-beab024d9ee7".into(),
                price: 204119.0,
                quantity: 4.0,
                quantity_unit: None,
            })
        );
    }

    #[test]
    fn the_commodity_kiosk_listing_is_not_a_trade() {
        // Hundreds of these per kiosk visit, from a tag with `>` inside.
        let line = r#"<2026-11-11T22:48:38.110Z> [Notice] <CEntityComponentCommodityUIProvider::LoadShopInventoryData::<lambda_1>::operator ()> AddingCommodityBox - playerId[200000000001] shopId[917335513303] shopName[SCShop_ht_delta_shubin_m_store] commodityName[ResourceType.Aluminum] Available Box Sizes:  boxSize[1] boxSize[2] boxSize[4] boxSize[8] boxSize[16] [Team_CoreGameplayFeatures][Shops][UI]"#;
        assert_eq!(kind(line), None);
    }

    #[test]
    fn shop_buy_requests_from_both_providers() {
        let kiosk = r#"<2026-10-19T04:20:15.836Z> [Notice] <CEntityComponentShopUIProvider::SendShopBuyRequest> Sending SShopBuyRequest - playerId[200000000001] shopId[875752473283] shopName[SCShop_RefineryAdmin_ITEM] kioskId[875752473277] client_price[19175.000000] itemClassGUID[27adea05-f94d-4439-872d-b043a631c34f] itemName[grin_tractor_01] quantity[1]  [Team_CoreGameplayFeatures][Shops][UI]"#;
        assert_eq!(
            kind(kiosk),
            Some(LogEventKind::ShopBuyRequested {
                shop: "SCShop_RefineryAdmin_ITEM".into(),
                item_guid: "27adea05-f94d-4439-872d-b043a631c34f".into(),
                item_name: "grin_tractor_01".into(),
                price: 19175.0,
                quantity: 1,
            })
        );
        let dealer = r#"<2026-10-19T04:31:13.672Z> [Notice] <CEntityComponentShoppingProvider::SendStandardItemBuyRequest> Sending SShopBuyRequest - playerId[200000000001] shopId[875755012217] shopName[SCShop_NewDeal_Interior_Lorville] kioskId[875755012215] client_price[8483738.000000] itemClassGUID[aaa12a00-a1e7-4dfc-8626-3bebb005209f] itemName[ARGO_MOTH] quantity[1] currencyType[UEC] [Team_CoreGameplayFeatures][Shops][UI]"#;
        assert_eq!(
            kind(dealer),
            Some(LogEventKind::ShopBuyRequested {
                shop: "SCShop_NewDeal_Interior_Lorville".into(),
                item_guid: "aaa12a00-a1e7-4dfc-8626-3bebb005209f".into(),
                item_name: "ARGO_MOTH".into(),
                price: 8483738.0,
                quantity: 1,
            })
        );
    }

    #[test]
    fn a_rental_request_is_its_own_event_not_a_purchase() {
        let line = r#"<2026-11-13T17:02:40.873Z> [Notice] <CEntityComponentShoppingProvider::SendRentalRequest> Sending SShopRentalRequest - playerId[200000000001] shopId[926683682375] shopName[SCShop_IAE_Rentals] kioskId[926683682368] client_price[0.000000] itemClassGUID[9f4e2cea-f7dc-4155-9cb2-95bb42a74141] itemName[RSI_Constellation_Andromeda_GS] offering[0] currencyType[UEC] [Team_CoreGameplayFeatures][Shops][UI]"#;
        assert_eq!(
            kind(line),
            Some(LogEventKind::ShopRentalRequested {
                shop: "SCShop_IAE_Rentals".into(),
                item_guid: "9f4e2cea-f7dc-4155-9cb2-95bb42a74141".into(),
                item_name: "RSI_Constellation_Andromeda_GS".into(),
                price: 0.0,
            })
        );
    }

    #[test]
    fn shop_flow_responses() {
        let kiosk = r#"<2026-08-23T01:44:36.552Z> [Notice] <CEntityComponentShopUIProvider::RmShopFlowResponse> Received ShopFlowResponse - playerId[200000000001] shopId[762462339896] shopName[SCShop_Novelty_Kiosk_NewBabbage] kioskId[762462339923] kioskState[BuyRequestProcessing] result[Success] type[Buying] [Team_CoreGameplayFeatures][Shops][UI]"#;
        assert_eq!(
            kind(kiosk),
            Some(LogEventKind::ShopFlowResponse {
                success: true,
                result: "Success".into()
            })
        );
        let refused = r#"<2026-08-28T06:10:44.657Z> [Notice] <CEntityComponentShoppingProvider::RmShopFlowResponse> Shop Flow Response - playerId[200000000001] result[AuthorityError] [Team_CoreGameplayFeatures][Shops][UI]"#;
        assert_eq!(
            kind(refused),
            Some(LogEventKind::ShopFlowResponse {
                success: false,
                result: "AuthorityError".into()
            })
        );
    }

    // The quantum and gear fixtures are from the LIVE log of 2026-11-14
    // (4.10.3). Made up in them: the ship's and the items' entity ids, the
    // player's inventory id, the client number after `[CL]` and the
    // inventory rank strings.

    #[test]
    fn a_quantum_target_is_selected() {
        let line = r#"<2026-11-14T15:03:23.593Z> [Notice] <Player Selected Quantum Target - Local> [ItemNavigation][CL][41200] | NOT AUTH | RSI_Constellation_Mk5_Centaurus_930000000001[930000000001]|CSCItemNavigation::OnPlayerSelectedQuantumTarget|Player has selected point Orison_LOC as their destination, routing locally [Team_CGP4][QuantumTravel]"#;
        assert_eq!(
            kind(line),
            Some(LogEventKind::QuantumTargetSelected {
                vehicle_class: Some("RSI_Constellation_Mk5_Centaurus".into()),
                destination: "Orison_LOC".into(),
            })
        );
        // A jump point's name has a `-` and digits of its own.
        let jump = r#"<2026-11-14T14:54:49.930Z> [Notice] <Player Selected Quantum Target - Local> [ItemNavigation][CL][41200] | NOT AUTH | RSI_Constellation_Mk5_Centaurus_930000000001[930000000001]|CSCItemNavigation::OnPlayerSelectedQuantumTarget|Player has selected point rs_ext_nyx-castra_jp1 as their destination, routing locally [Team_CGP4][QuantumTravel]"#;
        assert_eq!(
            kind(jump),
            Some(LogEventKind::QuantumTargetSelected {
                vehicle_class: Some("RSI_Constellation_Mk5_Centaurus".into()),
                destination: "rs_ext_nyx-castra_jp1".into(),
            })
        );
    }

    #[test]
    fn browsing_the_star_map_selects_nothing() {
        // The map asks what a destination would cost before one is set.
        let fuel = r#"<2026-11-14T15:03:21.723Z> [Notice] <Player Requested Fuel to Quantum Target - Server Routing> [ItemNavigation][CL][41200] | NOT AUTH | RSI_Constellation_Mk5_Centaurus_930000000001[930000000001]|CSCItemNavigation::OnPlayerRequestFuelToQuantumTarget|Player has requested fuel calculation to destination Orison_LOC [Team_CGP4][QuantumTravel]"#;
        assert_eq!(kind(fuel), None);
        let no_route = r#"<2026-11-14T15:03:14.965Z> [Notice] <Failed to get starmap route data!> [ItemNavigation][CL][41200] | NOT AUTH | RSI_Constellation_Mk5_Centaurus_930000000001[930000000001]|CSCItemNavigation::GetStarmapRouteSegmentData|No Route loaded! [Team_CGP4][QuantumTravel]"#;
        assert_eq!(kind(no_route), None);
    }

    #[test]
    fn a_quantum_route_names_where_it_starts() {
        // The origin is a display name, here one with a space.
        let line = r#"<2026-11-14T15:03:23.595Z> [Notice] <Calculate Route> [ItemNavigation][CL][41200] | NOT AUTH | RSI_Constellation_Mk5_Centaurus_930000000001[930000000001]|CSCItemNavigation::CalculateRoute|Projected Start Location is Nyx Gateway for route to destination Orison_LOC [Team_CGP4][QuantumTravel]"#;
        assert_eq!(
            kind(line),
            Some(LogEventKind::QuantumRouteCalculated {
                origin: "Nyx Gateway".into(),
                destination: "Orison_LOC".into(),
            })
        );
        // The lines that follow it say nothing more.
        let surface = r#"<2026-11-14T15:03:23.595Z> [Notice] <Calculate Route!> [ItemNavigation][CL][41200] | NOT AUTH | RSI_Constellation_Mk5_Centaurus_930000000001[930000000001]|CSCItemNavigation::CalculateRoute|Adding surface location Orison_LOC to end of route. New final index 1 [Team_CGP4][QuantumTravel]"#;
        let fuel = r#"<2026-11-14T15:03:23.595Z> [Notice] <Calculate Route> [ItemNavigation][CL][41200] | NOT AUTH | RSI_Constellation_Mk5_Centaurus_930000000001[930000000001]|CSCItemNavigation::CalculateRoute|Successfully calculated route to Orison_LOC fuel estimate 1198718.500000 [Team_CGP4][QuantumTravel]"#;
        assert_eq!(kind(surface), None);
        assert_eq!(kind(fuel), None);
    }

    #[test]
    fn the_quantum_drive_arrives() {
        let line = r#"<2026-11-14T15:08:19.338Z> [Notice] <Quantum Drive Arrived - Arrived at Final Destination> [ItemNavigation][CL][41200] | NOT AUTH | RSI_Constellation_Mk5_Centaurus_930000000001[930000000001]|CSCItemNavigation::OnQuantumDriveArrived|Quantum Drive has arrived at final destination [Team_CGP4][QuantumTravel]"#;
        assert_eq!(
            kind(line),
            Some(LogEventKind::QuantumArrived {
                vehicle_class: Some("RSI_Constellation_Mk5_Centaurus".into()),
            })
        );
    }

    #[test]
    fn an_item_arrives_in_a_weapon_slot() {
        let rifle = r#"<2026-11-14T14:21:27.876Z> [Notice] <AttachmentReceived> Player[VeeLume] Attachment[lbco_rifle_energy_01_mr01_930000000002, lbco_rifle_energy_01_mr01, 930000000002] Status[persistent] Port[wep_stocked_2] Elapsed[38.246002] [Team_CoreGameplayFeatures][Inventory]"#;
        assert_eq!(
            kind(rifle),
            Some(LogEventKind::HeldItemReceived {
                item_class: "lbco_rifle_energy_01_mr01".into(),
                port: "wep_stocked_2".into(),
            })
        );
        // Its magazine, logged beside it, sits on the rifle, not in a slot.
        let magazine = r#"<2026-11-14T14:21:27.876Z> [Notice] <AttachmentReceived> Player[VeeLume] Attachment[lbco_rifle_energy_01_mag_930000000003, lbco_rifle_energy_01_mag, 930000000003] Status[persistent] Port[magazine_attach] Elapsed[38.246113] [Team_CoreGameplayFeatures][Inventory]"#;
        assert_eq!(kind(magazine), None);
        // The client's placeholder while an equip request is under way.
        let placeholder = r#"<2026-11-14T14:24:50.086Z> [Notice] <AttachmentReceived> Player[VeeLume] Attachment[Inventory_LocalAttach_Item, Default, 6963] Status[local] Port[underbarrel_attach] Elapsed[0.047715] [Team_CoreGameplayFeatures][Inventory]"#;
        assert_eq!(kind(placeholder), None);
    }

    #[test]
    fn an_attachment_is_put_on_a_weapon() {
        let barrel = r#"<2026-11-14T14:25:50.369Z> [Notice] <EquipItem> Request[25] equip from Inventory[200000000001:Location:3700000001] Class[arma_barrel_supp_s2] Rank[amsabcdefghin] Port[Body_ItemPort:Armor_Undersuit:Armor_Torso:backpack:wep_stocked_2:barrel_attach] DependentRequest[4294967295] PostAction[None] [Team_CoreGameplayFeatures][Inventory]"#;
        assert_eq!(
            kind(barrel),
            Some(LogEventKind::AttachmentRequested {
                item_class: "arma_barrel_supp_s2".into(),
                holder_port: "wep_stocked_2".into(),
                port: "barrel_attach".into(),
            })
        );
        let optics = r#"<2026-11-14T14:25:58.313Z> [Notice] <EquipItem> Request[26] equip from Inventory[200000000001:Location:3700000001] Class[hdgw_optics_tsco_x4_s2] Rank[amsabcdefghjn] Port[Body_ItemPort:Armor_Undersuit:Armor_Torso:backpack:wep_stocked_2:optics_attach] DependentRequest[4294967295] PostAction[None] [Team_CoreGameplayFeatures][Inventory]"#;
        assert_eq!(
            kind(optics),
            Some(LogEventKind::AttachmentRequested {
                item_class: "hdgw_optics_tsco_x4_s2".into(),
                holder_port: "wep_stocked_2".into(),
                port: "optics_attach".into(),
            })
        );
    }

    #[test]
    fn other_equip_requests_are_not_attachments() {
        // A magazine into the armor's pouch: the same line, another port.
        let pouch = r#"<2026-11-14T14:26:15.929Z> [Notice] <EquipItem> Request[33] equip from Inventory[200000000001:Location:3700000001] Class[lbco_rifle_energy_01_mag] Rank[amsabcdefghkn] Port[Body_ItemPort:Armor_Undersuit:Armor_Torso:magazine_attach_1] DependentRequest[4294967295] PostAction[None] [Team_CoreGameplayFeatures][Inventory]"#;
        assert_eq!(kind(pouch), None);
        // The weapon itself, picked up to carry.
        let carried = r#"<2026-11-14T14:20:49.632Z> [Notice] <EquipItem> Request[15] equip from Inventory[200000000001:Location:3700000001] Class[lbco_rifle_energy_01_mr01] Rank[amsabcdefghln] Port[Body_ItemPort:inventory_pocket] DependentRequest[4294967295] PostAction[Carry] [Team_CoreGameplayFeatures][Inventory]"#;
        assert_eq!(kind(carried), None);
        // Equipping straight from a loot screen has no port path.
        let looted = r#"<2026-11-14T14:42:55.479Z> [Notice] <EquipItem> Equip looting entity[behr_gren_frag_01_930000000004] to port[grenade_attach_1] [Team_CoreGameplayFeatures][Inventory]"#;
        assert_eq!(kind(looted), None);
    }

    #[test]
    fn unrelated_and_torn_lines_parse_to_nothing() {
        let load = r#"<2026-05-30T13:20:14.660Z> [Notice] <StatObjLoad 0x800 Format> 'data/objectcontainers/pu/loc/…' - File exists in P4K [Team_Graphics][CoreTech]"#;
        assert_eq!(kind(load), None);
        assert_eq!(kind(""), None);
        assert_eq!(kind("<2026-11-13T16:13:05.561Z>"), None);
        assert_eq!(kind("<Join PU> shard[x] no timestamp"), None);
        // A notification cut off before its closing quote.
        assert_eq!(
            kind(
                r#"<2026-11-13T17:36:45.782Z> [Notice] <SHUDEvent_OnNotification> Added notification "Objective Complete: Go to"#
            ),
            None
        );
    }

    #[test]
    fn pre_4x_lines_with_leading_team_tags_still_find_their_event() {
        // The old layout put the team tags before the event (3.x); the
        // message shape is the 4.x one, so this is a synthetic reordering
        // of a real line, checking only that the tag is found.
        let line = r#"<2026-11-13T16:13:38.798Z> [Notice][Team_CoreGameplayFeatures][Inventory] <RequestLocationInventory> Player[VeeLume] requested inventory for Location[Nyx_Levski]"#;
        assert_eq!(
            kind(line),
            Some(LogEventKind::LocationInventoryRequested {
                location: "Nyx_Levski".into()
            })
        );
    }

    #[test]
    fn clean_title_only_removes_text_patch_spans() {
        assert_eq!(clean_title(" Plain Title "), "Plain Title");
        assert_eq!(
            clean_title("A <EM4>[BP]</EM4> <EM3>[✓]</EM3> [Solo]"),
            "A [Solo]"
        );
        // An unclosed tag is left alone rather than eating the title.
        assert_eq!(clean_title("A <EM4>[BP] B"), "A <EM4>[BP] B");
    }
}
