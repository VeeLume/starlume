// Home layout resolution — pure, so the ordering rules are testable without
// components. The saved layout is `AppSettings.home_cards` (Rust-side, so it
// survives the webview close — frontend rule 6); the available cards come
// from the shell plus the enabled modules.

import type { HomeCardPref } from "$lib/bindings";

/** The fields layout resolution needs from an available card. */
export interface CardSlot {
  /** `<owner>:<card>` — the id saved in the layout. */
  key: string;
  defaultVisible: boolean;
}

export type Placed<T extends CardSlot> = T & { visible: boolean };

/**
 * Order the available cards by the saved layout. Saved cards come first in
 * saved order with their saved visibility; cards the layout does not know
 * yet (a module enabled later) follow in registry order at their default
 * visibility. Saved ids that are not available are skipped.
 */
export function resolveLayout<T extends CardSlot>(available: T[], prefs: HomeCardPref[]): Placed<T>[] {
  const byKey = new Map(available.map((c) => [c.key, c]));
  const placed: Placed<T>[] = [];
  const seen = new Set<string>();
  for (const p of prefs) {
    const card = byKey.get(p.id);
    if (!card || seen.has(p.id)) continue;
    seen.add(p.id);
    placed.push({ ...card, visible: p.visible });
  }
  for (const card of available) {
    if (!seen.has(card.key)) placed.push({ ...card, visible: card.defaultVisible });
  }
  return placed;
}

/**
 * The layout to save for an arrangement. Saved entries for cards that are
 * not available right now (their module is disabled) are kept, each after
 * the card it followed before, so re-enabling the module restores its place.
 */
export function toPrefs(placed: Placed<CardSlot>[], previous: HomeCardPref[]): HomeCardPref[] {
  const out: HomeCardPref[] = placed.map((c) => ({ id: c.key, visible: c.visible }));
  const present = new Set(out.map((p) => p.id));
  let anchor: string | null = null;
  for (const p of previous) {
    if (present.has(p.id)) {
      anchor = p.id;
      continue;
    }
    const at: number = anchor === null ? 0 : out.findIndex((q) => q.id === anchor) + 1;
    out.splice(at, 0, p);
    present.add(p.id);
    anchor = p.id;
  }
  return out;
}

/** Move one card a step up (-1) or down (+1); out-of-range moves are no-ops. */
export function moveCard<T>(cards: T[], index: number, delta: -1 | 1): T[] {
  const to = index + delta;
  if (to < 0 || to >= cards.length) return cards;
  const next = cards.slice();
  [next[index], next[to]] = [next[to], next[index]];
  return next;
}
