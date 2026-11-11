// Your gear: the few things worth finding among the many (decision 'My
// Items opens on the few you care about…', 2026-11-11). What you crafted,
// full armour sets and weapons kitted beyond stock, below a place. A thing
// shows once, under the first heading it fits. Pure.

import { armourSets, categoryOf, isFull, isKitted, type ArmourSet } from "./classify";
import { stackIdentical, type Stack } from "./stack";
import { areaOf, isFolder, type ItemNode, type Node, type PlaceNode } from "./tree";

export interface Gear {
  crafted: Stack[];
  /** Full sets, each within one area (a set split over places is not one
   *  you can put on). */
  sets: ArmourSet[];
  kitted: Stack[];
}

/** The items a place holds, through containers but not into ships (their
 *  stowage is the ship's view) or into an item's attachments. */
export function itemsBelow(n: Node, keep: (x: ItemNode) => boolean): ItemNode[] {
  const out: ItemNode[] = [];
  const walk = (x: Node) => {
    for (const k of x.kids) {
      if (k.type === "ship") continue;
      if (k.type === "item" && keep(k)) out.push(k);
      if (k.type !== "item" || isFolder(k)) walk(k);
    }
  };
  walk(n);
  return out;
}

export function gearOf(list: ItemNode[]): Gear {
  const crafted = list.filter((x) => !!x.item.crafted);
  const kitted = list.filter((x) => !x.item.crafted && categoryOf(x) === "Weapons" && isKitted(x));
  const byArea = new Map<string, ItemNode[]>();
  for (const x of list) {
    if (categoryOf(x) !== "Armour" || x.item.crafted) continue;
    const a = areaOf(x)?.id ?? "";
    const l = byArea.get(a);
    if (l) l.push(x);
    else byArea.set(a, [x]);
  }
  const sets = [...byArea.values()]
    .flatMap((l) => armourSets(l).filter(isFull))
    .sort((a, b) => a.name.localeCompare(b.name));
  return { crafted: stackIdentical(crafted), sets, kitted: stackIdentical(kitted) };
}

export const gearCount = (g: Gear) =>
  g.crafted.reduce((s, x) => s + x.n, 0) + g.sets.length + g.kitted.reduce((s, x) => s + x.n, 0);

/** "New Babbage · Personal hangar · Bosco Weapon Display Rack": the
 *  location, the area and the container (when it isn't the area itself). */
export function whereLabel(n: Node): string {
  const area = areaOf(n);
  if (!area) return "";
  const loc = area.parent && area.parent.type === "place" && area.parent.level !== "root" ? area.parent.name : "";
  let box: Node | null = n.parent;
  while (box && box.type === "item" && !isFolder(box)) box = box.parent;
  const holder = box && box !== area && box.type !== "place" ? box.name : "";
  return [loc, area.name, holder].filter(Boolean).join(" · ");
}

/** Where a stack sits: one place's label, or how many places it spans. */
export function whereOfAll(nodes: ItemNode[]): string {
  const labels = [...new Set(nodes.map(whereLabel))];
  return labels.length === 1 ? labels[0] : `${labels.length} places`;
}

/** The areas (storage, hangars, on you…) below a place, biggest first: the
 *  way in from a place that holds nothing of its own, one click away. */
export function areasBelow(n: Node, count: (x: Node) => number): { area: PlaceNode; n: number }[] {
  const out: { area: PlaceNode; n: number }[] = [];
  const walk = (x: Node) => {
    for (const k of x.kids) {
      if (k.type !== "place") continue;
      if (k.level === "group") continue;
      if (k.level === "area") {
        const c = count(k);
        if (c > 0) out.push({ area: k, n: c });
      } else walk(k);
    }
  };
  walk(n);
  return out.sort((a, b) => b.n - a.n || a.area.name.localeCompare(b.area.name));
}
