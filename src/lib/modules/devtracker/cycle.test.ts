import { describe, expect, it } from "vitest";
import type { HotfixEntryView, HotfixView, PatchNoteView, ReleaseView } from "$lib/bindings";
import { devCycle, hotfixFor } from "./cycle";

const NOW = Date.parse("2026-10-15T08:00:00Z");
const at = (iso: string) => Date.parse(`${iso}T18:00:00Z`);

function note(stage: string | null, version: string, build: string, day: string, channel = "PTU"): PatchNoteView {
  return { key: `thread:${build}`, subject: "", version, channel, build, stage, author: "", time_ms: at(day) };
}

function release(version: string, date: string): ReleaseView {
  return {
    key: `${version}|${date}`,
    article_url: `https://robertsspaceindustries.com/${version}`,
    title: `Star Citizen Alpha ${version} LIVE`,
    version,
    channel: "LIVE",
    name: "RSI Discovery Month",
    date,
    date_text: date,
    build: null,
    changelist: null,
    build_info: [],
    highlights: [],
    bug_fixes: null,
    known_issues: null,
    image: null,
  };
}

function entry(channel: string | null, build: string, date: string): HotfixEntryView {
  return { date, headline: `Hotfix ${build}`, builds: [build], channel, items: [`fix in ${build}`] };
}

function central(open: boolean | null, entries: HotfixEntryView[], version = "4.10.2"): HotfixView {
  return { subject: "", version, channel_open: open, live_client_cl: null, edited_ms: 0, entries };
}

const ptu = [
  note("RC2", "4.10.2", "12878398", "2026-10-07"),
  note("RC1", "4.10.2", "12868798", "2026-10-06"),
  note(null, "4.10.2", "12858901", "2026-10-05"),
  note("Wave 3", "4.10.2", "12839153", "2026-10-01"),
];

describe("devCycle", () => {
  it("is on PTU, then RC, before the release", () => {
    expect(devCycle(ptu.slice(3), [], null, NOW)?.stage).toBe("ptu");
    expect(devCycle(ptu, [], null, NOW)?.stage).toBe("rc");
  });

  it("is LIVE after the release, hotfixes once one followed", () => {
    const releases = [release("4.10.2", "2026-10-09")];
    expect(devCycle(ptu, releases, null, NOW)?.stage).toBe("live");
    const hf = central(false, [entry(null, "12939222", "2026-10-14"), entry("LIVE", "12913470", "2026-10-13")]);
    const c = devCycle(ptu, releases, hf, NOW)!;
    expect(c.stage).toBe("hotfix");
    expect(c.version).toBe("4.10.2");
    expect(c.events.map((e) => e.title).slice(0, 4)).toEqual([
      "Server hotfix 12939222",
      "Hotfix 12913470 on LIVE",
      "4.10.2 on LIVE",
      "PTU RC2",
    ]);
  });

  it("moves to the next version's PTU when it starts", () => {
    const next = [note("Wave 1", "4.11", "13000000", "2026-10-14"), ...ptu];
    const hf = central(false, [entry("LIVE", "12913470", "2026-10-13")]);
    const c = devCycle(next, [release("4.10.2", "2026-10-09")], hf, NOW)!;
    expect(c.version).toBe("4.11");
    expect(c.stage).toBe("ptu");
  });

  it("flags an open HOTFIX channel", () => {
    const hf = central(true, [entry("HOTFIX", "12950000", "2026-10-15")]);
    const c = devCycle(ptu, [release("4.10.2", "2026-10-09")], hf, NOW)!;
    expect(c.hotfixOpen).toBe(true);
    expect(c.stage).toBe("hotfix");
    expect(c.events[0].title).toBe("Hotfix 12950000 on the HOTFIX channel");
  });

  it("leaves out what is older than the window, and hotfix threads once Hotfix Central was read", () => {
    const old = note("Wave 1", "4.10", "12000000", "2026-08-01");
    const thread = note(null, "4.10.2", "12890001", "2026-10-10", "Hotfix");
    const hf = central(false, []);
    const c = devCycle([thread, ...ptu, old], [], hf, NOW)!;
    expect(c.events.some((e) => e.key === old.key)).toBe(false);
    expect(c.events.some((e) => e.key === thread.key)).toBe(false);
    expect(devCycle([], [], null, NOW)).toBeNull();
  });
});

describe("hotfixFor", () => {
  it("finds the entry for an install's build", () => {
    const hf = central(false, [entry("LIVE", "12913470", "2026-10-13")]);
    expect(hotfixFor(hf, "12913470")?.date).toBe("2026-10-13");
    expect(hotfixFor(hf, "12881860")).toBeNull();
    expect(hotfixFor(null, "12913470")).toBeNull();
  });
});
