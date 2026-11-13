import { describe, expect, it } from "vitest";
import type { Entry, EntryKind, Session, WalletChange } from "$lib/bindings";
import {
  duration,
  entryAmount,
  entryGroup,
  entrySubject,
  entryTitle,
  missingSources,
  requestNote,
  sessionLength,
  signedAuec,
  summaryLine,
  timelineRows,
} from "./model";

// Every id and name below is made up, in the shape the game writes them.

const T0 = 1_794_586_248_019;

function entry(seq: number, at: number, kind: EntryKind, source: Entry["source"] = "log"): Entry {
  return { seq, at, source, locale_dependent: false, kind };
}

function session(timeline: Entry[], changes: WalletChange[] = []): Session {
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
    shards: [],
    timeline,
    timeline_dropped: 0,
    wallet: {
      opening: { at: T0, balance: 125_000 },
      latest: { at: T0, balance: 125_000 },
      net: 0,
      changes,
      changes_dropped: 0,
      zero_since: null,
      opened_after_zero: false,
    },
    next_seq: timeline.length,
  };
}

const purchase: EntryKind = {
  type: "shop_purchase",
  shop: "SCShop_NewBabbage_Tools",
  item_guid: "5e6f7a8b-aaaa-4bbb-8ccc-ddddeeeeffff",
  item_name: "grin_tractor_01",
  price: 19_175,
  quantity: 1,
  rental: false,
  outcome: "confirmed",
};

describe("duration", () => {
  it("reads in hours and minutes", () => {
    expect(duration(0)).toBe("under a minute");
    expect(duration(59_999)).toBe("under a minute");
    expect(duration(45 * 60_000)).toBe("45 min");
    expect(duration(2 * 3_600_000)).toBe("2 h");
    expect(duration(2 * 3_600_000 + 14 * 60_000)).toBe("2 h 14 min");
    expect(duration(-5)).toBe("under a minute");
  });

  it("runs a live session until now", () => {
    expect(sessionLength({ started_at: 1_000, ended_at: 4_000 }, 9_000)).toBe(3_000);
    expect(sessionLength({ started_at: 1_000, ended_at: null }, 9_000)).toBe(8_000);
  });
});

describe("signedAuec", () => {
  it("carries its sign", () => {
    expect(signedAuec(500)).toMatch(/^\+500 aUEC$/);
    expect(signedAuec(-500)).toMatch(/^−500 aUEC$/);
    expect(signedAuec(0)).toBe("±0 aUEC");
  });
});

describe("entries", () => {
  it("name what happened and what to", () => {
    const ended: EntryKind = {
      type: "mission_ended",
      mission_id: "0a1b2c3d-1111-4222-8333-444455556666",
      title: null,
      contract: "Covalex_LocalDelivery_Small",
      outcome: "abandoned",
    };
    expect(entryTitle(ended)).toBe("Mission abandoned");
    expect(entrySubject(ended)).toBe("Covalex_LocalDelivery_Small");
    expect(entryGroup(ended)).toBe("missions");

    expect(entryTitle(purchase)).toBe("Bought");
    expect(entrySubject(purchase)).toBe("grin_tractor_01");
    expect(entrySubject({ ...purchase, quantity: 3 })).toBe("3 × grin_tractor_01");
    expect(entryTitle({ ...purchase, rental: true })).toBe("Rented");
    expect(entryGroup(purchase)).toBe("money");

    expect(entryTitle({ type: "objective", change: "New", mission_id: null, text: "x" })).toBe(
      "New objective",
    );
    expect(entrySubject({ type: "died" })).toBeNull();
    expect(entryGroup({ type: "died" })).toBe("life");
    expect(entryGroup({ type: "vehicle_left", class_name: "a", name: "b" })).toBe("vehicles");
  });

  it("sign the amount the log states", () => {
    expect(entryAmount(purchase)).toBe(-19_175);
    expect(entryAmount({ type: "money", change: "Awarded", amount: 500 })).toBe(500);
    expect(entryAmount({ type: "money", change: "Fined", amount: 72_000 })).toBe(-72_000);
    expect(entryAmount({ type: "died" })).toBeNull();
  });

  it("say when a request was refused or never confirmed", () => {
    expect(requestNote(purchase)).toBeNull();
    expect(requestNote({ ...purchase, outcome: "rejected" })).toBe("refused");
    expect(requestNote({ ...purchase, outcome: "requested" })).toBe("not confirmed");
    expect(requestNote({ type: "died" })).toBeNull();
  });
});

describe("timelineRows", () => {
  it("puts an explained change on its entry and an untracked one on a row of its own", () => {
    const explained: WalletChange = {
      at: T0 + 60_300,
      from: 125_000,
      to: 105_825,
      delta: -19_175,
      cause: { type: "entry", seq: 0 },
    };
    const untracked: WalletChange = {
      at: T0 + 30_000,
      from: 105_825,
      to: 105_000,
      delta: -825,
      cause: { type: "untracked" },
    };
    const rows = timelineRows(
      session(
        [entry(0, T0 + 60_000, purchase), entry(1, T0 + 90_000, { type: "died" }, "telemetry")],
        [explained, untracked],
      ),
    );
    expect(rows.map((r) => r.title)).toEqual(["Balance changed", "Bought", "Died"]);
    expect(rows[0].note).toBe("untracked");
    expect(rows[0].change).toBe(untracked);
    expect(rows[0].source).toBe("telemetry");
    expect(rows[1].change).toBe(explained);
    expect(rows[1].stated).toBe(-19_175);
    expect(rows[2].change).toBeNull();
    expect(new Set(rows.map((r) => r.key)).size).toBe(3);
  });
});

describe("sources", () => {
  it("say what a session with one source lacks", () => {
    expect(missingSources({ log: true, telemetry: true })).toBeNull();
    expect(missingSources({ log: true, telemetry: false })).toContain("no wallet");
    expect(missingSources({ log: false, telemetry: true })).toContain("no shards");
  });
});

describe("summaryLine", () => {
  it("is the length, the missions and the last shard", () => {
    const line = summaryLine({
      id: "2026-11-13T16-10-48.019Z",
      started_at: T0,
      ended_at: T0 + 2 * 3_600_000,
      channel: "LIVE",
      branch: null,
      build: null,
      sources: { log: true, telemetry: false },
      shards: ["pub_euw1b_13165946_100", "pub_euw1b_13165946_042"],
      entries: 12,
      missions_completed: 2,
      opening_balance: null,
      latest_balance: null,
      net: 0,
    });
    expect(line).toBe("2 h · 2 missions completed · pub_euw1b_13165946_042");
  });
});
