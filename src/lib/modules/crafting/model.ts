// Crafting maths: what a recipe makes at the qualities on its sliders, and
// what you hold towards it. Pure, and on this side of the IPC because it has
// to re-evaluate as a slider moves. Mirrors sc-crafting's evaluation.

import type {
  BlueprintRowView,
  BlueprintSlotView,
  BlueprintStatView,
  MaterialView,
  ModifierRangeView,
  SlotModifierView,
  StockStackView,
} from "$lib/bindings";

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

export interface StatResult {
  /** The value as crafted; null when the item has no absolute figure. */
  value: number | null;
  /** Percent change; null when unchanged. */
  pct: number | null;
  /** Better for the item; null when unchanged or the direction is unclear. */
  improved: boolean | null;
}

/** A stat at each slot's quality. Factors add across slots (two -10% slots
 *  are -20%, not -19%): the game's rule, checked against scmdb. */
export function evalStat(stat: BlueprintStatView, slots: BlueprintSlotView[], qualities: number[]): StatResult {
  let factor = 1;
  let added = 0;
  if (stat.property != null) {
    slots.forEach((slot, i) => {
      for (const m of slot.modifiers) {
        if (m.property !== stat.property) continue;
        const e = evalModifier(m.ranges, qualities[i]);
        if (!e) continue;
        if (e.additive) added += e.value;
        else factor += e.value - 1;
      }
    });
  }
  const value = stat.base != null ? stat.base * factor + added : null;
  const changed = value != null && stat.base != null ? Math.abs(value - stat.base) > 1e-6 : Math.abs(factor - 1) > 1e-6;
  if (!changed) return { value, pct: null, improved: null };
  const pct = stat.base ? ((value! - stat.base) / Math.abs(stat.base)) * 100 : (factor - 1) * 100;
  const up = stat.base != null && value != null ? value > stat.base : factor > 1;
  return { value, pct, improved: stat.higherIsBetter == null ? null : up === stat.higherIsBetter };
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

/** The curve's reach across all qualities, `×0.80–1.20`. */
export function reachText(m: SlotModifierView): string {
  const ends = m.ranges.flatMap((r) => [r.atStart, r.atEnd]);
  if (ends.length === 0) return "";
  const lo = Math.min(...ends);
  const hi = Math.max(...ends);
  return m.ranges[0].additive ? `+${lo.toFixed(0)}–${hi.toFixed(0)}` : `×${lo.toFixed(2)}–${hi.toFixed(2)}`;
}

export const fmtPct = (p: number) => `${p > 0 ? "+" : p < 0 ? "−" : "±"}${Math.abs(p).toFixed(2)}%`;

export function fmtValue(v: number, unit: string): string {
  const a = Math.abs(v);
  const s = a >= 100 ? v.toFixed(0) : a >= 1 ? v.toFixed(2) : v.toFixed(3);
  return `${s.includes(".") ? s.replace(/\.?0+$/, "") : s}${unit}`;
}

export function fmtTime(seconds: number | null): string {
  if (seconds == null) return "—";
  const s = Math.round(seconds);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const pad = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(s % 60)}` : `${m}:${pad(s % 60)}`;
}

export const fmtNeed = (m: MaterialView) =>
  m.scu != null ? `${m.scu < 0.01 ? "<0.01" : m.scu.toFixed(2)} SCU` : m.count != null ? `${m.count} pc${m.count === 1 ? "" : "s"}` : "";

/** `Civilian B`, as the shops shelve a part; empty when it has neither. */
export function classText(b: { class: string | null; grade: number }): string {
  const grade = b.grade >= 1 && b.grade <= 4 ? "ABCD"[b.grade - 1] : "";
  return [b.class ?? "", b.class ? grade : ""].filter(Boolean).join(" ");
}

// ── Where a blueprint is shelved ────────────────────────────────────────────

export interface Shelf {
  /** The category tree's top level: ship parts share one, as in My Items. */
  top: string;
  /** The list's sections. */
  main: string;
  mainOrder: number;
  /** Item subtype, when the type has one worth filtering by. */
  kind: string;
  /** Armour's weight class. */
  weight: string;
  /** The list's sub-section under `main`. */
  sub: string;
  subOrder: number;
  /** Has a size worth showing (ship parts run from size 0). */
  sized: boolean;
}

/** The way the game's shops shelve parts. */
export const CLASSES = ["Military", "Stealth", "Competition", "Civilian", "Industrial"];
export const GRADES = ["A", "B", "C", "D"];
export const WEIGHTS = ["Light", "Medium", "Heavy", "Flight & other"];

/** A part's grade letter. Only parts with a class have a real one: armour
 *  and guns carry a placeholder. */
export const gradeLetter = (b: { class: string | null; grade: number }) =>
  b.class && b.grade >= 1 && b.grade <= 4 ? GRADES[b.grade - 1] : "";

const COMPONENTS: [string, string][] = [
  ["Cooler", "Coolers"],
  ["PowerPlant", "Power plants"],
  ["Shield", "Shields"],
  ["QuantumDrive", "Quantum drives"],
  ["Radar", "Radars"],
];
const MINING_SALVAGE: [string, string][] = [
  ["WeaponMining", "Mining lasers"],
  ["MiningModifier", "Mining modules"],
  ["SalvageHead", "Salvage heads"],
  ["SalvageModifier", "Salvage modules"],
  ["TractorBeam", "Tractor beams"],
];
const ARMOUR_SLOTS: [string, string][] = [
  ["Char_Armor_Helmet", "Helmets"],
  ["Char_Armor_Torso", "Core"],
  ["Char_Armor_Arms", "Arms"],
  ["Char_Armor_Legs", "Legs"],
  ["Char_Armor_Backpack", "Backpacks"],
  ["Char_Armor_Undersuit", "Undersuits"],
];
export const WEAPON_CLASSES = ["Pistols", "SMGs", "Rifles", "Shotguns", "Snipers", "LMGs", "Launchers", "Melee"];

/** A personal weapon's class. The game's subtype is only Small / Medium /
 *  Large, so the rest reads the name, as My Items does. */
function weaponClass(subType: string, name: string): string {
  if (subType === "Knife") return "Melee";
  if (subType === "Small") return "Pistols";
  if (subType === "Large" || /Launcher/.test(name)) return "Launchers";
  if (/Sniper|Crossbow/.test(name)) return "Snipers";
  if (/Shotgun/.test(name)) return "Shotguns";
  if (/LMG/.test(name)) return "LMGs";
  if (/SMG/.test(name)) return "SMGs";
  return "Rifles";
}

/** Armour's subtype is its weight; flight helmets and packs have their own. */
function weightOf(subType: string): string {
  const w = subType === "LightArmor" ? "Light" : subType;
  return WEIGHTS.includes(w) ? w : "Flight & other";
}

/** The catalog's taxonomy, from the item's own type and subtype. */
export function shelfOf(b: { itemType: string; subType: string; size: number; name: string }): Shelf {
  const t = b.itemType;
  const plain = { kind: "", weight: "", sized: false };
  const size = `Size ${b.size}`;
  if (t === "WeaponPersonal") {
    const kind = weaponClass(b.subType, b.name);
    return { ...plain, top: "Weapons", main: "Weapons", mainOrder: 10, kind, sub: kind, subOrder: WEAPON_CLASSES.indexOf(kind) };
  }
  const slot = ARMOUR_SLOTS.findIndex(([k]) => k === t);
  if (slot >= 0) {
    const kind = ARMOUR_SLOTS[slot][1];
    const weight = weightOf(b.subType);
    return { ...plain, top: "Armour", main: "Armour", mainOrder: 20, kind, weight, sub: `${kind} · ${weight}`, subOrder: slot * 10 + WEIGHTS.indexOf(weight) };
  }
  if (t.startsWith("Char_Clothing")) return { ...plain, top: "Clothing", main: "Clothing", mainOrder: 25, sub: "", subOrder: 0 };
  if (t === "WeaponAttachment") return { ...plain, top: "Attachments", main: "Attachments", mainOrder: 27, sub: "", subOrder: 0 };
  if (t === "WeaponGun") return { ...plain, top: "Ship parts", main: "Ship weapons", mainOrder: 30, kind: "Ship weapons", sub: size, subOrder: b.size, sized: true };
  const part = COMPONENTS.findIndex(([k]) => k === t);
  if (part >= 0) {
    const kind = COMPONENTS[part][1];
    return { ...plain, top: "Ship parts", main: kind, mainOrder: 40 + part, kind, sub: size, subOrder: b.size, sized: true };
  }
  const tool = MINING_SALVAGE.findIndex(([k]) => k === t);
  if (tool >= 0) {
    const kind = MINING_SALVAGE[tool][1];
    return { ...plain, top: "Mining & salvage", main: "Mining & salvage", mainOrder: 50, kind, sub: `${kind} · ${size}`, subOrder: tool * 10 + b.size, sized: true };
  }
  return { ...plain, top: "Other", main: "Other", mainOrder: 90, sub: "", subOrder: 0 };
}

/** Military A first, the way the shops shelve parts. */
export function shelfRank(b: { class: string | null; grade: number }): number {
  const c = b.class ? CLASSES.indexOf(b.class) : -1;
  return (c < 0 ? CLASSES.length : c) * 10 + b.grade;
}

// ── Variant families ────────────────────────────────────────────────────────

/** One catalog row: an item and its colourways. */
export interface Family {
  key: string;
  title: string;
  shelf: Shelf;
  maker: string;
  /** The plain one first. */
  variants: BlueprintRowView[];
  ownedCount: number;
  wantBlueprint: boolean;
  wantItem: boolean;
  craftable: boolean;
}

/** Blueprints folded by family, the plain variant first, then by label. */
export function foldFamilies(rows: BlueprintRowView[]): BlueprintRowView[][] {
  const out = new Map<string, BlueprintRowView[]>();
  for (const r of rows) {
    const list = out.get(r.family);
    if (list) list.push(r);
    else out.set(r.family, [r]);
  }
  const label = (r: BlueprintRowView) => r.variant ?? "";
  return [...out.values()].map((list) => list.sort((a, b) => label(a).localeCompare(label(b))));
}

// ── What you hold ───────────────────────────────────────────────────────────

export type Stock = Map<string, StockStackView[]>;

export function stockIndex(stacks: StockStackView[]): Stock {
  const out: Stock = new Map();
  for (const s of stacks) {
    const list = out.get(s.name);
    if (list) list.push(s);
    else out.set(s.name, [s]);
  }
  return out;
}

/** The amount a slot takes, in its material's unit. */
export const needOf = (m: MaterialView) => m.scu ?? m.count ?? 0;

/** How much you hold at `quality` or better. */
export function heldAt(stacks: StockStackView[] | undefined, quality: number): number {
  return (stacks ?? []).filter((s) => s.quality >= quality).reduce((a, s) => a + s.amount, 0);
}

/** The highest quality you hold enough of; null when you can't fill it. */
export function bestQuality(stacks: StockStackView[] | undefined, need: number): number | null {
  const sorted = [...(stacks ?? [])].sort((a, b) => b.quality - a.quality);
  let held = 0;
  for (const s of sorted) {
    held += s.amount;
    if (held >= need) return s.quality;
  }
  return null;
}

export type Readiness = "good" | "short" | "none";

/** A slot against your stock at a quality. */
export function readiness(stacks: StockStackView[] | undefined, m: MaterialView, quality: number): Readiness {
  if (!stacks?.length) return "none";
  return heldAt(stacks, quality) >= needOf(m) ? "good" : "short";
}

/** Whether your stock fills a whole recipe at its minimum qualities. A
 *  material two slots share has to cover both. */
export function canCraft(materials: MaterialView[], stock: Stock): boolean {
  if (materials.length === 0) return false;
  const needed = new Map<string, { need: number; min: number }>();
  for (const m of materials) {
    const e = needed.get(m.name);
    if (e) {
      e.need += needOf(m);
      e.min = Math.max(e.min, m.minQuality);
    } else needed.set(m.name, { need: needOf(m), min: m.minQuality });
  }
  return [...needed].every(([name, { need, min }]) => heldAt(stock.get(name), min) >= need);
}

/** A slot's stock in words, for someone reading it cold. `read` is false
 *  when the inventory has never been fetched: not the same as holding none. */
export function holdingText(stacks: StockStackView[] | undefined, m: MaterialView, quality: number, read: boolean): string {
  if (!read) return "Your inventory hasn't been read yet";
  if (!stacks?.length) return "You have none in your inventory";
  const bulk = stacks[0].bulk;
  const amount = (v: number) => (bulk ? `${v.toFixed(2)} SCU` : `${Math.floor(v)} pcs`);
  const held = heldAt(stacks, quality);
  const best = stacks.reduce((a, b) => (b.quality > a.quality ? b : a));
  if (held >= needOf(m)) return `You have ${amount(held)} at Q${quality} or better · ${best.place}`;
  if (held > 0) return `You have only ${amount(held)} at Q${quality} or better`;
  return `You have some, but your best is Q${best.quality}`;
}

export type Preset = "Min" | "Base" | "Best in stock" | "Max";
export const PRESETS: Preset[] = ["Min", "Base", "Best in stock", "Max"];

/** The quality a preset sets a slot to. "Best in stock" falls back to the
 *  base quality for a material you can't fill. */
export function presetQuality(preset: Preset, m: MaterialView, base: number, stacks: StockStackView[] | undefined): number {
  switch (preset) {
    case "Min":
      return m.minQuality;
    case "Base":
      return Math.max(base, m.minQuality);
    case "Max":
      return 1000;
    case "Best in stock":
      return bestQuality(stacks, needOf(m)) ?? Math.max(base, m.minQuality);
  }
}
