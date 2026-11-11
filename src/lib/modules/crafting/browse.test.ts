import { describe, expect, it } from "vitest";
import type { BlueprintRowView } from "$lib/bindings";
import { colourways, componentGrid, foldSets, setNameOf } from "./browse";
import { foldFamilies, shelfOf, type Family } from "./model";

let next = 1;
const row = (over: Partial<BlueprintRowView>): BlueprintRowView => ({
  guid: `g${next++}`,
  name: "X",
  family: "f",
  familyName: "X",
  variant: null,
  category: null,
  itemType: "Char_Armor_Torso",
  subType: "Heavy",
  size: 1,
  grade: 1,
  class: null,
  manufacturer: null,
  craftSeconds: 270,
  materials: [{ name: "Tungsten", scu: 0.1, count: null, minQuality: 0 }],
  ...over,
});
const piece = (set: string, piece: string, itemType: string, variants: (string | null)[], owned: string[] = []) =>
  variants.map((variant) =>
    row({
      name: `${set} ${piece}${variant ? ` ${variant}` : ""}`,
      family: `${set} ${piece}`,
      familyName: `${set} ${piece}`,
      variant,
      itemType,
      guid: owned.includes(variant ?? "Base") ? `own:${set}:${piece}:${variant}` : `g${next++}`,
    }),
  );
/** Families as the page builds them; owned when the guid says so. */
const families = (rows: BlueprintRowView[]): Family[] =>
  foldFamilies(rows).map((variants) => ({
    key: variants[0].family,
    title: variants[0].familyName,
    shelf: shelfOf(variants[0]),
    maker: "",
    variants,
    ownedCount: variants.filter((v) => v.guid.startsWith("own:")).length,
    wantBlueprint: false,
    wantItem: false,
    craftable: false,
  }));

describe("armour sets", () => {
  it("names a set by its pieces' names", () => {
    expect(setNameOf("Antium Core")).toBe("Antium");
    expect(setNameOf("ORC-mkV Legs")).toBe("ORC-mkV");
    expect(setNameOf("Tailwind Flight Suit")).toBe("Tailwind Flight Suit");
  });

  it("folds pieces into one row per set, counting blueprints and owned", () => {
    const fams = families([
      ...piece("Antium", "Core", "Char_Armor_Torso", [null, "Jet"], ["Jet"]),
      ...piece("Antium", "Helmet", "Char_Armor_Helmet", [null, "Jet", "Sand"], ["Sand"]),
      ...piece("Monde", "Arms", "Char_Armor_Arms", [null]),
    ]);
    const sets = foldSets(fams);
    expect(sets.map((s) => [s.name, s.blueprints, s.owned, [...s.byPiece.keys()]])).toEqual([
      ["Antium", 5, 2, ["Helmets", "Core"]],
      ["Monde", 1, 0, ["Arms"]],
    ]);
  });

  it("lays colourways against pieces, Base first, gaps empty", () => {
    const [set] = foldSets(
      families([
        ...piece("Antium", "Core", "Char_Armor_Torso", [null, "Jet"]),
        ...piece("Antium", "Helmet", "Char_Armor_Helmet", [null, "Sand"]),
      ]),
    );
    const g = colourways(set);
    expect(g.pieces).toEqual(["Helmets", "Core"]);
    expect(g.rows.map((r) => [r.name, r.cells.map((c) => (c ? c.variant.name : null))])).toEqual([
      ["Base", ["Antium Helmet", "Antium Core"]],
      ["Jet", [null, "Antium Core Jet"]],
      ["Sand", ["Antium Helmet Sand", null]],
    ]);
  });
});

describe("ship component tables", () => {
  it("puts sizes across and class and grade down, Military A first", () => {
    const cooler = (name: string, size: number, klass: string, grade: number, mats: string[]) =>
      row({
        name,
        family: name,
        familyName: name,
        itemType: "Cooler",
        subType: "UNDEFINED",
        size,
        grade,
        class: klass,
        materials: mats.map((m) => ({ name: m, scu: 0.1, count: null, minQuality: 0 })),
      });
    const g = componentGrid(
      families([
        cooler("Bracer", 1, "Military", 3, ["Borase"]),
        cooler("Polar", 1, "Military", 2, ["Lindinium"]),
        cooler("Permafrost", 2, "Military", 2, ["Lindinium"]),
        cooler("Gelid", 1, "Civilian", 1, ["Borase"]),
      ]),
    );
    expect(g.sizes).toEqual([1, 2]);
    expect(g.rows.map((r) => [r.klass, r.grade, r.cells.map((c) => c.map((f) => f.title)), r.recipe])).toEqual([
      ["Military", "B", [["Polar"], ["Permafrost"]], ["Lindinium"]],
      ["Military", "C", [["Bracer"], []], ["Borase"]],
      ["Civilian", "A", [["Gelid"], []], ["Borase"]],
    ]);
  });
});
