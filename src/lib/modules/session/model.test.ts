import { describe, expect, it } from "vitest";
import type {
  DisplayName,
  Entry,
  EntryKind,
  Session,
  SessionNames,
  SessionSummary,
  WalletChange,
} from "$lib/bindings";
import {
  PENDING,
  auec,
  clock,
  count,
  duration,
  mergeNames,
  missingSources,
  nameQuery,
  namesFrom,
  readSession,
  sessionLength,
  signed,
  signedAuec,
  summaryParts,
  unanswered,
  walletLine,
  type LogRow,
} from "./model";

// Every id and name below is made up, in the shape the game writes them.

const T0 = 1_794_586_248_019;
const MIN = 60_000;

const MISSION = "9d8c7b6a-1111-4222-8333-444455556666";
const ITEM = "5e6f7a8b-aaaa-4bbb-8ccc-ddddeeeeffff";
const RESOURCE = "0a1b2c3d-aaaa-4bbb-8ccc-ddddeeeeffff";

function entry(seq: number, at: number, kind: EntryKind, source: Entry["source"] = "log"): Entry {
  return { seq, at, source, locale_dependent: false, kind };
}

function session(timeline: Entry[], changes: WalletChange[] = [], over: Partial<Session> = {}): Session {
  const last = changes.at(-1)?.to ?? 125_000;
  return {
    id: "2026-11-13T16-10-48.019Z",
    started_at: T0,
    ended_at: null,
    last_activity_at: T0,
    live: true,
    end_reason: null,
    channel: "LIVE",
    env_tag: "PUB",
    branch: "sc-alpha-4.10.0",
    build: "13165946",
    sources: { log: true, telemetry: true },
    shards: [{ shard: "pub_euw1b_13165946_100", joined_at: T0, left_at: null }],
    timeline,
    timeline_dropped: 0,
    wallet: {
      opening: { at: T0, balance: 125_000 },
      latest: { at: T0, balance: last },
      net: last - 125_000,
      changes,
      changes_dropped: 0,
      zero_since: null,
      opened_after_zero: false,
    },
    next_seq: timeline.length,
    ...over,
  };
}

function change(at: number, from: number, to: number, seq: number | null): WalletChange {
  return {
    at,
    from,
    to,
    delta: to - from,
    cause: seq == null ? { type: "untracked" } : { type: "entry", seq },
  };
}

const named = (name: string, parents: string[] = []): DisplayName => ({ name, parents, resolved: true });

const book: SessionNames = {
  data_loaded: true,
  places: {
    Vega2_NewCorvo: named("New Corvo", ["Aremis", "Vega"]),
    Vega3_Tarnhold: named("Tarnhold", ["Brell", "Vega"]),
  },
  shops: { SCShop_TinkersBench_NewCorvo: { name: "Tinkers Bench", parents: [], resolved: false } },
  vehicles: { VEGA_Courier_Mk2: named("Vega Courier Mk II") },
  items: { [ITEM]: named("Pocket Multitool") },
  resources: { [RESOURCE]: named("Quartz") },
  contracts: { Vega_EscortHauler: { name: "Vega Escort Hauler", parents: [], resolved: false } },
  destinations: { NewCorvo_LOC: { name: "New Corvo", parents: [], resolved: false } },
  item_classes: {
    tnkr_rifle_01: named("Tinker Rifle"),
    tnkr_optics_x4_s2: named("Tinker 4x Scope"),
    tnkr_barrel_supp_s2: named("Tinker Suppressor"),
  },
};
const names = namesFrom(book);

const location = (place: string): EntryKind => ({ type: "location", location: place });
const jurisdiction = (j: string): EntryKind => ({ type: "jurisdiction", jurisdiction: j });
const entered: EntryKind = {
  type: "vehicle_entered",
  class_name: "VEGA_Courier_Mk2",
  name: "VEGA_Courier_Mk2_1234",
};
const left: EntryKind = { ...entered, type: "vehicle_left" };
const purchase: EntryKind = {
  type: "shop_purchase",
  shop: "SCShop_TinkersBench_NewCorvo",
  item_guid: ITEM,
  item_name: "tnkr_multitool_01",
  price: 19_000,
  quantity: 4,
  rental: false,
  outcome: "confirmed",
};

const trip: EntryKind = {
  type: "quantum_travel",
  from: "Tarn Gateway",
  to: "NewCorvo_LOC",
  vehicle: "VEGA_Courier_Mk2",
  arrived_at: T0 + 9 * MIN,
};
const attachments: EntryKind = {
  type: "attachments",
  weapon: "tnkr_rifle_01",
  holder_port: "wep_stocked_2",
  items: [
    { item: "tnkr_barrel_supp_s2", port: "barrel_attach", at: T0 + 3 * MIN },
    { item: "tnkr_optics_x4_s2", port: "optics_attach", at: T0 + 4 * MIN },
  ],
};

const titles = (rows: LogRow[]) => rows.map((r) => r.title);
const texts = (r: LogRow) => r.meta.map((m) => m.text);

describe("formatting", () => {
  it("says a duration in hours and minutes", () => {
    expect(duration(30_000)).toBe("under a minute");
    expect(duration(45 * MIN)).toBe("45 min");
    expect(duration(120 * MIN)).toBe("2 h");
    expect(duration(134 * MIN)).toBe("2 h 14 min");
  });

  it("signs an amount", () => {
    expect(signed(0)).toBe("±0");
    expect(signed(-825)).toMatch(/^−825$/);
    expect(signedAuec(25)).toBe("+25 aUEC");
  });

  it("counts with the right word", () => {
    expect(count(1, "shard")).toBe("1 shard");
    expect(count(3, "shard")).toBe("3 shards");
    expect(count(2, "log entry", "log entries")).toBe("2 log entries");
  });

  it("times a live session until now and an ended one until its end", () => {
    expect(sessionLength({ started_at: T0, ended_at: null }, T0 + 5 * MIN)).toBe(5 * MIN);
    expect(sessionLength({ started_at: T0, ended_at: T0 + MIN }, T0 + 5 * MIN)).toBe(MIN);
  });
});

describe("names", () => {
  const s = session([
    entry(0, T0, location("Vega2_NewCorvo")),
    entry(1, T0 + 1, purchase),
    entry(2, T0 + 2, entered, "telemetry"),
    entry(3, T0 + 3, left, "telemetry"),
    entry(4, T0 + 4, {
      type: "trade",
      side: "Sell",
      shop: "SCShop_TinkersBench_NewCorvo",
      resource_guid: RESOURCE,
      price: 900,
      quantity: 3,
      quantity_unit: "SCU",
      outcome: "requested",
    }),
    entry(5, T0 + 5, {
      type: "mission_ended",
      mission_id: MISSION,
      title: null,
      contract: "Vega_EscortHauler",
      outcome: "failed",
    }),
    entry(6, T0 + 6, trip),
    entry(7, T0 + 7, attachments),
  ]);

  it("asks for every internal name once", () => {
    expect(nameQuery(s)).toEqual({
      channel: "LIVE",
      places: ["Vega2_NewCorvo"],
      shops: ["SCShop_TinkersBench_NewCorvo"],
      vehicles: ["VEGA_Courier_Mk2"],
      items: [{ guid: ITEM, class_name: "tnkr_multitool_01" }],
      resources: [RESOURCE],
      contracts: ["Vega_EscortHauler"],
      destinations: ["NewCorvo_LOC"],
      item_classes: ["tnkr_rifle_01", "tnkr_barrel_supp_s2", "tnkr_optics_x4_s2"],
    });
  });

  it("asks only for what the store lacks", () => {
    expect(unanswered(nameQuery(s), book)).toBeNull();
    const rest = unanswered(nameQuery(s), { ...book, vehicles: {}, items: {} });
    expect(rest?.vehicles).toEqual(["VEGA_Courier_Mk2"]);
    expect(rest?.items).toEqual([{ guid: ITEM, class_name: "tnkr_multitool_01" }]);
    expect(rest?.places).toEqual([]);
    expect(unanswered(nameQuery(s), null)?.places).toEqual(["Vega2_NewCorvo"]);
  });

  it("adds new answers to the ones it has", () => {
    const more: SessionNames = {
      ...book,
      data_loaded: false,
      places: { Vega9_Far: named("Far") },
      shops: {},
    };
    const merged = mergeNames(book, more);
    expect(Object.keys(merged.places).sort()).toEqual(["Vega2_NewCorvo", "Vega3_Tarnhold", "Vega9_Far"]);
    expect(merged.shops).toEqual(book.shops);
    expect(merged.data_loaded).toBe(false);
  });

  it("never shows an internal name while the answer is on its way", () => {
    const rows = readSession(s, namesFrom(null), T0 + MIN).rows;
    expect(rows.find((r) => r.kind === "place")?.title).toBe(PENDING);
    expect(JSON.stringify(rows)).not.toContain("Vega2_NewCorvo");
    expect(JSON.stringify(rows)).not.toContain("SCShop_");
    expect(JSON.stringify(rows)).not.toContain("VEGA_Courier");
    expect(JSON.stringify(rows)).not.toContain("_LOC");
    expect(JSON.stringify(rows)).not.toContain("tnkr_");
  });
});

describe("readSession", () => {
  it("starts with the session's own row, and ends with one once it is over", () => {
    const live = readSession(session([]), names, T0 + MIN);
    expect(titles(live.rows)).toEqual(["Session started"]);
    expect(texts(live.rows[0])).toEqual(["joined a shard", "LIVE", "Game.log and telemetry"]);

    const over = readSession(
      session([], [], { live: false, ended_at: T0 + 115 * MIN, end_reason: "Quit" }),
      names,
      T0 + 500 * MIN,
    );
    expect(titles(over.rows)).toEqual(["Session started", "Session ended"]);
    expect(texts(over.rows[1])).toEqual(["1 h 55 min", "1 shard"]);
  });

  it("makes a place a row with what it lies in, and jurisdictions words on it", () => {
    const reading = readSession(
      session([
        entry(1, T0 + MIN, jurisdiction("Free Vega")),
        entry(2, T0 + MIN, location("Vega2_NewCorvo")),
        // On foot at the place: a second word for it.
        entry(3, T0 + 2 * MIN, jurisdiction("Corvo Council")),
        entry(4, T0 + 3 * MIN, entered, "telemetry"),
        // On the way: not the place's.
        entry(5, T0 + 4 * MIN, jurisdiction("Open Space")),
        entry(6, T0 + 9 * MIN, jurisdiction("Brell Mining")),
        entry(7, T0 + 10 * MIN, location("Vega3_Tarnhold")),
      ]),
      names,
      T0 + 11 * MIN,
    );
    const places = reading.rows.filter((r) => r.kind === "place");
    expect(titles(places)).toEqual(["New Corvo", "Tarnhold"]);
    expect(places[0].meta).toEqual([
      { text: "Aremis" },
      { text: "Vega" },
      { text: "Free Vega", chip: true },
      { text: "Corvo Council", chip: true },
    ]);
    expect(places[1].meta.filter((m) => m.chip)).toEqual([{ text: "Brell Mining", chip: true }]);
    // No row for a jurisdiction change.
    expect(reading.rows).toHaveLength(4);
    expect(reading.status.where).toEqual({
      name: "Tarnhold",
      system: "Vega",
      jurisdictions: ["Brell Mining"],
      arrivedAt: T0 + 10 * MIN,
    });
  });

  it("keeps only the arrival's jurisdiction without telemetry", () => {
    const reading = readSession(
      session(
        [
          entry(1, T0 + MIN, jurisdiction("Free Vega")),
          entry(2, T0 + MIN, location("Vega2_NewCorvo")),
          entry(3, T0 + 5 * MIN, jurisdiction("Open Space")),
        ],
        [],
        { sources: { log: true, telemetry: false } },
      ),
      names,
      T0 + 6 * MIN,
    );
    expect(reading.status.where?.jurisdictions).toEqual(["Free Vega"]);
  });

  it("folds entering and leaving a vehicle into one stay with its route", () => {
    const reading = readSession(
      session([
        entry(1, T0 + MIN, location("Vega2_NewCorvo")),
        entry(2, T0 + 2 * MIN, entered, "telemetry"),
        entry(3, T0 + 15 * MIN, location("Vega3_Tarnhold")),
        entry(4, T0 + 21 * MIN, left, "telemetry"),
        entry(5, T0 + 30 * MIN, entered, "telemetry"),
        entry(6, T0 + 32 * MIN, left, "telemetry"),
      ]),
      names,
      T0 + 40 * MIN,
    );
    const stays = reading.rows.filter((r) => r.kind === "ship");
    expect(titles(stays)).toEqual(["Vega Courier Mk II", "Vega Courier Mk II"]);
    expect(stays[0].took).toBe(19 * MIN);
    expect(texts(stays[0])).toEqual([`until ${clock(T0 + 21 * MIN)}`, "New Corvo → Tarnhold"]);
    // Boarded and left at one place: just the place.
    expect(texts(stays[1])).toEqual([`until ${clock(T0 + 32 * MIN)}`, "Tarnhold"]);
    expect(reading.status.ship).toEqual({
      aboard: null,
      last: { name: "Vega Courier Mk II", leftAt: T0 + 32 * MIN, took: 2 * MIN },
      stays: 2,
    });
  });

  it("shows a stay still going as running, and closes it with an ended session", () => {
    const timeline = [entry(1, T0 + 2 * MIN, entered, "telemetry")];
    const live = readSession(session(timeline), names, T0 + 10 * MIN);
    const stay = live.rows.find((r) => r.kind === "ship")!;
    expect(stay.running).toBe(true);
    expect(stay.took).toBe(8 * MIN);
    expect(live.status.ship.aboard).toEqual({ name: "Vega Courier Mk II", since: T0 + 2 * MIN });

    const over = readSession(
      session(timeline, [], { live: false, ended_at: T0 + 20 * MIN }),
      names,
      T0 + 99 * MIN,
    );
    const closed = over.rows.find((r) => r.kind === "ship")!;
    expect(closed.running).toBe(false);
    expect(closed.took).toBe(18 * MIN);
    expect(over.status.ship.aboard).toBeNull();
  });

  const missionEntries = [
    entry(1, T0 + MIN, { type: "mission_accepted", mission_id: MISSION, title: "Escort the Hauler" }),
    entry(2, T0 + MIN, { type: "objective", change: "New", mission_id: MISSION, text: "Meet the hauler" }),
    entry(3, T0 + 5 * MIN, {
      type: "objective",
      change: "Completed",
      mission_id: MISSION,
      text: "Meet the hauler",
    }),
    entry(4, T0 + 5 * MIN, { type: "objective", change: "New", mission_id: MISSION, text: "Fly along" }),
    entry(5, T0 + 9 * MIN, { type: "objective", change: "New", mission_id: MISSION, text: "Land" }),
  ];

  it("makes a running mission one row with its objectives", () => {
    const reading = readSession(session(missionEntries), names, T0 + 11 * MIN);
    const missions = reading.rows.filter((r) => r.kind === "mission");
    expect(missions).toHaveLength(1);
    const m = missions[0];
    expect(m.title).toBe("Escort the Hauler");
    expect(texts(m)).toEqual(["running", "objective 3"]);
    expect(m.running).toBe(true);
    expect(m.took).toBe(10 * MIN);
    expect(m.steps.map((s) => s.state)).toEqual(["done", "done", "current"]);
    expect(m.facets).toEqual(["missions"]);
    expect(reading.status.mission.running).toEqual({
      title: "Escort the Hauler",
      objective: "Land",
      objectiveNo: 3,
      objectiveSince: T0 + 9 * MIN,
      acceptedAt: T0 + MIN,
    });
  });

  it("puts a finished mission's payment on its row", () => {
    const ended = entry(6, T0 + 20 * MIN, {
      type: "mission_ended",
      mission_id: MISSION,
      title: "Escort the Hauler",
      contract: "Vega_EscortHauler",
      outcome: "completed",
    });
    const reading = readSession(
      session([...missionEntries, ended], [change(T0 + 20 * MIN, 125_000, 158_750, 6)]),
      names,
      T0 + 30 * MIN,
    );
    const m = reading.rows.find((r) => r.kind === "mission")!;
    expect(texts(m)).toEqual(["completed", "3 objectives"]);
    expect(m.took).toBe(19 * MIN);
    expect(m.running).toBe(false);
    expect(m.amount).toBe(33_750);
    expect(m.facets).toEqual(["missions", "money"]);
    expect(m.steps.every((s) => s.state === "done")).toBe(true);
    expect(reading.status.mission).toEqual({
      running: null,
      last: { title: "Escort the Hauler", outcome: "completed", endedAt: T0 + 20 * MIN, amount: 33_750 },
      count: 1,
    });
    expect(reading.changes).toBe(1);
    expect(reading.explained).toBe(1);
  });

  it("names a mission that only ended by its contract", () => {
    const reading = readSession(
      session([
        entry(1, T0 + MIN, {
          type: "mission_ended",
          mission_id: MISSION,
          title: null,
          contract: "Vega_EscortHauler",
          outcome: "failed",
        }),
      ]),
      names,
      T0 + 2 * MIN,
    );
    const m = reading.rows.find((r) => r.kind === "mission")!;
    expect(m.title).toBe("Vega Escort Hauler");
    expect(texts(m)).toEqual(["failed"]);
  });

  it("does not call a mission running once the session is over", () => {
    const reading = readSession(
      session(missionEntries, [], { live: false, ended_at: T0 + 30 * MIN }),
      names,
      T0 + 99 * MIN,
    );
    const m = reading.rows.find((r) => r.kind === "mission")!;
    expect(m.running).toBe(false);
    expect(texts(m)[0]).toBe("open when the session ended");
    expect(reading.status.mission.running).toBeNull();
  });

  it("reads a purchase with its shop, its place and what it cost", () => {
    const reading = readSession(
      session(
        [entry(1, T0 + MIN, location("Vega2_NewCorvo")), entry(2, T0 + 3 * MIN, purchase)],
        [change(T0 + 3 * MIN, 125_000, 106_000, 2)],
      ),
      names,
      T0 + 5 * MIN,
    );
    const buy = reading.rows.find((r) => r.kind === "buy")!;
    expect(buy.title).toBe("4 × Pocket Multitool");
    expect(texts(buy)).toEqual(["Tinkers Bench", "New Corvo"]);
    expect(buy.amount).toBe(-19_000);
    expect(buy.facets).toEqual(["money"]);
    const fact = (label: string) => buy.facts.find((f) => f.label === label)?.value;
    expect(fact("Quantity")).toBe("4");
    expect(fact("Each")).toBe(auec(4_750));
    expect(fact("Where")).toBe("New Corvo, Aremis, Vega");
    expect(buy.sources.map((s) => s.label)).toEqual(["Bought", "Charged"]);
  });

  it("gives a refused purchase a word and no amount", () => {
    const refused: EntryKind = { ...purchase, outcome: "rejected" } as EntryKind;
    const buy = readSession(session([entry(1, T0 + MIN, refused)]), names, T0 + 2 * MIN).rows.find(
      (r) => r.kind === "buy",
    )!;
    expect(texts(buy)).toContain("refused");
    expect(buy.amount).toBeNull();
  });

  it("gives a balance change nothing explains its own row", () => {
    const reading = readSession(
      session([], [change(T0 + 4 * MIN, 125_000, 124_000, null), change(T0 + 6 * MIN, 124_000, 99_000, 77)]),
      names,
      T0 + 8 * MIN,
    );
    const money = reading.rows.filter((r) => r.kind === "money");
    // The second names an entry that is no longer on the timeline.
    expect(money).toHaveLength(2);
    expect(money[0].title).toBe("Balance changed");
    expect(money[0].amount).toBe(-1_000);
    expect(reading.explained).toBe(0);
    expect(reading.changes).toBe(2);
  });

  it("makes a quantum trip one row with where from, the ship and how long", () => {
    const reading = readSession(
      session([
        entry(1, T0 + 5 * MIN, trip),
        entry(2, T0 + 20 * MIN, { ...trip, from: null, vehicle: null, arrived_at: null }),
      ]),
      names,
      T0 + 30 * MIN,
    );
    const [flown, set] = reading.rows.filter((r) => r.kind === "ship");
    expect(flown.title).toBe("Quantum travel to New Corvo");
    expect(texts(flown)).toEqual(["from Tarn Gateway", "Vega Courier Mk II"]);
    expect(flown.took).toBe(4 * MIN);
    expect(flown.facets).toEqual(["travel"]);
    expect(flown.facts.map((f) => f.label)).toEqual([
      "From",
      "To",
      "Ship",
      "Course set",
      "Arrived",
      "Course set to arrival",
    ]);
    // A course that was set, and no arrival: not called a trip.
    expect(set.title).toBe("Course set for New Corvo");
    expect(texts(set)).toEqual(["no arrival seen"]);
    expect(set.took).toBeNull();
    // A trip is not a stay in a ship.
    expect(reading.status.ship.stays).toBe(0);
  });

  it("makes the attachments put on one weapon one row", () => {
    const one: EntryKind = { ...attachments, weapon: null, items: attachments.items.slice(0, 1) };
    const reading = readSession(
      session([
        entry(1, T0 + MIN, location("Vega2_NewCorvo")),
        entry(2, T0 + 3 * MIN, attachments),
        entry(3, T0 + 8 * MIN, one),
      ]),
      names,
      T0 + 10 * MIN,
    );
    const [several, single] = reading.rows.filter((r) => r.title.startsWith("Fitted"));
    expect(several.title).toBe("Fitted 2 attachments");
    expect(texts(several)).toEqual(["on Tinker Rifle", "Tinker Suppressor", "Tinker 4x Scope"]);
    expect(several.facts).toEqual([
      { label: "Weapon", value: "Tinker Rifle" },
      { label: "Barrel", value: "Tinker Suppressor" },
      { label: "Optics", value: "Tinker 4x Scope" },
      { label: "Where", value: "New Corvo, Aremis, Vega" },
    ]);
    expect(several.sources.map((s) => s.label)).toEqual(["Asked for", "Last one"]);
    // One attachment, on a weapon the log never named.
    expect(single.title).toBe("Fitted Tinker Suppressor");
    expect(texts(single)).toEqual([]);
  });

  it("keeps the rows in time order", () => {
    const reading = readSession(
      session(
        [
          entry(1, T0 + 5 * MIN, location("Vega2_NewCorvo")),
          entry(2, T0 + 9 * MIN, { type: "died" }, "telemetry"),
        ],
        [change(T0 + 7 * MIN, 125_000, 124_000, null)],
      ),
      names,
      T0 + 10 * MIN,
    );
    expect(titles(reading.rows)).toEqual(["Session started", "New Corvo", "Balance changed", "Died"]);
    expect(reading.entries).toBe(2);
  });
});

describe("walletLine", () => {
  it("is nothing without a balance", () => {
    const s = session([]);
    s.wallet = { ...s.wallet, opening: null, latest: null };
    expect(walletLine(s, T0 + MIN, 300, 36)).toBeNull();
  });

  it("draws a flat line through the middle when nothing changed", () => {
    const line = walletLine(session([]), T0 + 60 * MIN, 300, 36)!;
    expect(line.step).toBe("M0 18 H300");
    expect(line.base).toBe(18);
  });

  it("steps at each change, from the opening balance", () => {
    const s = session(
      [],
      [change(T0 + 30 * MIN, 125_000, 100_000, null), change(T0 + 45 * MIN, 100_000, 150_000, null)],
      { live: false, ended_at: T0 + 60 * MIN },
    );
    const line = walletLine(s, T0 + 999 * MIN, 300, 36)!;
    // 150,000 is the top (2), 100,000 the bottom (34), 125,000 between.
    expect(line.step).toBe("M0 18 H150 V34 H225 V2 H300");
    expect(line.area).toBe("M0 18 H150 V34 H225 V2 H300 V18 H0 Z");
    expect(line.ticks[0]).toEqual({ label: clock(T0), pos: 0 });
    expect(line.ticks.at(-1)).toEqual({ label: clock(T0 + 60 * MIN), pos: 1 });
    expect(line.ticks).toHaveLength(3);
    expect(line.ticks[1].pos).toBeGreaterThanOrEqual(0.3);
    expect(line.ticks[1].pos).toBeLessThanOrEqual(0.7);
  });

  it("says now on a running session's last tick", () => {
    const line = walletLine(session([]), T0 + 30 * MIN, 300, 36)!;
    expect(line.ticks.at(-1)?.label).toBe(`now ${clock(T0 + 30 * MIN)}`);
  });
});

describe("history", () => {
  const summary: SessionSummary = {
    id: "2026-11-13T16-10-48.019Z",
    started_at: T0,
    ended_at: T0 + 115 * MIN,
    channel: "LIVE",
    branch: "sc-alpha-4.10.0",
    build: "13165946",
    sources: { log: true, telemetry: true },
    shards: ["pub_euw1b_13165946_100"],
    entries: 47,
    missions_completed: 1,
    opening_balance: 125_000,
    latest_balance: 100_000,
    net: -25_000,
  };

  it("gives a session's facts one at a time", () => {
    expect(summaryParts(summary)).toEqual(["1 h 55 min", "1 mission completed", "1 shard"]);
    expect(summaryParts({ ...summary, missions_completed: 0, shards: [] })).toEqual(["1 h 55 min"]);
  });

  it("says which source a session lacks", () => {
    expect(missingSources({ log: true, telemetry: true })).toBeNull();
    expect(missingSources({ log: false, telemetry: true })).toBe("telemetry only");
    expect(missingSources({ log: true, telemetry: false })).toBe("Game.log only");
  });
});
