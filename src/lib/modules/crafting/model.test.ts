import { describe, expect, it } from "vitest";
import type { BlueprintRowView, BlueprintSlotView, BlueprintStatView, MaterialView, StockStackView } from "$lib/bindings";
import { bestQuality, canCraft, foldFamilies, gradeLetter, holdingText, shelfRank, effectText, evalModifier, evalStat, fmtTime, heldAt, presetQuality, readiness, reachText, shelfOf, stockIndex } from "./model";

const band = (at0: number, at1000: number, additive = false) => ({
  additive,
  startQuality: 0,
  endQuality: 1000,
  atStart: at0,
  atEnd: at1000,
});
const mod = (property: string, transform = "percent", ranges = [band(0.8, 1.2)]) => ({
  property,
  name: property,
  transform,
  ranges,
});
const material = (over: Partial<MaterialView> = {}): MaterialView => ({
  name: "Iron",
  scu: 1,
  count: null,
  minQuality: 0,
  ...over,
});
const slot = (...modifiers: ReturnType<typeof mod>[]): BlueprintSlotView => ({
  name: "Frame",
  material: material(),
  modifiers,
});
const stat = (over: Partial<BlueprintStatView> = {}): BlueprintStatView => ({
  group: null,
  label: "Integrity",
  property: "hp",
  unit: " HP",
  higherIsBetter: true,
  base: 100,
  ...over,
});
const stack = (quality: number, amount: number, name = "Iron"): StockStackView => ({
  name,
  quality,
  amount,
  bulk: true,
  place: "Area18",
});

describe("a recipe's curves", () => {
  it("interpolates a band and clamps to it", () => {
    expect(evalModifier([band(0.8, 1.2)], 500)?.value).toBeCloseTo(1);
    const upper = { ...band(1, 2), startQuality: 500 };
    expect(evalModifier([upper], 100)?.value).toBeCloseTo(1);
  });

  it("picks the nearest band for a quality in a gap", () => {
    const low = { ...band(1, 1.1), endQuality: 400 };
    const high = { ...band(2, 3), startQuality: 900 };
    expect(evalModifier([low, high], 450)?.value).toBeCloseTo(1.1);
    expect(evalModifier([low, high], 850)?.value).toBeCloseTo(2);
  });

  it("adds factors across slots rather than compounding them", () => {
    const slots = [slot(mod("hp", "percent", [band(0.9, 0.9)])), slot(mod("hp", "percent", [band(0.9, 0.9)]))];
    const r = evalStat(stat(), slots, [500, 500]);
    expect(r.value).toBeCloseTo(80);
    expect(r.pct).toBeCloseTo(-20);
    expect(r.improved).toBe(false);
  });

  it("each slot reads its own quality", () => {
    const slots = [slot(mod("hp")), slot(mod("hp"))];
    expect(evalStat(stat(), slots, [1000, 0]).pct).toBeNull();
    expect(evalStat(stat(), slots, [1000, 500]).pct).toBeCloseTo(20);
  });

  it("a stat that improves as it shrinks reads a drop as better", () => {
    const r = evalStat(stat({ higherIsBetter: false }), [slot(mod("hp"))], [0]);
    expect(r.improved).toBe(true);
  });

  it("judges a negative base by the value, not the sign", () => {
    const r = evalStat(stat({ base: -30, higherIsBetter: false }), [slot(mod("hp"))], [1000]);
    expect(r.value).toBeCloseTo(-36);
    expect(r.improved).toBe(true);
  });

  it("shows a percent alone for a stat with no base", () => {
    const r = evalStat(stat({ base: null }), [slot(mod("hp"))], [1000]);
    expect(r.value).toBeNull();
    expect(r.pct).toBeCloseTo(20);
  });

  it("leaves a stat the recipe doesn't drive unchanged", () => {
    const r = evalStat(stat({ property: null }), [slot(mod("hp"))], [1000]);
    expect(r).toEqual({ value: 100, pct: null, improved: null });
  });

  it("reads a modifier as the game shows it", () => {
    expect(effectText(mod("a"), 1000)).toEqual({ factor: "×1.200", pct: expect.closeTo(20), up: true });
    expect(effectText(mod("a", "negatedPercent"), 1000)?.pct).toBeCloseTo(-20);
    expect(effectText(mod("a", "raw"), 0)?.pct).toBeCloseTo(-20);
    expect(effectText(mod("a"), 500)?.up).toBeNull();
    expect(reachText(mod("a"))).toBe("×0.80–1.20");
  });
});

describe("what you hold", () => {
  const stacks = [stack(800, 1), stack(600, 2), stack(300, 5)];

  it("counts only stacks at the quality or better", () => {
    expect(heldAt(stacks, 500)).toBe(3);
    expect(heldAt(undefined, 0)).toBe(0);
  });

  it("finds the highest quality you hold enough of", () => {
    expect(bestQuality(stacks, 1)).toBe(800);
    expect(bestQuality(stacks, 2.5)).toBe(600);
    expect(bestQuality(stacks, 100)).toBeNull();
  });

  it("says how a slot stands", () => {
    expect(readiness(stacks, material({ scu: 2 }), 500)).toBe("good");
    expect(readiness(stacks, material({ scu: 5 }), 500)).toBe("short");
    expect(readiness([], material(), 0)).toBe("none");
  });

  it("makes a material two slots share cover both", () => {
    const stock = stockIndex([stack(500, 1.5)]);
    expect(canCraft([material()], stock)).toBe(true);
    expect(canCraft([material(), material()], stock)).toBe(false);
  });

  it("holds a recipe to its minimum qualities", () => {
    const stock = stockIndex([stack(500, 1)]);
    expect(canCraft([material({ minQuality: 600 })], stock)).toBe(false);
  });

  it("can't craft a recipe with no materials", () => {
    expect(canCraft([], new Map())).toBe(false);
  });

  it("says what you hold in words", () => {
    const m = material({ scu: 2 });
    expect(holdingText(stacks, m, 500, false)).toBe("Your inventory hasn't been read yet");
    expect(holdingText(undefined, m, 500, true)).toBe("You have none in your inventory");
    expect(holdingText(stacks, m, 500, true)).toBe("You have 3.00 SCU at Q500 or better · Area18");
    expect(holdingText(stacks, m, 700, true)).toBe("You have only 1.00 SCU at Q700 or better");
    expect(holdingText(stacks, m, 900, true)).toBe("You have some, but your best is Q800");
  });

  it("sets slots from a preset", () => {
    const m = material({ minQuality: 200, scu: 1 });
    expect(presetQuality("Min", m, 500, stacks)).toBe(200);
    expect(presetQuality("Base", m, 500, stacks)).toBe(500);
    expect(presetQuality("Max", m, 500, stacks)).toBe(1000);
    expect(presetQuality("Best in stock", m, 500, stacks)).toBe(800);
    expect(presetQuality("Best in stock", m, 500, undefined)).toBe(500);
  });
});

describe("variant families", () => {
  const row = (family: string, variant: string | null) => ({ family, variant }) as BlueprintRowView;

  it("folds by family, the plain one first", () => {
    const folded = foldFamilies([row("a", "Snow"), row("b", null), row("a", null), row("a", "Aqua")]);
    expect(folded.map((f) => f.map((r) => r.variant))).toEqual([[null, "Aqua", "Snow"], [null]]);
  });
});

describe("shelving", () => {
  const kind = (itemType: string, subType = "UNDEFINED", size = 1, name = "") => ({ itemType, subType, size, name });

  it("files ship parts by size, size 0 included", () => {
    expect(shelfOf(kind("Cooler", "UNDEFINED", 0))).toMatchObject({
      top: "Ship parts",
      main: "Coolers",
      kind: "Coolers",
      sub: "Size 0",
      sized: true,
    });
  });

  it("files armour by slot", () => {
    expect(shelfOf(kind("Char_Armor_Helmet", "Heavy"))).toMatchObject({
      main: "Armour",
      kind: "Helmets",
      weight: "Heavy",
      sub: "Helmets · Heavy",
    });
  });

  it("puts flight gear apart from the weights", () => {
    expect(shelfOf(kind("Char_Armor_Helmet", "Helmet")).weight).toBe("Flight & other");
    expect(shelfOf(kind("Char_Armor_Backpack", "LightArmor")).weight).toBe("Light");
  });

  it("files weapons by class", () => {
    expect(shelfOf(kind("WeaponPersonal", "Small", 1, "S38 Pistol")).kind).toBe("Pistols");
    expect(shelfOf(kind("WeaponPersonal", "Medium", 1, "A03 Sniper Rifle")).kind).toBe("Snipers");
    expect(shelfOf(kind("WeaponPersonal", "Medium", 1, "C54 SMG")).kind).toBe("SMGs");
    expect(shelfOf(kind("WeaponPersonal", "Medium", 1, "P4-AR Rifle")).kind).toBe("Rifles");
  });

  it("only a part with a class has a grade", () => {
    expect(gradeLetter({ class: "Military", grade: 2 })).toBe("B");
    expect(gradeLetter({ class: null, grade: 1 })).toBe("");
  });

  it("shelves parts Military A first", () => {
    expect(shelfRank({ class: "Military", grade: 1 })).toBeLessThan(shelfRank({ class: "Military", grade: 2 }));
    expect(shelfRank({ class: "Military", grade: 4 })).toBeLessThan(shelfRank({ class: "Civilian", grade: 1 }));
    expect(shelfRank({ class: "Industrial", grade: 4 })).toBeLessThan(shelfRank({ class: null, grade: 1 }));
  });

  it("keeps an unknown kind on a shelf", () => {
    expect(shelfOf(kind(""))).toMatchObject({ main: "Other" });
  });
});

it("formats a craft time", () => {
  expect(fmtTime(390)).toBe("6:30");
  expect(fmtTime(3725)).toBe("1:02:05");
  expect(fmtTime(null)).toBe("—");
});
