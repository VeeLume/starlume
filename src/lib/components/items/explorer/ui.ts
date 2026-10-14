// Explorer chrome shared by its parts: icons per node and category, and the
// few number formats. Lucide for chrome (CLAUDE.md: icons for chrome/nav).

import {
  Archive,
  Box,
  CircleDashed,
  CircleOff,
  Cog,
  Cpu,
  Crosshair,
  Earth,
  Gem,
  Globe,
  HeartPulse,
  Magnet,
  MapPin,
  Navigation,
  Orbit,
  Package,
  Paintbrush,
  Pickaxe,
  Pill,
  Puzzle,
  Radar,
  Rocket,
  Shield,
  Shirt,
  Snowflake,
  Sun,
  User,
  Warehouse,
  Wrench,
  Zap,
} from "lucide-svelte";
import type { Category } from "../model/classify";
import type { LoadoutSection } from "../model/loadout";
import type { Node } from "../model/tree";

type IconComponent = typeof Box;

export function nodeIcon(n: Node): IconComponent {
  if (n.type === "ship") return Rocket;
  if (n.type === "place") {
    switch (n.level) {
      case "root":
        return Globe;
      case "system":
        return Sun;
      case "body":
        return Earth;
      case "station":
        return MapPin;
      case "group":
        return n.id === "g-away" ? CircleDashed : CircleOff;
      default:
        return n.area === "hangar" ? Warehouse : n.area === "carried" ? User : Archive;
    }
  }
  const t = n.item.kind.item_type;
  if (t.startsWith("Char_Armor")) return Shirt;
  if (t === "Turret" || t === "UtilityTurret") return Crosshair;
  return Package;
}

export const CATEGORY_ICON: Record<Category, IconComponent> = {
  Weapons: Crosshair,
  Armour: Shield,
  Clothing: Shirt,
  Attachments: Puzzle,
  Utility: Wrench,
  Consumables: Pill,
  Resources: Pickaxe,
  "Ship parts": Cpu,
  Liveries: Paintbrush,
  "Cargo & containers": Package,
  "Decor & collectibles": Gem,
  Other: Box,
};

export const SECTION_ICON: Record<LoadoutSection, IconComponent> = {
  Weapons: Crosshair,
  Missiles: Rocket,
  Mining: Pickaxe,
  Utility: Magnet,
  Shields: Shield,
  "Power plants": Zap,
  Coolers: Snowflake,
  Quantum: Orbit,
  Radar: Radar,
  "Life support": HeartPulse,
  Cargo: Package,
  Flight: Navigation,
  Paint: Paintbrush,
  Other: Cog,
};

export const fmtN = (n: number) => n.toLocaleString("en-US");

export function fmtTime(epochSeconds: number): string {
  return new Date(epochSeconds * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

export const SOURCE: Record<string, string> = { pledge: "Pledge", in_game: "In-game", rental: "Rental" };

/** The container browse's URL-backed filters. */
export interface BrowseFilters {
  cat: string;
  sub: string;
  w: string;
  /** "pieces" lists armour piece by piece; default is sets. */
  arm: string;
  kit: string;
}

/** The page's three views over one tree. */
export type View = "items" | "ships" | "res";
