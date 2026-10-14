// My Items explorer: the containment tree over the finder's rows.
//
//   Everywhere › (system ›) planet › location › Personal hangar | Storage ›
//   containers (racks, suits, backpacks, ships) › items › attachments
//
// "On you" is its own root branch; a storage at a planet itself ("Left in
// space") hangs under the planet, after its locations. Ships outside every
// stash hang under two virtual groups. Pure: built once per finder load,
// read by the explorer components (frontend rule 1: the store owns the
// finder, this is a derived view of it).

import type { OwnedItemView, PlaceView, ShipView } from "$lib/bindings";

export type PlaceLevel = "root" | "system" | "body" | "station" | "area" | "group";
/** What an area is: the finder's place kind. */
export type AreaKind = "storage" | "hangar" | "carried" | "adrift" | "other";

interface Base {
  id: string;
  name: string;
  kids: Node[];
  parent: Node | null;
}
export interface PlaceNode extends Base {
  type: "place";
  level: PlaceLevel;
  /** Secondary line: "microTech · Stanton", "Not accessible from a terminal". */
  sub: string;
  /** Set on areas. */
  area?: AreaKind;
}
export interface ShipNode extends Base {
  type: "ship";
  ship: ShipView;
  /** Not in any stash: fitted parts and contents unknown. */
  virtual: boolean;
}
export interface ItemNode extends Base {
  type: "item";
  item: OwnedItemView;
}
export type Node = PlaceNode | ShipNode | ItemNode;

export interface Tree {
  root: PlaceNode;
  /** Ships whose instance exists but sits in no readable stash. */
  away: PlaceNode;
  /** Ships never materialised. */
  unspawned: PlaceNode;
  byId: Map<string, Node>;
  /** Item nodes by kind GUID: "also elsewhere", where a saved part is now. */
  byKind: Map<string, ItemNode[]>;
  items: ItemNode[];
  ships: ShipNode[];
}

// ── Node facts ──────────────────────────────────────────────────────────────

export const isPlace = (n: Node): n is PlaceNode => n.type === "place";
export const isShip = (n: Node): n is ShipNode => n.type === "ship";
export const isItem = (n: Node): n is ItemNode => n.type === "item";

export function ownership(n: Node): string {
  return n.type === "item" ? n.item.ownership : n.type === "ship" ? n.ship.ownership : "owned";
}
export const isBricked = (n: Node) => ownership(n) === "bricked";
export const isRented = (n: Node) => ownership(n) === "rented";
export const stackOf = (n: Node) => (n.type === "item" ? n.item.stack : 1);
export const itemType = (n: Node) =>
  n.type === "item" ? n.item.kind.item_type : n.type === "ship" ? "NOITEM_Vehicle" : "";

/** Types that hold things you browse into; anything else holding children
 *  (a rifle with a scope) shows its children as attachments. */
const FOLDER = new Set([
  "Misc",
  "Flair_Wall",
  "Container",
  "InventoryContainer",
  "Cargo",
  "Turret",
  "MissileLauncher",
  "ToolArm",
  "UtilityTurret",
]);

export function isFolder(n: Node): boolean {
  if (n.type !== "item") return true;
  const t = n.item.kind.item_type;
  return n.kids.length > 0 && (FOLDER.has(t) || t.startsWith("Char_Armor"));
}

/** Bricked leftovers only show when asked for. */
export const visible = (n: Node, showBricked: boolean) => showBricked || !isBricked(n);

export function ancestors(n: Node): Node[] {
  const out: Node[] = [];
  for (let x: Node | null = n; x; x = x.parent) out.unshift(x);
  return out;
}

/** The nearest folder above a node (what it sits in). */
export function container(n: Node): Node | null {
  let x = n.parent;
  while (x && !isFolder(x)) x = x.parent;
  return x;
}

/** The area (storage, hangar, on you…) a node sits in. */
export function areaOf(n: Node): PlaceNode | null {
  for (let x: Node | null = n; x; x = x.parent) if (x.type === "place" && x.level === "area") return x;
  return null;
}

/** "Hurston › Lorville › Storage": the path without Everywhere and the system. */
export function pathLabel(n: Node): string {
  return ancestors(n)
    .filter((x) => !(x.type === "place" && (x.level === "root" || x.level === "system")))
    .map((x) => x.name)
    .join(" › ");
}

/** Item units in a subtree; ships themselves don't count as items. */
export function units(n: Node, keep: (n: ItemNode) => boolean): number {
  let c = 0;
  const walk = (x: Node) => {
    if (x.type === "item" && keep(x)) c += x.item.stack;
    x.kids.forEach(walk);
  };
  walk(n);
  return c;
}

/** Ships in a subtree, the node itself included. */
export function shipsIn(n: Node, keep: (n: ShipNode) => boolean): ShipNode[] {
  const out: ShipNode[] = [];
  const walk = (x: Node) => {
    if (x.type === "ship" && keep(x)) out.push(x);
    x.kids.forEach(walk);
  };
  walk(n);
  return out;
}

/** Items stowed in a ship (not fitted), without descending into ships it carries. */
export function stowed(ship: Node, keep: (n: ItemNode) => boolean): ItemNode[] {
  const out: ItemNode[] = [];
  const walk = (x: Node) => {
    for (const k of x.kids) {
      if (k.type === "ship") continue;
      if (k.type === "item" && !k.item.fitted && keep(k)) out.push(k);
      walk(k);
    }
  };
  walk(ship);
  return out;
}

// ── Building ────────────────────────────────────────────────────────────────

const AREA_NAME: Record<string, string> = {
  storage: "Storage",
  hangar: "Personal hangar",
  carried: "On you",
  adrift: "Left in space",
};

function placeNode(id: string, level: PlaceLevel, name: string, sub = ""): PlaceNode {
  return { type: "place", id, level, name, sub, kids: [], parent: null };
}

function attach(parent: Node, child: Node) {
  child.parent = parent;
  parent.kids.push(child);
}

export function buildTree(items: OwnedItemView[], ships: ShipView[]): Tree {
  const root = placeNode("root", "root", "Everywhere");
  const away = placeNode("g-away", "group", "Location not readable");
  const unspawned = placeNode("g-none", "group", "Not spawned");
  const systems = new Map<string, PlaceNode>();
  const bodies = new Map<string, PlaceNode>();
  const stations = new Map<string, PlaceNode>();
  const areas = new Map<string, PlaceNode>();
  let you: PlaceNode | null = null;
  const loose: PlaceNode[] = [];

  const system = (name: string) => {
    let s = systems.get(name);
    if (!s) systems.set(name, (s = placeNode(`y:${name}`, "system", name, "System")));
    return s;
  };
  const body = (sys: string, name: string) => {
    const k = `${sys}/${name}`;
    let b = bodies.get(k);
    if (!b) {
      bodies.set(k, (b = placeNode(`p:${k}`, "body", name, sys)));
      attach(system(sys), b);
    }
    return b;
  };

  /** The area node for a place, created with its location, planet and
   *  system on first use. Parents come nearest first: [planet, system] or
   *  [moon, planet, system]; the planet is the level below the system. */
  function area(p: PlaceView): PlaceNode {
    const key = `${p.kind}|${p.key || p.name}`;
    const known = areas.get(key);
    if (known) return known;
    const kind = (["storage", "hangar", "carried", "adrift"].includes(p.kind) ? p.kind : "other") as AreaKind;
    const a = placeNode(`a:${key}`, "area", AREA_NAME[kind] ?? p.name);
    a.area = kind;
    areas.set(key, a);
    const sys = p.parents.at(-1);
    if (kind === "carried") {
      you = a;
    } else if (kind === "adrift") {
      // The place is the planet (or a moon of one) itself.
      const planet = p.parents.length >= 2 ? p.parents.at(-2)! : p.name;
      a.sub = "Not accessible from a terminal";
      if (planet !== p.name) a.name = `Left in space near ${p.name}`;
      if (sys) attach(body(sys, planet), a);
      else loose.push(a);
    } else if (kind === "other" || !sys) {
      loose.push(a);
    } else {
      const stKey = p.key || `${p.parents.join("/")}/${p.name}`;
      let st = stations.get(stKey);
      if (!st) {
        st = placeNode(`s:${stKey}`, "station", p.name, p.parents.join(" · "));
        stations.set(stKey, st);
        if (p.parents.length >= 2) attach(body(sys, p.parents.at(-2)!), st);
        else attach(system(sys), st);
      }
      attach(st, a);
    }
    return a;
  }

  // Every instance as a node first, then linked: a holder may come later.
  const byGeid = new Map<string, ItemNode | ShipNode>();
  const shipNodes: ShipNode[] = [];
  for (const s of ships) {
    const placed = !!(s.geid && s.place);
    const n: ShipNode = { type: "ship", id: `ship:${s.key}`, name: s.kind.name, ship: s, virtual: !placed, kids: [], parent: null };
    shipNodes.push(n);
    if (placed) byGeid.set(s.geid!, n);
  }
  const itemNodes: ItemNode[] = items.map((item) => {
    const n: ItemNode = { type: "item", id: `item:${item.geid}`, name: item.kind.name, item, kids: [], parent: null };
    byGeid.set(item.geid, n);
    return n;
  });

  /** The nearest holder that is a node; else the place's area. */
  const parentOf = (holders: { geid: string }[], place: PlaceView): Node => {
    for (let i = holders.length - 1; i >= 0; i--) {
      const h = byGeid.get(holders[i].geid);
      if (h) return h;
    }
    return area(place);
  };
  for (const n of shipNodes) {
    const s = n.ship;
    if (!n.virtual) attach(parentOf(s.holders, s.place!), n);
    else if (s.ownership !== "bricked") attach(s.geid ? away : unspawned, n);
  }
  for (const n of itemNodes) attach(parentOf(n.item.holders, n.item.place), n);

  // Order: hangar before storage; locations biggest first; what was left
  // in space after a planet's locations.
  const size = (n: Node) => units(n, () => true) + shipsIn(n, () => true).length;
  for (const st of stations.values()) st.kids.sort((a) => ((a as PlaceNode).area === "hangar" ? -1 : 1));
  const rank = (n: Node) => (n.type === "place" && n.area === "adrift" ? 1 : 0);
  for (const b of bodies.values()) b.kids.sort((x, y) => rank(x) - rank(y) || size(y) - size(x));
  for (const s of systems.values()) s.kids.sort((x, y) => size(y) - size(x));
  const sysList = [...systems.values()].sort((x, y) => size(y) - size(x));
  // One system: its planets sit straight under Everywhere.
  const top: Node[] = sysList.length > 1 ? sysList : sysList.flatMap((s) => s.kids);
  for (const n of [...(you ? [you] : []), ...top, ...loose]) attach(root, n);

  const byId = new Map<string, Node>();
  const index = (n: Node) => {
    byId.set(n.id, n);
    n.kids.forEach(index);
  };
  [root, away, unspawned].forEach(index);
  const byKind = new Map<string, ItemNode[]>();
  for (const n of itemNodes) {
    const list = byKind.get(n.item.kind.guid);
    if (list) list.push(n);
    else byKind.set(n.item.kind.guid, [n]);
  }
  return { root, away, unspawned, byId, byKind, items: itemNodes, ships: shipNodes };
}
