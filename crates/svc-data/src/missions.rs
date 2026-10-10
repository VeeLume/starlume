//! Mission catalog cooking — `sc_missions::Missions` → serde-clean
//! [`MissionEntry`] views, ported from Hearth's `sc_loader/cook.rs` (the
//! proven projection) onto sc-holotable v0.16 (library `UecCurve` via the
//! `missions-payout` feature, bundled reward-currency registry, hauling-leg
//! manifests).
//!
//! Unlike the item/resource surfaces in [`crate::cooked`] (raw `LocaleKey`s,
//! resolved at query time), missions resolve **at cook time**: title/
//! description substitution and every GUID→name join need the live
//! [`Missions`] registries, and that bundle isn't serde — only these views
//! cross into the snapshot. The trade-off is fine: mission text is never
//! re-localized after the cook, same as the locale the indices were cooked
//! from.

use std::collections::{BTreeSet, HashMap, HashSet};

use sc_holotable::asset::{Datacore, Guid, LocaleKey, LocaleMap, RecordCollection};
use sc_holotable::items::Items;
use sc_holotable::missions::{
    Encounter, EntitySlot, EventPoints, LadderKind, Mission, Missions, NpcSlot, PrereqView,
    RewardAmount, ShipEncounter, SlotGroup, TagBag, UecCurve,
};
use sc_holotable::resources::Resources;
use sc_holotable::tags::Tags;
use serde::{Deserialize, Serialize};

pub use crate::crimestat::CrimestatRisk;

/// One displayed mission — a **pooled template**, not a raw contract
/// expansion. CIG spawns one contract per offered locality, so the raw list
/// has thousands of near-duplicates; the cook collapses contracts sharing the
/// player-meaningful identity (title + description + reward identity + payout
/// variant + cooked encounters) into one entry, aggregating localities into
/// [`Self::locations`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionEntry {
    /// Representative contract GUID, hex-string form. Stable id for UI keys.
    pub mission_id: String,
    /// Resolved title; `None` → the UI falls back to `debug_name`.
    /// `~mission(Var)` runtime markers render as readable `[Var]`.
    pub title: Option<String>,
    /// Raw title locale key (`@`-preserved as authored) — the INI key a
    /// text-patching consumer targets. Entries sharing one key form the
    /// key-level pool (coarser than this entry's collapse — see
    /// [`Self::facts`]).
    #[serde(default)]
    pub title_key: Option<String>,
    /// Internal contract debug name — fallback label + DCB cross-ref.
    pub debug_name: String,
    /// Resolved description, with `~mission(Var)` → `[Var]`.
    pub description: Option<String>,
    /// Raw description locale key (`@`-preserved as authored).
    #[serde(default)]
    pub description_key: Option<String>,
    /// Mission category (Bounty Hunter / Hauling / Salvage / …).
    pub category: Option<MissionCategory>,
    /// Reputation faction the UI groups + filters by.
    pub faction: Option<MissionFaction>,
    /// Difficulty profile (four 1–8 axes). Hidden in-game; drives the payout.
    pub difficulty: Option<MissionDifficulty>,
    pub payout: MissionPayout,
    /// Non-repeatable (`availability.once_only`).
    pub once_only: bool,
    pub shareable: bool,
    pub illegal: bool,
    /// Post-completion personal cooldown in seconds, if any.
    pub cooldown_seconds: Option<f32>,
    /// Cooldown after voluntarily abandoning, in seconds, if any.
    #[serde(default)]
    pub abandon_cooldown_seconds: Option<f32>,
    pub scrip: Vec<ScripReward>,
    pub reputation: Vec<RepReward>,
    pub item_rewards: Vec<ItemReward>,
    /// Blueprint-pool rewards — weighted pools the contract draws from
    /// (union across pooled contracts, deduped by pool).
    pub blueprint_rewards: Vec<BpPoolReward>,
    /// Event points, one per event the mission feeds (pooled members agree:
    /// the points are part of the pooling key).
    #[serde(default)]
    pub event_rewards: Vec<EventReward>,
    /// Reputation required to accept (faction + standing-tier window).
    pub rep_required: Vec<RepRequirement>,
    /// Missions that must be completed first (the chain gate).
    pub chain_required: Vec<MissionRef>,
    /// Where the mission is offered, grouped by star system.
    pub locations: Vec<MissionRegion>,
    /// Structured ship encounters (waves → slots → candidates).
    pub encounters: Vec<MissionEncounter>,
    /// Hauling manifest legs (commodity + SCU + box size). Empty for
    /// non-hauling contracts. The future mod-cargo join point.
    pub cargo: Vec<CargoLeg>,
    /// `~mission(Var)` runtime-substitution variables present in the
    /// title/description (e.g. `["Location", "CargoGradeToken"]`).
    pub placeholders: Vec<String>,
    /// How many raw contract expansions this entry collapses.
    pub instance_count: u32,
    /// What still diverges *within* this entry's collapsed pool — the axes
    /// the pooling key doesn't pin (see [`MissionPoolFacts`]). Cross-entry
    /// divergence (contracts sharing a title/description key but split into
    /// different entries) is recomputable from the entries themselves via
    /// [`Self::title_key`] / [`Self::description_key`].
    #[serde(default)]
    pub facts: MissionPoolFacts,
}

/// Within-pool divergence flags + crimestat, computed where the collapse
/// happens (the raw contracts aren't in the snapshot to recompute from).
///
/// The pooling key pins title/description/reward-identity/payout-variant,
/// so aUEC and blueprint-pool sets never mix within an entry; these are the
/// axes that still can. When a `*_mixed` flag is set, the representative's
/// value on that axis is one of several — an honest consumer surfaces the
/// ambiguity instead of presenting it as fact.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionPoolFacts {
    pub shareable_mixed: bool,
    pub once_only_mixed: bool,
    pub illegal_mixed: bool,
    pub cooldowns_mixed: bool,
    /// Scrip **amounts** differ across members (currencies are pinned by
    /// the pooling key).
    pub scrip_mixed: bool,
    /// Reputation amounts differ across members (factions are pinned).
    pub rep_mixed: bool,
    /// Encounter shape (waves/slots) differs across members.
    pub encounters_mixed: bool,
    /// Crimestat risk of the representative (unanimous across members
    /// unless [`Self::crimestat_mixed`]).
    pub crimestat: CrimestatRisk,
    pub crimestat_mixed: bool,
}

/// Resolved mission category — name + icon hint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionCategory {
    /// Display name (`"Hauling"`). `None` if the locale didn't resolve.
    pub name: Option<String>,
    /// `MissionType.IconName` — UI icon id, empty when none.
    pub icon: String,
}

/// Resolved reputation faction — stable GUID key + display name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionFaction {
    pub guid: String,
    pub name: Option<String>,
}

/// The four authored difficulty axes, each `1..=8` (`0` = unparsed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionDifficulty {
    pub mechanical_skill: u8,
    pub mental_load: u8,
    pub risk_of_loss: u8,
    pub game_knowledge: u8,
}

/// aUEC payout — fixed amount or engine-calculated (with the engine's own
/// reward-curve estimate), plus buy-in and time budget.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MissionPayout {
    /// True when the reward is engine-calculated.
    pub calculated: bool,
    /// Hardcoded fixed aUEC, when the contract carries one.
    pub fixed: Option<i32>,
    /// Estimated aUEC from the DCB's own `uecCurve` (exact against live
    /// payouts; rep bonus and party split are runtime and not modeled).
    pub estimate: Option<i32>,
    /// Upfront cost to accept, 0 when free.
    pub buy_in: i32,
    /// Time budget in minutes, 0 when none.
    pub time_to_complete: f32,
}

/// A typed-currency (scrip) reward.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScripReward {
    pub name: Option<String>,
    pub amount: i32,
}

/// A reputation reward. `amount` is `None` for engine-calculated rep.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepReward {
    pub faction_guid: Option<String>,
    pub amount: Option<i32>,
}

/// Points toward a limited-time event (RSI Discovery Month, Clean Air, …).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventReward {
    /// The event, from its progress record's name (`Iasi`, `CleanAir`) —
    /// the data carries no player-facing event title.
    pub event: String,
    /// The overall ladder first, then the category ladders the mission
    /// feeds.
    pub ladders: Vec<EventLadder>,
}

/// One event ladder the mission feeds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventLadder {
    /// The category ladder's label (`Collection`); `None` for the overall
    /// ladder.
    pub name: Option<String>,
    /// Points per completion.
    pub points: i32,
    /// The points are divided among the party members sharing the
    /// contract. Only the overall ladder can split; category points always
    /// go to every member in full.
    pub split_for_party: bool,
    /// Tier thresholds, ascending.
    pub tiers: Vec<i32>,
}

/// A non-currency item reward (ship unlock, collector item, …).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemReward {
    pub entity_guid: String,
    pub name: Option<String>,
    pub amount: i32,
}

/// One blueprint-pool reward: a weighted set the contract draws from, with
/// the chance the draw happens at all.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BpPoolReward {
    /// `BlueprintPoolRecord` name (prefix stripped). Empty if unnamed.
    pub pool_name: String,
    /// 0.0–1.0 chance the blueprint draw happens.
    pub chance: f32,
    pub blueprints: Vec<BpPoolEntry>,
}

/// One weighted blueprint inside a [`BpPoolReward`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BpPoolEntry {
    /// The blueprint record GUID — the cross-reference key the future
    /// tracker module's ownership set will decorate against.
    pub blueprint_record_guid: String,
    /// Resolved crafted-item display name.
    pub name: Option<String>,
    /// Relative pick-weight within the pool.
    pub weight: f32,
}

/// One rep-acceptance requirement — faction + standing-tier window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepRequirement {
    pub faction: Option<String>,
    /// Lower standing-tier bound (`"Neutral"`), resolved name.
    pub min_rank: Option<String>,
    pub max_rank: Option<String>,
    /// Numeric tier index of `min_rank` for ordering/range filters.
    pub min_rank_index: Option<i32>,
    pub max_rank_index: Option<i32>,
    /// True for an *exclusion* requirement (must NOT be in this range).
    pub exclude: bool,
}

/// A lean reference to another mission (chain prerequisites).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionRef {
    /// Matches [`MissionEntry::mission_id`].
    pub mission_id: String,
    pub title: Option<String>,
    pub once_only: bool,
}

/// One locality's worth of accept-locations — the "available in" card
/// (*Stanton — Hurston*, *Pyro — Region A*).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionRegion {
    /// System display name (`"Stanton"`, `"Pyro"`).
    pub system: String,
    /// Locality name (`"Hurston"`, `"Region A"`). Empty when unnamed.
    pub name: String,
    pub places: Vec<MissionPlace>,
}

/// One accept-location — a resolved place + its typed kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionPlace {
    /// Display name; `None` → UI falls back to `record_name`.
    pub name: Option<String>,
    /// Stable record-name stem (`"Pyro3"`), always present.
    pub record_name: String,
    /// Typed `LocationKind` (`"Planet"`, `"Station"`, …), when resolved.
    pub kind: Option<String>,
}

/// One encounter the mission spawns — ships, NPCs or other entities.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MissionEncounter {
    /// The encounter's mission-variable name (`"AmbushTarget"` / …).
    pub label: String,
    /// Combat-class tag (`"VeryEasy"` … `"Hard"`) when uniform.
    pub difficulty: Option<String>,
    pub waves: Vec<MissionWave>,
    /// What the encounter spawns: ship encounters fill
    /// [`MissionWave::ships`], NPC and entity encounters
    /// [`MissionWave::spawns`].
    #[serde(default)]
    pub kind: EncounterKind,
}

/// What an encounter spawns.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EncounterKind {
    #[default]
    Ships,
    /// FPS NPCs.
    Npcs,
    /// Other entities: creatures, mission items (relays, pipes, datapads).
    Entities,
}

/// One wave/phase of an encounter.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MissionWave {
    pub name: String,
    pub ships: Vec<ShipSlot>,
    /// Resolved cargo descriptors across the wave's slots (deduped).
    pub cargo: Vec<String>,
    /// NPC / entity slots (empty for ship encounters).
    #[serde(default)]
    pub spawns: Vec<SpawnSlot>,
}

/// One NPC or entity slot — how many of what, for whom.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpawnSlot {
    /// Total spawned over the slot's life (NPC `max_spawns`, entity
    /// `amount`); a range when an entity slot has alternatives.
    pub count_min: i32,
    pub count_max: i32,
    /// Most alive at once (NPC `max_concurrent`; entities: `count_max`).
    pub concurrent: i32,
    /// What spawns, as raw tag names: the NPC archetype (`PU_Soldier`),
    /// else its class (`Heavy`). Empty for entities — their wave name
    /// says what they are (`Relay`, `Kopion`).
    pub kinds: Vec<String>,
    pub factions: Vec<String>,
    /// Carries the friendly HUD marker (NPC `mission_allied_marker`).
    pub allied: bool,
    /// Tagged as a mission target (`AI ▸ Spawning ▸ Target`).
    pub target: bool,
}

/// One ship slot — how many of which candidate ships, and their factions.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ShipSlot {
    pub count_min: i32,
    pub count_max: i32,
    /// Candidate ship display names (the engine picks one per spawn).
    pub ships: Vec<String>,
    pub factions: Vec<String>,
}

/// One hauling-manifest leg — commodity + SCU range + max box size.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CargoLeg {
    /// Resolved commodity display name.
    pub commodity: Option<String>,
    /// The commodity's `ResourceType` GUID — joins the resource catalog.
    pub commodity_guid: Option<String>,
    pub min_scu: f32,
    pub max_scu: f32,
    /// Largest accepted container size; `-1` = no box constraint.
    pub max_box: f32,
}

/// Cook the mission catalog from one parsed `Datacore`. `items` / `resources`
/// come from the foundations build the caller already paid for.
pub(crate) fn build_missions(
    datacore: &Datacore,
    items: &Items,
    resources: &Resources,
    locale: &LocaleMap,
) -> Vec<MissionEntry> {
    let missions = Missions::build(datacore);
    // The engine's own reward curve, read from the DCB
    // (`GameMode.SC_Default.uecCurve`) — drives the payout estimates.
    // Validated exact against live payouts; the runtime rep bonus and party
    // split are out of a static view's reach. `None` (curve record missing)
    // just means estimates come back `None`.
    let curve = UecCurve::build(datacore);
    let pools = &missions.blueprints;

    // Collapse raw expansions into displayed missions. The key is the
    // player-meaningful identity: title + description + reward identity
    // (which BPs / faction / scrip / item kinds) + payout variant (difficulty
    // + buy-in + time — the visible aUEC the player differentiates by) +
    // the cooked encounters (what the player fights; a pool keeping one
    // member's encounters would hide the others — e.g. the Nyx patrol
    // variant with a Vanduul pool). Location is NOT in the key: it
    // aggregates into the entry as a facet the UI groups by.
    type PoolKey = (
        Option<LocaleKey>,
        Option<LocaleKey>,
        String,
        String,
        Vec<MissionEncounter>,
    );
    let mut groups: HashMap<PoolKey, Vec<&Mission>> = HashMap::new();
    for (_, m) in missions.iter() {
        let key = (
            m.title_key.clone(),
            m.description_key.clone(),
            reward_signature(m),
            payout_signature(m),
            build_encounters(m, &missions, items, locale),
        );
        groups.entry(key).or_default().push(m);
    }

    let mut out = Vec::with_capacity(groups.len());
    for ((.., encounters), members) in groups {
        // Members share title/description/rewards/payout/encounters; the
        // first is the representative. Localities are what vary →
        // aggregated.
        let members = members.as_slice();
        let rep = members[0];
        let r = &rep.rewards;
        let facts = build_facts(members, &missions);

        let payout = MissionPayout {
            calculated: matches!(r.uec, RewardAmount::Calculated),
            fixed: match r.uec {
                RewardAmount::Fixed(n) => Some(n),
                _ => None,
            },
            estimate: curve.as_ref().and_then(|c| rep.estimate_uec(c)),
            buy_in: rep.buy_in,
            time_to_complete: rep.time_to_complete,
        };
        let scrip = r
            .scrip
            .iter()
            .map(|s| ScripReward {
                name: missions
                    .currency
                    .display_name(&s.currency_guid, items, locale)
                    .map(str::to_owned),
                amount: s.amount,
            })
            .collect();
        let reputation = r
            .reputation
            .iter()
            .map(|rr| RepReward {
                faction_guid: rr.faction.as_ref().map(Guid::to_string),
                amount: rr.amount,
            })
            .collect();
        let item_rewards = r
            .items
            .iter()
            .map(|it| ItemReward {
                entity_guid: it.entity_class.to_string(),
                name: items
                    .name_key(&it.entity_class)
                    .and_then(|k| locale.resolve(k))
                    .map(str::to_owned),
                amount: it.amount,
            })
            .collect();
        let event_rewards = r
            .event_points
            .iter()
            .filter_map(|ep| event_reward(ep, &missions, locale))
            .collect();

        // Blueprint rewards: union of distinct pools across members.
        let mut seen_pools = HashSet::new();
        let mut blueprint_rewards = Vec::new();
        for m in members {
            for br in &m.rewards.blueprints {
                if !seen_pools.insert(br.pool_guid) {
                    continue;
                }
                let Some(pool) = pools.get(&br.pool_guid) else {
                    continue;
                };
                let blueprints = pool
                    .items
                    .iter()
                    .map(|e| BpPoolEntry {
                        blueprint_record_guid: e.blueprint.blueprint_record_guid.to_string(),
                        name: e.blueprint.display_name(locale).map(str::to_owned),
                        weight: e.weight,
                    })
                    .collect();
                blueprint_rewards.push(BpPoolReward {
                    pool_name: pool.name.clone(),
                    chance: br.chance,
                    blueprints,
                });
            }
        }

        let cargo = rep
            .cargo
            .iter()
            .map(|leg| CargoLeg {
                commodity: leg.commodity_name(resources, locale).map(str::to_owned),
                commodity_guid: leg.resource.as_ref().map(Guid::to_string),
                min_scu: leg.min_scu,
                max_scu: leg.max_scu,
                max_box: leg.max_box,
            })
            .collect();

        out.push(MissionEntry {
            mission_id: rep.id.to_string(),
            title: missions.title_text(rep, locale),
            title_key: rep.title_key.as_ref().map(|k| k.as_str().to_string()),
            debug_name: rep.debug_name.clone(),
            description: missions.description_text(rep, locale),
            description_key: rep.description_key.as_ref().map(|k| k.as_str().to_string()),
            category: build_category(rep, &missions, locale),
            faction: build_faction(rep, &missions, locale),
            difficulty: rep.difficulty.map(|d| MissionDifficulty {
                mechanical_skill: d.mechanical_skill,
                mental_load: d.mental_load,
                risk_of_loss: d.risk_of_loss,
                game_knowledge: d.game_knowledge,
            }),
            payout,
            once_only: rep.availability.once_only,
            shareable: rep.shareable,
            illegal: rep.illegal_flag,
            // Upstream unit misnomer: `DurationRange.mean_seconds` carries
            // the DCB's `personal_cooldown_time` raw, which is authored in
            // MINUTES (verified against live cooldowns in sc-langpatch;
            // e.g. certification missions read 15 → 15min in-game).
            // Convert here so this field's name stays truthful.
            cooldown_seconds: rep
                .availability
                .cooldowns
                .completion
                .as_ref()
                .map(|d| d.mean_seconds * 60.0),
            // Same minutes-in-a-seconds-field misnomer as above.
            abandon_cooldown_seconds: rep
                .availability
                .cooldowns
                .abandon
                .as_ref()
                .map(|d| d.mean_seconds * 60.0),
            scrip,
            reputation,
            item_rewards,
            blueprint_rewards,
            event_rewards,
            rep_required: build_rep_required(rep, &missions, locale),
            chain_required: build_chain(rep, &missions, locale),
            locations: build_locations(members, &missions, locale),
            encounters,
            cargo,
            placeholders: missions.unresolved_markers(rep, locale),
            instance_count: members.len() as u32,
            facts,
        });
    }

    // Stable, readable order: by title (debug_name fallback), then id.
    out.sort_by(|a, b| {
        a.title
            .as_deref()
            .unwrap_or(&a.debug_name)
            .cmp(b.title.as_deref().unwrap_or(&b.debug_name))
            .then_with(|| a.mission_id.cmp(&b.mission_id))
    });
    out
}

/// Within-pool divergence + crimestat for one collapsed entry. The
/// `*_mixed` axes come from the upstream divergence helpers on
/// [`Missions`]; crimestat is the `DontHarm*` classification
/// ([`crate::crimestat`]), per member so mixing is visible.
fn build_facts(members: &[&Mission], missions: &Missions) -> MissionPoolFacts {
    let ids: Vec<Guid> = members.iter().map(|m| m.id).collect();

    let mut crimestat: Option<CrimestatRisk> = None;
    let mut crimestat_mixed = false;
    for m in members {
        let risk = crate::crimestat::classify(m);
        match crimestat {
            None => crimestat = Some(risk),
            Some(prev) if prev != risk => {
                crimestat_mixed = true;
                // Keep the representative's (first member's) risk.
            }
            _ => {}
        }
    }

    MissionPoolFacts {
        shareable_mixed: missions.shareable_mixed(&ids),
        once_only_mixed: missions.once_only_mixed(&ids),
        illegal_mixed: missions.illegal_mixed(&ids),
        cooldowns_mixed: !missions.cooldowns_consistent(&ids),
        scrip_mixed: !missions.rewards_scrip_consistent(&ids),
        rep_mixed: !missions.rewards_rep_consistent(&ids),
        encounters_mixed: !missions.encounters_shape_consistent(&ids),
        crimestat: crimestat.unwrap_or_default(),
        crimestat_mixed,
    }
}

/// One mission's points toward an event, joined with the event's ladders
/// (labels + tiers). `None` when the event record didn't resolve.
fn event_reward(ep: &EventPoints, missions: &Missions, locale: &LocaleMap) -> Option<EventReward> {
    let event = missions.events.get(&ep.event?)?;
    let tiers = |kind: LadderKind| -> Vec<i32> {
        event
            .ladders
            .iter()
            .find(|l| l.kind == kind)
            .map(|l| l.tiers.iter().map(|t| t.min_points).collect())
            .unwrap_or_default()
    };
    let mut ladders = vec![EventLadder {
        name: None,
        points: ep.points,
        split_for_party: ep.split_for_party,
        tiers: tiers(LadderKind::Overall),
    }];
    for lp in &ep.ladders {
        let kind = LadderKind::Category(lp.tag);
        // Labels are journal text ("Transport:" on Clean Air) — trim the
        // trailing colon.
        let name = event
            .ladders
            .iter()
            .find(|l| l.kind == kind)
            .and_then(|l| locale.resolve(&l.text_key))
            .map(|s| s.trim().trim_end_matches(':').to_owned());
        ladders.push(EventLadder {
            name,
            points: lp.points,
            split_for_party: false,
            tiers: tiers(kind),
        });
    }
    Some(EventReward {
        // Display-only name from the record (`Iasi_ScenarioProgress` →
        // `Iasi`); the data links no player-facing event title.
        event: event
            .record_name
            .strip_suffix("_ScenarioProgress")
            .unwrap_or(&event.record_name)
            .to_owned(),
        ladders,
    })
}

/// Distinguishing reward **identity** for pooling — the *kinds* of payoff,
/// not amounts: BP pools, reputation faction, scrip currencies, item unlocks.
/// Two contracts sharing a title+description but different reward identity
/// are different missions (e.g. Region A/B vs C/D blueprint pools).
fn reward_signature(m: &Mission) -> String {
    let r = &m.rewards;
    let mut parts: Vec<String> = Vec::new();
    for br in &r.blueprints {
        parts.push(format!("b{}", br.pool_guid));
    }
    for rr in &r.reputation {
        if let Some(f) = &rr.faction {
            parts.push(format!("r{f}"));
        }
    }
    for s in &r.scrip {
        parts.push(format!("s{}", s.currency_guid));
    }
    for it in &r.items {
        parts.push(format!("i{}", it.entity_class));
    }
    // Event points are an amount, not just an identity: a pool must not
    // merge members that award different points.
    for ep in &r.event_points {
        let ladders: Vec<String> = ep
            .ladders
            .iter()
            .map(|l| format!("{}={}", l.tag, l.points))
            .collect();
        parts.push(format!(
            "e{:?}:{}:{}:{}",
            ep.event,
            ep.points,
            ep.split_for_party,
            ladders.join("+")
        ));
    }
    parts.sort();
    parts.dedup();
    parts.join(",")
}

/// Payout-variant signature — the visible aUEC differentiator. Driven by the
/// difficulty profile plus buy-in + time: same payout inputs ⇒ same displayed
/// reward, so those collapse; different inputs split (the "harder ⇒ bigger
/// payout" rows).
fn payout_signature(m: &Mission) -> String {
    let d = m
        .difficulty
        .map(|d| {
            format!(
                "{}-{}-{}-{}",
                d.mechanical_skill, d.mental_load, d.risk_of_loss, d.game_knowledge
            )
        })
        .unwrap_or_default();
    match m.rewards.uec {
        RewardAmount::Fixed(n) => format!("f{n}"),
        _ => format!("{d}|b{}|t{}", m.buy_in, m.time_to_complete),
    }
}

/// Resolve the mission category (`MissionType`) name + icon.
fn build_category(m: &Mission, missions: &Missions, locale: &LocaleMap) -> Option<MissionCategory> {
    let info = missions.mission_types.get(&m.category?)?;
    Some(MissionCategory {
        name: locale.resolve(&info.name_key).map(str::to_owned),
        icon: info.icon_name.clone(),
    })
}

/// Resolve the mission's reputation faction → display name + stable guid key.
fn build_faction(m: &Mission, missions: &Missions, locale: &LocaleMap) -> Option<MissionFaction> {
    let guid = m.faction?;
    let name = missions
        .factions
        .get(&guid)
        .and_then(|f| locale.resolve(&f.display_name_key))
        .map(str::to_owned);
    Some(MissionFaction {
        guid: guid.to_string(),
        name,
    })
}

/// Resolve the rep-acceptance requirements (faction + standing-tier window),
/// including career-contract rep gates surfaced as synthetic rep prereqs.
fn build_rep_required(m: &Mission, missions: &Missions, locale: &LocaleMap) -> Vec<RepRequirement> {
    let standing = |g: &Option<Guid>| {
        g.as_ref()
            .and_then(|g| missions.rep_standings.get(g))
            .and_then(|s| locale.resolve(&s.display_name_key))
            .map(str::to_owned)
    };
    // Tier index parsed from the standing record name —
    // `ReputationStanding_FactionRep_Rank2` → 2.
    let rank_index = |g: &Option<Guid>| {
        g.as_ref()
            .and_then(|g| missions.rep_standings.get(g))
            .and_then(|s| s.record_name.rsplit("Rank").next()?.parse::<i32>().ok())
    };
    m.prerequisites
        .iter()
        .filter_map(|p| match p {
            PrereqView::Reputation {
                faction,
                min_standing,
                max_standing,
                exclude,
                ..
            } => Some(RepRequirement {
                faction: faction
                    .as_ref()
                    .and_then(|g| missions.factions.get(g))
                    .and_then(|f| locale.resolve(&f.display_name_key))
                    .map(str::to_owned),
                min_rank: standing(min_standing),
                max_rank: standing(max_standing),
                min_rank_index: rank_index(min_standing),
                max_rank_index: rank_index(max_standing),
                exclude: *exclude,
            }),
            _ => None,
        })
        .collect()
}

/// Resolve the chain gate — prerequisite missions, deduped by title.
fn build_chain(m: &Mission, missions: &Missions, locale: &LocaleMap) -> Vec<MissionRef> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for id in missions.prerequisite_missions(m) {
        let Some(grantor) = missions.get(&id) else {
            continue;
        };
        let title = missions.title_text(grantor, locale);
        let dedupe_key = title.clone().unwrap_or_else(|| grantor.id.to_string());
        if seen.insert(dedupe_key) {
            out.push(MissionRef {
                mission_id: grantor.id.to_string(),
                title,
                once_only: grantor.availability.once_only,
            });
        }
    }
    out
}

/// Build the per-locality "available in" cards across all pooled members.
/// Each `MissionLocality` becomes one [`MissionRegion`] (*Stanton — Hurston*,
/// *Pyro — Region A*), deduped across members.
fn build_locations(
    members: &[&Mission],
    missions: &Missions,
    locale: &LocaleMap,
) -> Vec<MissionRegion> {
    let mut seen_loc: HashSet<Guid> = HashSet::new();
    let mut out: Vec<MissionRegion> = Vec::new();
    for m in members {
        for guid in &m.mission_span {
            if !seen_loc.insert(*guid) {
                continue;
            }
            let Some(view) = missions.localities.get(guid) else {
                continue;
            };
            let system = view
                .systems
                .iter()
                .next()
                .map(|s| s.display().to_string())
                .unwrap_or_default();
            // Dedupe places by record name within the locality.
            let mut seen_place: HashSet<String> = HashSet::new();
            let mut places = Vec::new();
            for loc in &view.locations {
                if !seen_place.insert(loc.record_name.clone()) {
                    continue;
                }
                places.push(MissionPlace {
                    name: loc.display_name(locale).map(str::to_owned),
                    record_name: loc.record_name.clone(),
                    kind: loc.kind.as_ref().map(|k| k.as_dcb_str().to_string()),
                });
            }
            // Prefer the planet's name when the locality wraps a single
            // planet — Stanton localities are record-named `Stanton1`..
            // `Stanton4` but a player knows them as Hurston / Crusader /
            // ArcCorp / microTech. Keep the cleaned region name for
            // multi-planet localities (Pyro `RegionA` spans several planets).
            let planets: Vec<&str> = places
                .iter()
                .filter(|p| p.kind.as_deref() == Some("Planet"))
                .filter_map(|p| p.name.as_deref())
                .collect();
            let name = if planets.len() == 1 {
                planets[0].to_string()
            } else {
                clean_locality_name(&view.name)
            };
            out.push(MissionRegion {
                system,
                name,
                places,
            });
        }
    }
    out.sort_by(|a, b| a.system.cmp(&b.system).then_with(|| a.name.cmp(&b.name)));
    out
}

/// Insert spaces at lower→upper / lower→digit boundaries so a locality record
/// stem reads as a label (`"RegionA"` → `"Region A"`).
fn clean_locality_name(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    let mut prev_lower = false;
    for ch in s.chars() {
        if prev_lower && (ch.is_uppercase() || ch.is_ascii_digit()) {
            out.push(' ');
        }
        out.push(ch);
        prev_lower = ch.is_lowercase();
    }
    out
}

/// Structured encounters — waves → slots. Ships carry candidates + counts +
/// factions + resolved cargo; NPCs carry spawn counts, archetype and
/// faction; entities carry their amount (the wave name says what they are).
fn build_encounters(
    m: &Mission,
    missions: &Missions,
    items: &Items,
    locale: &LocaleMap,
) -> Vec<MissionEncounter> {
    let tree = &missions.tag_tree;
    let difficulty = m.combat_class().map(str::to_owned);
    let mut out = Vec::new();
    for enc in &m.encounters {
        let encounter = match enc {
            Encounter::Ships(s) => ship_encounter(s, missions, items, locale),
            Encounter::Npcs(n) => MissionEncounter {
                label: n.variable_name.clone(),
                difficulty: None,
                kind: EncounterKind::Npcs,
                waves: n
                    .phases
                    .iter()
                    .map(|phase| {
                        spawn_wave(&phase.name, phase.all_options().map(|o| npc_slot(o, tree)))
                    })
                    .collect(),
            },
            Encounter::Entities(e) => MissionEncounter {
                label: e.variable_name.clone(),
                difficulty: None,
                kind: EncounterKind::Entities,
                waves: e
                    .phases
                    .iter()
                    .map(|phase| {
                        spawn_wave(
                            &phase.name,
                            phase.groups.iter().map(|g| entity_slot(g, tree)),
                        )
                    })
                    .collect(),
            },
            Encounter::Unknown { .. } => continue,
        };
        if !encounter.waves.is_empty() {
            out.push(MissionEncounter {
                difficulty: difficulty.clone(),
                ..encounter
            });
        }
    }
    out
}

/// A non-ship wave; `None`-like (no slots) waves are kept so phase order
/// survives, but callers drop encounters with no waves.
fn spawn_wave(name: &str, slots: impl Iterator<Item = SpawnSlot>) -> MissionWave {
    MissionWave {
        name: name.to_string(),
        ships: Vec::new(),
        cargo: Vec::new(),
        spawns: slots.collect(),
    }
}

/// One NPC slot. Archetype from the character tags, else the NPC class
/// (some factions' slots tag only a class); counts from `AutoSpawnSettings`
/// (`1` when a slot carries none — not observed on LIVE).
fn npc_slot(slot: &NpcSlot, tree: &Tags) -> SpawnSlot {
    let tags = &slot.character_tags;
    let mut kinds: Vec<String> = tags.archetypes(tree).map(str::to_owned).collect();
    if kinds.is_empty() {
        kinds = tags.npc_classes(tree).map(str::to_owned).collect();
    }
    let counts = slot.spawn_counts;
    let total = counts.map_or(1, |c| c.max_spawns);
    SpawnSlot {
        count_min: total,
        count_max: total,
        concurrent: counts.map_or(1, |c| c.max_concurrent),
        kinds,
        factions: tags.factions(tree).map(str::to_owned).collect(),
        allied: slot.mission_allied_marker,
        target: is_target(&slot.identifier_tags, tree),
    }
}

/// One entity group — the amount range across its alternatives.
fn entity_slot(group: &SlotGroup<EntitySlot>, tree: &Tags) -> SpawnSlot {
    let (lo, hi) = group.concurrent_range;
    let mut factions: BTreeSet<String> = BTreeSet::new();
    let mut target = false;
    for opt in &group.options {
        factions.extend(opt.positive.factions(tree).map(str::to_owned));
        target |= is_target(&opt.positive, tree);
    }
    SpawnSlot {
        count_min: lo,
        count_max: hi,
        concurrent: hi,
        kinds: Vec::new(),
        factions: factions.into_iter().collect(),
        allied: false,
        target,
    }
}

fn is_target(bag: &TagBag, tree: &Tags) -> bool {
    bag.spawn_identifiers(tree).any(|t| t == "Target")
}

/// Ship encounter → waves of slots (candidates + counts + factions) +
/// resolved cargo per wave.
fn ship_encounter(
    s: &ShipEncounter,
    missions: &Missions,
    items: &Items,
    locale: &LocaleMap,
) -> MissionEncounter {
    let tree = &missions.tag_tree;
    let mut waves = Vec::new();
    for phase in &s.phases {
        let mut ships = Vec::new();
        let mut cargo: BTreeSet<String> = BTreeSet::new();
        for group in &phase.groups {
            let mut ship_names: BTreeSet<String> = BTreeSet::new();
            let mut factions: BTreeSet<String> = BTreeSet::new();
            for opt in &group.options {
                for c in &opt.candidates {
                    if let Some(name) = missions.ships.display_name(&c.entity_guid, items, locale) {
                        ship_names.insert(name.to_string());
                    }
                }
                for f in opt.positive.factions(tree) {
                    factions.insert(f.to_string());
                }
                for cg in opt.positive.cargo(tree) {
                    cargo.insert(cg.to_string());
                }
            }
            ships.push(ShipSlot {
                count_min: group.concurrent_range.0,
                count_max: group.concurrent_range.1,
                ships: ship_names.into_iter().collect(),
                factions: factions.into_iter().collect(),
            });
        }
        waves.push(MissionWave {
            name: phase.name.clone(),
            ships,
            cargo: cargo.into_iter().collect(),
            spawns: Vec::new(),
        });
    }
    MissionEncounter {
        label: s.variable_name.clone(),
        difficulty: None,
        waves,
        kind: EncounterKind::Ships,
    }
}
