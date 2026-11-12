// How the catalog is laid out for browsing (decision 'My Items opens on the
// few you care about, and one field is its only scope line', 2026-11-11, and
// the UX board's Crafting sketch 7): armour as one row per set with its
// colourways on demand, everything else as one row per item. Pure.

import type { BlueprintRowView } from "$lib/bindings";
import { WEIGHTS, type Family } from "./model";

// ── Armour sets ─────────────────────────────────────────────────────────────

/** Armour pieces in the order a set reads, as the shelf names them. */
export const PIECES = ["Helmets", "Core", "Arms", "Legs", "Backpacks", "Undersuits"];
/** Four letters each, so a set row's pieces fit side by side when narrow. */
export const PIECE_TINY: Record<string, string> = {
  Helmets: "Helm",
  Core: "Core",
  Arms: "Arms",
  Legs: "Legs",
  Backpacks: "Pack",
  Undersuits: "Suit",
};
export const PIECE_SHORT: Record<string, string> = {
  Helmets: "Helmet",
  Core: "Core",
  Arms: "Arms",
  Legs: "Legs",
  Backpacks: "Pack",
  Undersuits: "Suit",
};

/** "Antium Core" → "Antium": the set a piece belongs to, by its name (the
 *  blueprint data has no set id). */
export const setNameOf = (title: string) =>
  title.replace(/\s+(Helmet|Core|Arms|Legs|Backpack|Undersuit|Armor|Armour)\b.*$/, "").trim() || title;

export interface CraftSet {
  key: string;
  name: string;
  /** The body pieces' weight, else the first piece's. */
  weight: string;
  families: Family[];
  /** Piece (shelf kind) → its families, in PIECES order. */
  byPiece: Map<string, Family[]>;
  blueprints: number;
  owned: number;
  minTime: number | null;
  maxTime: number | null;
  /** The recipes' materials, distinct, in recipe order. */
  materials: string[];
}

export function foldSets(armour: Family[]): CraftSet[] {
  const m = new Map<string, Family[]>();
  for (const f of armour) {
    const k = setNameOf(f.title);
    const l = m.get(k);
    if (l) l.push(f);
    else m.set(k, [f]);
  }
  return [...m.entries()]
    .map(([name, families]) => {
      const byPiece = new Map<string, Family[]>();
      for (const p of PIECES) {
        const l = families.filter((f) => f.shelf.kind === p);
        if (l.length) byPiece.set(p, l);
      }
      const body = families.filter((f) => ["Core", "Arms", "Legs"].includes(f.shelf.kind));
      const weights = (body.length ? body : families).map((f) => f.shelf.weight);
      const weight = [...weights].sort((a, b) => weights.filter((w) => w === b).length - weights.filter((w) => w === a).length)[0];
      const times = families.flatMap((f) => (f.variants[0].craftSeconds != null ? [f.variants[0].craftSeconds] : []));
      return {
        key: `set:${name}`,
        name,
        weight,
        families,
        byPiece,
        blueprints: families.reduce((s, f) => s + f.variants.length, 0),
        owned: families.reduce((s, f) => s + f.ownedCount, 0),
        minTime: times.length ? Math.min(...times) : null,
        maxTime: times.length ? Math.max(...times) : null,
        materials: [...new Set(families.flatMap((f) => f.variants[0].materials.map((x) => x.name)))],
      };
    })
    .sort((a, b) => WEIGHTS.indexOf(a.weight) - WEIGHTS.indexOf(b.weight) || a.name.localeCompare(b.name));
}

/** A set's colourways × pieces: "Base" first, then by name; a cell is null
 *  where that piece has no such colourway. */
export function colourways(set: CraftSet): { pieces: string[]; rows: { name: string; cells: ({ family: Family; variant: BlueprintRowView } | null)[] }[] } {
  const pieces = [...set.byPiece.keys()];
  const label = (v: BlueprintRowView) => v.variant ?? "Base";
  const names = [...new Set(set.families.flatMap((f) => f.variants.map(label)))].sort(
    (a, b) => Number(b === "Base") - Number(a === "Base") || a.localeCompare(b),
  );
  return {
    pieces,
    rows: names.map((name) => ({
      name,
      cells: pieces.map((p) => {
        for (const f of set.byPiece.get(p) ?? []) {
          const v = f.variants.find((x) => label(x) === name);
          if (v) return { family: f, variant: v };
        }
        return null;
      }),
    })),
  };
}
