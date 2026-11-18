// Your standing for Contracts: reputation rows per faction and scope, and the
// contracts you have completed (the mission token wallet). Account data read
// from CIG's backend through svc-dossier: the cached read is free and
// ungated; the refresh is the unofficial-API call the backend gates with
// require_unofficial("standing").
//
// The store owns it (frontend rule 1), loads the cache at once (rule 2) and
// reloads on `standing:changed` (rule 5). An unread standing is not a rank:
// pages show every mission as "not in your standing" until it is read.

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { commands, type PlayerStandingView, type RepRowView } from "$lib/bindings";

let _rows = $state<RepRowView[]>([]);
let _completed = $state<Set<string>>(new Set());
let _fetchedAt = $state<number | null>(null);
let _refreshing = $state(false);
let _error = $state("");
let _unlisten: Promise<UnlistenFn> | null = null;

function apply(s: PlayerStandingView) {
  _rows = s.reputations;
  _completed = new Set(s.completed_contracts);
  _fetchedAt = s.fetched_at;
}

export const standingStore = {
  /** Your reputation rows; empty until read. */
  get rows() {
    return _rows;
  },
  /** Contract record names you hold a completion token for. */
  get completed() {
    return _completed;
  },
  /** Epoch seconds of the last read, or null when never read. */
  get fetchedAt() {
    return _fetchedAt;
  },
  /** Whether a standing was ever read (an unread one says nothing). */
  get known() {
    return _fetchedAt != null;
  },
  get refreshing() {
    return _refreshing;
  },
  get error() {
    return _error;
  },
};

/** Load the cached standing (no network) and follow later refreshes. */
export async function loadStanding(): Promise<void> {
  _unlisten ??= listen("standing:changed", () => void loadStanding());
  apply(await commands.standingCached());
}

/** Read it live (gated: online, unofficial APIs and the "standing" feature). */
export async function refreshStanding(): Promise<void> {
  _error = "";
  _refreshing = true;
  const result = await commands.standingRefresh();
  _refreshing = false;
  if (result.status === "ok") apply(result.data);
  else _error = result.error.message;
}
