// How the Plans page words and draws what the board holds. Pure: no store,
// no IPC. Every figure here is one the board carries; nothing is estimated.

import type { LedgerRow, PlanStep, StepKind, Track, TrackKind, Trip, TripStop } from "$lib/bindings";

/** The tracks of the next-session card, one per kind of work, in the order
 *  the board picks the next one (decision 'Plans show parallel tracks by
 *  kind of work', 2026-11-16). */
export const TRACKS: { id: TrackKind; label: string }[] = [
  { id: "shopping", label: "Shopping run" },
  { id: "mining", label: "Mining" },
  { id: "refining", label: "Refinery" },
  { id: "missions", label: "Missions" },
  { id: "crafting", label: "Crafting" },
];

export function trackLabel(kind: TrackKind): string {
  return TRACKS.find((t) => t.id === kind)?.label ?? kind;
}

/** The card's line for a track without stops to fly; empty when its stops
 *  say it all. Short facts, no explanations (playtest 2026-11-16). */
export function trackNote(track: Track): string {
  if (track.trip.stops.length > 0) return "";
  if (track.waitsOn) return `Waits on ${trackLabel(track.waitsOn).toLowerCase()}`;
  if (track.ready === 0) return "Nothing ready";
  if (track.kind === "crafting") return `${track.ready} ready to craft`;
  return `${track.ready} ready, no place on the map`;
}

/** The word a step's kind is shown as. */
export const KIND_TEXT: Record<StepKind, string> = {
  fit: "fit",
  buy: "buy",
  mine: "mine",
  refine: "refine",
  gather: "get",
  craft: "craft",
  earn: "mission",
  find: "find",
};

/** The steps a plan shows: its own, and other plans' mining in the same
 *  load (feeding a refine step it shows, or at a place it mines at): one
 *  run mines them anyway, so they show beside its own, dimmer. Board
 *  order. */
export function planSteps(all: PlanStep[], plan: string): { step: PlanStep; other: boolean }[] {
  const own = all.filter((s) => s.plans.includes(plan));
  const feeds = new Set(own.filter((s) => s.kind === "refine").flatMap((s) => s.after));
  const sites = new Set(own.filter((s) => s.kind === "mine").flatMap((s) => (s.at?.site ? [s.at.site] : [])));
  const near = (s: PlanStep) => s.kind === "mine" && (feeds.has(s.id) || (!!s.at?.site && sites.has(s.at.site)));
  return all.filter((s) => s.plans.includes(plan) || near(s)).map((step) => ({ step, other: !step.plans.includes(plan) }));
}

export function lowerFirst(text: string): string {
  return text ? text[0].toLowerCase() + text.slice(1) : text;
}

/** "8:30", "1:02:10": a craft's time as the game counts it. */
export function clock(seconds: number): string {
  const s = Math.round(seconds);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const pad = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(s % 60)}` : `${m}:${pad(s % 60)}`;
}

/** "45 s", "4 min", "1 h 12 min": a stretch of time in words. */
export function span(seconds: number): string {
  if (seconds < 60) return `${Math.round(seconds)} s`;
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return `${minutes} min`;
  const rest = minutes % 60;
  return rest ? `${Math.floor(minutes / 60)} h ${rest} min` : `${Math.floor(minutes / 60)} h`;
}

export function gm(value: number): string {
  return `${value >= 10 ? value.toFixed(0) : value.toFixed(1)} Gm`;
}

/** What the step's right-hand figure says: its craft time or its price.
 *  Empty when the data gives neither. */
export function stepFigure(step: Pick<PlanStep, "seconds" | "cost">): string {
  if (step.seconds != null) return clock(step.seconds);
  if (step.cost != null) return `${Math.round(step.cost).toLocaleString("en-US")} aUEC`;
  return "";
}

/** The session's headline: how many places it visits. */
export function stopsText(session: Trip): string {
  const n = session.stops.length;
  return n === 0 ? "No trip" : n === 1 ? "1 stop" : `${n} stops`;
}

/** Beside the headline: the time the data can vouch for. The trip to its
 *  last stop when a drive timed it (travel, arrivals and the timed work;
 *  "~" when a leg is an estimate), plus the crafting among the steps. */
export function timeText(session: Trip): string {
  const parts: string[] = [];
  if (session.stops.length > 0 && session.travelSeconds != null) {
    parts.push(`${session.estimate ? "~" : ""}${span(session.travelSeconds)} trip`);
  }
  if (session.workSeconds > 0) parts.push(`${clock(session.workSeconds)} crafting`);
  return parts.join(" and ");
}

/** One stop of the session, in the order it is flown: what to do there,
 *  then where it is and the jump that gets you there. */
export type TimelineRow = { key: string; n: number; what: string; where: string; full: string };

/** `from` is the place the session starts at, when it is known. The row
 *  names only the place; the jump there is in the tooltip (playtest
 *  2026-11-16: no lines that state what the page already shows). */
export function timeline(session: Trip, from: string | null = null): TimelineRow[] {
  let here = from;
  const rows = session.stops.map((stop, i) => {
    const { what, full } = stopText(stop);
    const jump = jumpText(stop, here);
    const row = { key: stop.site, n: i + 1, what, where: stop.place, full: jump ? `${full}\n\n${jump}` : full };
    here = stop.place;
    return row;
  });
  if (rows.length <= TIMELINE_ROWS) return rows;
  // The card keeps its height: the stops past the reserved rows fold into
  // the last one, each still named in its tooltip.
  const shown = rows.slice(0, TIMELINE_ROWS - 1);
  const rest = rows.slice(TIMELINE_ROWS - 1);
  return [
    ...shown,
    {
      key: "more",
      n: TIMELINE_ROWS,
      what: `${rest.length} more stops`,
      where: rest.map((r) => r.where).join(", "),
      full: rest.map((r) => `${r.n}. ${r.where}\n${r.full}`).join("\n\n"),
    },
  ];
}

/** A stop's steps in one line: the first two named, the rest counted. */
function stopText(stop: TripStop): { what: string; full: string } {
  const titles = stop.steps.map((s) => s.title);
  const shown = titles.slice(0, 2).map((t, i) => (i === 0 ? t : lowerFirst(t)));
  const more = titles.length - shown.length;
  return {
    what: more > 0 ? `${shown.join(", ")} and ${more} more` : shown.join(", "),
    full: titles.join("\n"),
  };
}

/** The jump to a stop from the place before, when there is one. */
function jumpText(stop: TripStop, here: string | null): string | null {
  if (!stop.leg) return null;
  const jump = `${gm(stop.leg.gm)} from ${here ?? "the last stop"}`;
  const time = stop.leg.seconds != null ? `, ${span(stop.leg.seconds)}` : "";
  const gate = stop.leg.crosses ? ", through a jump point" : "";
  return `${jump}${time}${gate}`;
}

/** The rows a session's list shows (the card reserves this height so it
 *  does not move between tracks); more stops fold into the last row. */
export const TIMELINE_ROWS = 3;

export function amount(row: Pick<LedgerRow, "bulk">, value: number): string {
  if (!row.bulk) return String(Math.ceil(value));
  if (value >= 10) return value.toFixed(0);
  if (value >= 1) return String(Number(value.toFixed(1)));
  return String(Number(value.toFixed(2)));
}

/** A ledger row as two bars on one scale: what is needed, and what of it
 *  is held. The scale is the largest need of its unit, so SCU rows compare
 *  with SCU rows. A need too small to see still gets a sliver. */
export function ledgerBars(rows: LedgerRow[]): { need: number; held: number; state: "good" | "warn" | "bad" }[] {
  const max = (bulk: boolean) => Math.max(0, ...rows.filter((r) => r.bulk === bulk).map((r) => r.need));
  const scale = { true: max(true), false: max(false) };
  return rows.map((r) => {
    const top = scale[String(r.bulk) as "true" | "false"] || 1;
    const need = Math.max(3, (r.need / top) * 100);
    const held = r.held > 0 ? Math.max(3, (Math.min(r.held, r.need) / top) * 100) : 0;
    return { need, held, state: r.held + 1e-6 >= r.need ? "good" : r.held > 0 ? "warn" : "bad" };
  });
}

/** "16 h ago", "3 d ago": how old a read is. */
export function ago(unixSeconds: number, now: number): string {
  const s = Math.max(0, now / 1000 - unixSeconds);
  if (s < 90) return "just now";
  if (s < 3600) return `${Math.round(s / 60)} min ago`;
  if (s < 48 * 3600) return `${Math.round(s / 3600)} h ago`;
  return `${Math.round(s / 86400)} d ago`;
}
