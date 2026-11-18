// Contracts catalog: what one mission row shows, as pure functions over the
// cooked `MissionEntryView` (frontend rule 1: the data lives in the catalog
// store; this only shapes it). Each column of the row is a value over its
// caption, so every helper returns that pair. Design: Firefly note
// 'Contracts design direction: verdicts on the mock passes', pass 2.

import type { BpPoolRewardView, MissionEntryView, RepRequirementView, RepRowView } from "$lib/bindings";

/** A row column: the value, and the caption under it (unit, source or detail). */
export interface Cell {
  value: string;
  sub: string;
}

const fmt = (n: number) => n.toLocaleString("en-US");

/** The display name, falling back to the record's debug name. */
export const missionTitle = (m: MissionEntryView) => m.title ?? m.debug_name;

/**
 * The rank requirement on the mission's own faction (the one the row's ladder
 * draws). Falls back to the first inclusive requirement for missions whose
 * gate sits on another faction only.
 */
export function ownRequirement(m: MissionEntryView): RepRequirementView | null {
  const inclusive = m.rep_required.filter((r) => !r.exclude);
  const own = inclusive.filter((r) => r.faction != null && r.faction === m.faction?.name);
  // Several requirements on one faction must all hold: the window is their
  // intersection, so the strictest lower bound wins.
  const pick = (own.length ? own : inclusive).toSorted((a, b) => (b.min_rank_index ?? 0) - (a.min_rank_index ?? 0));
  return pick[0] ?? null;
}

/** The ladder a requirement is measured on, and the window inside it. */
export interface Ladder {
  /** Rank names, lowest first. */
  ranks: string[];
  /** Reputation each rank begins at. */
  reps: number[];
  /** Each rank's standing GUID (what the reputation service reports). */
  ids: string[];
  /** Inclusive indices of the mission's window. */
  lo: number;
  hi: number;
}

export function ladderOf(req: RepRequirementView): Ladder | null {
  if (req.ladder.length === 0) return null;
  const ranks = req.ladder.map((r, i) => r.name ?? `Rank ${i}`);
  const at = (name: string | null, index: number | null, fallback: number) => {
    const byName = name != null ? ranks.indexOf(name) : -1;
    if (byName >= 0) return byName;
    if (index != null && index >= 0 && index < ranks.length) return index;
    return fallback;
  };
  const lo = at(req.min_rank, req.min_rank_index, 0);
  const hi = Math.max(lo, at(req.max_rank, req.max_rank_index, ranks.length - 1));
  return { ranks, reps: req.ladder.map((r) => r.min_reputation), ids: req.ladder.map((r) => r.id), lo, hi };
}

/** The window in words: "Jr. Contractor or higher", "Neutral only". */
export function windowText(l: Ladder): string {
  const lo = l.ranks[l.lo];
  if (l.lo === l.hi) return `${lo} only`;
  if (l.hi === l.ranks.length - 1) return l.lo === 0 ? "Any rank" : `${lo} or higher`;
  return `${lo} to ${l.ranks[l.hi]}`;
}

/**
 * A requirement's rank range in words, from its names alone (an excluded
 * faction's ladder may not be cooked): "Neutral or higher" when the range
 * reaches that faction's top, else "Neutral to Exalted".
 */
export function rangeText(r: RepRequirementView): string {
  const l = ladderOf(r);
  if (l) return windowText(l).replace(/^Any rank$/, "any rank");
  if (r.min_rank && r.max_rank) return r.min_rank === r.max_rank ? r.min_rank : `${r.min_rank} to ${r.max_rank}`;
  return r.min_rank ? `${r.min_rank} or higher` : "any rank";
}

/**
 * What the mission needs beyond its own faction's rank, shown after the
 * category: another faction's rank, or missions to finish first.
 */
export function needsNote(m: MissionEntryView, rows: RepRowView[] = [], completed: Set<string> = new Set()): string | null {
  const own = ownRequirement(m);
  // With your standing read, a requirement you already meet says nothing.
  const met = (r: RepRequirementView) => {
    const l = ladderOf(r);
    const row = rowFor(r, rows);
    if (!l || !row) return false;
    const i = rankIndex(l, row);
    return i >= l.lo && i <= l.hi;
  };
  const other = m.rep_required.find(
    (r) => !r.exclude && r.faction !== own?.faction && r.faction !== m.faction?.name && !met(r),
  );
  if (other) return `needs ${other.faction ?? "another faction"}${other.min_rank ? ` ${other.min_rank}` : ""}`;
  // No row for that faction is not a rank: the note stays until a row says it does not apply.
  const closed = m.rep_required.find((r) => r.exclude && (!rowFor(r, rows) || met(r)));
  if (closed) return `closed at ${closed.faction ?? "a faction"} ${rangeText(closed)}`;
  const [first, ...rest] = stepsLeft(m, completed);
  if (first) {
    const name = first.title ?? "an earlier mission";
    return rest.length ? `after ${name} and ${rest.length} more` : `after ${name}`;
  }
  return null;
}

/** Where it is offered: the region over its system and place count. */
export function whereCell(m: MissionEntryView): Cell {
  const regions = m.locations;
  if (regions.length === 0) return { value: "—", sub: "no place in the data" };
  const places = regions.reduce((n, r) => n + r.places.length, 0);
  const placesText = `${places} place${places === 1 ? "" : "s"}`;
  if (regions.length === 1) {
    const r = regions[0];
    return { value: r.name || r.system, sub: r.name ? `${r.system}, ${placesText}` : placesText };
  }
  const systems = [...new Set(regions.map((r) => r.system))];
  if (systems.length === 1) return { value: `${regions.length} regions`, sub: `${systems[0]}, ${placesText}` };
  return { value: `${systems.length} systems`, sub: systems.join(", ") };
}

/** aUEC over the mission's own faction rep. */
export function paysCell(m: MissionEntryView): Cell {
  const uec = m.payout.fixed ?? m.payout.estimate;
  const value = uec != null ? fmt(uec) : m.payout.calculated ? "calculated" : "—";
  const own = m.reputation.find((r) => r.faction_guid === m.faction?.guid) ?? m.reputation[0];
  const sub = !own ? "no rep" : own.amount == null ? "rep, calculated" : `${own.amount >= 0 ? "+" : ""}${fmt(own.amount)} rep`;
  return { value, sub };
}

/** The game's time budget. Your own times join in a later slice. */
export function timeCell(m: MissionEntryView): Cell {
  const t = m.payout.time_to_complete;
  if (!(t > 0)) return { value: "—", sub: "no estimate" };
  return { value: `${Math.round(t)} min`, sub: "game estimate" };
}

const poolSize = (p: BpPoolRewardView) => p.blueprints.length;

/**
 * The blueprint column: the pool size over what it means for you. `owns` is
 * the owned-blueprint test; `known` says whether the owned set was ever read
 * (an unread set says nothing about what is new).
 */
export function blueprintCell(m: MissionEntryView, owns: (guid: string) => boolean, known: boolean): Cell {
  const pools = m.blueprint_rewards;
  if (pools.length === 0) return { value: "none", sub: "" };
  if (pools.length > 1) {
    const total = pools.reduce((n, p) => n + poolSize(p), 0);
    return { value: `${pools.length} pools`, sub: `${total} blueprints` };
  }
  const p = pools[0];
  const value = `${poolSize(p)} in pool`;
  if (known) {
    const fresh = p.blueprints.filter((b) => !owns(b.blueprint_record_guid)).length;
    return { value, sub: fresh === 0 ? "all yours" : `${fresh} new to you` };
  }
  return { value, sub: p.chance >= 1 ? "1 per run" : `${Math.round(p.chance * 100)}% per run` };
}

/** The chance one run grants a given blueprint, across all of a mission's pools. */
export function blueprintChance(m: MissionEntryView, guid: string): number {
  let miss = 1;
  for (const p of m.blueprint_rewards) {
    const total = p.blueprints.reduce((n, b) => n + b.weight, 0);
    const entry = p.blueprints.find((b) => b.blueprint_record_guid === guid);
    if (entry && total > 0) miss *= 1 - p.chance * (entry.weight / total);
  }
  return 1 - miss;
}

/** Runs on average, and runs until the chance of at least one drop passes 90%. */
export function runsFor(chance: number): { average: number; ninety: number } | null {
  if (!(chance > 0)) return null;
  if (chance >= 1) return { average: 1, ninety: 1 };
  // The epsilon keeps float noise (1 - 0.8 = 0.19999…) from adding a run.
  const up = (x: number) => Math.ceil(x - 1e-9);
  return { average: up(1 / chance), ninety: up(Math.log(0.1) / Math.log(1 - chance)) };
}

/** "1 in 8" for a chance, rounded to the nearest whole "in". */
export function oddsText(chance: number): string {
  if (!(chance > 0)) return "—";
  if (chance >= 1) return "every run";
  return `1 in ${Math.round(1 / chance)}`;
}

/** The blueprint's display name, from any pool that lists it. */
export function blueprintName(ms: MissionEntryView[], guid: string): string | null {
  for (const m of ms)
    for (const p of m.blueprint_rewards)
      for (const b of p.blueprints) if (b.blueprint_record_guid === guid) return b.name ?? null;
  return null;
}

/**
 * Whether a mission is open at a rank, by the rank's name on the mission's
 * own ladder. No rank requirement means open at any rank; a ladder without
 * that rank name (another faction's naming) is not open there.
 */
export function opensAt(ladder: Ladder | null, rank: string): boolean {
  if (!ladder) return true;
  const i = ladder.ranks.indexOf(rank);
  return i >= 0 && i >= ladder.lo && i <= ladder.hi;
}

/** The most common ladder's rank names, for the rank picker. */
export function commonLadder(ladders: (Ladder | null)[]): string[] {
  const counts = new Map<string, { ranks: string[]; n: number }>();
  for (const l of ladders) {
    if (!l) continue;
    const key = l.ranks.join("\u0000");
    const e = counts.get(key) ?? { ranks: l.ranks, n: 0 };
    e.n += 1;
    counts.set(key, e);
  }
  return [...counts.values()].sort((a, b) => b.n - a.n)[0]?.ranks ?? [];
}

/** Whether any pool holds a blueprint you do not own yet. */
export function hasNewBlueprint(m: MissionEntryView, owns: (guid: string) => boolean): boolean {
  return m.blueprint_rewards.some((p) => p.blueprints.some((b) => !owns(b.blueprint_record_guid)));
}

// ── Your standing (slice 2) ───────────────────────────────────────────────

/** Your reputation row for a requirement: same org, same ladder. */
export function rowFor(req: RepRequirementView, rows: RepRowView[]): RepRowView | null {
  if (!req.faction_geid) return null;
  return rows.find((r) => r.org_geid === req.faction_geid && (!req.scope || r.scope_id === req.scope)) ?? null;
}

/** Where a row sits on a ladder: by its standing id, else by its score. */
export function rankIndex(l: Ladder, row: RepRowView): number {
  const byId = row.standing_id ? l.ids.indexOf(row.standing_id) : -1;
  if (byId >= 0) return byId;
  let at = -1;
  l.reps.forEach((min, i) => {
    if (row.score >= min) at = i;
  });
  return at;
}

/**
 * What a mission is to you, given your rows and completed contracts:
 * - open: your rank fits and nothing else stands in the way;
 * - step: another faction's rank or an earlier mission first;
 * - higher: your rank is below the window;
 * - unknown: no row for its faction (no row is NOT Neutral: shown, never hidden);
 * - closed: a faction rank you hold shuts it (an exclusion);
 * - outgrown: your rank is above the window;
 * - done: once only and already completed.
 */
export type Standing = "open" | "step" | "higher" | "unknown" | "closed" | "outgrown" | "done";

export function standingOf(m: MissionEntryView, rows: RepRowView[], completed: Set<string>): Standing {
  if (m.once_only && m.contracts.some((c) => completed.has(c))) return "done";
  const inWindow = (r: RepRequirementView): boolean | null => {
    const l = ladderOf(r);
    const row = rowFor(r, rows);
    if (!l || !row) return null;
    const i = rankIndex(l, row);
    return i >= l.lo && i <= l.hi;
  };
  for (const r of m.rep_required.filter((x) => x.exclude)) if (inWindow(r) === true) return "closed";
  const own = ownRequirement(m);
  if (own) {
    const l = ladderOf(own);
    const row = rowFor(own, rows);
    if (!row) return "unknown";
    if (l) {
      const i = rankIndex(l, row);
      if (i < l.lo) return "higher";
      if (i > l.hi) return "outgrown";
    }
  }
  const others = m.rep_required.filter((r) => !r.exclude && r.faction !== own?.faction);
  if (others.some((r) => inWindow(r) !== true)) return "step";
  if (stepsLeft(m, completed).length > 0) return "step";
  return "open";
}

/** Chain steps not yet completed, by their contract names. */
export function stepsLeft(m: MissionEntryView, completed: Set<string>) {
  return m.chain_required.filter((c) => !c.contracts.some((n) => completed.has(n)));
}

/** Group order and headings in the catalog. */
export const STANDING_GROUPS: { id: Standing; label: string }[] = [
  { id: "open", label: "In your rank" },
  { id: "step", label: "A step first" },
  { id: "higher", label: "Higher rank" },
  { id: "unknown", label: "Not in your standing" },
  { id: "closed", label: "Closed to you" },
  { id: "outgrown", label: "Outgrown" },
  { id: "done", label: "Done" },
];

// ── The Standing tab ──────────────────────────────────────────────────────

/** One faction on the Standing tab: its ladder, your place on it, its missions per rank. */
export interface FactionSummary {
  name: string;
  geid: string | null;
  ladder: Ladder | null;
  /** Your rank on the ladder, -1 when no row. */
  you: number;
  /** Your score, null when no row. */
  score: number | null;
  /** Rep the next rank starts at, null at the top or without a ladder. */
  next: number | null;
  missions: number;
  withBlueprint: number;
  /** Missions open at your rank (window holds it), null without a row. */
  open: number | null;
  /** Per rank: missions whose window starts there, how many give a blueprint, median aUEC. */
  perRank: { missions: number; blueprints: number; median: number | null }[];
}

const median = (xs: number[]) => {
  if (xs.length === 0) return null;
  const s = xs.toSorted((a, b) => a - b);
  return s[Math.floor(s.length / 2)];
};

/**
 * Summaries for every faction that gives missions, yours first (by score),
 * then by mission count. Event payouts over 5M aUEC stay out of the medians.
 */
export function factionSummaries(ms: MissionEntryView[], rows: RepRowView[]): FactionSummary[] {
  const by = new Map<string, MissionEntryView[]>();
  for (const m of ms) {
    const name = m.faction?.name;
    if (!name) continue;
    const list = by.get(name) ?? [];
    list.push(m);
    by.set(name, list);
  }
  const out: FactionSummary[] = [];
  for (const [name, list] of by) {
    const reqs = list.map(ownRequirement).filter((r): r is RepRequirementView => r != null && r.faction === name);
    const ranks = commonLadder(reqs.map(ladderOf));
    const sample = reqs.find((r) => ladderOf(r)?.ranks.join() === ranks.join());
    const ladder = sample ? ladderOf(sample) : null;
    const row = sample ? rowFor(sample, rows) : null;
    const you = ladder && row ? rankIndex(ladder, row) : -1;
    const perRank = (ladder?.ranks ?? []).map(() => ({ missions: 0, blueprints: 0, pays: [] as number[] }));
    let open = 0;
    for (const m of list) {
      const r = ownRequirement(m);
      const l = r && r.faction === name ? ladderOf(r) : null;
      const at = l && ladder && l.ranks.join() === ladder.ranks.join() ? l.lo : ladder ? 0 : -1;
      if (at >= 0 && perRank[at]) {
        perRank[at].missions += 1;
        if (m.blueprint_rewards.length) perRank[at].blueprints += 1;
        const pay = m.payout.fixed ?? m.payout.estimate;
        if (pay != null && pay < 5_000_000) perRank[at].pays.push(pay);
      }
      if (row && (!l || (you >= l.lo && you <= l.hi))) open += 1;
    }
    out.push({
      name,
      geid: list[0].faction?.geid ?? null,
      ladder,
      you,
      score: row?.score ?? null,
      next: ladder && you >= 0 && you + 1 < ladder.reps.length ? ladder.reps[you + 1] : null,
      missions: list.length,
      withBlueprint: list.filter((m) => m.blueprint_rewards.length > 0).length,
      open: row ? open : null,
      perRank: perRank.map((p) => ({ missions: p.missions, blueprints: p.blueprints, median: median(p.pays) })),
    });
  }
  return out.sort((a, b) => (b.score != null ? 1 : 0) - (a.score != null ? 1 : 0) || b.missions - a.missions);
}
