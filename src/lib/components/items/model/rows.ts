// A kind as rows (decision 'My Items opens on the few you care about, and one
// field is its only scope line', item 4; UX board sketch 16): one row per
// model, the variants you hold named in it, so 17 pistols read as 6 rows.
// What you care about (crafted, or a weapon kitted beyond stock) keeps a row
// of its own, first. Pure.

import { categoryOf, isKitted } from "./classify";
import type { ItemNode } from "./tree";

/** `Arclight "Glimmer" Pistol` → model `Arclight Pistol`, variant `Glimmer`;
 *  a name without a quoted part is its own model, variant null. */
export function modelOf(name: string): { model: string; variant: string | null } {
  const m = name.match(/["“]([^"”]+)["”]/);
  if (!m) return { model: name, variant: null };
  const model = name
    .replace(m[0], " ")
    .replace(/[\s ]+/g, " ")
    .trim();
  return { model, variant: m[1].trim() };
}

/** Crafted, or a weapon with parts beyond its stock ones. */
export const isCared = (n: ItemNode) => !!n.item.crafted || (categoryOf(n) === "Weapons" && isKitted(n));

export interface ModelRow {
  key: string;
  label: string;
  /** The first node: what a click selects. */
  node: ItemNode;
  nodes: ItemNode[];
  /** Units (stacks counted). */
  n: number;
  /** The variants held, `plain` for the unnamed one, most first. */
  variants: { label: string; n: number }[];
  /** Crafted or kitted: shown first, with its details. */
  cared: boolean;
}

export function modelRows(list: ItemNode[]): ModelRow[] {
  const cared: ModelRow[] = [];
  const m = new Map<string, ItemNode[]>();
  for (const x of list) {
    if (isCared(x)) {
      cared.push({ key: `cared:${x.id}`, label: x.name, node: x, nodes: [x], n: x.item.stack, variants: [], cared: true });
      continue;
    }
    const k = `${categoryOf(x)}|${modelOf(x.name).model}`;
    const l = m.get(k);
    if (l) l.push(x);
    else m.set(k, [x]);
  }
  const rows = [...m.entries()].map(([key, nodes]) => {
    const v = new Map<string, number>();
    for (const x of nodes) {
      const label = modelOf(x.name).variant ?? "plain";
      v.set(label, (v.get(label) ?? 0) + x.item.stack);
    }
    const variants = [...v.entries()]
      .map(([label, n]) => ({ label, n }))
      .sort((a, b) => Number(a.label === "plain") - Number(b.label === "plain") || a.label.localeCompare(b.label));
    return {
      key,
      label: modelOf(nodes[0].name).model,
      node: nodes[0],
      nodes,
      n: nodes.reduce((s, x) => s + x.item.stack, 0),
      variants,
      cared: false,
    };
  });
  const byLabel = (a: ModelRow, b: ModelRow) => a.label.localeCompare(b.label);
  return [...cared.sort(byLabel), ...rows.sort(byLabel)];
}

/** "Executive Edition · Glimmer · plain ×2": what a row holds, in words. A
 *  row that is all one plain kind says nothing. */
export function variantsText(r: ModelRow): string {
  if (r.variants.length === 1 && r.variants[0].label === "plain") return "";
  return r.variants.map((v) => (v.n > 1 ? `${v.label} ×${v.n}` : v.label)).join(" · ");
}
