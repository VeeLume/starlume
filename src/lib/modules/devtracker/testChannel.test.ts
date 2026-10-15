import { describe, expect, it } from "vitest";
import type { HotfixEntryView, HotfixView, PatchNoteView } from "$lib/bindings";
import { aheadOfLive, hotfixFor, liveHotfixSince, testChannel } from "./testChannel";

function note(channel: string, version: string | null, build: string | null, time_ms = 0): PatchNoteView {
  return { key: `${channel}-${build}`, subject: "", version, channel, build, stage: null, author: "", time_ms };
}

const live = { version: "4.10.2", changelist: "12881860" };

function entry(channel: string | null, builds: string[], date: string): HotfixEntryView {
  return { date, headline: `Hotfix ${builds[0]}`, builds, channel, items: ["a fix"] };
}

function central(open: boolean | null, entries: HotfixEntryView[]): HotfixView {
  return { subject: "", version: "4.10.2", channel_open: open, live_client_cl: null, edited_ms: 0, entries };
}

describe("aheadOfLive", () => {
  it("compares versions first", () => {
    expect(aheadOfLive(note("PTU", "4.11", "1"), live)).toBe(true);
    expect(aheadOfLive(note("PTU", "4.10.2", "12878398"), live)).toBe(false);
    expect(aheadOfLive(note("PTU", "4.10.1", "99999999"), live)).toBe(false);
  });

  it("compares builds for the same version (hotfixes)", () => {
    expect(aheadOfLive(note("Hotfix", "4.10.2", "12890001"), live)).toBe(true);
    expect(aheadOfLive(note("Hotfix", "4.10.2", null), live)).toBe(false);
  });

  it("counts an unknown LIVE as ahead", () => {
    expect(aheadOfLive(note("PTU", "4.10.2", "1"), null)).toBe(true);
    expect(aheadOfLive(note("PTU", "4.10.2", "1"), { version: null, changelist: null })).toBe(true);
  });
});

describe("testChannel", () => {
  it("is closed once LIVE caught up", () => {
    const t = testChannel([note("LIVE", "4.10.2", "12881860"), note("PTU", "4.10.2", "12878398")], live);
    expect(t).toMatchObject({ kind: "notes", open: false, note: { channel: "PTU" } });
  });

  it("prefers an open channel over a newer closed note", () => {
    const t = testChannel([note("PTU", "4.10.2", "12878398"), note("Hotfix", "4.10.2", "12890001")], live);
    expect(t).toMatchObject({ open: true, note: { channel: "Hotfix" } });
  });

  it("opens for the next PTU cycle", () => {
    const t = testChannel([note("PTU", "4.11", "13000000"), note("Hotfix", "4.10.2", "12890001")], live);
    expect(t).toMatchObject({ open: true, note: { channel: "PTU", version: "4.11" } });
  });

  it("is null without test-channel notes", () => {
    expect(testChannel([note("LIVE", "4.10.2", "1")], live)).toBeNull();
  });

  it("takes an open HOTFIX channel from Hotfix Central first", () => {
    const hf = central(true, [entry(null, ["12939222"], "2026-10-14"), entry("HOTFIX", ["12950000"], "2026-10-15")]);
    const t = testChannel([note("PTU", "4.11", "13000000")], live, hf);
    expect(t).toMatchObject({ kind: "hotfix", open: true, entry: { builds: ["12950000"] } });
  });

  it("falls back to the notes while the HOTFIX channel is closed", () => {
    const hf = central(false, [entry("LIVE", ["12913470"], "2026-10-13")]);
    const t = testChannel([note("Hotfix", "4.10.2", "12890001"), note("PTU", "4.10.2", "12878398")], live, hf);
    expect(t).toMatchObject({ kind: "notes", open: false, note: { channel: "PTU" } });
  });
});

describe("hotfix lookups", () => {
  const hf = central(false, [
    entry(null, ["12939222"], "2026-10-14"),
    entry("LIVE", ["12913470"], "2026-10-13"),
    entry("LIVE", ["12572603"], "2026-09-08"),
  ]);

  it("finds the newest LIVE hotfix since the notes", () => {
    expect(liveHotfixSince(hf, "2026-10-09")?.builds).toEqual(["12913470"]);
    expect(liveHotfixSince(hf, "2026-10-20")).toBeNull();
    expect(liveHotfixSince(null, "2026-10-09")).toBeNull();
  });

  it("finds the entry for an install's build", () => {
    expect(hotfixFor(hf, "12913470")?.date).toBe("2026-10-13");
    expect(hotfixFor(hf, "12881860")).toBeNull();
    expect(hotfixFor(hf, null)).toBeNull();
  });
});
