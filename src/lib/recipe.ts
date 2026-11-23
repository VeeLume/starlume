// What a recipe's materials do at a quality, and what you hold of them:
// the maths Crafting's recipe sheet and Ships' crafted parts share (a
// module never imports another, so it lives here). Pure; mirrors
// sc-crafting's evaluation (Rust: svc_data::craft).

import type { MaterialView, ModifierRangeView, SlotModifierView, StockStackView } from "$lib/bindings";

/** Linear across a band, clamped to it. */
function evalRange(r: ModifierRangeView, q: number): number {
  if (r.endQuality <= r.startQuality) return r.atStart;
  const clamped = Math.min(Math.max(q, r.startQuality), r.endQuality);
  return r.atStart + ((r.atEnd - r.atStart) * (clamped - r.startQuality)) / (r.endQuality - r.startQuality);
}

export interface Effect {
  value: number;
  additive: boolean;
}

/** A modifier at a quality: the band holding it, else the nearest. */
export function evalModifier(ranges: ModifierRangeView[], q: number): Effect | null {
  if (ranges.length === 0) return null;
  const distance = (r: ModifierRangeView) => (q < r.startQuality ? r.startQuality - q : Math.max(0, q - r.endQuality));
  const band = ranges.reduce((best, r) => (distance(r) < distance(best) ? r : best));
  return { value: evalRange(band, q), additive: band.additive };
}

/** A modifier's reading at a quality, the way the game shows it. `up` is
 *  which way it moves the stat; null when it doesn't. */
export function effectText(m: SlotModifierView, q: number): { factor: string; pct: number | null; up: boolean | null } | null {
  const e = evalModifier(m.ranges, q);
  if (!e) return null;
  if (e.additive) return { factor: `${e.value >= 0 ? "+" : ""}${e.value.toFixed(1)}`, pct: null, up: e.value === 0 ? null : e.value > 0 };
  const change = (e.value - 1) * 100;
  return {
    factor: `×${e.value.toFixed(3)}`,
    pct: m.transform === "negatedPercent" ? -change : change,
    up: Math.abs(change) < 0.005 ? null : change > 0,
  };
}

export const fmtPct = (p: number) => `${p > 0 ? "+" : p < 0 ? "−" : "±"}${Math.abs(p).toFixed(2)}%`;

/** The amount a slot takes, in its material's unit. */
export const needOf = (m: MaterialView) => m.scu ?? m.count ?? 0;

/** How much you hold at `quality` or better. */
export function heldAt(stacks: StockStackView[] | undefined, quality: number): number {
  return (stacks ?? []).filter((s) => s.quality >= quality).reduce((a, s) => a + s.amount, 0);
}

/** What you hold of a material at a quality, in a few words for a
 *  material's line on the recipe sheet. */
export function heldShort(stacks: StockStackView[] | undefined, m: MaterialView, quality: number, read: boolean): string {
  if (!read) return "inventory not read";
  if (!stacks?.length) return "none held";
  const bulk = stacks[0].bulk;
  const amount = (v: number) => (bulk ? `${v.toFixed(2)} SCU` : `${Math.floor(v)} pcs`);
  const held = heldAt(stacks, quality);
  if (held >= needOf(m)) return `${amount(held)} held at Q${quality}+`;
  if (held > 0) return `only ${amount(held)} at Q${quality}+`;
  const best = stacks.reduce((a, b) => (b.quality > a.quality ? b : a));
  return `best held Q${best.quality}`;
}

