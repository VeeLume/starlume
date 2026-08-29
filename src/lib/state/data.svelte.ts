// Game Data section state — per-install cache/load status + the browse
// channel shared by every catalog page (items / resources / missions /
// manufacturers). Backed by svc-data through the data* commands; everything
// is local file reads.

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  commands,
  type DataStatusView,
  type ItemDetailView,
  type ResourceRowView,
  type ManufacturerRowView,
  type ItemTypeFacetView,
  type MissionEntryView,
} from "$lib/bindings";

// Hand-typed: the `data:progress` event payload (plain emit in
// src-tauri/src/data.rs — kept in sync manually, the notifications pattern).
type DataProgress = {
  channel: string;
  stage:
    | "loading-snapshot"
    | "opening-p4k"
    | "extracting"
    | "parsing"
    | "cooking"
    | "saving";
};

const STAGE_LABELS: Record<DataProgress["stage"], string> = {
  "loading-snapshot": "Checking snapshots…",
  "opening-p4k": "Opening Data.p4k…",
  extracting: "Extracting assets…",
  parsing: "Parsing DataCore… (the long part)",
  cooking: "Building indices…",
  saving: "Saving snapshots…",
};

let _statuses = $state<DataStatusView[]>([]);
let _statusLoaded = $state(false);
/** channel → live stage label while a load runs. */
let _loading = $state<Record<string, string>>({});
let _error = $state("");
/** The channel the catalog pages browse — a loaded channel, or null. */
let _channel = $state<string | null>(null);

export const dataStore = {
  get statuses() {
    return _statuses;
  },
  get statusLoaded() {
    return _statusLoaded;
  },
  get loading() {
    return _loading;
  },
  get error() {
    return _error;
  },
  get channel() {
    return _channel;
  },
};

/** Pick the browse channel (catalog pages read `dataStore.channel`). */
export function setChannel(channel: string | null): void {
  _channel = channel;
}

/** A channel the catalogs can browse right now: cooked bundle in memory, or
 *  a processed snapshot on disk (queries fast-reload it in under a second —
 *  the startup warm's product). */
const browsable = (s: DataStatusView) => s.loaded || s.predicted_tier === "processed";

/**
 * Resolve the browse channel for a catalog page: the current pick if still
 * browsable, else the default channel (newest PU build, warmed at startup),
 * else the first browsable one. Refreshes statuses when they haven't been
 * fetched yet. `null` → nothing is browsable; the page shows its "load data
 * first" prompt.
 */
export async function ensureChannel(): Promise<string | null> {
  if (!_statusLoaded) await loadStatus();
  const candidates = _statuses.filter(browsable);
  if (_channel && candidates.some((s) => s.channel === _channel)) return _channel;
  _channel = (candidates.find((s) => s.is_default) ?? candidates[0])?.channel ?? null;
  return _channel;
}

/** Rescan installs + refresh every status card. */
export async function loadStatus(): Promise<void> {
  const result = await commands.dataStatus();
  if (result.status === "ok") {
    _statuses = result.data;
    _error = "";
  } else {
    _error = result.error.message;
  }
  _statusLoaded = true;
}

/** Subscribe to load-progress events. The root layout holds this listener
 *  for the app's lifetime, so background warms show progress everywhere. */
export async function listenForDataProgress(): Promise<UnlistenFn> {
  return listen<DataProgress>("data:progress", (event) => {
    const { channel, stage } = event.payload;
    _loading = { ..._loading, [channel]: STAGE_LABELS[stage] ?? stage };
  });
}

/** Clear stale progress labels (a load finished — `data:changed` arrived). */
export function clearAllLoading(): void {
  _loading = {};
}

/**
 * Run the load waterfall for a channel (sub-second when cached, up to ~45s
 * on a cold parse — progress streams via the event listener).
 */
export async function startLoad(channel: string): Promise<void> {
  _error = "";
  _loading = { ..._loading, [channel]: "Starting…" };
  const result = await commands.dataLoad(channel);
  const { [channel]: _done, ...rest } = _loading;
  _loading = rest;
  if (result.status === "ok") {
    _statuses = _statuses.map((s) => (s.channel === result.data.channel ? result.data : s));
  } else {
    _error = result.error.message;
  }
}

/** Delete cached snapshots for a channel; next load is a full parse. */
export async function wipe(channel: string): Promise<void> {
  _error = "";
  const result = await commands.dataWipe(channel);
  if (result.status === "ok") await loadStatus();
  else _error = result.error.message;
}

// ── Browse queries (thin wrappers; the page owns the result state) ────────

export async function itemDetail(
  channel: string,
  guid: string,
): Promise<ItemDetailView | string> {
  const result = await commands.dataItemDetail(channel, guid);
  return result.status === "ok" ? result.data : result.error.message;
}

export async function listResources(channel: string): Promise<ResourceRowView[] | string> {
  const result = await commands.dataResources(channel);
  return result.status === "ok" ? result.data : result.error.message;
}

export async function listManufacturers(
  channel: string,
): Promise<ManufacturerRowView[] | string> {
  const result = await commands.dataManufacturers(channel);
  return result.status === "ok" ? result.data : result.error.message;
}

export async function listItemTypes(channel: string): Promise<ItemTypeFacetView[] | string> {
  const result = await commands.dataItemTypes(channel);
  return result.status === "ok" ? result.data : result.error.message;
}

export async function listMissions(channel: string): Promise<MissionEntryView[] | string> {
  const result = await commands.dataMissions(channel);
  return result.status === "ok" ? result.data : result.error.message;
}
