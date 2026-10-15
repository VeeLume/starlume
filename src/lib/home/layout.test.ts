import { describe, expect, it } from "vitest";
import { moveCard, resolveLayout, toPrefs, type CardSlot } from "./layout";

const card = (key: string, defaultVisible = true): CardSlot => ({ key, defaultVisible });

describe("resolveLayout", () => {
  it("uses registry order and default visibility without a saved layout", () => {
    const placed = resolveLayout([card("shell:a"), card("m:b", false)], []);
    expect(placed.map((c) => [c.key, c.visible])).toEqual([
      ["shell:a", true],
      ["m:b", false],
    ]);
  });

  it("puts saved cards first, then unknown ones at their default", () => {
    const placed = resolveLayout(
      [card("shell:a"), card("m:b"), card("n:c")],
      [
        { id: "m:b", visible: false },
        { id: "shell:a", visible: true },
      ],
    );
    expect(placed.map((c) => [c.key, c.visible])).toEqual([
      ["m:b", false],
      ["shell:a", true],
      ["n:c", true],
    ]);
  });

  it("skips saved ids that are not available, and duplicates", () => {
    const placed = resolveLayout(
      [card("shell:a")],
      [
        { id: "gone:x", visible: true },
        { id: "shell:a", visible: false },
        { id: "shell:a", visible: true },
      ],
    );
    expect(placed.map((c) => [c.key, c.visible])).toEqual([["shell:a", false]]);
  });
});

describe("toPrefs", () => {
  it("keeps unavailable cards after the card they followed", () => {
    const previous = [
      { id: "shell:a", visible: true },
      { id: "off:x", visible: false },
      { id: "m:b", visible: true },
    ];
    const placed = resolveLayout([card("shell:a"), card("m:b")], previous);
    // The user moves b above a while module "off" is disabled.
    const moved = moveCard(placed, 1, -1);
    expect(toPrefs(moved, previous)).toEqual([
      { id: "m:b", visible: true },
      { id: "shell:a", visible: true },
      { id: "off:x", visible: false },
    ]);
  });

  it("keeps a leading unavailable card at the front", () => {
    const previous = [
      { id: "off:x", visible: true },
      { id: "shell:a", visible: true },
    ];
    const placed = resolveLayout([card("shell:a")], previous);
    expect(toPrefs(placed, previous)).toEqual(previous);
  });
});

describe("moveCard", () => {
  it("ignores moves past either end", () => {
    const xs = ["a", "b"];
    expect(moveCard(xs, 0, -1)).toBe(xs);
    expect(moveCard(xs, 1, 1)).toBe(xs);
    expect(moveCard(xs, 0, 1)).toEqual(["b", "a"]);
  });
});
