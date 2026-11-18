import { describe, expect, it } from "vitest";
import type { LedgerRow, PlanStep, StepKind, Track, TrackKind, Trip, TripStop } from "$lib/bindings";
import { ago, amount, clock, focusView, ledgerBars, planSteps, span, stepFigure, stopsText, timeText, timeline, trackNote } from "./model";

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
    expect(stepFigure({ seconds: 1200, cost: null, kind: "earn" })).toBe("≈20 min CIG");
    expect(stepFigure({ seconds: 5400, cost: null, kind: "rep", secondsFrom: "game" })).toBe("≈1 h 30 min CIG");
    expect(stepFigure({ seconds: 1440, cost: null, kind: "run", secondsFrom: "yours" })).toBe("≈24 min yours");
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
    secondsFrom: null,
    cost: null,
    odds: null,
    sources: 1,
    after,
    parts: 0,
    rocks: null,
    scu: 0,
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

describe("a session focus", () => {
  const ps = (id: string, kind: StepKind, plans: string[], site: string | null, more: Partial<PlanStep> = {}): PlanStep => ({
    id,
    kind,
    title: id,
    detail: "",
    plans,
    blocked: false,
    at: site ? { name: `Place ${site}`, site } : null,
    seconds: null,
    secondsFrom: null,
    cost: null,
    odds: null,
    sources: 1,
    after: [],
    parts: 0,
    rocks: null,
    scu: 0,
    ...more,
  });
  const at = (site: string, l: TripStop["leg"], ids: [string, StepKind][]): TripStop => ({
    site,
    place: `Place ${site}`,
    at: null,
    leg: l,
    steps: ids.map(([id, kind]) => ({ id, kind, title: id, seconds: null })),
  });
  const track = (kind: TrackKind, stops: TripStop[]): Track => ({ kind, open: 0, ready: 0, waitsOn: null, trip: trip(stops, null) });
  const steps = [
    ps("buy1", "buy", ["a"], "s1"),
    ps("m1", "mine", ["a"], "m1"),
    ps("m2", "mine", ["b"], "m2"),
    ps("refine:Pyro:0", "refine", ["a", "b"], "r", { after: ["m1", "m2"] }),
    ps("craft1", "craft", ["a"], null, { blocked: true, seconds: 600 }),
  ];
  const tracks = [
    track("shopping", [at("s1", leg(10, 300), [["buy1", "buy"]])]),
    track("mining", [at("m1", leg(20, 400, true), [["m1", "mine"]]), at("m2", leg(5, 100), [["m2", "mine"]])]),
  ];

  it("takes a kind of work across plans, a load ending at its refinery", () => {
    const v = focusView(steps, tracks, { kind: "activity", track: "mining" });
    expect(v.groups.map((g) => g.title)).toEqual(["This session"]);
    const [load] = v.groups[0].runs;
    expect(load.title).toBe("Mining in Pyro");
    expect(load.figure).toBe("25 Gm, ~8 min flying");
    expect(load.steps.map((f) => f.step.id)).toEqual(["m1", "m2", "refine:Pyro:0"]);
    expect(v.card).toEqual({
      distance: "25 Gm",
      what: "2 stops, 1 jump",
      tiles: [
        { value: "~8 min", label: "flying" },
        { value: "—", label: "mining" },
        { value: "2", label: "plans" },
        { value: "3", label: "steps" },
      ],
      route: "Place m1 → Place m2",
    });
  });

  it("takes one plan, with another plan's mining in its load dimmed and what waits apart", () => {
    const v = focusView(steps, tracks, { kind: "plan", plan: "a" });
    const runs = v.groups[0].runs;
    expect(runs.map((r) => r.title)).toEqual(["Shopping run", "Mining in Pyro"]);
    expect(runs[0].figure).toBe("10 Gm, ~5 min flying");
    // The mining run starts where the trip does: from the session's start.
    const from = focusView(steps, tracks, { kind: "activity", track: "mining" }, "New Babbage");
    expect(from.groups[0].runs[0].figure).toBe("25 Gm from New Babbage, ~8 min flying");
    expect(runs[1].steps.map((f) => [f.step.id, f.other])).toEqual([
      ["m1", false],
      ["m2", true],
      ["refine:Pyro:0", false],
    ]);
    expect(v.groups[1].runs[0].steps.map((f) => f.step.id)).toEqual(["craft1"]);
    expect(v.card.distance).toBe("35 Gm");
    expect(v.card.what).toBe("3 stops, 1 jump");
  });

  it("counts rocks for mining, and parts and aUEC for a shopping run", () => {
    const rocks = [
      ps("m1", "mine", ["a"], "m1", { rocks: { toBreak: 4, toScan: 15 }, scu: 9.5 }),
      ps("m2", "mine", ["b"], "m2", { rocks: { toBreak: 1, toScan: 5 }, scu: 2 }),
    ];
    const mining = focusView(rocks, tracks, { kind: "activity", track: "mining" });
    expect(mining.card.tiles.slice(1, 3)).toEqual([
      { value: "~20", label: "rocks to scan" },
      { value: "12", label: "SCU to mine" },
    ]);
    expect(stepFigure(rocks[0])).toBe("4 of ~15 rocks");

    const buys = [
      ps("buy1", "buy", ["a"], "s1", { parts: 2, cost: 768000 }),
      ps("buy2", "buy", ["b"], "s1", { parts: 7, cost: 560000 }),
    ];
    const shopping = focusView(buys, tracks, { kind: "activity", track: "shopping" });
    expect(shopping.card.tiles.slice(1)).toEqual([
      { value: "9", label: "parts to buy" },
      { value: "1.33 M", label: "aUEC" },
      { value: "2", label: "plans" },
    ]);
  });

  it("adds up the missions' time, saying whose times they are", () => {
    const missions = [
      ps("rep1", "rep", ["a"], "s1", { seconds: 1440, secondsFrom: "yours" }),
      ps("run1", "run", ["b"], null, { seconds: 600, secondsFrom: "game" }),
    ];
    const missionTracks = [track("missions", [at("s1", leg(59, 240), [["rep1", "rep"]])])];
    const v = focusView(missions, missionTracks, { kind: "activity", track: "missions" }, "New Babbage");
    expect(v.groups[0].runs[0].figure).toBe("59 Gm from New Babbage, ~4 min flying, ≈34 min on missions");
    expect(v.card.tiles[1]).toEqual({ value: "≈34 min", label: "missions, yours and estimates" });
    const mine = focusView(missions.slice(0, 1), missionTracks, { kind: "activity", track: "missions" });
    expect(mine.card.tiles[1]).toEqual({ value: "≈24 min", label: "missions, your times" });
  });

  it("says when nothing is ready", () => {
    const v = focusView(steps, tracks, { kind: "activity", track: "crafting" });
    expect(v.card.distance).toBe("Nothing ready");
    expect(v.card.what).toBe("1 waiting");
  });
});
