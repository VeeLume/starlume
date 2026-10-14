// Resources in the explorer: stacks folded per material, one unit per
// material (bulk in SCU, hand-gathered pieces counted, never mixed), and
// the quality mix the heat strip draws.

import { isBulk } from "./classify";
import { areaOf, type ItemNode } from "./tree";

/** A stack's amount in its material's unit: SCU for bulk, pieces else. */
export function amountOf(n: ItemNode): number {
  const r = n.item.resource;
  if (!r) return 0;
  return isBulk(n) ? r.scu * n.item.stack : n.item.stack;
}

export function fmtAmount(bulk: boolean, v: number): string {
  if (!bulk) return `${v.toLocaleString("en-US")} pc${v === 1 ? "" : "s"}`;
  if (v > 0 && v < 0.01) return "<0.01 SCU";
  return `${v.toFixed(v >= 100 ? 0 : v >= 10 ? 1 : 2)} SCU`;
}

export const qualityOf = (n: ItemNode) => n.item.resource?.quality ?? 0;
/** The quality where stock sits; the line on the strip. */
export const STOCK_QUALITY = 500;
/** The quality filter's steps. */
export const QUALITY_STEPS = [0, 300, 500, 700, 900];

export interface Material {
  name: string;
  bulk: boolean;
  nodes: ItemNode[];
  total: number;
  /** Boxes for bulk, pieces else. */
  units: number;
  /** Best quality; 0 when the material carries none (creature parts). */
  best: number;
  /** Amount at Q500 or better. */
  good: number;
  /** Area names it sits in ("New Babbage", "On you"). */
  places: Set<string>;
}

/** Resource stacks folded per material; with `min`, only stacks at or
 *  above that quality. */
export function materials(list: ItemNode[], min = 0): Material[] {
  const m = new Map<string, Material>();
  for (const n of list) {
    const r = n.item.resource;
    if (!r || r.quality < min) continue;
    let e = m.get(r.name);
    if (!e) {
      e = { name: r.name, bulk: isBulk(n), nodes: [], total: 0, units: 0, best: 0, good: 0, places: new Set() };
      m.set(r.name, e);
    }
    const a = amountOf(n);
    e.nodes.push(n);
    e.total += a;
    e.units += n.item.stack;
    e.best = Math.max(e.best, r.quality);
    if (r.quality >= STOCK_QUALITY) e.good += a;
    const area = areaOf(n);
    e.places.add(n.item.place.kind === "carried" ? "On you" : (area?.parent?.name ?? n.item.place.name));
  }
  return [...m.values()];
}

/** Amount per quality band of 100 (ten bands on 0–1000). */
export function qualityBands(list: ItemNode[]): number[] {
  const b = Array<number>(10).fill(0);
  for (const n of list) b[Math.min(9, Math.floor(qualityOf(n) / 100))] += amountOf(n);
  return b;
}

/** The heat strip's cells: each band's share of the material, so every row
 *  reads on its own and no scale is needed. */
export function qualityMix(list: ItemNode[]): { amount: number; share: number }[] {
  const bands = qualityBands(list);
  const total = bands.reduce((a, b) => a + b, 0) || 1;
  return bands.map((amount) => ({ amount, share: amount / total }));
}

/** Rated: the material carries a quality at all. */
export const isRated = (list: ItemNode[]) => list.some((n) => qualityOf(n) > 0);
