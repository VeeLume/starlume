// Which test channel the News page's second card shows, and whether it is
// open — pure, so the rules are tested. A test channel (PTU, EPTU, Hotfix,
// Tech Preview) is open while its newest build is ahead of LIVE; once LIVE
// has caught up, the card shrinks to a quiet line.

import type { PatchNoteView } from "$lib/bindings";
import { compareVersions } from "$lib/home/version";

export interface TestChannel {
  note: PatchNoteView;
  /** A build newer than LIVE is out on this channel. */
  open: boolean;
}

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

/** The card's subject: the newest note on a channel that is open, else the
 *  newest test-channel note (closed). `notes` are newest first; LIVE notes
 *  are skipped (RSI's articles cover LIVE). */
export function testChannel(notes: PatchNoteView[], live: LiveBuild | null): TestChannel | null {
  const tests = notes.filter((n) => n.channel && n.channel !== "LIVE");
  const open = tests.find((n) => aheadOfLive(n, live));
  if (open) return { note: open, open: true };
  return tests[0] ? { note: tests[0], open: false } : null;
}
