import { describe, expect, it } from "vitest";
import type { PatchNoteView } from "$lib/bindings";
import { aheadOfLive, testChannel } from "./testChannel";

function note(channel: string, version: string | null, build: string | null, time_ms = 0): PatchNoteView {
  return { key: `${channel}-${build}`, subject: "", version, channel, build, stage: null, author: "", time_ms };
}

const live = { version: "4.10.2", changelist: "12881860" };

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
    expect(t?.open).toBe(false);
    expect(t?.note.channel).toBe("PTU");
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
});
