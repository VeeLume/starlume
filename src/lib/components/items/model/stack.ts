// Identical items stack into one ×N card: same kind, state and contents
// (for a weapon, the same kit). Crafted copies never stack with uncrafted.

import { kitOf } from "./classify";
import type { ItemNode } from "./tree";

export interface Stack {
  key: string;
  /** The representative (the one a click selects). */
  node: ItemNode;
  n: number;
  nodes: ItemNode[];
}

function contents(n: ItemNode): string {
  if (n.item.kind.item_type === "WeaponPersonal") {
    return kitOf(n)
      .map((k) => k.part.item.kind.guid + k.state)
      .sort()
      .join(",");
  }
  return n.kids
    .map((k) => k.name)
    .sort()
    .join(",");
}

/** Stacks in display order: crafted first, then by name. */
export function stackIdentical(list: ItemNode[]): Stack[] {
  const m = new Map<string, Stack>();
  for (const n of list) {
    const i = n.item;
    const key = [
      i.kind.guid,
      i.ownership,
      i.ammo ?? "",
      i.crafted ? JSON.stringify(i.crafted) : "",
      i.resource ? `${i.resource.quality}` : "",
      contents(n),
    ].join("|");
    const e = m.get(key);
    if (e) {
      e.n += i.stack;
      e.nodes.push(n);
    } else m.set(key, { key, node: n, n: i.stack, nodes: [n] });
  }
  return [...m.values()].sort(
    (a, b) => Number(!!b.node.item.crafted) - Number(!!a.node.item.crafted) || a.node.name.localeCompare(b.node.name),
  );
}
