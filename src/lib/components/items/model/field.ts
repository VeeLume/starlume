// What the scope field offers (decision 'My Items opens on the few you care
// about, and one field is its only scope line', 2026-11-11). Focused and
// empty: where you can go without knowing a word (your gear, the biggest
// places, the kinds here). Typing: kinds, places and things that match,
// counted inside the current place, with hits outside it offered last.
// Pure.

import { CATEGORIES, SUBCATEGORIES, categoryOf, subcategoryOf, type Category } from "./classify";
import { whereLabel } from "./gear";
import { ancestors, type ItemNode, type Node, type PlaceNode, type Tree } from "./tree";

export type Suggestion =
  | { type: "crafted"; label: string; note: string; n: number }
  | { type: "place"; node: Node; label: string; note: string; n: number }
  | { type: "kind"; cat: Category; sub: string; label: string; note: string; n: number }
  | { type: "thing"; node: ItemNode; label: string; note: string; n: number };

export interface SuggestionGroup {
  title: string;
  items: Suggestion[];
}

const sum = (list: ItemNode[]) => list.reduce((s, x) => s + x.item.stack, 0);

/** The kinds a list holds, in the game's order. */
export function kindsIn(list: ItemNode[]): { cat: Category; n: number }[] {
  const m = new Map<Category, number>();
  for (const x of list) m.set(categoryOf(x), (m.get(categoryOf(x)) ?? 0) + x.item.stack);
  return CATEGORIES.filter((c) => m.has(c)).map((cat) => ({ cat, n: m.get(cat)! }));
}

/** "New Babbage · microTech": where a place is, without Everywhere and the
 *  system. */
export function placeNote(n: Node): string {
  return ancestors(n)
    .filter((x) => x !== n && !(x.type === "place" && (x.level === "root" || x.level === "system")))
    .map((x) => x.name)
    .reverse()
    .join(" · ");
}

export interface FieldContext {
  tree: Tree;
  /** The current place's items, every kind (what the kinds count). */
  kindsFrom: ItemNode[];
  /** The current place's items within the kind in the field, if any (what
   *  things count). */
  here: ItemNode[];
  /** Every visible item in the tree, for hits outside the place. */
  all: ItemNode[];
  /** Units below a node under the current filters. */
  count: (n: Node) => number;
  /** The biggest areas, for the empty field. */
  areas: { area: PlaceNode; n: number }[];
  crafted: number;
}

/** What to offer for `text` (trimmed, any case). */
export function suggest(text: string, ctx: FieldContext): SuggestionGroup[] {
  const t = text.trim().toLowerCase();
  const groups: SuggestionGroup[] = [];
  const push = (title: string, items: Suggestion[]) => items.length && groups.push({ title, items });

  if (!t) {
    if (ctx.crafted) push("Your gear", [{ type: "crafted", label: "Crafted", note: "", n: ctx.crafted }]);
    push(
      "Places",
      ctx.areas.slice(0, 5).map((a) => ({ type: "place", node: a.area, label: a.area.name, note: placeNote(a.area), n: a.n })),
    );
    push(
      "Kinds here",
      kindsIn(ctx.kindsFrom).map((k) => ({ type: "kind", cat: k.cat, sub: "", label: k.cat, note: "", n: k.n })),
    );
    return groups;
  }

  // Kinds: a category, or a sub kind under its category.
  const kinds: Suggestion[] = [];
  for (const k of kindsIn(ctx.kindsFrom)) {
    if (k.cat.toLowerCase().includes(t)) kinds.push({ type: "kind", cat: k.cat, sub: "", label: k.cat, note: "", n: k.n });
    const list = ctx.kindsFrom.filter((x) => categoryOf(x) === k.cat);
    for (const s of SUBCATEGORIES[k.cat] ?? []) {
      if (!s.toLowerCase().includes(t)) continue;
      const n = sum(list.filter((x) => subcategoryOf(x, k.cat) === s));
      if (n) kinds.push({ type: "kind", cat: k.cat, sub: s, label: s, note: k.cat, n });
    }
  }
  push("Kinds here", kinds.slice(0, 4));

  // Things, one row per kind of item, most held first.
  const things = (list: ItemNode[]) => {
    const m = new Map<string, { node: ItemNode; n: number; nodes: ItemNode[] }>();
    for (const x of list) {
      if (!x.name.toLowerCase().includes(t)) continue;
      const e = m.get(x.item.kind.guid);
      if (e) {
        e.n += x.item.stack;
        e.nodes.push(x);
      } else m.set(x.item.kind.guid, { node: x, n: x.item.stack, nodes: [x] });
    }
    return [...m.values()].sort((a, b) => b.n - a.n || a.node.name.localeCompare(b.node.name));
  };
  push(
    "Things here",
    things(ctx.here)
      .slice(0, 6)
      .map((e) => ({ type: "thing", node: e.node, label: e.node.name, note: "", n: e.n })),
  );

  // Places by name.
  const places: Suggestion[] = [];
  for (const n of ctx.tree.byId.values()) {
    if (n.type !== "place" || !["body", "station", "area"].includes(n.level)) continue;
    if (!n.name.toLowerCase().includes(t)) continue;
    const c = ctx.count(n);
    if (c) places.push({ type: "place", node: n, label: n.name, note: placeNote(n), n: c });
  }
  push("Places", places.sort((a, b) => b.n - a.n).slice(0, 4));

  // The same things elsewhere: what you hold, wherever it is.
  const inside = new Set(ctx.here);
  push(
    "Elsewhere",
    things(ctx.all.filter((x) => !inside.has(x)))
      .slice(0, 3)
      .map((e) => ({
        type: "thing",
        node: e.node,
        label: e.node.name,
        note: e.nodes.length > 1 ? `${new Set(e.nodes.map(whereLabel)).size} places` : whereLabel(e.node),
        n: e.n,
      })),
  );
  return groups;
}
