import { describe, expect, it } from "vitest";
import type { LedgerRow, Trip, TripStop } from "$lib/bindings";
import { ago, amount, clock, ledgerBars, span, stepFigure, stopsText, timeText, timeline } from "./model";

const stop = (site: string, leg: TripStop["leg"], titles: string[]): TripStop => ({
  site,
  place: `Place ${site}`,
  leg,
  steps: titles.map((title, i) => ({ id: `${site}:${i}`, kind: "fit", title, seconds: null })),
});

const trip = (stops: TripStop[], travelSeconds: number | null, workSeconds = 0): Trip => ({
  choice: "most",
  stops,
  why: "",
  travelSeconds,
  workSeconds,
});

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
    [stop("a", null, ["Fit the Part a you own"]), stop("b", { gm: 31.9, seconds: 171 }, ["Do one", "Do two", "Do three"])],
    171,
    510,
  );

  it("heads with the stops and the time the data vouches for", () => {
    expect(stopsText(two)).toBe("2 stops");
    expect(timeText(two)).toBe("3 min in quantum and 8:30 crafting");
    expect(stopsText(trip([], 0))).toBe("No trip");
    expect(timeText(trip([], 0))).toBe("");
  });

  it("counts jumps when no drive timed them", () => {
    expect(timeText(trip(two.stops, null))).toBe("1 jump");
  });

  it("lists each stop as what to do, then where and the jump there", () => {
    const rows = timeline(two, "Place a");
    expect(rows.map((r) => r.n)).toEqual([1, 2]);
    expect(rows[0]).toMatchObject({ what: "Fit the Part a you own", where: "at Place a, where you are" });
    expect(rows[1]).toMatchObject({
      what: "Do one, do two and 1 more",
      where: "at Place b, 32 Gm from Place a, 3 min in quantum",
    });
  });

  it("leaves a jump's time out when it has none, and a start it does not know", () => {
    const rows = timeline(trip([stop("b", { gm: 2.04, seconds: null }, ["Do one"])], null), "Home");
    expect(rows[0].where).toBe("at Place b, 2.0 Gm from Home");
    expect(timeline(trip([stop("b", null, ["Do one"])], null))[0].where).toBe("at Place b");
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
