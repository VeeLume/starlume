// A ship career's look: its hue and its glyph, shared by the home page's
// tiles and anywhere a ship is shown as a card. Careers as LIVE 4.10 names
// them (`vehicleCareer`); an unknown one falls back to the accent.

import {
  Compass,
  Crosshair,
  Flag,
  Layers,
  Package,
  Pickaxe,
  Rocket,
  Sparkles,
  Target,
  Truck,
  Wrench,
} from "lucide-svelte";

export type CareerLook = { hue: string; icon: typeof Rocket };

const LOOKS: Record<string, CareerLook> = {
  Combat: { hue: "var(--hue-rose)", icon: Crosshair },
  "Snub Fighter": { hue: "var(--hue-rose)", icon: Crosshair },
  Gunship: { hue: "var(--hue-rose)", icon: Target },
  Transporter: { hue: "var(--hue-sand)", icon: Package },
  Transport: { hue: "var(--hue-sand)", icon: Package },
  Exploration: { hue: "var(--hue-sky)", icon: Compass },
  Industrial: { hue: "var(--hue-apricot)", icon: Pickaxe },
  Support: { hue: "var(--hue-teal)", icon: Wrench },
  Competition: { hue: "var(--accent)", icon: Flag },
  "Multi-Role": { hue: "var(--accent)", icon: Layers },
  Starter: { hue: "var(--hue-teal)", icon: Sparkles },
  Ground: { hue: "var(--hue-sand)", icon: Truck },
};

export function careerLook(career: string | null | undefined): CareerLook {
  return (career && LOOKS[career]) || { hue: "var(--accent)", icon: Rocket };
}
