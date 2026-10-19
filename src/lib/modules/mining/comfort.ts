// Comfort zones and the two scores (design pass 5): a loadout against every
// rock that spawns, from its comfort solve (one per loadout, per
// distribution: class shares and mass bars counted by class).
//
// Measured on one ore, a rock counts by how much of that ore it holds (the
// chance it has the ore times its share when it does): a Gold rock with a
// trace of Bexalite barely moves Bexalite's figures, so they say how much
// of the ore you'd find sits in rocks you crack cleanly.
//
// The rocks you'd meet are the spawn tables' rock types, each by its share
// of its table, every table counted the same. Everyday is the share of
// those that go easy or manageable; Reach is, of every rock type at every
// table counted the same, how much it can break at all (100 only for a
// loadout that breaks everything that spawns). A comfort zone draws the same
// rocks by mass on a log scale, coloured by the class the loadout has there.

import type { AreaView, ComfortView, PlaceNodeView, PlaceSystemView } from "$lib/bindings";
import { MASS_SCALE, massAt } from "./format";

/** One rock type at one table: what the scores and zones sum over. `ores`:
 *  each ore's expected share of the rock (chance × share when present). */
export type ComfortRow = { dist: string; share: number; ores: Map<string, number>; systems: Set<string> };

/** Which rocks count: one ore's (element id; "" every ore), one system's
 *  ("all" every system). */
export type ComfortFilter = { ore: string; sys: string };

/** Every rollable rock type at every table, with the systems its table
 *  spawns in (from the place tree; a table on two systems counts in both). */
export function comfortRows(areas: AreaView[], systems: PlaceSystemView[]): ComfortRow[] {
  const sysOf = new Map<string, Set<string>>();
  const walk = (nodes: PlaceNodeView[], sys: string) => {
    for (const n of nodes) {
      for (const a of n.areas) {
        const s = sysOf.get(a) ?? new Set<string>();
        s.add(sys);
        sysOf.set(a, s);
      }
      walk(n.children, sys);
    }
  };
  for (const s of systems) walk(s.nodes, s.name);
  return areas.flatMap((a) =>
    a.rocks
      .filter((r) => r.rollable)
      .map((r) => ({
        dist: r.dist,
        share: r.share,
        ores: new Map(r.ores.map((o) => [o.element, o.chance * o.meanShare] as const)),
        systems: sysOf.get(a.id) ?? new Set<string>(),
      })),
  );
}

const counts = (rows: ComfortRow[], f: ComfortFilter) =>
  rows.filter((r) => (!f.ore || r.ores.has(f.ore)) && (f.sys === "all" || r.systems.has(f.sys)));
/** How much of the ore a row holds: 1 without an ore filter. */
const content = (r: ComfortRow, f: ComfortFilter) => (f.ore ? (r.ores.get(f.ore) ?? 0) : 1);

/** Everyday, Reach (fractions), and the class mix of the rocks you'd meet
 *  (too much power, easy, manageable, hard, can't break). `n`: the rows that
 *  counted (none: nothing of that ore in that system). */
export type Scores = { everyday: number; reach: number; mix: number[]; n: number };

export function scoresOf(comfort: ComfortView[], rows: ComfortRow[], f: ComfortFilter): Scores {
  const by = new Map(comfort.map((c) => [c.dist, c] as const));
  let w = 0;
  let good = 0;
  let reach = 0;
  let rw = 0;
  let n = 0;
  const mix = [0, 0, 0, 0, 0];
  for (const r of counts(rows, f)) {
    const c = by.get(r.dist)?.classes;
    if (!c) continue;
    const k = content(r, f);
    w += r.share * k;
    good += r.share * k * (c.easy + c.manageable);
    [c.tooMuchPower, c.easy, c.manageable, c.hard, c.impossible].forEach((x, i) => (mix[i] += r.share * k * x));
    reach += k * (1 - c.impossible);
    rw += k;
    n++;
  }
  return { everyday: w ? good / w : 0, reach: rw ? reach / rw : 0, mix: mix.map((x) => (w ? x / w : 0)), n };
}

/** Log-scale bins across the finder's mass scale. */
export const BINS = 56;

/** The rocks you'd meet by mass: per bin, the weight in each class (too
 *  much power, easy, manageable, hard, can't break). A solve's linear bar
 *  is spread over the log bins it covers. */
export function binsOf(comfort: ComfortView[], rows: ComfortRow[], f: ComfortFilter): number[][] {
  const by = new Map(comfort.map((c) => [c.dist, c] as const));
  const bins = Array.from({ length: BINS }, () => [0, 0, 0, 0, 0]);
  for (const r of counts(rows, f)) {
    const c = by.get(r.dist);
    if (!c) continue;
    const k = content(r, f);
    const total = c.bars.reduce((s, b) => s + b.counts.reduce((x, y) => x + y, 0), 0);
    if (!total) continue;
    for (const b of c.bars) {
      const n = b.counts.reduce((x, y) => x + y, 0);
      if (!n) continue;
      const lo = Math.floor((massAt(Math.max(b.from, MASS_SCALE.min)) / 100) * BINS);
      const hi = Math.min(BINS - 1, Math.floor((massAt(b.to) / 100) * BINS));
      const span = Math.max(1, hi - lo + 1);
      for (let j = lo; j <= hi; j++)
        for (let i = 0; i < 5; i++) bins[j][i] += ((b.counts[i] / total) * r.share * k) / span;
    }
  }
  return bins;
}

/** The tallest bin: one scale for charts drawn side by side (their outline
 *  is the same rocks, so any of them gives it). */
export const topOf = (bins: number[][]) => Math.max(1e-9, ...bins.map((b) => b.reduce((x, y) => x + y, 0)));

/** Class colours in the solve's order, and the order a chart stacks them
 *  (easy at the bottom, can't break on top). */
export const BIN_HUES = ["var(--hue-apricot)", "var(--good)", "var(--warn)", "var(--bad)", "var(--text-faint)"];
export const STACK = [1, 2, 3, 0, 4];
