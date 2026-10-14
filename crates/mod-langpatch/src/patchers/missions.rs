//! Mission enhancer — title tags + description blocks for the contracts
//! panel, pool-first over the cooked mission catalog.
//!
//! Port of sc-langpatch's `mission_enhancer` (its biggest renderer). The
//! old module materialized everything from the raw DataCore (~3.8k lines
//! across 9 files); here the svc-data cook already resolved missions,
//! blueprints, encounters, regions and ranks, so only the *rendering
//! decisions* port:
//!
//! - **Two independent pool passes** — entries regrouped by stripped
//!   `title_key` / `description_key` (the cook pools finer: title + desc +
//!   reward-identity + payout-variant). Within-entry divergence comes from
//!   the cooked [`svc_data::MissionPoolFacts`] flags; cross-entry
//!   divergence is recomputed here over the group.
//! - **Titles** get trailing tags (`[BP] [Solo] [Uniq] [Illegal] [CS
//!   Risk]`), only for unanimous facts; one `[~]` marks "behavior varies —
//!   see description".
//! - **Descriptions** get blueprint / mission-info / encounter / region
//!   blocks, with a `Variants (N)` section when pool members render
//!   differently (dedup by rendered diff — data-level mixing that doesn't
//!   survive rendering collapses back to the singleton path).
//!
//! Deliberate deltas from the standalone module:
//!
//! - **Owned-blueprints features are gone** (`owned_blueprints`,
//!   `owned_title_tag`): they read Hearth's `hearth-export` JSON, a
//!   contract that dies with standalone langpatch. Ownership returns via
//!   the tracker module's catalog-decoration point, not a file handshake.
//! - **Encounter rendering is simpler and honest to the cook**: the cook
//!   flattens slot alternatives (no "One of:" trees, no per-option skill
//!   tiers). Ship pools, spawn counts, factions, cargo descriptors and the
//!   CombatClass tier all survive.
//! - **NPC encounters render from the data, not phase names**: per
//!   archetype and faction with real spawn counts (`4x Soldier (Pyro
//!   Outlaw)`, targets and friendlies marked), where the old module showed
//!   one `NPCs: N` total parsed out of designer phase names.

use std::collections::{BTreeMap, BTreeSet};

use svc_data::{
    BpPoolReward, CookedData, CrimestatRisk, EncounterKind, MissionEncounter, MissionEntry,
};

use crate::format::{Color, NEWLINE, PARAGRAPH_BREAK, apply_color, bracket, bullet, header};
use crate::ops::{ChoiceOption, OpSet, OptionKind, PatchOp, PatcherConfig, PatcherOption};

pub struct MissionEnhancer;

impl crate::Patcher for MissionEnhancer {
    fn id(&self) -> &'static str {
        "mission_enhancer"
    }

    fn name(&self) -> &'static str {
        "Mission Enhancer"
    }

    fn description(&self) -> &'static str {
        "Enrich mission titles and descriptions with blueprint rewards, cooldowns, and more"
    }

    fn options(&self) -> Vec<PatcherOption> {
        let toggle = |id: &str, label: &str, description: &str| PatcherOption {
            id: id.into(),
            label: label.into(),
            description: description.into(),
            kind: OptionKind::Bool,
            default: "true".into(),
        };
        vec![
            toggle(
                "blueprint_tag",
                "Blueprint Tag",
                "Add [BP] to titles of missions that reward blueprints",
            ),
            toggle(
                "blueprint_list",
                "Blueprint List",
                "Append blueprint item list to mission descriptions",
            ),
            toggle(
                "solo_tag",
                "Solo Tag",
                "Add [Solo] to titles of solo-only missions",
            ),
            toggle(
                "once_tag",
                "One-Time Tag",
                "Add [Uniq] to titles of one-time-only missions",
            ),
            toggle(
                "illegal_tag",
                "Illegal Tag",
                "Add [Illegal] to titles of illegal missions",
            ),
            toggle(
                "mission_info",
                "Mission Info",
                "Append cooldown, rep reward, scrip, and payout to descriptions",
            ),
            PatcherOption {
                id: "crimestat_tag".into(),
                label: "Crimestat Risk Tag".into(),
                description: "Mark missions where killing friendly NPCs gives crimestat".into(),
                kind: OptionKind::Choice {
                    choices: vec![
                        ChoiceOption {
                            value: "off".into(),
                            label: "Off".into(),
                        },
                        ChoiceOption {
                            value: "simple".into(),
                            label: "Simple [CS Risk]".into(),
                        },
                        ChoiceOption {
                            value: "colored".into(),
                            label: "Emphasised (underline / highlight)".into(),
                        },
                    ],
                },
                default: "colored".into(),
            },
            toggle(
                "ship_encounters",
                "Ship Encounters",
                "Show hostile and allied ship types in mission descriptions",
            ),
            toggle(
                "cargo_info",
                "Cargo Info",
                "Show cargo descriptors (Full/Half/Scraps, HighValue/LowValue) on encounter ships",
            ),
            toggle(
                "region_info",
                "Region Info",
                "Append the region / body where the mission is offered",
            ),
            PatcherOption {
                id: "owned_blueprints".into(),
                label: "Owned Blueprints".into(),
                description: "Mark or hide blueprints you already own in the reward list \
                              (needs a blueprint fetch — Settings → Online)"
                    .into(),
                kind: OptionKind::Choice {
                    choices: vec![
                        ChoiceOption {
                            value: "off".into(),
                            label: "Off".into(),
                        },
                        ChoiceOption {
                            value: "mark".into(),
                            label: format!("Mark owned ({OWNED_MARK})"),
                        },
                        ChoiceOption {
                            value: "hide".into(),
                            label: "Hide owned".into(),
                        },
                    ],
                },
                default: "mark".into(),
            },
            toggle(
                "owned_title_tag",
                "Owned-Complete Title Tag",
                "Add a mark to titles of missions whose blueprint rewards you already own",
            ),
        ]
    }

    /// Owned rendering makes the output depend on the player's blueprint set —
    /// salt the op-set cache with it so a change re-derives (and the shell's
    /// fingerprint re-applies). `None` when owned rendering is off.
    fn cache_salt(
        &self,
        config: &PatcherConfig,
        owned: Option<&crate::OwnedSet>,
    ) -> Option<String> {
        let mode = OwnedMode::from_str(config.get_str("owned_blueprints", "mark"));
        let title = config.get_bool("owned_title_tag", true);
        if mode == OwnedMode::Off && !title {
            return None;
        }
        Some(crate::ops::stable_hash(owned?))
    }

    fn derive(
        &self,
        cooked: &CookedData,
        config: &PatcherConfig,
        owned: Option<&crate::OwnedSet>,
    ) -> anyhow::Result<OpSet> {
        let title_opts = TitleOptions {
            blueprint: config.get_bool("blueprint_tag", true),
            solo: config.get_bool("solo_tag", true),
            once: config.get_bool("once_tag", true),
            illegal: config.get_bool("illegal_tag", true),
            crimestat: CrimestatTagMode::from_str(config.get_str("crimestat_tag", "colored")),
            owned_tag: config.get_bool("owned_title_tag", true),
        };
        let desc_opts = DescOptions {
            blueprint_list: config.get_bool("blueprint_list", true),
            mission_info: config.get_bool("mission_info", true),
            ship_encounters: config.get_bool("ship_encounters", true),
            cargo_info: config.get_bool("cargo_info", true),
            region_info: config.get_bool("region_info", true),
            owned_mode: OwnedMode::from_str(config.get_str("owned_blueprints", "mark")),
        };

        let manufacturer_prefixes = build_manufacturer_prefixes(cooked);

        // ── Regroup entries by stripped loc key ─────────────────────────
        let mut title_pools: BTreeMap<&str, Vec<&MissionEntry>> = BTreeMap::new();
        let mut desc_pools: BTreeMap<&str, Vec<&MissionEntry>> = BTreeMap::new();
        for e in &cooked.missions {
            if let Some(k) = stripped(&e.title_key) {
                title_pools.entry(k).or_default().push(e);
            }
            if let Some(k) = stripped(&e.description_key) {
                desc_pools.entry(k).or_default().push(e);
            }
        }

        let mut patches: Vec<(String, PatchOp)> = Vec::new();

        // ── Title pool pass ─────────────────────────────────────────────
        for (key, members) in &title_pools {
            if cooked
                .locale
                .resolve(key)
                .filter(|v| !v.is_empty())
                .is_none()
            {
                continue;
            }
            let facts = PoolFacts::build(members);
            let owned_complete = owned.is_some_and(|o| pool_all_owned(members, o));
            let tags = render_title_tags(&facts, title_opts, owned_complete);
            if !tags.is_empty() {
                patches.push((key.to_string(), PatchOp::Suffix(format!(" {tags}"))));
            }
        }

        // ── Description pool pass ───────────────────────────────────────
        for (key, members) in &desc_pools {
            if cooked
                .locale
                .resolve(key)
                .filter(|v| !v.is_empty())
                .is_none()
            {
                continue;
            }
            let facts = PoolFacts::build(members);
            let suffix = render_description(&facts, &manufacturer_prefixes, desc_opts, owned);
            if !suffix.is_empty() {
                patches.push((key.to_string(), PatchOp::Suffix(suffix)));
            }
        }

        Ok(OpSet {
            renames: Vec::new(),
            patches,
        })
    }
}

fn stripped(key: &Option<String>) -> Option<&str> {
    key.as_deref()
        .map(|k| k.strip_prefix('@').unwrap_or(k))
        .filter(|k| !k.is_empty())
}

// ── Pool facts ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TriState {
    Unanimous(bool),
    Mixed,
}

impl TriState {
    /// Collect `(value, mixed_within_entry)` pairs — any within-entry
    /// mixing makes the whole pool axis Mixed regardless of the
    /// representatives' values.
    fn collect<I: Iterator<Item = (bool, bool)>>(iter: I) -> Self {
        let mut value: Option<bool> = None;
        for (v, mixed) in iter {
            if mixed {
                return TriState::Mixed;
            }
            match value {
                None => value = Some(v),
                Some(prev) if prev != v => return TriState::Mixed,
                _ => {}
            }
        }
        TriState::Unanimous(value.unwrap_or(false))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BlueprintState {
    /// No member rewards any blueprint pool.
    None,
    /// Every member rewards the same set of pools.
    AllSamePool,
    /// Every member rewards at least one pool, but the sets differ.
    AllDifferentPools,
    /// Some members reward blueprints and others don't.
    MixedPresence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CrimestatState {
    Unanimous(CrimestatRisk),
    Mixed,
}

struct PoolFacts<'a> {
    /// Pool members in catalog order.
    members: Vec<&'a MissionEntry>,
    blueprint_state: BlueprintState,
    shareable: TriState,
    once_only: TriState,
    illegal: TriState,
    crimestat: CrimestatState,
    uec_consistent: bool,
    scrip_consistent: bool,
    event_consistent: bool,
    rep_consistent: bool,
    cooldowns_consistent: bool,
    encounters_consistent: bool,
    /// What most members share, per axis — rendered once above the
    /// variants, which then list only where they deviate.
    baseline: Baseline<'a>,
    /// Distinct non-empty per-member region labels (single-line form).
    region_labels: Vec<String>,
}

/// One encounter slot of a member: its raw label plus the occurrence index
/// (a mission can carry the same label twice).
type EncounterSlot = (String, usize);

/// The value more than half of the pool members share, per axis. The outer
/// `None` means no majority (every variant shows its own value);
/// `Some(None)` means the majority has nothing on that axis.
#[derive(Default)]
struct Baseline<'a> {
    cooldown: Option<Option<String>>,
    rep: Option<Option<String>>,
    scrip: Option<Option<String>>,
    event: Option<Option<String>>,
    /// The majority blueprint-pool identity ([`blueprint_identity`]).
    blueprints: Option<BTreeSet<String>>,
    /// A member carrying the majority pools — renders them (owned marks
    /// need the pool data, not just the identity).
    blueprint_source: Option<&'a MissionEntry>,
    /// Per encounter slot, in first-seen order across members.
    encounters: Vec<(EncounterSlot, Option<Option<&'a MissionEncounter>>)>,
}

impl<'a> Baseline<'a> {
    fn build(members: &[&'a MissionEntry]) -> Self {
        let lines = |f: fn(&MissionEntry) -> Option<String>| {
            majority(&members.iter().map(|m| f(m)).collect::<Vec<_>>())
        };
        let identities: Vec<BTreeSet<String>> =
            members.iter().map(|m| blueprint_identity(m)).collect();
        let blueprints = majority(&identities);
        let blueprint_source = blueprints.as_ref().and_then(|b| {
            members
                .iter()
                .zip(&identities)
                .find(|(_, id)| *id == b)
                .map(|(m, _)| *m)
        });

        let per_member: Vec<Vec<(EncounterSlot, &'a MissionEncounter)>> =
            members.iter().map(|m| encounter_slots(m)).collect();
        let mut slots: Vec<EncounterSlot> = Vec::new();
        for (slot, _) in per_member.iter().flatten() {
            if !slots.contains(slot) {
                slots.push(slot.clone());
            }
        }
        let encounters = slots
            .into_iter()
            .map(|slot| {
                let values: Vec<Option<&MissionEncounter>> = per_member
                    .iter()
                    .map(|own| own.iter().find(|(s, _)| *s == slot).map(|(_, e)| *e))
                    .collect();
                let base = majority(&values);
                (slot, base)
            })
            .collect();

        Baseline {
            cooldown: lines(cooldown_line),
            rep: lines(rep_line),
            scrip: lines(scrip_line),
            event: lines(event_line),
            blueprints,
            blueprint_source,
            encounters,
        }
    }

    /// The majority encounters, in slot order — the top block.
    fn encounters(&self) -> Vec<&'a MissionEncounter> {
        self.encounters
            .iter()
            .filter_map(|(_, base)| base.flatten())
            .collect()
    }
}

/// The value more than half of `values` share, if any.
fn majority<T: PartialEq + Clone>(values: &[T]) -> Option<T> {
    values
        .iter()
        .find(|v| values.iter().filter(|w| w == v).count() * 2 > values.len())
        .cloned()
}

/// A member's encounters keyed by slot (label + occurrence).
fn encounter_slots(m: &MissionEntry) -> Vec<(EncounterSlot, &MissionEncounter)> {
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    m.encounters
        .iter()
        .map(|e| {
            let n = seen.entry(e.label.as_str()).or_default();
            let slot = (e.label.clone(), *n);
            *n += 1;
            (slot, e)
        })
        .collect()
}

impl<'a> PoolFacts<'a> {
    fn build(members: &[&'a MissionEntry]) -> Self {
        let blueprint_state = classify_blueprints(members);

        let shareable = TriState::collect(
            members
                .iter()
                .map(|m| (m.shareable, m.facts.shareable_mixed)),
        );
        let once_only = TriState::collect(
            members
                .iter()
                .map(|m| (m.once_only, m.facts.once_only_mixed)),
        );
        let illegal = TriState::collect(members.iter().map(|m| (m.illegal, m.facts.illegal_mixed)));

        let crimestat = classify_crimestat(members);

        // Cross-entry consistency; within-entry `*_mixed` flags fold in.
        // (aUEC and payout-variant are pinned by the cook's pooling key, so
        // there is no within-entry uec flag — cross-entry compare suffices.)
        let uec_consistent = all_equal(members.iter().map(|m| m.payout));
        let scrip_consistent = !members.iter().any(|m| m.facts.scrip_mixed)
            && all_equal(members.iter().map(|m| &m.scrip));
        // Event points are in the cook's pooling key: no within-entry mix.
        let event_consistent = all_equal(members.iter().map(|m| &m.event_rewards));
        let rep_consistent = !members.iter().any(|m| m.facts.rep_mixed)
            && all_equal(members.iter().map(|m| &m.reputation));
        let cooldowns_consistent = !members.iter().any(|m| m.facts.cooldowns_mixed)
            && all_equal(members.iter().map(|m| {
                (
                    m.cooldown_seconds.map(f32::to_bits),
                    m.abandon_cooldown_seconds.map(f32::to_bits),
                )
            }));
        let encounters_consistent = !members.iter().any(|m| m.facts.encounters_mixed)
            && all_equal(members.iter().map(|m| &m.encounters));

        let mut region_labels: Vec<String> = Vec::new();
        for m in members {
            let label = region_label_of(m);
            if !label.is_empty() && !region_labels.contains(&label) {
                region_labels.push(label);
            }
        }

        PoolFacts {
            members: members.to_vec(),
            blueprint_state,
            shareable,
            once_only,
            illegal,
            crimestat,
            uec_consistent,
            scrip_consistent,
            event_consistent,
            rep_consistent,
            cooldowns_consistent,
            encounters_consistent,
            baseline: Baseline::build(members),
            region_labels,
        }
    }

    /// Any axis the description renderer breaks out into variants?
    fn has_variants(&self) -> bool {
        self.members.len() > 1
            && (matches!(
                self.blueprint_state,
                BlueprintState::AllDifferentPools | BlueprintState::MixedPresence
            ) || !self.uec_consistent
                || !self.scrip_consistent
                || !self.event_consistent
                || !self.rep_consistent
                || !self.cooldowns_consistent
                || !self.encounters_consistent
                || self.region_labels.len() > 1
                || matches!(self.shareable, TriState::Mixed)
                || matches!(self.once_only, TriState::Mixed)
                || matches!(self.illegal, TriState::Mixed))
    }

    /// Mixed axes outside blueprints — drives the title's `[~]` marker.
    fn has_non_blueprint_mixing(&self) -> bool {
        matches!(self.shareable, TriState::Mixed)
            || matches!(self.once_only, TriState::Mixed)
            || matches!(self.illegal, TriState::Mixed)
    }
}

fn all_equal<T: PartialEq>(mut iter: impl Iterator<Item = T>) -> bool {
    match iter.next() {
        None => true,
        Some(first) => iter.all(|v| v == first),
    }
}

/// One member's blueprint-reward identity — the set of pools it draws
/// from, each identified by pool name + sorted blueprint GUIDs.
fn blueprint_identity(m: &MissionEntry) -> BTreeSet<String> {
    m.blueprint_rewards
        .iter()
        .map(|bp| {
            let mut guids: Vec<&str> = bp
                .blueprints
                .iter()
                .map(|b| b.blueprint_record_guid.as_str())
                .collect();
            guids.sort_unstable();
            format!("{}|{}", bp.pool_name, guids.join(","))
        })
        .collect()
}

fn classify_blueprints(members: &[&MissionEntry]) -> BlueprintState {
    if members.is_empty() {
        return BlueprintState::None;
    }
    let sets: Vec<BTreeSet<String>> = members.iter().map(|m| blueprint_identity(m)).collect();
    let with_count = sets.iter().filter(|s| !s.is_empty()).count();
    if with_count == 0 {
        return BlueprintState::None;
    }
    if with_count < sets.len() {
        return BlueprintState::MixedPresence;
    }
    if sets.windows(2).all(|w| w[0] == w[1]) {
        BlueprintState::AllSamePool
    } else {
        BlueprintState::AllDifferentPools
    }
}

fn classify_crimestat(members: &[&MissionEntry]) -> CrimestatState {
    let mut current: Option<CrimestatRisk> = None;
    for m in members {
        if m.facts.crimestat_mixed {
            return CrimestatState::Mixed;
        }
        match current {
            None => current = Some(m.facts.crimestat),
            Some(prev) if prev != m.facts.crimestat => return CrimestatState::Mixed,
            _ => {}
        }
    }
    CrimestatState::Unanimous(current.unwrap_or(CrimestatRisk::None))
}

// ── Regions ─────────────────────────────────────────────────────────────────
//
// The cook gives structured locations (system + locality name), so the old
// module's region-label string parsing dissolves into a structural merge:
// bodies accumulate per system (first-seen order, BTree-sorted systems),
// an unnamed locality means "system-wide" and is redundant once any body
// covers that system.

/// One region piece: `(system, Some(body))` or `(system, None)` = system-wide.
type RegionPiece = (String, Option<String>);

fn region_pieces_of(m: &MissionEntry) -> Vec<RegionPiece> {
    m.locations
        .iter()
        .map(|r| {
            // A locality named like its system ("Nyx: Nyx") carries no
            // extra information — treat it as system-wide.
            let body = if r.name.is_empty() || r.name == r.system {
                None
            } else {
                Some(r.name.clone())
            };
            (r.system.clone(), body)
        })
        .collect()
}

/// Merge pieces into one string per system: `"Stanton: Hurston, Crusader"`,
/// `"Pyro (system-wide)"` (only when no bodies cover Pyro).
fn merge_region_pieces(pieces: &[RegionPiece]) -> Vec<String> {
    let mut by_system: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut systemwide: Vec<&str> = Vec::new();
    for (system, body) in pieces {
        if system.is_empty() {
            continue;
        }
        match body {
            Some(b) => {
                let entry = by_system.entry(system.as_str()).or_default();
                if !entry.contains(&b.as_str()) {
                    entry.push(b.as_str());
                }
            }
            None => {
                if !systemwide.contains(&system.as_str()) {
                    systemwide.push(system.as_str());
                }
            }
        }
    }
    let mut parts: Vec<String> = Vec::new();
    for (sys, bodies) in &by_system {
        parts.push(format!("{sys}: {}", bodies.join(", ")));
    }
    for sys in &systemwide {
        if !by_system.contains_key(sys) {
            parts.push(format!("{sys} (system-wide)"));
        }
    }
    parts
}

/// One member's single-line region label (variant-label context).
fn region_label_of(m: &MissionEntry) -> String {
    merge_region_pieces(&region_pieces_of(m)).join(" / ")
}

// ── Owned blueprints (gRPC-sourced; re-added 2026-08-30) ─────────────────────

/// In-game mark for an owned blueprint. **ASCII only**: the SC font drops `✓`
/// and other non-ASCII glyphs (verified in-game 2026-09-26; standalone
/// sc-langpatch learned the same in 5db39d3), so `x` stands in as the
/// "ticked" mark (`[x]` in the title). The in-app catalog keeps `✓`.
const OWNED_MARK: &str = "x";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OwnedMode {
    Off,
    Mark,
    Hide,
}

impl OwnedMode {
    fn from_str(s: &str) -> Self {
        match s {
            "off" => OwnedMode::Off,
            "hide" => OwnedMode::Hide,
            _ => OwnedMode::Mark,
        }
    }
}

fn owns(owned: Option<&crate::OwnedSet>, guid: &str) -> bool {
    owned.is_some_and(|s| s.contains(guid))
}

/// True when the pool's members reward at least one blueprint and the player
/// owns **every** one — drives the owned-complete title tag.
fn pool_all_owned(members: &[&MissionEntry], owned: &crate::OwnedSet) -> bool {
    let mut any = false;
    for m in members {
        for pool in &m.blueprint_rewards {
            for bp in &pool.blueprints {
                any = true;
                if !owned.contains(&bp.blueprint_record_guid) {
                    return false;
                }
            }
        }
    }
    any
}

// ── Title tags ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TitleOptions {
    blueprint: bool,
    solo: bool,
    once: bool,
    illegal: bool,
    crimestat: CrimestatTagMode,
    /// Append an owned-complete mark when every reward blueprint is owned.
    owned_tag: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CrimestatTagMode {
    Off,
    Simple,
    Colored,
}

impl CrimestatTagMode {
    fn from_str(s: &str) -> Self {
        match s {
            "off" => CrimestatTagMode::Off,
            "simple" => CrimestatTagMode::Simple,
            _ => CrimestatTagMode::Colored,
        }
    }
}

/// Trailing tag string (no leading space); empty when nothing applies.
/// Only unanimous facts produce explicit tags; `[~]` flags non-blueprint
/// mixing ("behavior varies — see description").
fn render_title_tags(facts: &PoolFacts<'_>, opts: TitleOptions, owned_complete: bool) -> String {
    let mut tags: Vec<String> = Vec::new();

    if opts.blueprint {
        match facts.blueprint_state {
            BlueprintState::AllSamePool => tags.push(apply_color(Color::Highlight, bracket("BP"))),
            BlueprintState::AllDifferentPools => {
                tags.push(apply_color(Color::Highlight, bracket("BP*")))
            }
            BlueprintState::MixedPresence => {
                tags.push(apply_color(Color::Highlight, bracket("BP?")))
            }
            BlueprintState::None => {}
        }
    }

    // Owned-complete marker sits next to the BP tag ("grants BP / already have
    // them all" reads together), underlined to stand out from title text.
    if opts.owned_tag && owned_complete {
        tags.push(apply_color(Color::Underline, bracket(OWNED_MARK)));
    }

    if opts.solo && facts.shareable == TriState::Unanimous(false) {
        tags.push(bracket("Solo"));
    }
    if opts.once && facts.once_only == TriState::Unanimous(true) {
        tags.push(bracket("Uniq"));
    }
    if opts.illegal && facts.illegal == TriState::Unanimous(true) {
        tags.push(bracket("Illegal"));
    }

    if !matches!(opts.crimestat, CrimestatTagMode::Off)
        && let CrimestatState::Unanimous(risk) = facts.crimestat
        && risk != CrimestatRisk::None
    {
        tags.push(crimestat_tag(risk, opts.crimestat));
    }

    if facts.has_non_blueprint_mixing() {
        tags.push(bracket("~"));
    }

    tags.join(" ")
}

fn crimestat_tag(risk: CrimestatRisk, mode: CrimestatTagMode) -> String {
    match (mode, risk) {
        (CrimestatTagMode::Colored, CrimestatRisk::High) => {
            apply_color(Color::Highlight, bracket("CS Risk"))
        }
        (CrimestatTagMode::Colored, CrimestatRisk::Moderate) => {
            apply_color(Color::Underline, bracket("CS Risk"))
        }
        _ => bracket("CS Risk"),
    }
}

// ── Description blocks ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
struct DescOptions {
    blueprint_list: bool,
    mission_info: bool,
    ship_encounters: bool,
    cargo_info: bool,
    region_info: bool,
    /// How owned blueprints render in the "Potential Blueprints" list.
    owned_mode: OwnedMode,
}

/// Render the suffix appended to the description's INI value (leading
/// `\n\n` included). Empty when nothing renders.
fn render_description(
    facts: &PoolFacts<'_>,
    manufacturer_prefixes: &[String],
    opts: DescOptions,
    owned: Option<&crate::OwnedSet>,
) -> String {
    let Some(head) = facts.members.first().copied() else {
        return String::new();
    };

    let mut blocks: Vec<String> = Vec::new();

    if !facts.has_variants() {
        push_singleton_blocks(&mut blocks, head, facts, manufacturer_prefixes, opts, owned);
    } else {
        // Data-level mixing exists — but if every member's rendered diff
        // lines collapse to one group, it didn't survive rendering (e.g.
        // cooldowns differing by seconds that round to the same minutes).
        // Fall back to singleton to avoid a "Variants (1)" wart.
        let labels = resolve_variant_labels(&facts.members);
        let groups = group_by_diff_lines(facts, &labels, manufacturer_prefixes, opts);
        if groups.len() <= 1 {
            push_singleton_blocks(&mut blocks, head, facts, manufacturer_prefixes, opts, owned);
        } else {
            push_variants_blocks(
                &mut blocks,
                facts,
                &groups,
                manufacturer_prefixes,
                opts,
                owned,
            );
        }
    }

    if blocks.is_empty() {
        return String::new();
    }
    format!("{PARAGRAPH_BREAK}{}", blocks.join(PARAGRAPH_BREAK))
}

fn push_singleton_blocks(
    blocks: &mut Vec<String>,
    head: &MissionEntry,
    facts: &PoolFacts<'_>,
    manufacturer_prefixes: &[String],
    opts: DescOptions,
    owned: Option<&crate::OwnedSet>,
) {
    if opts.blueprint_list {
        for bp in &head.blueprint_rewards {
            blocks.push(blueprint_block(bp, owned, opts.owned_mode));
        }
    }
    if opts.mission_info
        && let Some(info) = mission_info_block(
            [
                cooldown_line(head),
                rep_line(head),
                scrip_line(head),
                event_line(head),
            ]
            .into_iter()
            .flatten()
            .collect(),
        )
    {
        blocks.push(info);
    }
    if opts.ship_encounters
        && let Some(enc) = encounter_block(
            &head.encounters.iter().collect::<Vec<_>>(),
            "Encounters",
            manufacturer_prefixes,
            opts.cargo_info,
        )
    {
        blocks.push(enc);
    }
    if opts.region_info
        && let Some(region) = region_block(facts)
    {
        blocks.push(region);
    }
}

fn push_variants_blocks(
    blocks: &mut Vec<String>,
    facts: &PoolFacts<'_>,
    groups: &[DiffGroup<'_>],
    manufacturer_prefixes: &[String],
    opts: DescOptions,
    owned: Option<&crate::OwnedSet>,
) {
    // Top section — what most variants share (the baseline); the variants
    // below list only where they deviate from it.
    let base = &facts.baseline;
    if opts.blueprint_list
        && let Some(source) = base.blueprint_source
    {
        for bp in &source.blueprint_rewards {
            blocks.push(blueprint_block(bp, owned, opts.owned_mode));
        }
    }
    if opts.mission_info
        && let Some(info) = mission_info_block(
            [&base.cooldown, &base.rep, &base.scrip, &base.event]
                .into_iter()
                .filter_map(|line| line.clone().flatten())
                .collect(),
        )
    {
        blocks.push(info);
    }
    if opts.ship_encounters
        && let Some(enc) = encounter_block(
            &base.encounters(),
            "Encounters",
            manufacturer_prefixes,
            opts.cargo_info,
        )
    {
        blocks.push(enc);
    }
    // Region placement rule: all variants share one region → keep the
    // block at top (labels then disambiguate on other axes); regions
    // differ → the region appears as the variant label instead.
    if opts.region_info
        && facts.region_labels.len() == 1
        && let Some(region) = region_block(facts)
    {
        blocks.push(region);
    }

    if let Some(block) = render_variants_section(groups) {
        blocks.push(block);
    }
}

fn blueprint_block(bp: &BpPoolReward, owned: Option<&crate::OwnedSet>, mode: OwnedMode) -> String {
    let mut s = header("Potential Blueprints");
    if bp.chance < 1.0 {
        s.push_str(&format!(" ({}% chance)", (bp.chance * 100.0) as i32));
    }
    // (name, owned) for resolvable entries, alphabetical (the pool's own order
    // is descending pick-weight — noisy in a help dialog). Unresolved dropped.
    let mut entries: Vec<(&str, bool)> = bp
        .blueprints
        .iter()
        .filter_map(|b| {
            b.name
                .as_deref()
                .map(|n| (n, owns(owned, &b.blueprint_record_guid)))
        })
        .collect();
    entries.sort_by_key(|(n, _)| n.to_lowercase());

    // Owned-count summary in the header (glyph-independent, so it reads even
    // if the per-item mark drops in the game font). Only when a set is loaded.
    if owned.is_some() {
        let total = entries.len();
        let owned_n = entries.iter().filter(|(_, o)| *o).count();
        if total > 0 && owned_n > 0 {
            s.push_str(&if owned_n == total {
                format!(" · all {total} owned")
            } else {
                format!(" · {owned_n}/{total} owned")
            });
        }
    }

    for (name, is_owned) in entries {
        // Hide mode: owned entries drop out (the header count still records them).
        if is_owned && mode == OwnedMode::Hide {
            continue;
        }
        s.push_str(NEWLINE);
        if is_owned && mode == OwnedMode::Mark {
            s.push_str(&format!("{OWNED_MARK} {name}"));
        } else {
            s.push_str(&bullet(name));
        }
    }
    s
}

/// The Mission Info block from its lines (cooldown, rep, scrip). No UEC
/// line: the contracts panel shows the reward itself — UEC renders only
/// inside variants, where it tells them apart ([`variant_diff_lines`]).
fn mission_info_block(lines: Vec<String>) -> Option<String> {
    if lines.is_empty() {
        return None;
    }
    Some(format!(
        "{}{NEWLINE}{}",
        header("Mission Info"),
        lines.join(NEWLINE)
    ))
}

/// `Cooldown: 30min (abandon: 10min)` — the abandon part only when it
/// differs from the completion cooldown by more than half a minute. A
/// contract with no completion cooldown gets no line, whatever its abandon
/// cooldown (mostly a 1min default — noise).
fn cooldown_line(m: &MissionEntry) -> Option<String> {
    let seconds = m.cooldown_seconds?;
    if seconds <= 0.0 {
        return None;
    }
    let personal = format_minutes(seconds / 60.0);
    match m
        .abandon_cooldown_seconds
        .filter(|a| *a > 0.0 && (a - seconds).abs() > 30.0)
    {
        Some(abandon) => Some(format!(
            "Cooldown: {personal} (abandon: {})",
            format_minutes(abandon / 60.0)
        )),
        None => Some(format!("Cooldown: {personal}")),
    }
}

/// Format a duration in minutes as the smallest sensible unit: `"48h"`
/// for whole-hour spans of 2h+, `"30min"`, `"45s"` for sub-minute
/// fractions, `"<1s"` for vanishingly small but non-zero values. Never
/// emits `"0min"`.
fn format_minutes(minutes: f32) -> String {
    if minutes <= 0.0 {
        // Caller filters this out — defensive.
        return "0min".to_string();
    }
    if minutes < 1.0 / 60.0 {
        return "<1s".to_string();
    }
    if minutes < 1.0 {
        return format!("{}s", (minutes * 60.0).round() as i32);
    }
    let rounded = minutes.round() as i32;
    if rounded >= 120 && rounded % 60 == 0 {
        return format!("{}h", rounded / 60);
    }
    format!("{rounded}min")
}

fn rep_line(m: &MissionEntry) -> Option<String> {
    let total: i32 = m
        .reputation
        .iter()
        .filter_map(|r| r.amount)
        .filter(|a| *a > 0)
        .sum();
    (total > 0).then(|| format!("Rep: {total} XP"))
}

/// `Event: 486 pts (split in party) · Collection 486 pts (each)` — one
/// figure per ladder, each with its own points and share rule: the overall
/// ladder first, then the category ladders it also counts toward. Folding a
/// category into the overall figure when the counts matched made the split
/// note read as covering both, but category points never divide. The share
/// rule only shows when some ladder splits — "each" is the default.
fn event_line(m: &MissionEntry) -> Option<String> {
    let parts: Vec<String> = m
        .event_rewards
        .iter()
        .filter_map(|ev| {
            let (overall, categories) = ev.ladders.split_first()?;
            let any_split = ev.ladders.iter().any(|l| l.split_for_party);
            let figure = |name: Option<&str>, l: &svc_data::EventLadder| {
                let mut s = match name {
                    Some(name) => format!("{name} {} pts", l.points),
                    None => format!("{} pts", l.points),
                };
                if any_split {
                    s.push_str(if l.split_for_party {
                        " (split in party)"
                    } else {
                        " (each)"
                    });
                }
                s
            };
            let mut figures = vec![figure(None, overall)];
            figures.extend(
                categories
                    .iter()
                    .filter_map(|c| Some(figure(Some(c.name.as_deref()?), c))),
            );
            Some(figures.join(" · "))
        })
        .collect();
    (!parts.is_empty()).then(|| format!("Event: {}", parts.join("; ")))
}

fn scrip_line(m: &MissionEntry) -> Option<String> {
    // One entry per distinct currency, amounts summed within it.
    let mut by_name: BTreeMap<String, i32> = BTreeMap::new();
    for s in &m.scrip {
        if s.amount <= 0 {
            continue;
        }
        *by_name
            .entry(s.name.clone().unwrap_or_default())
            .or_insert(0) += s.amount;
    }
    if by_name.is_empty() {
        return None;
    }
    let parts: Vec<String> = by_name
        .into_iter()
        .map(|(name, amt)| {
            if name.is_empty() {
                format!("{amt} scrip")
            } else {
                format!("{amt} {name}")
            }
        })
        .collect();
    Some(format!("Scrip: {}", parts.join(", ")))
}

/// The payout: the fixed amount, or for engine-calculated rewards the
/// cook's own `uecCurve` estimate (exact against live payouts; rep bonus
/// and party split are runtime and not modeled).
fn uec_line(m: &MissionEntry) -> Option<String> {
    let amount = m
        .payout
        .fixed
        .filter(|n| *n > 0)
        .or_else(|| m.payout.estimate.filter(|n| m.payout.calculated && *n > 0))?;
    Some(format!("UEC: {}", thousands(amount)))
}

/// `145250` → `145,250`, the way the contracts panel prints rewards — so a
/// player matches a variant against the Reward box at a glance. (No `~`
/// for estimates: the game font renders it as a minus sign.)
fn thousands(n: i32) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

fn region_block(facts: &PoolFacts<'_>) -> Option<String> {
    let pieces: Vec<RegionPiece> = facts
        .members
        .iter()
        .flat_map(|m| region_pieces_of(m))
        .collect();
    let entries = merge_region_pieces(&pieces);
    if entries.is_empty() {
        return None;
    }
    // One star system per row — vertical stacking beats `" / "` for any
    // pool touching multiple systems.
    Some(format!(
        "{}{NEWLINE}{}",
        header("Available at"),
        entries.join(NEWLINE)
    ))
}

// ── Encounters ──────────────────────────────────────────────────────────────

struct EncounterRendering {
    body: String,
    /// Summed spawn-count range across encounters (max across waves
    /// within each). The cook can't split allied from hostile ships, so
    /// this counts every spawn.
    ship_range: (i32, i32),
    /// NPCs spawned over the mission, friendlies (HUD-marked) excluded.
    hostile_npcs: i32,
    /// The CombatClass tier, when uniform across encounters.
    combat_class: Option<String>,
}

fn encounter_block(
    encounters: &[&MissionEncounter],
    title: &str,
    manufacturer_prefixes: &[String],
    include_cargo: bool,
) -> Option<String> {
    if encounters.is_empty() {
        return None;
    }
    let rendering = render_encounters(encounters, manufacturer_prefixes, include_cargo);
    if rendering.body.is_empty() {
        return None;
    }
    let heading = format_encounter_heading(title, &rendering);
    Some(format!("{}{NEWLINE}{}", header(heading), rendering.body))
}

/// `Encounters · 2-4 ships · 12 NPCs · Hard` — counts and tier omitted when
/// absent. The counts cover exactly the encounters listed under the heading.
fn format_encounter_heading(title: &str, r: &EncounterRendering) -> String {
    let (lo, hi) = r.ship_range;
    let mut parts: Vec<String> = vec![title.to_string()];
    if hi > 0 {
        parts.push(if lo == hi {
            format!("{lo} ship{}", if lo == 1 { "" } else { "s" })
        } else {
            format!("{lo}-{hi} ships")
        });
    }
    if r.hostile_npcs > 0 {
        let n = r.hostile_npcs;
        parts.push(format!("{n} NPC{}", if n == 1 { "" } else { "s" }));
    }
    if let Some(cc) = &r.combat_class {
        parts.push(cc.clone());
    }
    parts.join(" · ")
}

fn render_encounters(
    encounters: &[&MissionEncounter],
    manufacturer_prefixes: &[String],
    include_cargo: bool,
) -> EncounterRendering {
    let mut lines: Vec<String> = Vec::new();
    let mut total = (0i32, 0i32);
    let mut hostile_npcs = 0i32;

    for enc in encounters {
        if enc.kind != EncounterKind::Ships {
            hostile_npcs += render_spawn_encounter(enc, &mut lines);
            continue;
        }
        // Spawn range: per-wave sum of slot ranges, max across waves
        // (waves are sequential — the player faces one at a time).
        let mut enc_range = (0i32, 0i32);
        for wave in &enc.waves {
            let lo: i32 = wave.ships.iter().map(|s| s.count_min.max(0)).sum();
            let hi: i32 = wave.ships.iter().map(|s| s.count_max.max(0)).sum();
            enc_range.0 = enc_range.0.max(lo);
            enc_range.1 = enc_range.1.max(hi);
        }
        total.0 += enc_range.0;
        total.1 += enc_range.1;

        let multi_wave = enc.waves.len() > 1;
        for (i, wave) in enc.waves.iter().enumerate() {
            // Label line — encounter variable name, plus the wave name
            // when the encounter has several. Underlined as a scan
            // anchor when the player skims a long block.
            let mut label = encounter_label(&enc.label);
            if multi_wave {
                let wave_name = pretty_identifier(&wave.name);
                let wave_part = if wave_name.is_empty() {
                    format!("Wave {}", i + 1)
                } else {
                    wave_name
                };
                if label.is_empty() {
                    label = wave_part;
                } else {
                    label = format!("{label} — {wave_part}");
                }
            }
            if !label.is_empty() {
                lines.push(apply_color(Color::Underline, label));
            }

            for slot in &wave.ships {
                let names = compact_ship_names(&slot.ships, manufacturer_prefixes);
                let mut line = bullet(compose_count_and_pool(
                    slot.count_min,
                    slot.count_max,
                    &names,
                ));
                if !slot.factions.is_empty() {
                    let factions: Vec<String> =
                        slot.factions.iter().map(|f| pretty_identifier(f)).collect();
                    line.push_str(&format!(" ({})", factions.join(", ")));
                }
                lines.push(line);
            }

            if include_cargo && !wave.cargo.is_empty() {
                let cargo: Vec<String> = wave.cargo.iter().map(|c| pretty_identifier(c)).collect();
                lines.push(format!("Cargo: {}", cargo.join(", ")));
            }
        }
    }

    // CombatClass — uniform across encounters or omitted.
    let classes: BTreeSet<&str> = encounters
        .iter()
        .filter_map(|e| e.difficulty.as_deref())
        .collect();
    let combat_class = if classes.len() == 1 {
        classes.into_iter().next().map(str::to_owned)
    } else {
        None
    };

    EncounterRendering {
        body: lines.join(NEWLINE),
        ship_range: total,
        hostile_npcs,
        combat_class,
    }
}

/// An NPC or entity encounter: the label, then one line per distinct
/// (what, factions, friendly, target), counts summed across phases — the
/// total spawned over the mission. Returns the hostile NPC count.
fn render_spawn_encounter(enc: &MissionEncounter, lines: &mut Vec<String>) -> i32 {
    let npcs = enc.kind == EncounterKind::Npcs;
    // (what, factions, allied, target) → (min, max), in first-seen order.
    type Key = (String, String, bool, bool);
    let mut rows: Vec<(Key, (i32, i32))> = Vec::new();
    for wave in &enc.waves {
        for slot in &wave.spawns {
            // NPCs: archetype / class. Entities: the wave name says what
            // they are (`Relay`, `Kopion`).
            let what = if npcs {
                let kinds: Vec<String> = slot.kinds.iter().map(|k| npc_kind(k)).collect();
                kinds.join(" / ")
            } else {
                pretty_identifier(&wave.name)
            };
            let factions: Vec<String> =
                slot.factions.iter().map(|f| pretty_identifier(f)).collect();
            let key = (what, factions.join(", "), slot.allied, slot.target);
            let (lo, hi) = (slot.count_min.max(0), slot.count_max.max(0));
            match rows.iter_mut().find(|(k, _)| *k == key) {
                Some((_, sum)) => {
                    sum.0 += lo;
                    sum.1 += hi;
                }
                None => rows.push((key, (lo, hi))),
            }
        }
    }
    if rows.is_empty() {
        return 0;
    }

    let fallback = if npcs { "NPCs" } else { "Entities" };
    let label = encounter_label(&enc.label);
    lines.push(apply_color(
        Color::Underline,
        if label.is_empty() {
            fallback.to_string()
        } else {
            label
        },
    ));

    // The target mark only informs when it singles some slots out —
    // defend missions tag every enemy as a target.
    let mark_targets = !rows
        .iter()
        .filter(|((.., allied, _), _)| !allied)
        .all(|((.., target), _)| *target);

    let mut hostile = 0;
    for ((what, factions, allied, target), (lo, hi)) in rows {
        if npcs && !allied {
            hostile += hi;
        }
        let count = if hi > lo {
            format!("{lo}-{hi}")
        } else {
            format!("{hi}")
        };
        let mut line = match (what.is_empty(), npcs) {
            (false, _) => format!("{count}x {what}"),
            (true, true) => format!("{count} NPC{}", if hi == 1 { "" } else { "s" }),
            (true, false) => format!("{count} entit{}", if hi == 1 { "y" } else { "ies" }),
        };
        if !factions.is_empty() {
            line.push_str(&format!(" ({factions})"));
        }
        if target && mark_targets {
            line.push_str(" · target");
        }
        if allied {
            line.push_str(" · friendly");
        }
        lines.push(bullet(line));
    }
    hostile
}

/// `PU_Soldier_Elite` → `Soldier Elite`, `PU_sniper` → `Sniper`; class
/// tags (`Heavy`) pass through.
fn npc_kind(tag: &str) -> String {
    let pretty = pretty_identifier(tag.strip_prefix("PU_").unwrap_or(tag));
    let mut chars = pretty.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => pretty,
    }
}

/// `"3x Cutlass, Sabre"` / `"1-3x Scythe"`; bare `"2 ships"` when the
/// pool is empty (unresolved candidates).
fn compose_count_and_pool(lo: i32, hi: i32, ships: &[String]) -> String {
    let (lo, hi) = (lo.max(0), hi.max(0));
    let count = if hi > lo {
        format!("{lo}-{hi}")
    } else {
        format!("{hi}")
    };
    if ships.is_empty() {
        format!("{count} ship{}", if hi == 1 { "" } else { "s" })
    } else {
        format!("{count}x {}", ships.join(", "))
    }
}

/// Strip manufacturer prefixes, then collapse hull variants sharing a
/// base name (`Avenger Stalker` + `Avenger Warlock` → `Avenger`).
fn compact_ship_names(names: &[String], manufacturer_prefixes: &[String]) -> Vec<String> {
    let stripped: Vec<String> = names
        .iter()
        .map(|n| {
            manufacturer_prefixes
                .iter()
                .find_map(|p| n.strip_prefix(p.as_str()))
                .unwrap_or(n)
                .to_string()
        })
        .collect();
    collapse_variants(&stripped)
}

/// Collapse hull variants into base hull names where multiple variants of
/// the same base are present. Single-variant entries keep their full name.
fn collapse_variants(names: &[String]) -> Vec<String> {
    let mut groups: Vec<(String, Vec<&str>)> = Vec::new();
    for name in names {
        let base = name.split_whitespace().next().unwrap_or(name);
        if let Some(g) = groups.iter_mut().find(|(b, _)| b == base) {
            g.1.push(name);
        } else {
            groups.push((base.to_string(), vec![name]));
        }
    }
    groups
        .into_iter()
        .map(|(base, variants)| {
            if variants.len() == 1 {
                variants[0].to_string()
            } else {
                base
            }
        })
        .collect()
}

/// "First word + space" prefixes for stripping from ship display names,
/// from the cooked manufacturer catalog. Per manufacturer:
///
/// 1. the localized first word (`"Aegis "`, `"Crusader "`) — matches
///    long-form display names;
/// 2. the short code (`"AEGS "`) — empty on current DCBs, kept for when
///    upstream data recovers;
/// 3. the name's acronym (`"MISC "`, `"RSI "`) and its dotted form
///    (`"C.O. "`) — display names mix the long form and the
///    abbreviation, and the abbreviation isn't in the DCB anywhere.
fn build_manufacturer_prefixes(cooked: &CookedData) -> Vec<String> {
    // Reject prefixes shorter than 3 chars — a `"A "` prefix would catch
    // any ship name starting with `A`.
    const MIN_PREFIX_LEN: usize = 3;

    let mut prefixes: Vec<String> = Vec::new();
    let mut push = |word: &str| {
        if word.len() < MIN_PREFIX_LEN {
            return;
        }
        let prefix = format!("{word} ");
        if !prefixes.contains(&prefix) {
            prefixes.push(prefix);
        }
    };
    for m in cooked.manufacturers() {
        if let Some(name) = m.name.as_deref() {
            if let Some(first) = name.split_whitespace().next() {
                push(first);
            }
            let initials: Vec<char> = name
                .split_whitespace()
                .filter_map(|w| w.chars().next())
                .filter(|c| c.is_alphabetic())
                .map(|c| c.to_ascii_uppercase())
                .collect();
            if initials.len() >= 2 {
                let acronym: String = initials.iter().collect();
                push(&acronym);
                let dotted: String = initials.iter().map(|c| format!("{c}.")).collect();
                push(&dotted);
            }
        }
        push(m.code.as_str());
    }
    prefixes.sort();
    prefixes
}

// ── Variants ────────────────────────────────────────────────────────────────

/// One pool member + the disambiguator shown next to its block.
struct VariantLabel<'a> {
    entry: &'a MissionEntry,
    label: String,
    /// Structured region pieces behind the label (empty for rank /
    /// debug-hint / numeric labels) — lets grouped labels merge
    /// structurally instead of re-parsing strings.
    pieces: Vec<RegionPiece>,
    /// Rank suffix, when the label carries one.
    rank: Option<String>,
    used_numeric: bool,
}

/// Label resolution priority: region → mission rank (on collision or no
/// region) → debug-name hints → numeric `Variant N`.
fn resolve_variant_labels<'a>(members: &[&'a MissionEntry]) -> Vec<VariantLabel<'a>> {
    let regions: Vec<String> = members.iter().map(|m| region_label_of(m)).collect();

    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for r in &regions {
        if !r.is_empty() {
            *counts.entry(r.as_str()).or_default() += 1;
        }
    }

    let mut labels: Vec<VariantLabel<'a>> = Vec::with_capacity(members.len());
    let mut numeric_seq = 0usize;

    for (idx, m) in members.iter().enumerate() {
        let region = &regions[idx];
        let collide = !region.is_empty() && counts.get(region.as_str()).copied().unwrap_or(0) > 1;

        if !region.is_empty() {
            let rank = collide.then(|| mission_rank(m)).flatten();
            let label = match &rank {
                Some(r) => format!("{region} · {r}"),
                None => region.clone(),
            };
            labels.push(VariantLabel {
                entry: m,
                label,
                pieces: region_pieces_of(m),
                rank,
                used_numeric: false,
            });
        } else if let Some(rank) = mission_rank(m) {
            labels.push(VariantLabel {
                entry: m,
                label: rank.clone(),
                pieces: Vec::new(),
                rank: Some(rank),
                used_numeric: false,
            });
        } else if let Some(hint) = parse_debug_name_hints(&m.debug_name) {
            labels.push(VariantLabel {
                entry: m,
                label: hint,
                pieces: Vec::new(),
                rank: None,
                used_numeric: false,
            });
        } else {
            numeric_seq += 1;
            labels.push(VariantLabel {
                entry: m,
                label: format!("Variant {numeric_seq}"),
                pieces: Vec::new(),
                rank: None,
                used_numeric: true,
            });
        }
    }

    labels
}

/// First usable rank name from the entry's rep requirements — the cook
/// already resolved standing names (`"Mercenary"`).
fn mission_rank(m: &MissionEntry) -> Option<String> {
    m.rep_required
        .iter()
        .filter(|r| !r.exclude)
        .find_map(|r| r.min_rank.clone())
}

/// Scan an internal `debug_name` for tokens that also show up as in-game
/// mobiGlas tags: location (Stanton planet number, Pyro region letter,
/// Nyx) and difficulty rank. Conservative — only explicit token shapes
/// the SC mission generator uses consistently.
fn parse_debug_name_hints(debug_name: &str) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();

    let tokens: Vec<&str> = debug_name.split('_').collect();

    let mut location: Option<String> = None;
    for (i, t) in tokens.iter().enumerate() {
        if let Some(letter) = t.strip_prefix("Region")
            && letter.len() == 1
            && letter.chars().all(|c| c.is_ascii_uppercase())
        {
            let system = i.checked_sub(1).and_then(|j| tokens.get(j)).copied();
            location = Some(match system {
                Some("Pyro") => format!("Pyro Region {letter}"),
                Some("Stanton") => format!("Stanton Region {letter}"),
                _ => format!("Region {letter}"),
            });
            break;
        }
        if let Some(rest) = t.strip_prefix("Stanton")
            && !rest.is_empty()
            && rest.chars().next().is_some_and(|c| c.is_ascii_digit())
        {
            location = Some(format!("Stanton {rest}"));
            break;
        }
        if *t == "Pyro" && i + 1 < tokens.len() {
            // Lone `Pyro` still tells the player the system; keep looking
            // for a more specific Region* token.
            location = Some("Pyro".to_string());
        }
        if *t == "Nyx" {
            location = Some("Nyx".to_string());
        }
    }
    if let Some(l) = location {
        parts.push(l);
    }

    let difficulty = tokens.iter().find_map(|t| match *t {
        "VeryEasy" | "VE" => Some("Very Easy"),
        "Easy" | "E" => Some("Easy"),
        "Medium" | "M" => Some("Medium"),
        "Hard" | "H" => Some("Hard"),
        "VeryHard" | "VH" => Some("Very Hard"),
        "Super" | "S" => Some("Super"),
        _ => None,
    });
    if let Some(d) = difficulty {
        parts.push(d.to_string());
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" · "))
    }
}

/// One variant after dedup-by-rendered-content — members producing
/// identical diff lines collapse into one entry (many pools have N
/// generator outputs identical to the player).
struct DiffGroup<'a> {
    labels: Vec<&'a VariantLabel<'a>>,
    diff_lines: Vec<String>,
}

fn group_by_diff_lines<'a>(
    facts: &PoolFacts<'_>,
    labels: &'a [VariantLabel<'a>],
    manufacturer_prefixes: &[String],
    opts: DescOptions,
) -> Vec<DiffGroup<'a>> {
    let mut groups: Vec<DiffGroup<'a>> = Vec::new();
    for v in labels {
        let diff = variant_diff_lines(facts, v.entry, manufacturer_prefixes, opts);
        match groups.iter_mut().find(|g| g.diff_lines == diff) {
            Some(existing) => {
                if !existing.labels.iter().any(|l| l.label == v.label) {
                    existing.labels.push(v);
                }
            }
            None => groups.push(DiffGroup {
                labels: vec![v],
                diff_lines: diff,
            }),
        }
    }
    groups
}

fn render_variants_section(groups: &[DiffGroup<'_>]) -> Option<String> {
    if groups.is_empty() {
        return None;
    }
    let count = groups.len();
    // Variants list only how they differ from the blocks above.
    let mut s = header(format!("Variants ({count}) · differences only"));

    // Two groups can collide on the combined label when they share a
    // region but differ on another axis — a numeric suffix keeps them
    // distinguishable.
    let mut group_labels: Vec<String> = groups
        .iter()
        .map(|g| combine_group_labels(&g.labels))
        .collect();
    let mut label_counts: BTreeMap<&str, usize> = BTreeMap::new();
    for l in &group_labels {
        *label_counts.entry(l.as_str()).or_default() += 1;
    }
    if label_counts.values().any(|c| *c > 1) {
        let colliding: BTreeSet<String> = label_counts
            .iter()
            .filter(|(_, c)| **c > 1)
            .map(|(l, _)| l.to_string())
            .collect();
        let mut seen: BTreeMap<String, usize> = BTreeMap::new();
        for l in group_labels.iter_mut() {
            if colliding.contains(l.as_str()) {
                let n = seen.entry(l.clone()).or_insert(0);
                *n += 1;
                *l = format!("{l} ({n})");
            }
        }
    }

    for (g, label) in groups.iter().zip(group_labels.iter()) {
        s.push_str(PARAGRAPH_BREAK);
        s.push_str(&header(format!("· {label}")));
        if g.diff_lines.is_empty() {
            // The baseline variant: everything as listed above.
            s.push_str(&format!("{NEWLINE}  as above"));
        }
        for line in &g.diff_lines {
            s.push_str(NEWLINE);
            s.push_str("  ");
            s.push_str(line);
        }
    }

    Some(s)
}

/// Combine the labels of members that grouped together. Region labels
/// with the same rank suffix merge structurally (`"Stanton: Hurston"` +
/// `"Stanton: ArcCorp"` → `"Stanton: Hurston, ArcCorp"`); different
/// ranks stay separate entries. Numeric labels are ignored when any real
/// label is present.
fn combine_group_labels(labels: &[&VariantLabel<'_>]) -> String {
    let real: Vec<&&VariantLabel<'_>> = labels.iter().filter(|l| !l.used_numeric).collect();
    if real.is_empty() {
        return labels.first().map(|l| l.label.clone()).unwrap_or_default();
    }

    // Group by rank suffix; merge region pieces within each rank group.
    // Labels without pieces (rank-only / debug-hint) pass through their
    // label text.
    let mut by_rank: Vec<(Option<&str>, Vec<RegionPiece>, Vec<&str>)> = Vec::new();
    for l in &real {
        let rank = l.rank.as_deref();
        let slot = match by_rank.iter_mut().find(|(r, _, _)| *r == rank) {
            Some(s) => s,
            None => {
                by_rank.push((rank, Vec::new(), Vec::new()));
                by_rank.last_mut().expect("just pushed")
            }
        };
        if l.pieces.is_empty() {
            if !slot.2.contains(&l.label.as_str()) {
                slot.2.push(l.label.as_str());
            }
        } else {
            slot.1.extend(l.pieces.iter().cloned());
        }
    }

    let rendered: Vec<String> = by_rank
        .into_iter()
        .filter_map(|(rank, pieces, passthrough)| {
            let mut parts = merge_region_pieces(&pieces);
            for p in passthrough {
                // Rank-only labels already carry the rank in the text.
                if rank.is_some_and(|r| r == p) {
                    continue;
                }
                parts.push(p.to_string());
            }
            let merged = parts.join(" / ");
            match (merged.is_empty(), rank) {
                (true, Some(r)) => Some(r.to_string()),
                (true, None) => None,
                (false, Some(r)) if !pieces.is_empty() => Some(format!("{merged} · {r}")),
                (false, _) => Some(merged),
            }
        })
        .collect();
    rendered.join(" / ")
}

fn variant_diff_lines(
    facts: &PoolFacts<'_>,
    m: &MissionEntry,
    manufacturer_prefixes: &[String],
    opts: DescOptions,
) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let base = &facts.baseline;

    // UEC first: it is the line a player matches against the panel's
    // Reward box to tell which variant they are looking at. Only when the
    // payouts differ — otherwise it identifies nothing.
    if opts.mission_info
        && !facts.uec_consistent
        && let Some(line) = uec_line(m)
    {
        lines.push(line);
    }

    // Info lines where this member deviates from the baseline (or its own
    // value is one of several within the entry).
    if opts.mission_info {
        // (baseline, own line, own value uncertain, text when absent)
        let info = [
            (
                &base.cooldown,
                cooldown_line(m),
                m.facts.cooldowns_mixed,
                "No cooldown",
            ),
            (&base.rep, rep_line(m), m.facts.rep_mixed, "No rep reward"),
            (&base.scrip, scrip_line(m), m.facts.scrip_mixed, "No scrip"),
            (&base.event, event_line(m), false, "No event points"),
        ];
        for (baseline, own, mixed, none) in info {
            if mixed || *baseline != Some(own.clone()) {
                lines.push(own.unwrap_or_else(|| none.to_string()));
            }
        }
    }

    // Blueprints where this member's pools differ from the baseline —
    // header line + bullets, matching the singleton layout (comma-joined
    // long lists were unreadable).
    if opts.blueprint_list && base.blueprints.as_ref() != Some(&blueprint_identity(m)) {
        if m.blueprint_rewards.is_empty() {
            lines.push("No blueprint".to_string());
        } else {
            for bp in &m.blueprint_rewards {
                let mut names: Vec<&str> = bp
                    .blueprints
                    .iter()
                    .filter_map(|b| b.name.as_deref())
                    .collect();
                names.sort_by_key(|n| n.to_lowercase());
                if names.is_empty() {
                    lines.push("Blueprints: (pool empty)".to_string());
                    continue;
                }
                let chance = if bp.chance < 1.0 {
                    format!(" ({}% chance)", (bp.chance * 100.0) as i32)
                } else {
                    String::new()
                };
                lines.push(format!("Blueprints{chance}:"));
                for name in names {
                    lines.push(bullet(name));
                }
            }
        }
    }

    // Per-flag deltas — only when that flag is mixed across the pool.
    if matches!(facts.shareable, TriState::Mixed) {
        lines.push(
            if m.shareable {
                "Shareable"
            } else {
                "Solo only"
            }
            .to_string(),
        );
    }
    if matches!(facts.once_only, TriState::Mixed) && m.once_only {
        lines.push("One-time only".to_string());
    }
    if matches!(facts.illegal, TriState::Mixed) && m.illegal {
        lines.push("Illegal".to_string());
    }

    // Encounters, slot by slot: only those that differ from the baseline
    // render here; a baseline encounter this member lacks is named.
    if opts.ship_encounters {
        let mine = encounter_slots(m);
        let mut own: Vec<&MissionEncounter> = Vec::new();
        let mut missing: Vec<String> = Vec::new();
        for (slot, baseline) in &base.encounters {
            let mine_here = mine.iter().find(|(s, _)| s == slot).map(|(_, e)| *e);
            if *baseline == Some(mine_here) {
                continue;
            }
            match (mine_here, baseline) {
                (Some(e), _) => own.push(e),
                (None, Some(Some(b))) => missing.push(match encounter_label(&b.label) {
                    l if l.is_empty() => "No NPCs".to_string(),
                    l => format!("No {l}"),
                }),
                (None, _) => {}
            }
        }
        let rendering = render_encounters(&own, manufacturer_prefixes, opts.cargo_info);
        if !rendering.body.is_empty() {
            let heading = format_encounter_heading("Encounters", &rendering);
            let indent = format!("{NEWLINE}  ");
            let body = rendering.body.replace(NEWLINE, &indent);
            lines.push(format!("{heading}:{NEWLINE}  {body}"));
        }
        lines.extend(missing);
    }

    lines
}

// ── Identifier prettifying ──────────────────────────────────────────────────

/// Pretty-print a CamelCase / snake_case identifier as space-separated
/// words (`MissionTargets` → `Mission Targets`, `EnemyNPCs` → `Enemy
/// NPCs`). Trailing `_BP` / leading `BP_` noise is stripped first.
fn pretty_identifier(s: &str) -> String {
    let trimmed = strip_noise_affixes(s);
    if trimmed.is_empty() {
        return String::new();
    }
    let chars: Vec<char> = trimmed.chars().collect();
    let mut out = String::with_capacity(chars.len() + 4);
    for i in 0..chars.len() {
        let c = chars[i];
        if c == '_' {
            if !out.ends_with(' ') && !out.is_empty() {
                out.push(' ');
            }
            continue;
        }
        if i > 0 {
            let prev = chars[i - 1];
            // Split an uppercase run before its last letter only when a
            // real word follows (two+ lowercase chars): `URLPath` → `URL
            // Path`, but `NPCs` keeps its plural `s`.
            let two_lower_follow = chars.get(i + 1).map(|n| n.is_lowercase()).unwrap_or(false)
                && chars.get(i + 2).map(|n| n.is_lowercase()).unwrap_or(false);
            let boundary = (prev.is_lowercase() && c.is_uppercase())
                || (prev.is_ascii_digit() && c.is_alphabetic())
                || (prev.is_alphabetic() && c.is_ascii_digit())
                || (prev.is_uppercase() && c.is_uppercase() && two_lower_follow);
            if boundary && !out.ends_with(' ') {
                out.push(' ');
            }
        }
        out.push(c);
    }
    while out.contains("  ") {
        out = out.replace("  ", " ");
    }
    out.trim().to_string()
}

/// Encounter label: prettified variable name minus the engine's
/// `SpawnDescription(s)` boilerplate (`Scouts_SpawnDescriptions` →
/// `Scouts`, `SalvageSpawnDescription` → `Salvage`).
/// Readable encounter label from its mission-variable name. Generator
/// boilerplate (a leading `Property`; `Spawn Description(s)` as prefix,
/// suffix or the whole name) drops; an empty result means "no meaningful
/// label".
fn encounter_label(raw: &str) -> String {
    let pretty = pretty_identifier(raw);
    let pretty = pretty
        .strip_prefix("Property ")
        .map(str::to_owned)
        .unwrap_or(pretty);
    for noise in ["Spawn Descriptions", "Spawn Description"] {
        if pretty == noise {
            return String::new();
        }
        if let Some(rest) = pretty.strip_suffix(&format!(" {noise}")) {
            return rest.to_string();
        }
        if let Some(rest) = pretty.strip_prefix(&format!("{noise} ")) {
            return rest.to_string();
        }
    }
    pretty
}

fn strip_noise_affixes(s: &str) -> &str {
    let mut t = s.trim();
    if let Some(rest) = t.strip_suffix("_BP") {
        t = rest;
    }
    if let Some(rest) = t.strip_prefix("BP_") {
        t = rest;
    }
    t
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use svc_data::{MissionPayout, MissionPoolFacts, MissionRegion, ScripReward, SpawnSlot};

    fn base_entry() -> MissionEntry {
        MissionEntry {
            mission_id: "0".into(),
            title: None,
            title_key: Some("@mission_title_test".into()),
            debug_name: "Test_Mission".into(),
            description: None,
            description_key: Some("@mission_desc_test".into()),
            category: None,
            faction: None,
            difficulty: None,
            payout: MissionPayout {
                calculated: false,
                fixed: None,
                estimate: None,
                buy_in: 0,
                time_to_complete: 0.0,
            },
            once_only: false,
            shareable: true,
            illegal: false,
            cooldown_seconds: None,
            abandon_cooldown_seconds: None,
            scrip: Vec::new(),
            reputation: Vec::new(),
            item_rewards: Vec::new(),
            blueprint_rewards: Vec::new(),
            event_rewards: Vec::new(),
            rep_required: Vec::new(),
            chain_required: Vec::new(),
            locations: Vec::new(),
            encounters: Vec::new(),
            cargo: Vec::new(),
            placeholders: Vec::new(),
            instance_count: 1,
            facts: MissionPoolFacts::default(),
        }
    }

    fn region(system: &str, name: &str) -> MissionRegion {
        MissionRegion {
            system: system.into(),
            name: name.into(),
            places: Vec::new(),
        }
    }

    const OPTS: TitleOptions = TitleOptions {
        blueprint: true,
        solo: true,
        once: true,
        illegal: true,
        crimestat: CrimestatTagMode::Colored,
        owned_tag: true,
    };

    #[test]
    fn title_tags_render_unanimous_flags() {
        let mut a = base_entry();
        a.shareable = false;
        a.once_only = true;
        a.illegal = true;
        let facts = PoolFacts::build(&[&a]);
        assert_eq!(
            render_title_tags(&facts, OPTS, false),
            "[Solo] [Uniq] [Illegal]"
        );
    }

    #[test]
    fn title_tags_mixed_flags_collapse_to_tilde() {
        let mut a = base_entry();
        a.illegal = true;
        let b = base_entry();
        let facts = PoolFacts::build(&[&a, &b]);
        assert_eq!(render_title_tags(&facts, OPTS, false), "[~]");
    }

    #[test]
    fn title_tags_within_entry_mixing_counts_as_mixed() {
        let mut a = base_entry();
        a.once_only = true;
        a.facts.once_only_mixed = true;
        let facts = PoolFacts::build(&[&a]);
        assert_eq!(render_title_tags(&facts, OPTS, false), "[~]");
    }

    #[test]
    fn title_tags_crimestat_modes() {
        let mut a = base_entry();
        a.facts.crimestat = CrimestatRisk::High;
        let facts = PoolFacts::build(&[&a]);
        assert_eq!(
            render_title_tags(&facts, OPTS, false),
            "<EM4>[CS Risk]</EM4>"
        );
        let simple = TitleOptions {
            crimestat: CrimestatTagMode::Simple,
            ..OPTS
        };
        assert_eq!(render_title_tags(&facts, simple, false), "[CS Risk]");
        let off = TitleOptions {
            crimestat: CrimestatTagMode::Off,
            ..OPTS
        };
        assert_eq!(render_title_tags(&facts, off, false), "");
    }

    #[test]
    fn merge_region_pieces_collapses_systems() {
        let pieces = vec![
            ("Stanton".to_string(), Some("Hurston".to_string())),
            ("Stanton".to_string(), Some("Crusader".to_string())),
            ("Stanton".to_string(), Some("Hurston".to_string())),
            ("Pyro".to_string(), None),
        ];
        assert_eq!(
            merge_region_pieces(&pieces),
            vec!["Stanton: Hurston, Crusader", "Pyro (system-wide)"]
        );
    }

    #[test]
    fn merge_region_pieces_drops_systemwide_when_bodies_present() {
        let pieces = vec![
            ("Pyro".to_string(), Some("Bloom".to_string())),
            ("Pyro".to_string(), None),
        ];
        assert_eq!(merge_region_pieces(&pieces), vec!["Pyro: Bloom"]);
    }

    #[test]
    fn description_singleton_renders_info_and_region() {
        let mut a = base_entry();
        a.payout.fixed = Some(5000);
        a.cooldown_seconds = Some(1800.0);
        a.scrip = vec![ScripReward {
            name: Some("MG".into()),
            amount: 5,
        }];
        a.locations = vec![region("Stanton", "Hurston")];
        let facts = PoolFacts::build(&[&a]);
        let opts = DescOptions {
            blueprint_list: true,
            mission_info: true,
            ship_encounters: true,
            cargo_info: true,
            region_info: true,
            owned_mode: OwnedMode::Off,
        };
        let out = render_description(&facts, &[], opts, None);
        assert!(out.starts_with(PARAGRAPH_BREAK), "leading separator: {out}");
        assert!(out.contains("<EM4>Mission Info</EM4>"), "{out}");
        assert!(out.contains("Cooldown: 30min"), "{out}");
        assert!(out.contains("Scrip: 5 MG"), "{out}");
        // The panel shows the reward itself; UEC only identifies variants.
        assert!(!out.contains("UEC"), "{out}");
        assert!(out.contains("<EM4>Available at</EM4>"), "{out}");
        assert!(out.contains("Stanton: Hurston"), "{out}");
    }

    #[test]
    fn description_variants_split_by_region_and_payout() {
        let mut a = base_entry();
        a.payout.fixed = Some(1000);
        a.locations = vec![region("Stanton", "Hurston")];
        let mut b = base_entry();
        b.payout.fixed = Some(2000);
        b.locations = vec![region("Pyro", "Bloom")];
        let facts = PoolFacts::build(&[&a, &b]);
        assert!(facts.has_variants());
        let opts = DescOptions {
            blueprint_list: true,
            mission_info: true,
            ship_encounters: true,
            cargo_info: true,
            region_info: true,
            owned_mode: OwnedMode::Off,
        };
        let out = render_description(&facts, &[], opts, None);
        assert!(out.contains("Variants (2)"), "{out}");
        assert!(out.contains("Stanton: Hurston"), "{out}");
        assert!(out.contains("Pyro: Bloom"), "{out}");
        assert!(out.contains("UEC: 1,000"), "{out}");
        assert!(out.contains("UEC: 2,000"), "{out}");
    }

    #[test]
    fn variants_show_only_deviations_from_the_majority() {
        let pool = BpPoolReward {
            pool_name: "Scrapers".into(),
            chance: 1.0,
            blueprints: vec![svc_data::BpPoolEntry {
                blueprint_record_guid: "g1".into(),
                name: Some("Hart Scraper Module".into()),
                weight: 1.0,
            }],
        };
        let target = encounter("Target_BP", "Aurora", "Criminal");
        let mut a = base_entry();
        a.blueprint_rewards = vec![pool.clone()];
        a.cooldown_seconds = Some(1800.0);
        a.encounters = vec![target.clone()];
        a.locations = vec![region("Stanton", "Hurston")];
        let mut b = a.clone();
        b.locations = vec![region("Pyro", "Bloom")];
        let mut c = a.clone();
        c.blueprint_rewards = Vec::new();
        c.cooldown_seconds = Some(172_800.0);
        c.encounters = vec![encounter("Target_BP", "Scorpius", "Criminal")];
        c.locations = vec![region("Nyx", "")];

        let facts = PoolFacts::build(&[&a, &b, &c]);
        let opts = DescOptions {
            blueprint_list: true,
            mission_info: true,
            ship_encounters: true,
            cargo_info: true,
            region_info: true,
            owned_mode: OwnedMode::Off,
        };
        let out = render_description(&facts, &[], opts, None);
        // The majority (a, b) renders once on top…
        assert_eq!(out.matches("Hart Scraper Module").count(), 1, "{out}");
        assert_eq!(out.matches("Aurora").count(), 1, "{out}");
        assert!(
            out.contains("<EM4>Mission Info</EM4>\\nCooldown: 30min"),
            "{out}"
        );
        // …the two majority variants merge and read "as above"…
        assert!(out.contains("Variants (2) · differences only"), "{out}");
        assert!(out.contains("as above"), "{out}");
        // …and the deviating one lists only its differences.
        assert!(out.contains("Cooldown: 48h"), "{out}");
        assert!(out.contains("No blueprint"), "{out}");
        assert!(out.contains("1x Scorpius (Criminal)"), "{out}");
    }

    #[test]
    fn description_identical_variants_collapse_to_singleton() {
        // Data-level mixing (cooldowns differ by seconds) that rounds to
        // the same rendered minutes — must NOT produce "Variants (1)".
        let mut a = base_entry();
        a.cooldown_seconds = Some(1800.0);
        let mut b = base_entry();
        b.cooldown_seconds = Some(1801.0);
        let facts = PoolFacts::build(&[&a, &b]);
        assert!(facts.has_variants(), "data-level mixing expected");
        let opts = DescOptions {
            blueprint_list: true,
            mission_info: true,
            ship_encounters: true,
            cargo_info: true,
            region_info: true,
            owned_mode: OwnedMode::Off,
        };
        let out = render_description(&facts, &[], opts, None);
        assert!(!out.contains("Variants"), "{out}");
        assert!(out.contains("Cooldown: 30min"), "{out}");
    }

    #[test]
    fn cooldown_line_shows_abandon_only_when_it_differs() {
        let mut m = base_entry();
        m.cooldown_seconds = Some(1800.0);
        m.abandon_cooldown_seconds = Some(600.0);
        assert_eq!(
            cooldown_line(&m).as_deref(),
            Some("Cooldown: 30min (abandon: 10min)")
        );
        // Within half a minute → the same cooldown, no abandon part.
        m.abandon_cooldown_seconds = Some(1810.0);
        assert_eq!(cooldown_line(&m).as_deref(), Some("Cooldown: 30min"));
        // No completion cooldown → no line, even with an abandon one.
        m.cooldown_seconds = None;
        m.abandon_cooldown_seconds = Some(3600.0);
        assert_eq!(cooldown_line(&m), None);
    }

    #[test]
    fn event_line_shows_points_category_and_split() {
        use svc_data::{EventLadder, EventReward};
        let ladder = |name: Option<&str>, points, split| EventLadder {
            name: name.map(str::to_string),
            points,
            split_for_party: split,
            tiers: vec![2000, 4000],
        };
        let mut m = base_entry();
        assert_eq!(event_line(&m), None);
        m.event_rewards = vec![EventReward {
            event: "Iasi".into(),
            ladders: vec![
                ladder(None, 1142, true),
                ladder(Some("Collection"), 1142, false),
            ],
        }];
        // Each ladder keeps its own figure and share rule — equal counts
        // are not folded together (the split never covers the category).
        assert_eq!(
            event_line(&m).as_deref(),
            Some("Event: 1142 pts (split in party) · Collection 1142 pts (each)")
        );
        // Nothing splits → no share rule ("each" is the default).
        m.event_rewards[0].ladders[0].split_for_party = false;
        m.event_rewards[0].ladders[1].points = 900;
        assert_eq!(
            event_line(&m).as_deref(),
            Some("Event: 1142 pts · Collection 900 pts")
        );
    }

    #[test]
    fn description_variants_split_on_abandon_cooldown() {
        let mut a = base_entry();
        a.cooldown_seconds = Some(1800.0);
        a.abandon_cooldown_seconds = Some(600.0);
        a.locations = vec![region("Stanton", "Hurston")];
        let mut b = base_entry();
        b.cooldown_seconds = Some(1800.0);
        b.abandon_cooldown_seconds = Some(1800.0);
        b.locations = vec![region("Pyro", "Bloom")];
        let facts = PoolFacts::build(&[&a, &b]);
        let opts = DescOptions {
            blueprint_list: true,
            mission_info: true,
            ship_encounters: true,
            cargo_info: true,
            region_info: true,
            owned_mode: OwnedMode::Off,
        };
        let out = render_description(&facts, &[], opts, None);
        assert!(out.contains("Variants (2)"), "{out}");
        assert!(out.contains("Cooldown: 30min (abandon: 10min)"), "{out}");
    }

    fn encounter(label: &str, ship: &str, faction: &str) -> MissionEncounter {
        MissionEncounter {
            label: label.into(),
            difficulty: None,
            waves: vec![svc_data::MissionWave {
                name: String::new(),
                ships: vec![svc_data::ShipSlot {
                    count_min: 1,
                    count_max: 1,
                    ships: vec![ship.into()],
                    factions: vec![faction.into()],
                }],
                cargo: Vec::new(),
                spawns: Vec::new(),
            }],
            kind: EncounterKind::Ships,
        }
    }

    fn npc(count: i32, kind: &str, faction: &str, allied: bool, target: bool) -> SpawnSlot {
        SpawnSlot {
            count_min: count,
            count_max: count,
            concurrent: count,
            kinds: vec![kind.into()],
            factions: vec![faction.into()],
            allied,
            target,
        }
    }

    fn npc_encounter(label: &str, phases: Vec<Vec<SpawnSlot>>) -> MissionEncounter {
        MissionEncounter {
            label: label.into(),
            difficulty: None,
            waves: phases
                .into_iter()
                .map(|spawns| svc_data::MissionWave {
                    name: String::new(),
                    ships: Vec::new(),
                    cargo: Vec::new(),
                    spawns,
                })
                .collect(),
            kind: EncounterKind::Npcs,
        }
    }

    #[test]
    fn npc_encounters_aggregate_across_phases() {
        let enc = npc_encounter(
            "BP_SpawnDescriptions",
            vec![
                vec![npc(3, "PU_Soldier", "Pyro_Outlaw", false, false)],
                vec![npc(2, "PU_sniper", "Pyro_Outlaw", false, false)],
                vec![npc(1, "PU_Soldier", "Pyro_Outlaw", false, false)],
                vec![npc(1, "PU_Juggernaut", "Pyro_Outlaw", false, true)],
                vec![npc(2, "Light", "Civilians", true, false)],
            ],
        );
        let out = encounter_block(&[&enc], "Encounters", &[], true).unwrap();
        // Friendlies don't count toward the heading.
        assert!(out.contains("Encounters · 7 NPCs"), "{out}");
        // Pure boilerplate label → the kind fallback.
        assert!(out.contains(">NPCs<"), "{out}");
        assert!(out.contains("- 4x Soldier (Pyro Outlaw)"), "{out}");
        assert!(out.contains("- 2x Sniper (Pyro Outlaw)"), "{out}");
        assert!(
            out.contains("- 1x Juggernaut (Pyro Outlaw) · target"),
            "{out}"
        );
        assert!(out.contains("- 2x Light (Civilians) · friendly"), "{out}");
    }

    #[test]
    fn target_mark_drops_when_every_enemy_is_a_target() {
        let enc = npc_encounter(
            "Property_Wave1Enemys",
            vec![vec![
                npc(4, "PU_Soldier", "Ninetails", false, true),
                npc(2, "PU_CQC", "Ninetails", false, true),
                npc(2, "Medium", "PrivateSecurity", true, false),
            ]],
        );
        let out = encounter_block(&[&enc], "Encounters", &[], true).unwrap();
        assert!(out.contains(">Wave 1 Enemys<"), "{out}");
        assert!(out.contains("- 4x Soldier (Ninetails)"), "{out}");
        assert!(!out.contains("target"), "{out}");
    }

    #[test]
    fn npc_kind_prettifies_archetypes() {
        assert_eq!(npc_kind("PU_Soldier_Elite"), "Soldier Elite");
        assert_eq!(npc_kind("PU_sniper"), "Sniper");
        assert_eq!(npc_kind("Heavy"), "Heavy");
    }

    #[test]
    fn variants_list_only_their_own_encounters() {
        let escort = encounter("Escort_BP", "Cutlass", "Civilians");
        let mut a = base_entry();
        a.encounters = vec![escort.clone(), encounter("Targets_BP", "Scythe", "Vanduul")];
        let mut b = base_entry();
        b.encounters = vec![escort, encounter("Targets_BP", "Gladiator", "Criminal")];
        let facts = PoolFacts::build(&[&a, &b]);
        let opts = DescOptions {
            blueprint_list: true,
            mission_info: true,
            ship_encounters: true,
            cargo_info: true,
            region_info: true,
            owned_mode: OwnedMode::Off,
        };
        let out = render_description(&facts, &[], opts, None);
        assert!(out.contains("<EM4>Encounters · 1 ship</EM4>"), "{out}");
        assert_eq!(out.matches("Cutlass").count(), 1, "shared once: {out}");
        assert!(out.contains("Variants (2)"), "{out}");
        assert!(out.contains("Scythe (Vanduul)"), "{out}");
        assert!(out.contains("Gladiator (Criminal)"), "{out}");
    }

    #[test]
    fn uec_line_prefers_fixed_then_estimate() {
        let mut m = base_entry();
        m.payout.fixed = Some(1234);
        assert_eq!(uec_line(&m).as_deref(), Some("UEC: 1,234"));
        m.payout.fixed = None;
        m.payout.calculated = true;
        m.payout.estimate = Some(521_250);
        assert_eq!(uec_line(&m).as_deref(), Some("UEC: 521,250"));
        m.payout.estimate = None;
        assert_eq!(uec_line(&m), None);
    }

    #[test]
    fn minutes_format() {
        assert_eq!(format_minutes(30.0), "30min");
        assert_eq!(format_minutes(1.5), "2min");
        assert_eq!(format_minutes(0.5), "30s");
        assert_eq!(format_minutes(0.001), "<1s");
        assert_eq!(format_minutes(2880.0), "48h");
        assert_eq!(format_minutes(90.0), "90min");
        assert_eq!(format_minutes(125.0), "125min");
    }

    #[test]
    fn pretty_identifier_shapes() {
        assert_eq!(pretty_identifier("MissionTargets"), "Mission Targets");
        assert_eq!(pretty_identifier("Wave1"), "Wave 1");
        assert_eq!(pretty_identifier("EnemyNPCs"), "Enemy NPCs");
        assert_eq!(pretty_identifier("LargeCombatShip"), "Large Combat Ship");
        assert_eq!(pretty_identifier("ShipToDefend_BP"), "Ship To Defend");
        assert_eq!(pretty_identifier("BP_Hostile"), "Hostile");
        assert_eq!(pretty_identifier("Salvage_Wave_2"), "Salvage Wave 2");
        assert_eq!(pretty_identifier(""), "");
        assert_eq!(pretty_identifier("_BP"), "");
    }

    #[test]
    fn encounter_labels_drop_spawn_boilerplate() {
        assert_eq!(encounter_label("Scouts_SpawnDescriptions"), "Scouts");
        assert_eq!(encounter_label("SalvageSpawnDescription"), "Salvage");
        assert_eq!(encounter_label("MissionTargets"), "Mission Targets");
        assert_eq!(encounter_label("BP_SpawnDescriptions"), "");
        assert_eq!(encounter_label("Property_Defenders"), "Defenders");
        assert_eq!(
            encounter_label("BP_SpawnDescription_EliminateSpecific"),
            "Eliminate Specific"
        );
    }

    #[test]
    fn collapse_variants_shapes() {
        let single = vec!["Avenger Stalker".to_string()];
        assert_eq!(collapse_variants(&single), vec!["Avenger Stalker"]);
        let multi = vec!["Avenger Stalker".to_string(), "Avenger Warlock".to_string()];
        assert_eq!(collapse_variants(&multi), vec!["Avenger"]);
    }

    #[test]
    fn debug_name_hints() {
        assert_eq!(
            parse_debug_name_hints("CFP_Pyro_RegionA_E_FaunaCave_MissingPerson"),
            Some("Pyro Region A · Easy".to_string())
        );
        assert_eq!(
            parse_debug_name_hints("Vaughn_Stanton1_Assassination_VeryEasy"),
            Some("Stanton 1 · Very Easy".to_string())
        );
        assert_eq!(
            parse_debug_name_hints("RedWind_Nyx_Medium_RecoverCargo"),
            Some("Nyx · Medium".to_string())
        );
        assert_eq!(parse_debug_name_hints("SomeWeirdNameWithNoTokens"), None);
        assert_eq!(parse_debug_name_hints(""), None);
    }

    #[test]
    fn compose_count_and_pool_shapes() {
        assert_eq!(
            compose_count_and_pool(3, 3, &["Cutlass".into(), "Sabre".into()]),
            "3x Cutlass, Sabre"
        );
        assert_eq!(
            compose_count_and_pool(1, 3, &["Scythe".into()]),
            "1-3x Scythe"
        );
        assert_eq!(compose_count_and_pool(2, 2, &[]), "2 ships");
        assert_eq!(compose_count_and_pool(1, 1, &[]), "1 ship");
    }

    fn bp_entry(guid: &str, name: &str) -> svc_data::BpPoolEntry {
        svc_data::BpPoolEntry {
            blueprint_record_guid: guid.into(),
            name: Some(name.into()),
            weight: 1.0,
        }
    }

    fn owned_set(guids: &[&str]) -> crate::OwnedSet {
        guids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn blueprint_block_marks_owned_and_summarizes() {
        let pool = BpPoolReward {
            pool_name: "P".into(),
            chance: 1.0,
            blueprints: vec![
                bp_entry("g1", "Alpha"),
                bp_entry("g2", "Beta"),
                bp_entry("g3", "Gamma"),
            ],
        };
        let owned = owned_set(&["g1", "g3"]);

        let mark = blueprint_block(&pool, Some(&owned), OwnedMode::Mark);
        assert!(mark.contains("2/3 owned"), "{mark}");
        assert!(mark.contains(&format!("{OWNED_MARK} Alpha")), "{mark}");
        assert!(mark.contains(&format!("{OWNED_MARK} Gamma")), "{mark}");
        assert!(mark.contains("- Beta"), "{mark}");

        let hide = blueprint_block(&pool, Some(&owned), OwnedMode::Hide);
        assert!(hide.contains("2/3 owned"), "{hide}");
        assert!(!hide.contains("Alpha"), "owned hidden: {hide}");
        assert!(hide.contains("- Beta"), "{hide}");

        // No owned set → no marks, no summary (undecorated default).
        let none = blueprint_block(&pool, None, OwnedMode::Mark);
        assert!(!none.contains("owned"), "{none}");
        assert!(none.contains("- Alpha"), "{none}");
    }

    #[test]
    fn owned_complete_gates_the_title_tag() {
        let mut a = base_entry();
        a.blueprint_rewards = vec![BpPoolReward {
            pool_name: "P".into(),
            chance: 1.0,
            blueprints: vec![bp_entry("g1", "Alpha"), bp_entry("g2", "Beta")],
        }];
        let members = [&a];

        // Owns both → owned-complete.
        assert!(pool_all_owned(&members, &owned_set(&["g1", "g2"])));
        // Owns one → not complete.
        assert!(!pool_all_owned(&members, &owned_set(&["g1"])));

        let facts = PoolFacts::build(&members);
        // Match the bracketed tag, not the bare mark: `x` is a letter and
        // could appear in other tag text.
        let owned_tag = bracket(OWNED_MARK);
        let tags = render_title_tags(&facts, OPTS, true);
        assert!(
            tags.contains(&owned_tag),
            "owned-complete tag present: {tags}"
        );
        // Gate off → no owned tag even when complete.
        let off = TitleOptions {
            owned_tag: false,
            ..OPTS
        };
        assert!(!render_title_tags(&facts, off, true).contains(&owned_tag));
    }
}
