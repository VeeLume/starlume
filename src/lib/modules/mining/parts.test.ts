import { describe, expect, it } from "vitest";
import type { LaserInputView, MiningCatalogView, MiningLaserView } from "$lib/bindings";
import { changeLabel, withPart } from "./parts";

const laser = (id: string, name: string): MiningLaserView => ({
  id,
  name,
  size: 1,
  moduleSlots: 2,
  power: 2340,
  extractionPower: 1850,
  optimalRangeM: 60,
  maxRangeM: 180,
  throttleMinimum: 0.1,
  modifiers: { resistance: null, instability: null, windowSize: null, windowRate: null } as MiningLaserView["modifiers"],
  blueprint: `bp-${id}`,
});
const catalog = { lasers: [laser("a", "Arbor MH1 Mining Laser"), laser("h", "Helix I Mining Laser")], modules: [] } as unknown as MiningCatalogView;
const head = (over: Partial<LaserInputView> = {}): LaserInputView => ({
  laser: "a",
  modules: [],
  owner: 0,
  who: null,
  crew: null,
  distanceM: null,
  off: false,
  ...over,
});

describe("a crafted head", () => {
  it("keeps its mark while its laser stays", () => {
    const next = withPart(catalog, [head({ quality: 900 })], "L0", "a");
    expect(next[0].quality).toBe(900);
  });

  it("loses its mark with another laser: the mark was the old one's", () => {
    const next = withPart(catalog, [head({ quality: 900 })], "L0", "h");
    expect(next[0].laser).toBe("h");
    expect(next[0].quality).toBeNull();
  });

  it("says a mark as a change against the saved loadout", () => {
    expect(changeLabel(catalog, [head()], [head({ quality: 800 })])).toBe("crafted Q800");
    expect(changeLabel(catalog, [head({ quality: 800 })], [head({ quality: null })])).toBe("as sold");
    expect(changeLabel(catalog, [head()], [head()])).toBe("as saved");
  });
});
