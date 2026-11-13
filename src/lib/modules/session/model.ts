// Session log: how a session reads on the page. Pure (no stores, no IPC), so
// vitest covers it. The wording is UI text (English, like the rest); numbers
// and times follow the user's formatting locale.

import type { EntryKind, Session, SessionSummary, WalletChange } from "$lib/bindings";

/** "2 h 14 min", "45 min", "under a minute". */
export function duration(ms: number): string {
  const minutes = Math.floor(Math.max(0, ms) / 60_000);
  if (minutes < 1) return "under a minute";
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  if (h === 0) return `${m} min`;
  return m === 0 ? `${h} h` : `${h} h ${m} min`;
}

/** How long a session ran; a live one runs until `now`. */
export function sessionLength(
  session: Pick<Session, "started_at" | "ended_at">,
  now: number,
): number {
  return (session.ended_at ?? now) - session.started_at;
}

const whole = new Intl.NumberFormat(undefined, { maximumFractionDigits: 0 });

/** "125,000 aUEC". */
export function auec(amount: number): string {
  return `${whole.format(amount)} aUEC`;
}

/** "+25,000 aUEC" / "−825 aUEC" / "±0 aUEC". */
export function signedAuec(amount: number): string {
  const sign = amount > 0 ? "+" : amount < 0 ? "−" : "±";
  return `${sign}${whole.format(Math.abs(amount))} aUEC`;
}

/** The time of day ("14:23:05"). */
export function clock(ms: number): string {
  return new Date(ms).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

/** Day and time ("13 Nov 2026, 17:10"). */
export function dayTime(ms: number): string {
  return new Date(ms).toLocaleString([], { dateStyle: "medium", timeStyle: "short" });
}

/** What the timeline's "Show" filter sorts entries into. */
export type EntryGroup = "session" | "places" | "missions" | "money" | "vehicles" | "life";

export const ENTRY_GROUPS: { value: EntryGroup; label: string }[] = [
  { value: "missions", label: "Missions" },
  { value: "money", label: "Money" },
  { value: "places", label: "Places" },
  { value: "vehicles", label: "Vehicles" },
  { value: "life", label: "Deaths and respawns" },
  { value: "session", label: "Shards" },
];

export function entryGroup(kind: EntryKind): EntryGroup {
  switch (kind.type) {
    case "shard_joined":
      return "session";
    case "location":
    case "jurisdiction":
      return "places";
    case "mission_accepted":
    case "objective":
    case "mission_ended":
    case "blueprint_received":
      return "missions";
    case "refinery_order_completed":
    case "trade":
    case "shop_purchase":
    case "money":
      return "money";
    case "vehicle_entered":
    case "vehicle_left":
      return "vehicles";
    case "died":
    case "downed":
    case "respawned":
    case "spawned":
      return "life";
  }
}

const OUTCOME = {
  completed: "Mission completed",
  failed: "Mission failed",
  abandoned: "Mission abandoned",
  withdrawn: "Mission withdrawn",
  ended: "Mission ended",
} as const;

const OBJECTIVE = {
  New: "New objective",
  Completed: "Objective completed",
  Withdrawn: "Objective withdrawn",
} as const;

/** The row's heading: what kind of thing happened. Never game text. */
export function entryTitle(kind: EntryKind): string {
  switch (kind.type) {
    case "shard_joined":
      return "Joined shard";
    case "location":
      return "Arrived";
    case "jurisdiction":
      return "Entered jurisdiction";
    case "mission_accepted":
      return "Mission accepted";
    case "objective":
      return OBJECTIVE[kind.change];
    case "mission_ended":
      return OUTCOME[kind.outcome];
    case "blueprint_received":
      return "Blueprint received";
    case "refinery_order_completed":
      return "Refinery order completed";
    case "trade":
      return kind.side === "Buy" ? "Bought cargo" : "Sold cargo";
    case "shop_purchase":
      return kind.rental ? "Rented" : "Bought";
    case "money":
      return kind.change === "Awarded" ? "Awarded" : "Fined";
    case "vehicle_entered":
      return "Entered vehicle";
    case "vehicle_left":
      return "Left vehicle";
    case "died":
      return "Died";
    case "downed":
      return "Downed";
    case "respawned":
      return "Respawned";
    case "spawned":
      return "Spawned";
  }
}

/** The row's subject: the game's own text or name for it (a mission title,
 *  a place, an item class). Rendered through RichText. `null` = none. */
export function entrySubject(kind: EntryKind): string | null {
  switch (kind.type) {
    case "shard_joined":
      return kind.shard;
    case "location":
      return kind.location;
    case "jurisdiction":
      return kind.jurisdiction;
    case "mission_accepted":
      return kind.title;
    case "objective":
      return kind.text;
    case "mission_ended":
      return kind.title ?? kind.contract;
    case "blueprint_received":
      return kind.name;
    case "refinery_order_completed":
      return kind.location;
    case "trade":
      return kind.shop;
    case "shop_purchase":
      return kind.quantity > 1 ? `${kind.quantity} × ${kind.item_name}` : kind.item_name;
    case "vehicle_entered":
    case "vehicle_left":
      return kind.class_name;
    default:
      return null;
  }
}

/** The amount the log states for an entry, signed for the wallet; `null`
 *  when it states none. What the client asked for, not what was charged. */
export function entryAmount(kind: EntryKind): number | null {
  switch (kind.type) {
    case "money":
      return kind.change === "Awarded" ? kind.amount : -kind.amount;
    case "trade":
      return kind.side === "Buy" ? -kind.price : kind.price;
    case "shop_purchase":
      return -kind.price;
    default:
      return null;
  }
}

/** Why a request's row needs a word: it was refused, or nothing confirmed
 *  it. `null` for a confirmed one and for entries that are not requests. */
export function requestNote(kind: EntryKind): string | null {
  if (kind.type !== "trade" && kind.type !== "shop_purchase") return null;
  if (kind.outcome === "rejected") return "refused";
  // The log has no answer line for commodity trades.
  if (kind.outcome === "requested") return "not confirmed";
  return null;
}

/** One row of the timeline: an entry, or a balance change no entry explains. */
export interface TimelineRow {
  key: string;
  at: number;
  title: string;
  group: EntryGroup;
  source: "log" | "telemetry";
  /** Game text or a game name, for RichText. */
  subject: string | null;
  /** What the log says the entry cost or paid. */
  stated: number | null;
  /** The balance change the entry explains (or, for an untracked row, is). */
  change: WalletChange | null;
  note: string | null;
  localeDependent: boolean;
  kind: EntryKind | null;
}

/** The session as timeline rows, oldest first: every entry with the balance
 *  change it explains, plus one row per change nothing explains. */
export function timelineRows(session: Session): TimelineRow[] {
  const explained = new Map<number, WalletChange>();
  const rows: TimelineRow[] = [];
  for (const change of session.wallet.changes) {
    if (change.cause.type === "entry") {
      explained.set(change.cause.seq, change);
    } else {
      rows.push({
        key: `w${change.at}:${change.to}`,
        at: change.at,
        title: "Balance changed",
        group: "money",
        source: "telemetry",
        subject: null,
        stated: null,
        change,
        note: "untracked",
        localeDependent: false,
        kind: null,
      });
    }
  }
  for (const entry of session.timeline) {
    rows.push({
      key: `e${entry.seq}`,
      at: entry.at,
      title: entryTitle(entry.kind),
      group: entryGroup(entry.kind),
      source: entry.source,
      subject: entrySubject(entry.kind),
      stated: entryAmount(entry.kind),
      change: explained.get(entry.seq) ?? null,
      note: requestNote(entry.kind),
      localeDependent: entry.locale_dependent,
      kind: entry.kind,
    });
  }
  // Stable: rows of one instant keep entries after the change they follow.
  return rows.sort((a, b) => a.at - b.at);
}

/** Which of the two sources a session is missing, as the page says it. */
export function missingSources(sources: { log: boolean; telemetry: boolean }): string | null {
  if (!sources.log && !sources.telemetry) return null;
  if (!sources.log) return "Telemetry only: no shards, places, missions or purchases.";
  if (!sources.telemetry) return "Game.log only: no wallet, vehicles or deaths.";
  return null;
}

/** A history row's one-line summary. */
export function summaryLine(s: SessionSummary): string {
  const parts = [duration(sessionLength(s, s.started_at))];
  if (s.missions_completed > 0) {
    parts.push(
      s.missions_completed === 1 ? "1 mission completed" : `${s.missions_completed} missions completed`,
    );
  }
  if (s.shards.length > 0) parts.push(s.shards[s.shards.length - 1]);
  return parts.join(" · ");
}
