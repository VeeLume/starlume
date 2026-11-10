// Each shelf's look: an icon for its tile, a category hue for its ground.
// Hues say what kind of thing something is and never mark it clickable
// (design direction, 2026-10-15).

import { Box, Crosshair, Orbit, Pickaxe, Pill, Puzzle, Radar, Rocket, Shield, ShieldHalf, Shirt, Snowflake, Swords, Zap } from "lucide-svelte";

export interface ShelfLook {
  icon: typeof Box;
  hue: string;
}

const LOOK: Record<string, ShelfLook> = {
  Weapons: { icon: Crosshair, hue: "var(--hue-apricot)" },
  Armour: { icon: Shield, hue: "var(--hue-sky)" },
  Clothing: { icon: Shirt, hue: "var(--hue-rose)" },
  Attachments: { icon: Puzzle, hue: "var(--hue-sand)" },
  "Ship weapons": { icon: Swords, hue: "var(--hue-rose)" },
  Missiles: { icon: Rocket, hue: "var(--hue-rose)" },
  Coolers: { icon: Snowflake, hue: "var(--hue-teal)" },
  "Power plants": { icon: Zap, hue: "var(--hue-teal)" },
  Shields: { icon: ShieldHalf, hue: "var(--hue-teal)" },
  "Quantum drives": { icon: Orbit, hue: "var(--hue-teal)" },
  Radars: { icon: Radar, hue: "var(--hue-teal)" },
  "Mining & salvage": { icon: Pickaxe, hue: "var(--hue-sand)" },
  Consumables: { icon: Pill, hue: "var(--hue-teal)" },
};

const OTHER: ShelfLook = { icon: Box, hue: "var(--text-dim)" };

export const lookOf = (shelf: string): ShelfLook => LOOK[shelf] ?? OTHER;
