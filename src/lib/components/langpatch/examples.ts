// Text Patching's list-detail page — pure, so the words are tested: the
// one-line summary under each patcher, and the fixed before/after
// examples for the patchers that have no live preview yet (only Mission
// Enhancer previews from the game data). The examples are real strings
// from a 4.10.2 LIVE global.ini; they show what an option does, not this
// build's every change.

import type { PatcherInfoView } from "$lib/bindings";

/** An option's current value (the stored one, else its default). */
export function optionValue(patcher: PatcherInfoView, id: string): string | undefined {
  const option = patcher.options.find((o) => o.id === id);
  return patcher.values[id] ?? option?.default;
}

/** "11 of 11 on · Mark (x)", "M1C Bracer", "Off" — the row's second line. */
export function patcherSummary(patcher: PatcherInfoView): string {
  if (!patcher.enabled) return "Off";
  const parts: string[] = [];
  const toggles = patcher.options.filter((o) => o.kind.type === "Bool");
  if (toggles.length) {
    const on = toggles.filter((o) => optionValue(patcher, o.id) === "true").length;
    parts.push(`${on} of ${toggles.length} on`);
  }
  for (const o of patcher.options) {
    if (o.kind.type !== "Choice") continue;
    const value = optionValue(patcher, o.id);
    const label = o.kind.choices.find((c) => c.value === value)?.label;
    if (label) parts.push(label);
  }
  return parts.join(" · ") || "On";
}

export interface ExampleRow {
  /** What the game shows, or where the text sits. */
  from: string;
  to: string;
  /** Shown in the game's highlight (an emphasised style). */
  emphasised?: boolean;
}

export interface PatcherExample {
  /** Column heads: "In the game" / "Patched". */
  fromHead: string;
  rows: ExampleRow[];
  note?: string;
}

/** The fixed examples for a patcher at its current settings; `null` for a
 *  patcher without any (Mission Enhancer previews live). */
export function patcherExample(patcher: PatcherInfoView): PatcherExample | null {
  switch (patcher.id) {
    case "component_grades": {
      const format = optionValue(patcher, "format");
      const name = (base: string, size: number) =>
        format === "compact_prefix"
          ? `M${size}C ${base}`
          : format === "short_prefix"
            ? `MIL${size}C ${base}`
            : `${base} Military C`;
      return {
        fromHead: "In the game",
        rows: [
          { from: "Bracer (cooler, size 1, military, grade C)", to: name("Bracer", 1) },
          { from: "FullStop (shield, size 2, military, grade C)", to: name("FullStop", 2) },
        ],
      };
    }
    case "illegal_goods": {
      const emphasised = optionValue(patcher, "display") === "color_coded";
      return {
        fromHead: "In the game",
        rows: [{ from: "Gasping Weevil Eggs", to: "[!] Gasping Weevil Eggs", emphasised }],
        note: "The commodity's description also lists where it is outlawed.",
      };
    }
    case "weapon_enhancer": {
      const on = (id: string) => optionValue(patcher, id) !== "false";
      return {
        fromHead: "In the game",
        rows: [
          { from: "Omnisky III Cannon", to: on("size_prefix") ? "S1 Omnisky III Cannon" : "Omnisky III Cannon" },
          {
            from: "Marksman I Missile",
            to: `${on("size_prefix") ? "S1 " : ""}${on("missile_type_prefix") ? "[IR] " : ""}Marksman I Missile`,
          },
        ],
        note: on("weapon_stats")
          ? "Descriptions gain a Weapon Stats block: alpha damage, penetration, projectile speed, capacitor."
          : undefined,
      };
    }
    case "label_fixes":
      return {
        fromHead: "Where",
        rows: [
          { from: "Mining scanner: instability", to: "Instab.:" },
          { from: "Mining scanner: resistance", to: "Resist.:" },
          { from: "Commodity: Hephaestanite", to: "Heph" },
          { from: "Commodity: Hephaestanite (raw)", to: "Heph (Raw)" },
        ],
      };
    default:
      return null;
  }
}
