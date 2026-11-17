import { describe, expect, it } from "vitest";
import type { LedgerRow, PlanStep, Track, Trip, TripStop } from "$lib/bindings";
import { ago, amount, clock, ledgerBars, planSteps, span, stepFigure, stopsText, timeText, timeline, trackNote } from "./model";

const stop = (site: string, leg: TripStop["leg"], titles: string[]): TripStop => ({
  site,
  place: `Place ${site}`,
  at: null,
  leg,
  steps: titles.map((title, i) => ({ id: `${site}:${i}`, kind: "fit", title, seconds: null })),
});

const trip = (stops: TripStop[], travelSeconds: number | null, workSeconds = 0, estimate = false): Trip => ({
  stops,
  travelSeconds,
  workSeconds,
  estimate,
  unplaced: 0,
});

const leg = (gm: number, seconds: number | null, crosses = false) => ({ gm, seconds, crosses });

const row = (material: string, need: number, held: number, bulk = true): LedgerRow => ({
  material,
  minQuality: 0,
  bulk,
  need,
  held,
  plans: ["p1"],
});

describe("times", () => {
  it("counts a craft like the game does", () => {
    expect(clock(510)).toBe("8:30");
    expect(clock(3730)).toBe("1:02:10");
  });
  it("words a stretch of time", () => {
    expect(span(45)).toBe("45 s");
    expect(span(254)).toBe("4 min");
    expect(span(4320)).toBe("1 h 12 min");
    expect(span(7200)).toBe("2 h");
  });
  it("words the age of a read", () => {
    const now = 1_000_000_000;
    expect(ago(now / 1000 - 30, now)).toBe("just now");
    expect(ago(now / 1000 - 16 * 3600, now)).toBe("16 h ago");
    expect(ago(now / 1000 - 3 * 86400, now)).toBe("3 d ago");
  });
});

describe("a step's figure", () => {
  it("is its craft time, else its price, else nothing", () => {
    expect(stepFigure({ seconds: 510, cost: null })).toBe("8:30");
    expect(stepFigure({ seconds: null, cost: 41300 })).toBe("41,300 aUEC");
    expect(stepFigure({ seconds: null, cost: null })).toBe("");
  });
});

describe("the session card", () => {
  const two = trip(
    [stop("a", null, ["Fit the Part a you own"]), stop("b", leg(31.9, 171), ["Do one", "Do two", "Do three"])],
    171,
    510,
  );

  it("heads with the stops and the time the data vouches for", () => {
    expect(stopsText(two)).toBe("2 stops");
    expect(timeText(two)).toBe("3 min trip and 8:30 crafting");
    expect(timeText(trip(two.stops, 171, 0, true))).toBe("~3 min trip");
    expect(stopsText(trip([], 0))).toBe("No trip");
    expect(timeText(trip([], 0))).toBe("");
  });

  it("states no trip time when no drive ordered it", () => {
    expect(timeText(trip(two.stops, null))).toBe("");
  });

  it("lists each stop as what to do and the place, the jump there in the tooltip", () => {
    const rows = timeline(two, "Place a");
    expect(rows.map((r) => r.n)).toEqual([1, 2]);
    expect(rows[0]).toMatchObject({ what: "Fit the Part a you own", where: "Place a" });
    expect(rows[0].full).not.toContain("Gm");
    expect(rows[1]).toMatchObject({ what: "Do one, do two and 1 more", where: "Place b" });
    expect(rows[1].full).toContain("32 Gm from Place a, 3 min");
  });

  it("says when a leg goes through a jump point", () => {
    const rows = timeline(trip([stop("b", leg(40, 700, true), ["Do one"])], 700), "Home");
    expect(rows[0].full).toBe("Do one\n\n40 Gm from Home, 12 min, through a jump point");
  });

  it("leaves a jump's time out when it has none, and a start it does not know", () => {
    const rows = timeline(trip([stop("b", leg(2.04, null), ["Do one"])], null), "Home");
    expect(rows[0].full).toBe("Do one\n\n2.0 Gm from Home");
    expect(timeline(trip([stop("b", null, ["Do one"])], null))[0].full).toBe("Do one");
  });

  it("folds stops past the reserved rows into the last one", () => {
    const five = trip(["a", "b", "c", "d", "e"].map((s) => stop(s, null, [`Do ${s}`])), 600);
    const rows = timeline(five);
    expect(rows.length).toBe(3);
    expect(rows[2]).toMatchObject({ what: "3 more stops", where: "Place c, Place d, Place e" });
  });
});

describe("a track's note", () => {
  const track = (over: Partial<Track>): Track => ({
    kind: "refining",
    open: 1,
    ready: 0,
    waitsOn: null,
    trip: trip([], null),
    ...over,
  });

  it("is a short fact only when the track has no stops", () => {
    expect(trackNote(track({ waitsOn: "mining" }))).toBe("Waits on mining");
    expect(trackNote(track({}))).toBe("Nothing ready");
    expect(trackNote(track({ kind: "crafting", ready: 2 }))).toBe("2 ready to craft");
    expect(trackNote(track({ ready: 1, trip: trip([stop("a", null, ["Do"])], 60) }))).toBe("");
  });
});

describe("the ledger", () => {
  it("draws rows of one unit on one scale, a small need still visible", () => {
    const bars = ledgerBars([row("Ore A", 12, 6), row("Ore B", 0.06, 0.06), row("Ore C", 6, 0), row("Gem D", 3, 1, false)]);
    expect(bars[0]).toEqual({ need: 100, held: 50, state: "warn" });
    expect(bars[1]).toEqual({ need: 3, held: 3, state: "good" });
    expect(bars[2]).toEqual({ need: 50, held: 0, state: "bad" });
    // Pieces are scaled among pieces.
    expect(bars[3].need).toBe(100);
  });
  it("words amounts in their unit", () => {
    expect(amount({ bulk: true }, 12)).toBe("12");
    expect(amount({ bulk: true }, 2.5)).toBe("2.5");
    expect(amount({ bulk: true }, 0.06)).toBe("0.06");
    expect(amount({ bulk: false }, 3)).toBe("3");
  });
});

describe("a plan's steps", () => {
  const step = (id: string, kind: PlanStep["kind"], plans: string[], site: string | null, after: string[] = []): PlanStep => ({
    id,
    kind,
    title: id,
    detail: "",
    plans,
    blocked: false,
    at: site ? { name: site, site } : null,
    seconds: null,
    cost: null,
    odds: null,
    sources: 1,
    after,
  });
  const all = [
    step("mine-iron", "mine", ["a"], "moon"),
    step("mine-ice", "mine", ["b"], "moon"),
    step("mine-tin", "mine", ["b"], "ring"),
    step("mine-far", "mine", ["b"], "elsewhere"),
    step("refine", "refine", ["a", "b"], "station", ["mine-iron", "mine-ice", "mine-tin"]),
    step("craft-b", "craft", ["b"], null),
  ];
  it("shows another plan's mining in the same load, dimmer", () => {
    const shown = planSteps(all, "a").map((s) => [s.step.id, s.other]);
    expect(shown).toEqual([
      ["mine-iron", false],
      ["mine-ice", true],
      ["mine-tin", true],
      ["refine", false],
    ]);
  });
});
