// Owned ships, said one way on both pages that show them. My Items answers
// "what do I own and where is it"; Ships answers "what can this ship be".
// The words they share live here: the two loadouts a copy has in the game,
// where a copy came from, when a rental ends, why a copy has nothing fitted
// to read, and the links from one page to the same ship on the other.

import type { CopySourceView, NoReadView } from "$lib/bindings";

/** What a claim restores (stock until it was changed). */
export const SAVED = "Saved";
/** What is on the ship in the inventory's last read. */
export const FITTED = "Fitted now";

/** Where a copy came from; nothing for a source the game doesn't name. */
export const SOURCE: Record<string, string> = { pledge: "Pledge", in_game: "In-game", rental: "Rental" };

/** Why a copy has no "Fitted now" (what the inventory says, no guess). */
export const NO_READ: Record<NoReadView, string> = {
  not_spawned: "not spawned, nothing to read",
  away: "location not readable",
  bricked: "bricked, nothing to read",
};

/** When a rental ends, the day with the time: "16 Nov 14:00". A rental
 *  runs for days, so the time alone doesn't say when. */
export function rentalEnd(epochSeconds: number, locales?: Intl.LocalesArgument): string {
  const d = new Date(epochSeconds * 1000);
  const day = d.toLocaleDateString(locales ?? [], { day: "numeric", month: "short" });
  const time = d.toLocaleTimeString(locales ?? [], { hour: "2-digit", minute: "2-digit" });
  return `${day} ${time}`;
}

/** What names a copy. */
export interface CopyFacts {
  source: CopySourceView;
  place: string | null;
  rentalExpiry: number | null;
}

/** A heading per copy of one ship class, by what the copy is: "Pledge,
 *  Area 18 hangar", "Rental, ends 16 Nov 14:00". "copy N" is added only to
 *  copies nothing else tells apart. */
export function copyHeadings(copies: CopyFacts[], end: (t: number) => string = rentalEnd): string[] {
  const plain = copies.map((c) =>
    [SOURCE[c.source], c.place, c.rentalExpiry ? `ends ${end(c.rentalExpiry)}` : null].filter(Boolean).join(", "),
  );
  const seen = new Map<string, number>();
  return plain.map((h) => {
    if (plain.filter((x) => x === h).length < 2) return h || "Your ship";
    const n = (seen.get(h) ?? 0) + 1;
    seen.set(h, n);
    return h ? `${h}, copy ${n}` : `Copy ${n}`;
  });
}

/** My Items, Ships view, on the ship with this key (its entitlement URN).
 *  A bricked leftover only shows there with "Show bricked" on. */
export function myItemsShipHref(key: string, bricked = false): string {
  const id = `ship:${key}`;
  const p = new URLSearchParams({ view: "ships", at: id, sel: id });
  if (bricked) p.set("bricked", "1");
  return `/items?${p}`;
}
