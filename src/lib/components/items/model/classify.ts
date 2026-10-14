// How the explorer files items: the in-game categories with their sub
// filters, armour folded into sets, weapons with their kit. Pure functions
// over the finder's kinds (item type + sub type from the game data).

import type { ItemKindView } from "$lib/bindings";
import type { ItemNode, Node } from "./tree";

export const CATEGORIES = [
  "Weapons",
  "Armour",
  "Clothing",
  "Attachments",
  "Utility",
  "Consumables",
  "Resources",
  "Ship parts",
  "Liveries",
  "Cargo & containers",
  "Decor & collectibles",
  "Other",
] as const;
export type Category = (typeof CATEGORIES)[number];

const SHIP_PART: Record<string, string> = {
  WeaponGun: "Guns",
  Missile: "Missiles",
  Turret: "Mounts",
  MissileLauncher: "Mounts",
  Shield: "Shields",
  PowerPlant: "Power plants",
  Cooler: "Coolers",
  QuantumDrive: "Quantum",
  JumpDrive: "Quantum",
  Radar: "Radar",
  LifeSupportGenerator: "Life support",
  MiningModifier: "Mining",
  WeaponMining: "Mining",
  ToolArm: "Mining",
  FlightController: "Flight",
  UtilityTurret: "Utility",
};

/** Tools the game files as personal weapons (ParaMed is a "Small" weapon,
 *  like a pistol). */
const TOOL = /ParaMed|Cambio|Multi-Tool|Tractor Beam|Extinguisher|QuikFlare/;

/** The game data's sub type, "" when undefined. */
export const subType = (k: ItemKindView) => (k.sub_type === "UNDEFINED" ? "" : k.sub_type);

export function categoryOf(n: ItemNode): Category {
  const k = n.item.kind;
  const t = k.item_type;
  const s = subType(k);
  if (n.item.resource) return "Resources";
  if (t === "WeaponPersonal") return s === "Gadget" || s === "Grenade" || TOOL.test(k.name) ? "Utility" : "Weapons";
  if (t.startsWith("Char_Armor")) return "Armour";
  if (t.startsWith("Char_Clothing")) return "Clothing";
  if (t === "WeaponAttachment") return "Attachments";
  if (["MobiGlas", "RemovableChip", "TractorBeam"].includes(t)) return "Utility";
  if (["FPS_Consumable", "Food", "Drink"].includes(t)) return "Consumables";
  if (SHIP_PART[t]) return "Ship parts";
  if (t === "Paints") return "Liveries";
  if (["Cargo", "Container", "InventoryContainer"].includes(t)) return "Cargo & containers";
  if (t.startsWith("Flair_") || ["Misc", "Usable", "Char_Flair"].includes(t)) return "Decor & collectibles";
  return "Other";
}

export const ARMOUR_SLOT: Record<string, string> = {
  Char_Armor_Helmet: "Helmet",
  Char_Armor_Torso: "Core",
  Char_Armor_Arms: "Arms",
  Char_Armor_Legs: "Legs",
  Char_Armor_Backpack: "Backpack",
  Char_Armor_Undersuit: "Undersuit",
};

/** Sub filters per category, in their display order. */
export const SUBCATEGORIES: Partial<Record<Category, string[]>> = {
  Weapons: ["Pistols", "SMGs", "Rifles", "Shotguns", "Snipers", "LMGs", "Launchers", "Melee"],
  Armour: ["Helmet", "Core", "Arms", "Legs", "Backpack", "Undersuit"],
  Clothing: ["Hats", "Shirts", "Jackets", "Legs", "Feet", "Hands"],
  Attachments: ["Optics", "Barrel", "Underbarrel", "Magazines", "Utility"],
  Utility: ["Multi-tools", "Tractor beams", "Medical", "Throwables", "Keycards & drives", "Other"],
  Consumables: ["Medical", "Food", "Drinks"],
  Resources: ["Bulk (SCU)", "Pieces"],
  "Ship parts": [
    "Guns",
    "Missiles",
    "Mounts",
    "Shields",
    "Power plants",
    "Coolers",
    "Quantum",
    "Radar",
    "Life support",
    "Mining",
    "Flight",
    "Utility",
  ],
};

const CLOTHING: Record<string, string> = {
  Char_Clothing_Hat: "Hats",
  Char_Clothing_Torso_0: "Shirts",
  Char_Clothing_Torso_1: "Jackets",
  Char_Clothing_Legs: "Legs",
  Char_Clothing_Feet: "Feet",
  Char_Clothing_Hands: "Hands",
};
const ATTACHMENT: Record<string, string> = {
  IronSight: "Optics",
  Barrel: "Barrel",
  BottomAttachment: "Underbarrel",
  Magazine: "Magazines",
};

export function subcategoryOf(n: ItemNode, cat: Category): string | null {
  const k = n.item.kind;
  const t = k.item_type;
  const s = subType(k);
  const nm = k.name;
  switch (cat) {
    case "Weapons":
      if (s === "Knife") return "Melee";
      if (s === "Small") return "Pistols";
      if (s === "Large" || /Launcher/.test(nm)) return "Launchers";
      if (/Sniper|Crossbow/.test(nm)) return "Snipers";
      if (/Shotgun/.test(nm)) return "Shotguns";
      if (/LMG/.test(nm)) return "LMGs";
      if (/SMG/.test(nm)) return "SMGs";
      return "Rifles";
    case "Armour":
      return ARMOUR_SLOT[t] ?? null;
    case "Clothing":
      return CLOTHING[t] ?? "Jackets";
    case "Attachments":
      return ATTACHMENT[s] ?? "Utility";
    case "Utility":
      if (/Multi-Tool/.test(nm)) return "Multi-tools";
      if (/Tractor/.test(nm) || t === "TractorBeam") return "Tractor beams";
      if (/ParaMed|Cambio|Medical/.test(nm)) return "Medical";
      if (s === "Grenade" || /QuikFlare/.test(nm)) return "Throwables";
      if (t === "RemovableChip") return "Keycards & drives";
      return "Other";
    case "Consumables":
      return t === "FPS_Consumable" ? "Medical" : t === "Food" ? "Food" : "Drinks";
    case "Resources":
      return isBulk(n) ? "Bulk (SCU)" : "Pieces";
    case "Ship parts":
      return SHIP_PART[t] ?? "Utility";
    default:
      return null;
  }
}

/** A resource in cargo boxes, measured in SCU (vs hand-gathered pieces). */
export const isBulk = (n: ItemNode) => n.item.kind.item_type === "Cargo";

// ── Armour ──────────────────────────────────────────────────────────────────

export type Weight = "Light" | "Medium" | "Heavy" | "Other";

/** Armour weight class from the game data's sub type. */
export function weightOf(n: ItemNode): Weight {
  const s = subType(n.item.kind);
  if (s === "Light" || s === "Medium" || s === "Heavy") return s;
  return s === "LightArmor" ? "Light" : "Other";
}

/** "Geist Armor Core Golden Blossom" → "Geist Golden Blossom": the set a
 *  piece belongs to. */
export const setName = (name: string) =>
  name
    .replace(/\b(Helmet|Core|Arms|Legs|Backpack|Undersuit|Armor)\b/g, "")
    .replace(/\s+/g, " ")
    .trim();

export const PIECES = ["Helmet", "Core", "Arms", "Legs", "Backpack", "Undersuit"];
/** What makes a set full. */
export const FULL_SET = ["Helmet", "Core", "Arms", "Legs"];

export interface ArmourSet {
  name: string;
  items: ItemNode[];
  weight: Weight;
  /** Piece → count. */
  have: Map<string, number>;
  /** Pieces of the full set owned (of four). */
  got: number;
}

/** The armour sets among a list of pieces. A set's weight is its core,
 *  arms and legs' most common class. Largest sets first. */
export function armourSets(list: ItemNode[]): ArmourSet[] {
  const m = new Map<string, ItemNode[]>();
  for (const n of list) {
    const k = setName(n.item.kind.name);
    const l = m.get(k);
    if (l) l.push(n);
    else m.set(k, [n]);
  }
  return [...m.entries()]
    .map(([name, items]) => {
      const body = items.filter((n) =>
        ["Char_Armor_Torso", "Char_Armor_Arms", "Char_Armor_Legs"].includes(n.item.kind.item_type),
      );
      const weights = (body.length ? body : items).map(weightOf);
      const count = (w: Weight) => weights.filter((x) => x === w).length;
      const weight = [...weights].sort((a, b) => count(b) - count(a))[0];
      const have = new Map<string, number>();
      for (const n of items) {
        const p = ARMOUR_SLOT[n.item.kind.item_type];
        if (p) have.set(p, (have.get(p) ?? 0) + n.item.stack);
      }
      return { name, items, weight, have, got: FULL_SET.filter((p) => have.has(p)).length };
    })
    .sort((a, b) => b.items.length - a.items.length || a.name.localeCompare(b.name));
}

// ── Weapon kit ──────────────────────────────────────────────────────────────

export type Slot = "OPT" | "BRL" | "UND" | "MAG" | "UTL";
export const SLOT_NAME: Record<Slot, string> = {
  OPT: "Optic",
  BRL: "Barrel",
  UND: "Underbarrel",
  MAG: "Magazine",
  UTL: "Utility",
};
const SLOT_OF: Record<string, Slot> = { IronSight: "OPT", Barrel: "BRL", BottomAttachment: "UND", Magazine: "MAG" };

/** `stock`: came with it; `added`: not in its stock loadout; `fitted`: stock
 *  unknown (a looted weapon has no stock loadout of its own). */
export type KitState = "stock" | "added" | "fitted";
export interface KitPart {
  part: ItemNode;
  slot: Slot;
  state: KitState;
}

/** A weapon's attachments by slot, each marked against its stock loadout. */
export function kitOf(weapon: Node): KitPart[] {
  if (weapon.type !== "item") return [];
  const stock = weapon.item.stock;
  const pool = stock ? stock.flatMap((s) => (s.item ? [s.item.guid] : [])) : null;
  return weapon.kids
    .filter((k): k is ItemNode => k.type === "item" && k.item.kind.item_type === "WeaponAttachment")
    .map((part) => {
      const slot = SLOT_OF[subType(part.item.kind)] ?? "UTL";
      let state: KitState = "fitted";
      if (pool) {
        const i = pool.indexOf(part.item.kind.guid);
        if (i >= 0) {
          pool.splice(i, 1);
          state = "stock";
        } else state = "added";
      }
      return { part, slot, state };
    });
}

/** Kitted: anything added to stock; without stock data, anything beyond a
 *  magazine. */
export const isKitted = (weapon: Node) =>
  kitOf(weapon).some((k) => k.state === "added" || (k.state === "fitted" && k.slot !== "MAG"));

/** "Myers (4x Telescopic)" → "Myers 4x"; other trailing parentheses dropped
 *  (the full name goes in the tooltip). */
export const shortPartName = (name: string) =>
  name.replace(/\s*\((\d+x)[^)]*\)/, " $1").replace(/\s*\([^)]*\)$/, "");
