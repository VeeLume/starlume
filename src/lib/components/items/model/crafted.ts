// Crafted items: the stat deltas their recipe rolled, and the quality the
// UI shows for them.

import type { CraftedView } from "$lib/bindings";

export interface StatDelta {
  name: string;
  /** Percent change, one decimal: `-7.6`. */
  pct: number;
  /** 1 = the better roll, -1 = worse, 0 = unknown which way helps. */
  good: 1 | -1 | 0;
}

/** Stat deltas for display. The three recoil stats move together on every
 *  recipe and read as one; the two temperature limits read as one range. */
export function statDeltas(c: CraftedView): StatDelta[] {
  const out: StatDelta[] = [];
  for (const p of c.properties) {
    const pct = Math.round((p.factor - 1) * 1000) / 10;
    const raw = p.name ?? "Unknown stat";
    const name = raw.startsWith("Recoil") ? "Recoil" : /^(Min|Max) Temp$/.test(raw) ? "Temp. range" : raw;
    const up = p.factor_up_is_better;
    const good: StatDelta["good"] = up == null || pct === 0 ? 0 : p.factor > 1 === up ? 1 : -1;
    if (!out.some((o) => o.name === name && o.pct === pct)) out.push({ name, pct, good });
  }
  return out;
}

/** The data records no quality for a crafted item. The UI shows the
 *  amount-weighted mean of its resource ingredients' qualities, labelled
 *  as that ("avg ingredient Q"), never as a game value. Item ingredients
 *  count in pieces, not µSCU, so they stay out of the weighting unless
 *  there is nothing else. `null` without ingredients. */
export function avgIngredientQuality(c: CraftedView): number | null {
  const res = c.ingredients.filter((i) => i.kind === "resource");
  const use = res.length ? res : c.ingredients;
  const total = use.reduce((s, i) => s + i.amount, 0);
  if (!total) return null;
  return Math.round(use.reduce((s, i) => s + i.quality * i.amount, 0) / total);
}
