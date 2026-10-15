// The dev cycle on the News page's second card — pure, so the rules are
// tested. The usual cycle: a version runs on PTU (waves), then release
// candidates, ships to LIVE, then gets hotfixes — while the next version's
// PTU starts. Three sources feed it: the Patch Notes forum threads in the
// staff feed (PTU / EPTU / Tech Preview), RSI's LIVE patch notes, and
// Hotfix Central (hotfixes get no thread of their own).

import type { HotfixEntryView, HotfixView, PatchNoteView, ReleaseView } from "$lib/bindings";
import { compareVersions } from "$lib/home/version";

export type CycleStage = "ptu" | "rc" | "live" | "hotfix";

/** The stages in order, for the stepper. */
export const STAGES: CycleStage[] = ["ptu", "rc", "live", "hotfix"];

/** Where a timeline row opens. */
export type CycleTarget =
  | { type: "post"; key: string }
  | { type: "notes"; url: string }
  | { type: "hotfix" };

export interface CycleEvent {
  key: string;
  /** For ordering; date-only sources sit at noon UTC. */
  time_ms: number;
  /** `test` (PTU, EPTU, Tech Preview), `live` (a release), `hotfix`. */
  kind: "test" | "live" | "hotfix";
  /** "PTU", "EPTU", "LIVE", "HOTFIX"; `null` for a server-only hotfix. */
  channel: string | null;
  version: string | null;
  /** "PTU RC2", "4.10.2 on LIVE", "Hotfix 12913470 on LIVE". */
  title: string;
  /** One line under the title: a build, the patch name, the first fix. */
  detail: string | null;
  target: CycleTarget;
}

export interface Cycle {
  /** The version the cycle is about: the newest seen. */
  version: string | null;
  stage: CycleStage;
  /** Hotfix Central says the HOTFIX channel is open. */
  hotfixOpen: boolean;
  /** Newest first. */
  events: CycleEvent[];
}

/** How far back the timeline reaches. */
export const WINDOW_MS = 30 * 24 * 3600 * 1000;

const dayMs = (iso: string) => Date.parse(`${iso}T12:00:00Z`);

function hotfixEvent(e: HotfixEntryView, i: number, version: string | null): CycleEvent | null {
  if (!e.date) return null;
  const build = e.builds[0] ?? "";
  const title =
    e.channel === "LIVE"
      ? `Hotfix ${build} on LIVE`
      : e.channel === "HOTFIX"
        ? `Hotfix ${build} on the HOTFIX channel`
        : `Server hotfix ${build}`;
  return {
    key: `hotfix:${i}:${build}`,
    time_ms: dayMs(e.date),
    kind: "hotfix",
    channel: e.channel === "LIVE" || e.channel === "HOTFIX" ? e.channel : null,
    version,
    title: title.replace(/\s+/g, " ").trim(),
    detail: e.items[0] ?? null,
    target: { type: "hotfix" },
  };
}

/** The cycle from the three sources, as of `now`. `null` when none has
 *  anything in the window. */
export function devCycle(
  notes: PatchNoteView[],
  releases: ReleaseView[],
  hotfix: HotfixView | null,
  now: number,
): Cycle | null {
  const since = now - WINDOW_MS;
  const events: CycleEvent[] = [];

  for (const n of notes) {
    // LIVE notices are RSI's articles' job; hotfix threads Hotfix Central's
    // once it was read.
    if (!n.channel || n.channel === "LIVE" || (hotfix && n.channel === "Hotfix")) continue;
    if (n.time_ms < since) continue;
    events.push({
      key: n.key,
      time_ms: n.time_ms,
      kind: "test",
      channel: n.channel.toUpperCase(),
      version: n.version,
      title: [n.channel, n.stage].filter(Boolean).join(" "),
      detail: n.build ? `${n.version ?? ""} · build ${n.build}`.replace(/^ · /, "") : n.version,
      target: { type: "post", key: n.key },
    });
  }
  for (const r of releases) {
    if (r.channel !== "LIVE" || !r.date || dayMs(r.date) < since) continue;
    events.push({
      key: `live:${r.key}`,
      time_ms: dayMs(r.date),
      kind: "live",
      channel: "LIVE",
      version: r.version,
      title: `${r.version ?? "?"} on LIVE`,
      detail: r.name,
      target: { type: "notes", url: r.article_url },
    });
  }
  (hotfix?.entries ?? []).forEach((e, i) => {
    const ev = hotfixEvent(e, i, hotfix?.version ?? null);
    if (ev && ev.time_ms >= since) events.push(ev);
  });

  if (!events.length) return null;
  events.sort((a, b) => b.time_ms - a.time_ms);

  const version = events
    .map((e) => e.version)
    .filter((v): v is string => !!v)
    .reduce<string | null>((max, v) => (!max || compareVersions(v, max) > 0 ? v : max), null);

  let stage: CycleStage = "ptu";
  const release = events.find((e) => e.kind === "live" && e.version === version);
  if (release) {
    const after = events.some(
      (e) => e.kind === "hotfix" && e.version === version && e.time_ms >= release.time_ms,
    );
    stage = after || (hotfix?.channel_open && hotfix.version === version) ? "hotfix" : "live";
  } else {
    const latest = events.find((e) => e.kind === "test" && e.version === version);
    stage = latest && /\bRC\d*\b/i.test(latest.title) ? "rc" : "ptu";
  }

  return { version, stage, hotfixOpen: hotfix?.channel_open === true, events };
}

/** The hotfix entry that names `build` (an install's changelist). */
export function hotfixFor(hotfix: HotfixView | null, build: string | null): HotfixEntryView | null {
  if (!hotfix || !build) return null;
  return hotfix.entries.find((e) => e.builds.includes(build)) ?? null;
}
