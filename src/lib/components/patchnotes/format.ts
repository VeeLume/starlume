// Patch-note display helpers — pure, so the splitting rules are tested.

/** A headline item split for display: older articles (≤ 4.10.1) write one
 *  feature group per title — "Gameplay: Orison Relief Support - Ground
 *  Vehicle Soft Death - …" — so the label and the " - "-joined items come
 *  apart. Plain titles ("RSI Constellation Mk IV") stay one item. */
export interface HighlightGroup {
  label: string | null;
  items: string[];
}

export function highlightGroup(h: string): HighlightGroup {
  const colon = h.indexOf(": ");
  const label = colon > 0 && colon < 40 ? h.slice(0, colon) : null;
  const rest = label ? h.slice(colon + 2) : h;
  const items = rest
    .split(" - ")
    .map((s) => s.trim())
    .filter(Boolean);
  return { label, items: items.length ? items : [rest] };
}

/** "Oct 9" this year, "Mar 25, 2025" before; the user's formatting locale. */
export function shortDate(iso: string | null | undefined, fallback = "", now = new Date()): string {
  if (!iso) return fallback;
  const d = new Date(`${iso}T00:00:00`);
  if (Number.isNaN(d.getTime())) return fallback;
  return d.toLocaleDateString(
    [],
    d.getFullYear() === now.getFullYear()
      ? { month: "short", day: "numeric" }
      : { month: "short", day: "numeric", year: "numeric" },
  );
}
