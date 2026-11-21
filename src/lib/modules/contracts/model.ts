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
/** A title the locale really has: some missions ship a placeholder
 *  (`<= UNINITIALIZED =>`) as their title. */
const realTitle = (t: string | null) => (t && t.trim() && !/PLACEHOLDER|UNINITIALIZED/.test(t) ? t : null);
export const missionTitle = (m: MissionEntryView) => realTitle(m.title) ?? m.debug_name;

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
export function needsNote(
  m: MissionEntryView,
  rows: RepRowView[] = [],
  completed: Set<string> = new Set(),
  tags: Map<string, number> = new Map(),
): string | null {
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
  const [gate] = gatesLeft(m, completed, tags);
  if (gate) {
    const [first, ...rest] = gate.gate.missions;
    const name = first?.title ?? "an earlier mission";
    const runs = gate.gate.need === 1 ? "" : `${gate.left} ${gate.have > 0 ? "more " : ""}runs of `;
    return rest.length
      ? `after ${runs}${name} or ${rest.length === 1 ? "1 other" : `${rest.length} others`}`
      : `after ${runs}${name}`;
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

/** Your average over the runs the Session Log measured (`yours`, minutes by
 *  contract record name), else the game's time budget; the caption names
 *  which (Plans' time order: yours before CIG's). */
export function timeCell(m: MissionEntryView, yours?: Partial<Record<string, number>> | null): Cell {
  const mine = m.contracts.map((c) => yours?.[c]).filter((x): x is number => x != null && x > 0);
  if (mine.length > 0) {
    const avg = mine.reduce((a, b) => a + b, 0) / mine.length;
    return { value: `${Math.max(1, Math.round(avg))} min`, sub: "your time" };
  }
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

/** A pool's draw as the game runs it: a run that draws (at `chance`) gives
 *  one blueprint of the pool you do not own yet, so the pool is used up in
 *  `left` drawing runs, and every mission on the same pool gives the same. */
export type PoolDraw = { chance: number; left: number };

/** The draw from the mission's pool that holds blueprint `guid`: the
 *  blueprints in it you lack (`owns`; `guid` always counts). Of several
 *  pools holding it, the one used up soonest; null when none holds it. */
export function poolDraw(m: MissionEntryView, guid: string, owns: (g: string) => boolean = () => false): PoolDraw | null {
  let best: PoolDraw | null = null;
  for (const p of m.blueprint_rewards) {
    if (!p.blueprints.some((b) => b.blueprint_record_guid === guid)) continue;
    const left = p.blueprints.filter((b) => b.blueprint_record_guid === guid || !owns(b.blueprint_record_guid)).length;
    const d = { chance: Math.min(Math.max(p.chance, 0), 1), left };
    if (!best || (runsAll(d) ?? Infinity) < (runsAll(best) ?? Infinity)) best = d;
  }
  return best;
}

/** Runs until the pool has given all it holds for you (on average when a
 *  run may not draw); null when it never draws. */
export function runsAll(d: PoolDraw | null): number | null {
  if (!d || !(d.chance > 0) || d.left <= 0) return null;
  return Math.ceil(d.left / d.chance - 1e-9);
}

/** The chance one run gives blueprint `guid`: one of the pool's blueprints
 *  you lack, evenly. 0 when no pool holds it. */
export function blueprintChance(m: MissionEntryView, guid: string, owns?: (g: string) => boolean): number {
  const d = poolDraw(m, guid, owns);
  return d && d.left > 0 ? d.chance / d.left : 0;
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

export function standingOf(
  m: MissionEntryView,
  rows: RepRowView[],
  completed: Set<string>,
  tags: Map<string, number> = new Map(),
): Standing {
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
  if (gatesLeft(m, completed, tags).length > 0) return "step";
  return "open";
}

type Gate = MissionEntryView["chain_gates"][number];

/** Where you are on one chain gate: the points you have toward it (the
 * wallet's counters for its tags, or the missions you completed where it
 * keeps none) and what is left. */
export function gateProgress(g: Gate, completed: Set<string>, tags: Map<string, number> = new Map()) {
  const counted = g.tags.reduce((n, t) => n + (tags.get(t.toLowerCase()) ?? 0), 0);
  const ways = g.missions.filter((c) => c.contracts.some((n) => completed.has(n))).length;
  const have = Math.max(counted, ways);
  return { gate: g, have, left: Math.max(0, g.need - have) };
}

/** The chain gates still shut, in order. Every gate must open, each by its
 * points from any of its missions (each grants the tag it counts). */
export function gatesLeft(m: MissionEntryView, completed: Set<string>, tags: Map<string, number> = new Map()) {
  return m.chain_gates.map((g) => gateProgress(g, completed, tags)).filter((p) => p.left > 0);
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
  /** The ladder's reputation scope GUID (with the GEID, how a rank is planned). */
  scope: string | null;
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
  /** Most rep with this faction one run of a mission open to you gives, null without a row or a fixed amount. */
  bestRep: number | null;
  /** Kinds of work (mission categories), most missions first; `open` = open to you. */
  kinds: { name: string; missions: number; open: number }[];
  /** Missions you completed a contract of (the token wallet; plain repeatables leave none). */
  done: number;
  /** Blueprint record GUIDs the missions open to you can give. */
  openBlueprints: string[];
}

/** Rep with the mission's own faction one run gives, null when calculated or none. */
export function ownRep(m: MissionEntryView): number | null {
  return m.reputation.find((r) => r.faction_guid === m.faction?.guid)?.amount ?? null;
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
export function factionSummaries(
  ms: MissionEntryView[],
  rows: RepRowView[],
  completed: Set<string> = new Set(),
): FactionSummary[] {
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
    let bestRep: number | null = null;
    let done = 0;
    const kinds = new Map<string, { name: string; missions: number; open: number }>();
    const openBlueprints = new Set<string>();
    for (const m of list) {
      const kindName = m.category?.name ?? "Other";
      const kind = kinds.get(kindName) ?? { name: kindName, missions: 0, open: 0 };
      kind.missions += 1;
      kinds.set(kindName, kind);
      if (m.contracts.some((c) => completed.has(c))) done += 1;
      const r = ownRequirement(m);
      const l = r && r.faction === name ? ladderOf(r) : null;
      const at = l && ladder && l.ranks.join() === ladder.ranks.join() ? l.lo : ladder ? 0 : -1;
      if (at >= 0 && perRank[at]) {
        perRank[at].missions += 1;
        if (m.blueprint_rewards.length) perRank[at].blueprints += 1;
        const pay = m.payout.fixed ?? m.payout.estimate;
        if (pay != null && pay < 5_000_000) perRank[at].pays.push(pay);
      }
      if (row && (!l || (you >= l.lo && you <= l.hi))) {
        open += 1;
        kind.open += 1;
        const rep = ownRep(m);
        if (rep != null && rep > (bestRep ?? 0)) bestRep = rep;
        for (const p of m.blueprint_rewards) for (const b of p.blueprints) openBlueprints.add(b.blueprint_record_guid);
      }
    }
    out.push({
      name,
      geid: sample?.faction_geid ?? list[0].faction?.geid ?? null,
      scope: sample?.scope ?? null,
      ladder,
      you,
      score: row?.score ?? null,
      next: ladder && you >= 0 && you + 1 < ladder.reps.length ? ladder.reps[you + 1] : null,
      missions: list.length,
      withBlueprint: list.filter((m) => m.blueprint_rewards.length > 0).length,
      open: row ? open : null,
      perRank: perRank.map((p) => ({ missions: p.missions, blueprints: p.blueprints, median: median(p.pays) })),
      bestRep,
      kinds: [...kinds.values()].sort((a, b) => b.missions - a.missions || a.name.localeCompare(b.name)),
      done,
      openBlueprints: [...openBlueprints],
    });
  }
  return out.sort((a, b) => (b.score != null ? 1 : 0) - (a.score != null ? 1 : 0) || b.missions - a.missions);
}
