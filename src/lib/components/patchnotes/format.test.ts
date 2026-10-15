import { describe, expect, it } from "vitest";
import { highlightGroup } from "./format";

describe("highlightGroup", () => {
  it("splits an old-style feature title into label and items", () => {
    expect(
      highlightGroup("Gameplay: Orison Relief Support - Ground Vehicle Soft Death - Loot Quality"),
    ).toEqual({
      label: "Gameplay",
      items: ["Orison Relief Support", "Ground Vehicle Soft Death", "Loot Quality"],
    });
  });

  it("keeps a plain title whole", () => {
    expect(highlightGroup("RSI Constellation Mk IV")).toEqual({
      label: null,
      items: ["RSI Constellation Mk IV"],
    });
  });

  it("does not split hyphens inside words or commas", () => {
    expect(highlightGroup("Ships & Vehicles: Pricing, Claims, & Availability - BUL-H4 Armor")).toEqual({
      label: "Ships & Vehicles",
      items: ["Pricing, Claims, & Availability", "BUL-H4 Armor"],
    });
  });
});
