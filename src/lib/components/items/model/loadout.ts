// A ship's loadout as the community calculators show it: one section per
// component group, one card per hardpoint (mount › weapon), each line the
// part fitted now beside what is saved (what a claim restores) and stock.
// Identical lines and identical hardpoints fold ×N.

import type { ItemKindView, LoadoutSlotView, ShipView } from "$lib/bindings";

export const LOADOUT_SECTIONS = [
  "Weapons",
  "Missiles",
  "Mining",
  "Utility",
  "Shields",
  "Power plants",
  "Coolers",
  "Quantum",
  "Radar",
  "Life support",
  "Cargo",
  "Flight",
  "Paint",
  "Other",
] as const;
export type LoadoutSection = (typeof LOADOUT_SECTIONS)[number];

const SECTION: Record<string, LoadoutSection> = {
  WeaponGun: "Weapons",
  Missile: "Missiles",
  MissileLauncher: "Missiles",
  WeaponMining: "Mining",
  MiningModifier: "Mining",
  TractorBeam: "Utility",
  UtilityTurret: "Utility",
  Shield: "Shields",
  PowerPlant: "Power plants",
  Cooler: "Coolers",
  QuantumDrive: "Quantum",
  JumpDrive: "Quantum",
  Radar: "Radar",
  LifeSupportGenerator: "Life support",
  Container: "Cargo",
  Cargo: "Cargo",
  InventoryContainer: "Cargo",
  FlightController: "Flight",
  Paints: "Paint",
};
const sectionOf = (type: string | undefined): LoadoutSection => (type && SECTION[type]) || "Other";

/** Mounts: what holds the part at a hardpoint (gimbal, rack, mining arm). */
const MOUNT = new Set(["Turret", "MissileLauncher", "ToolArm", "UtilityTurret"]);
/** Ports that are a ship's plumbing, not a choice. */
const SKIP_TYPE = new Set(["Usable", "ControlPanel", "Relay"]);
const HIDE_NAME = new Set(["Fuse"]);

/** "hardpoint_mining_arm/hardpoint_mining_laser" → "Mining arm › mining laser". */
export function portLabel(path: string[]): string {
  const out: string[] = [];
  for (const raw of path) {
    const s = raw
      .replace(/^hardpoint_/i, "")
      .replace(/^\$slot_/, "")
      .replace(/^BONE_ItemPort_Consumable_(\d+)/, "module $1")
      .replace(/_attach$/, "")
      .replace(/_/g, " ");
    if (/^class \d+$/.test(s)) continue;
    if (out.at(-1) !== s) out.push(s);
  }
  const t = out.join(" › ");
  return t.charAt(0).toUpperCase() + t.slice(1);
}

/** `ok`: fitted as saved; `off`: differs from saved; `unknown`: the
 *  inventory doesn't report it (missiles never appear in stash trees). */
export type LineStatus = "ok" | "off" | "unknown";

export interface PortRow {
  path: string[];
  stock: ItemKindView | null;
  /** What a claim restores: the saved loadout, else stock. */
  saved: ItemKindView | null;
  fitted: ItemKindView | null;
  status: LineStatus;
  /** Saved differs from stock. */
  custom: boolean;
}

const same = (a: ItemKindView | null, b: ItemKindView | null) => (a?.guid ?? null) === (b?.guid ?? null);

/** One row per loadout port, joined with what is fitted there now.
 *  `null` when the ship has no loadout data (loadouts not fetched). */
export function portRows(ship: ShipView): PortRow[] | null {
  if (!ship.saved && !ship.stock) return null;
  const key = (p: string[]) => p.join("/");
  const toMap = (slots: LoadoutSlotView[] | null) => new Map((slots ?? []).map((s) => [key(s.path), s]));
  const stock = toMap(ship.stock);
  // No saved loadout recorded: a claim delivers stock.
  const saved = ship.saved ? toMap(ship.saved) : stock;
  const paths = [...new Set([...saved.keys(), ...stock.keys()])];
  const fitted = ship.fitted.map((f) => ({ f, used: false }));
  const rows: PortRow[] = [];
  for (const p of paths) {
    const s = stock.get(p)?.item ?? null;
    const v = saved.get(p)?.item ?? null;
    const type = (v ?? s)?.item_type;
    if (type && SKIP_TYPE.has(type)) continue;
    const path = (saved.get(p) ?? stock.get(p))!.path;
    const port = path.at(-1);
    const cands = fitted.filter((x) => !x.used && x.f.port === port);
    const hit = cands.find((x) => x.f.kind.guid === (v ?? s)?.guid) ?? cands[0];
    if (hit) hit.used = true;
    const now = hit?.f.kind ?? null;
    if (!s && !v && !now) continue;
    const status: LineStatus = type === "Missile" && !now ? "unknown" : same(now, v) ? "ok" : "off";
    rows.push({ path, stock: s, saved: v, fitted: now, status, custom: !same(v, s) });
  }
  return rows;
}

export interface LoadoutLine {
  /** What the line shows: fitted, or for unknown lines the saved part. */
  part: ItemKindView | null;
  fitted: ItemKindView | null;
  saved: ItemKindView | null;
  stock: ItemKindView | null;
  status: LineStatus;
  custom: boolean;
  /** Below the hardpoint: 0 = on it, 1 = on its mount. */
  depth: number;
  n: number;
}

export interface Hardpoint {
  key: string;
  section: LoadoutSection;
  mount: ItemKindView | null;
  lines: LoadoutLine[];
  n: number;
}

const lineKey = (l: LoadoutLine) =>
  [l.fitted?.guid, l.saved?.guid, l.stock?.guid, l.status, l.depth].join("|");

function foldLines(lines: LoadoutLine[]): LoadoutLine[] {
  const m = new Map<string, LoadoutLine>();
  for (const l of lines) {
    const k = lineKey(l);
    const e = m.get(k);
    if (e) e.n += l.n;
    else m.set(k, { ...l });
  }
  return [...m.values()];
}

function foldHardpoints(hps: Hardpoint[]): Hardpoint[] {
  const m = new Map<string, Hardpoint>();
  for (const h of hps) {
    const k = [h.section, h.mount?.guid ?? "", h.lines.map((l) => `${lineKey(l)}/${l.n}`).join(";")].join("|");
    const e = m.get(k);
    if (e) e.n += h.n;
    else m.set(k, { ...h });
  }
  return [...m.values()];
}

/** Hardpoints from the loadouts, joined with what is fitted. */
function fromRows(ship: ShipView, rows: PortRow[]): Hardpoint[] {
  const byHardpoint = new Map<string, PortRow[]>();
  for (const r of rows) {
    const k = r.path[0];
    const l = byHardpoint.get(k);
    if (l) l.push(r);
    else byHardpoint.set(k, [r]);
  }
  const out: Hardpoint[] = [];
  for (const [key, rs] of byHardpoint) {
    let lines: LoadoutLine[] = rs
      .map((r) => ({
        part: r.status === "unknown" ? (r.saved ?? r.stock) : (r.fitted ?? r.saved ?? r.stock),
        fitted: r.fitted,
        saved: r.saved,
        stock: r.stock,
        status: r.status,
        custom: r.custom,
        depth: r.path.length - 1,
        n: 1,
      }))
      .filter((l) => l.part && !HIDE_NAME.has(l.part.name));
    if (!lines.length) continue;
    // The mount: a gimbal / rack / arm row at the hardpoint above deeper
    // rows, or one fitted there that the loadout doesn't list.
    let mount: ItemKindView | null = null;
    const top = lines.find((l) => l.depth === 0 && l.part && MOUNT.has(l.part.item_type));
    if (top && lines.some((l) => l.depth > 0)) {
      mount = top.part;
      lines = lines.filter((l) => l !== top);
    } else {
      mount = ship.fitted.find((f) => f.port === key && f.depth === 0 && MOUNT.has(f.kind.item_type))?.kind ?? null;
    }
    const lead = lines.find((l) => !MOUNT.has(l.part!.item_type)) ?? lines[0];
    out.push({ key, section: sectionOf(lead.part?.item_type), mount, lines: foldLines(lines), n: 1 });
  }
  return foldHardpoints(out);
}

/** Hardpoints from the fitted parts alone (no loadout data): what is on the
 *  ship, grouped by the part at depth 0 (the finder lists fitted parts in
 *  walk order, each followed by what is fitted on it). */
function fromFitted(ship: ShipView): Hardpoint[] {
  const groups: { top: ItemKindView; key: string; below: ItemKindView[] }[] = [];
  for (const f of ship.fitted) {
    if (f.depth === 0) groups.push({ top: f.kind, key: f.port ?? f.geid, below: [] });
    else groups.at(-1)?.below.push(f.kind);
  }
  const line = (k: ItemKindView, depth: number): LoadoutLine => ({
    part: k,
    fitted: k,
    saved: null,
    stock: null,
    status: "ok",
    custom: false,
    depth,
    n: 1,
  });
  return foldHardpoints(
    groups
      .map((g) => {
        const lines = (g.below.length ? g.below.map((k) => line(k, 1)) : [line(g.top, 0)]).filter(
          (l) => !HIDE_NAME.has(l.part!.name),
        );
        const lead = lines.find((l) => !MOUNT.has(l.part!.item_type)) ?? lines[0];
        return {
          key: g.key,
          section: sectionOf(lead?.part?.item_type ?? g.top.item_type),
          mount: g.below.length ? g.top : null,
          lines: foldLines(lines),
          n: 1,
        };
      })
      .filter((h) => h.lines.length > 0),
  );
}

export interface Loadout {
  /** Built from saved/stock loadouts (vs fitted parts alone). */
  compared: boolean;
  hardpoints: Hardpoint[];
  /** Lines (×N counted) fitted differently from saved. */
  off: number;
  /** Lines (×N counted) whose saved part differs from stock. */
  custom: number;
}

export function loadoutOf(ship: ShipView): Loadout {
  const rows = portRows(ship);
  const hardpoints = rows ? fromRows(ship, rows) : fromFitted(ship);
  const lines = hardpoints.flatMap((h) => h.lines.map((l) => ({ l, n: l.n * h.n })));
  const count = (f: (l: LoadoutLine) => boolean) => lines.filter((x) => f(x.l)).reduce((s, x) => s + x.n, 0);
  return {
    compared: !!rows,
    hardpoints,
    off: count((l) => l.status === "off"),
    custom: count((l) => l.custom),
  };
}

/** Fitted-vs-saved count for tiles and the detail panel; `null` without
 *  loadout data or a stash to read the fitted parts from. */
export function offSaved(ship: ShipView): number | null {
  if (!ship.geid || !ship.place || (!ship.saved && !ship.stock)) return null;
  return loadoutOf(ship).off;
}
