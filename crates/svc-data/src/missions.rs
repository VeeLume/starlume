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
use sc_holotable::missions::{Encounter, Mission, Missions, PrereqView, RewardAmount, UecCurve};
use sc_holotable::resources::Resources;
use serde::{Deserialize, Serialize};

/// One displayed mission — a **pooled template**, not a raw contract
/// expansion. CIG spawns one contract per offered locality, so the raw list
/// has thousands of near-duplicates; the cook collapses contracts sharing the
/// player-meaningful identity (title + description + reward identity + payout
/// variant) into one entry, aggregating localities into [`Self::locations`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionEntry {
    /// Representative contract GUID, hex-string form. Stable id for UI keys.
    pub mission_id: String,
    /// Resolved title; `None` → the UI falls back to `debug_name`.
    /// `~mission(Var)` runtime markers render as readable `[Var]`.
    pub title: Option<String>,
    /// Internal contract debug name — fallback label + DCB cross-ref.
    pub debug_name: String,
    /// Resolved description, with `~mission(Var)` → `[Var]`.
    pub description: Option<String>,
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
    pub scrip: Vec<ScripReward>,
    pub reputation: Vec<RepReward>,
    pub item_rewards: Vec<ItemReward>,
    /// Blueprint-pool rewards — weighted pools the contract draws from
    /// (union across pooled contracts, deduped by pool).
    pub blueprint_rewards: Vec<BpPoolReward>,
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

/// One ship encounter the mission spawns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionEncounter {
    /// The encounter's mission-variable name (`"AmbushTarget"` / …).
    pub label: String,
    /// Combat-class tag (`"VeryEasy"` … `"Hard"`) when uniform.
    pub difficulty: Option<String>,
    pub waves: Vec<MissionWave>,
}

/// One wave/phase of an encounter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissionWave {
    pub name: String,
    pub ships: Vec<ShipSlot>,
    /// Resolved cargo descriptors across the wave's slots (deduped).
    pub cargo: Vec<String>,
}

/// One ship slot — how many of which candidate ships, and their factions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    // + buy-in + time — the visible aUEC the player differentiates by).
    // Location and encounter are NOT in the key: they aggregate into the
    // entry as facets the UI groups by.
    type PoolKey = (Option<LocaleKey>, Option<LocaleKey>, String, String);
    let mut groups: HashMap<PoolKey, Vec<&Mission>> = HashMap::new();
    for (_, m) in missions.iter() {
        let key = (
            m.title_key.clone(),
            m.description_key.clone(),
            reward_signature(m),
            payout_signature(m),
        );
        groups.entry(key).or_default().push(m);
    }

    let mut out = Vec::with_capacity(groups.len());
    for members in groups.values() {
        // Members share title/description/rewards/payout; the first is the
        // representative. Localities are what vary → aggregated.
        let rep = members[0];
        let r = &rep.rewards;

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
            debug_name: rep.debug_name.clone(),
            description: missions.description_text(rep, locale),
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
            cooldown_seconds: rep
                .availability
                .cooldowns
                .completion
                .as_ref()
                .map(|d| d.mean_seconds),
            scrip,
            reputation,
            item_rewards,
            blueprint_rewards,
            rep_required: build_rep_required(rep, &missions, locale),
            chain_required: build_chain(rep, &missions, locale),
            locations: build_locations(members, &missions, locale),
            encounters: build_encounters(rep, &missions, items, locale),
            cargo,
            placeholders: missions.unresolved_markers(rep, locale),
            instance_count: members.len() as u32,
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

/// Structured ship encounters — waves → slots (ship candidates + counts +
/// factions) + resolved cargo. NPC / entity encounters are skipped for now.
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
        let Encounter::Ships(s) = enc else { continue };
        let mut waves = Vec::new();
        for phase in &s.phases {
            let mut ships = Vec::new();
            let mut cargo: BTreeSet<String> = BTreeSet::new();
            for group in &phase.groups {
                let mut ship_names: BTreeSet<String> = BTreeSet::new();
                let mut factions: BTreeSet<String> = BTreeSet::new();
                for opt in &group.options {
                    for c in &opt.candidates {
                        if let Some(name) =
                            missions.ships.display_name(&c.entity_guid, items, locale)
                        {
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
            });
        }
        if !waves.is_empty() {
            out.push(MissionEncounter {
                label: s.variable_name.clone(),
                difficulty: difficulty.clone(),
                waves,
            });
        }
    }
    out
}
