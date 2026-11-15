import { describe, expect, it } from "vitest";
import { bandText, markAt, slotMinimums, snapNearest, snapUp, stopsOf, type QualityBand } from "./quality";

// A real material's bands (build 13165946): every mined resource has these
// eight edges and its own value in each.
const EDGES = [
  [0, 399],
  [400, 599],
  [600, 699],
  [700, 799],
  [800, 899],
  [900, 949],
  [950, 998],
  [999, 1000],
];
const bands = (values: number[]): QualityBand[] => values.map((value, i) => ({ from: EDGES[i][0], to: EDGES[i][1], value }));
const AGRICIUM = bands([346, 588, 667, 796, 852, 943, 971, 1000]);
const ICE = bands([322, 561, 659, 714, 873, 922, 966, 1000]);

describe("stopsOf", () => {
  it("is every quality the material comes in", () => {
    expect(stopsOf(AGRICIUM)).toEqual([346, 588, 667, 796, 852, 943, 971, 1000]);
  });
  it("starts at the slot's own minimum", () => {
    expect(stopsOf(AGRICIUM, 800)).toEqual([852, 943, 971, 1000]);
    expect(stopsOf(AGRICIUM, 1)).toHaveLength(8);
  });
  it("is empty without bands", () => {
    expect(stopsOf([], 500)).toEqual([]);
  });
});

describe("snapping", () => {
  const stops = stopsOf(ICE);
  it("a minimum asks for the next quality up", () => {
    expect(snapUp(stops, 500)).toBe(561);
    // Ice in the 700 band is Q714: a minimum of 750 only takes the next.
    expect(snapUp(stops, 750)).toBe(873);
    expect(snapUp(stops, 714)).toBe(714);
    expect(snapUp(stops, 0)).toBe(322);
  });
  it("a slider goes to the nearest", () => {
    expect(snapNearest(stops, 500)).toBe(561);
    expect(snapNearest(stops, 750)).toBe(714);
    expect(snapNearest(stops, 1000)).toBe(1000);
  });
  it("without stops a quality stays as it is", () => {
    expect(snapUp([], 640)).toBe(640);
    expect(snapNearest([], 640)).toBe(640);
  });
});

describe("bandText", () => {
  it("names the band a quality falls in", () => {
    expect(bandText(AGRICIUM, 588)).toBe("400 to 599");
    expect(bandText(AGRICIUM, 796)).toBe("700 to 799");
    expect(bandText([], 500)).toBe("");
  });
});

describe("markAt", () => {
  it("places a quality between evenly spaced stops", () => {
    const stops = [300, 500, 700];
    expect(markAt(stops, 300)).toBe(0);
    expect(markAt(stops, 500)).toBe(0.5);
    expect(markAt(stops, 600)).toBe(0.75);
    expect(markAt(stops, 900)).toBe(1);
    expect(markAt([500], 400)).toBe(0);
  });
});

describe("slotMinimums", () => {
  it("is the one quality for every slot of an older plan", () => {
    expect(slotMinimums({ quality: 700, slots: [] }, 3)).toEqual([700, 700, 700]);
  });
  it("is each slot's own where the plan sets them", () => {
    expect(slotMinimums({ quality: 750, slots: [500, 750] }, 3)).toEqual([500, 750, 750]);
  });
});
