// Which test channel the News page's second card shows, and whether it is
// open — pure, so the rules are tested. The HOTFIX channel is open when
// Hotfix Central says so (hotfixes get no patch-note thread); PTU, EPTU and
// Tech Preview are open while their newest patch-note build is ahead of
// LIVE. Once nothing is open, the card shrinks to a quiet line.

import type { HotfixEntryView, HotfixView, PatchNoteView } from "$lib/bindings";
import { compareVersions } from "$lib/home/version";

export type TestChannel =
  /** From the patch-note threads; `open` = a build ahead of LIVE is out. */
  | { kind: "notes"; note: PatchNoteView; open: boolean }
  /** Hotfix Central says the HOTFIX channel is open; `entry` is what is on
   *  it (the newest HOTFIX entry, else the newest entry). */
  | { kind: "hotfix"; hotfix: HotfixView; entry: HotfixEntryView | null; open: true };

/** What LIVE is on (from RSI's patch notes); `null` = not known yet. */
export interface LiveBuild {
  version: string | null;
  changelist: string | null;
}

/** The note's build is ahead of LIVE: a newer version, or the same version
 *  with a newer build (hotfixes keep LIVE's version). Unknown LIVE counts
 *  as ahead — better a loud card than a hidden test cycle. */
export function aheadOfLive(note: PatchNoteView, live: LiveBuild | null): boolean {
  if (!live?.version) return true;
  if (!note.version) return false;
  const c = compareVersions(note.version, live.version);
  if (c !== 0) return c > 0;
  const build = parseInt(note.build ?? "", 10);
  const liveBuild = parseInt(live.changelist ?? "", 10);
  return Number.isFinite(build) && Number.isFinite(liveBuild) && build > liveBuild;
}

/** The card's subject: an open HOTFIX channel, else the newest note on a
 *  channel that is open, else the newest test-channel note (closed).
 *  `notes` are newest first; LIVE notes are skipped (RSI's articles cover
 *  LIVE). Hotfix-titled notes are left to Hotfix Central when it was read. */
export function testChannel(
  notes: PatchNoteView[],
  live: LiveBuild | null,
  hotfix: HotfixView | null = null,
): TestChannel | null {
  if (hotfix?.channel_open) {
    const entry = hotfix.entries.find((e) => e.channel === "HOTFIX") ?? hotfix.entries[0] ?? null;
    return { kind: "hotfix", hotfix, entry, open: true };
  }
  const tests = notes.filter(
    (n) => n.channel && n.channel !== "LIVE" && !(hotfix && n.channel === "Hotfix"),
  );
  const open = tests.find((n) => aheadOfLive(n, live));
  if (open) return { kind: "notes", note: open, open: true };
  return tests[0] ? { kind: "notes", note: tests[0], open: false } : null;
}

/** The newest hotfix that went to LIVE after the LIVE notes' date — what
 *  changed since the notes, for the quiet card. */
export function liveHotfixSince(hotfix: HotfixView | null, notesDate: string | null): HotfixEntryView | null {
  const entry = hotfix?.entries.find((e) => e.channel === "LIVE") ?? null;
  if (!entry?.date || !notesDate) return null;
  return entry.date >= notesDate ? entry : null;
}

/** The hotfix entry that names `build` (an install's changelist). */
export function hotfixFor(hotfix: HotfixView | null, build: string | null): HotfixEntryView | null {
  if (!hotfix || !build) return null;
  return hotfix.entries.find((e) => e.builds.includes(build)) ?? null;
}
