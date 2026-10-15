// Feed shaping for the dev tracker page — pure, so the grouping rules are
// tested. Chat is flat and grouped into runs; forum posts stay one card each.

import type { DevPostView } from "$lib/bindings";

/** Chat messages one developer sent in one lobby, close together. */
export interface ChatRun {
  kind: "run";
  key: string;
  author: string;
  author_id: string;
  place: string | null;
  /** Oldest first, the way a chat reads. */
  messages: DevPostView[];
}

export type FeedItem = ChatRun | { kind: "post"; key: string; post: DevPostView };

/** Messages further apart than this start a new run. */
export const RUN_GAP_MS = 45 * 60 * 1000;

/** Group a newest-first feed: consecutive chat messages by the same author
 *  in the same lobby, each within {@link RUN_GAP_MS} of the next, become one
 *  run. Forum posts and anything in between break a run. Callers group per
 *  day, so a run never spans midnight. */
export function groupFeed(posts: DevPostView[]): FeedItem[] {
  const out: FeedItem[] = [];
  for (const p of posts) {
    const last = out.at(-1);
    if (
      p.kind === "chat" &&
      last?.kind === "run" &&
      last.author_id === p.author_id &&
      last.place === p.place &&
      last.messages[0].time_ms - p.time_ms <= RUN_GAP_MS
    ) {
      last.messages.unshift(p);
    } else if (p.kind === "chat") {
      out.push({ kind: "run", key: p.key, author: p.author, author_id: p.author_id, place: p.place, messages: [p] });
    } else {
      out.push({ kind: "post", key: p.key, post: p });
    }
  }
  return out;
}

const HUES = ["--hue-sky", "--hue-apricot", "--hue-rose", "--hue-teal", "--hue-sand"];

/** A stable category hue for a name (monograms). */
export function nameHue(name: string): string {
  let h = 0;
  for (const c of name) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return `var(${HUES[h % HUES.length]})`;
}

/** Two letters for a monogram: the name without its CIG tag
 *  ("Nightrider-CIG" → "Ni", "CIG-SeanT" → "Se"). */
export function initials(name: string): string {
  const bare = name.replace(/^CIG[-_]|[-_]CIG$/i, "") || name;
  const letters = [...bare.replace(/[^\p{L}\p{N}]/gu, "")];
  if (!letters.length) return "?";
  return letters[0].toUpperCase() + (letters[1] ?? "").toLowerCase();
}

/** Text split around @mentions, for highlighting without `{@html}`. */
export function mentionParts(text: string): { text: string; mention: boolean }[] {
  const out: { text: string; mention: boolean }[] = [];
  const re = /(^|[\s(])(@[A-Za-z0-9_-]+)/g;
  let at = 0;
  for (const m of text.matchAll(re)) {
    const start = m.index + m[1].length;
    if (start > at) out.push({ text: text.slice(at, start), mention: false });
    out.push({ text: m[2], mention: true });
    at = start + m[2].length;
  }
  if (at < text.length) out.push({ text: text.slice(at), mention: false });
  return out;
}
