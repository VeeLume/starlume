// Game data state — per-install cache/load status, the build cache and its
// archive (Settings → Game data), and the browse channel shared by every
// catalog page (items / resources / missions / manufacturers). Backed by
// svc-data through the data* commands; everything is local file reads.

import {
  commands,
  type DataBuildView,
  type DataStatusView,
  type ItemDetailView,
  type ResourceRowView,
  type ManufacturerRowView,
  type ItemTypeFacetView,
  type MissionEntryView,
} from "$lib/bindings";

let _statuses = $state<DataStatusView[]>([]);
let _statusLoaded = $state(false);
/** Channels whose Load was pressed and hasn't returned. Covers the gap
 *  before the backend's `data-load` activity appears (the install rescan),
 *  and a load that never needs one (already in memory). Live stage labels
 *  come from the activity store, not from here. */
let _starting = $state<Record<string, true>>({});
let _error = $state("");
/** The channel the catalog pages browse — a loaded channel, or null. */
let _channel = $state<string | null>(null);
/** Every build on disk, current and archived, newest first. Fetched by the
 *  Settings → Game data page; kept fresh on `data:changed` once fetched. */
let _builds = $state<DataBuildView[]>([]);
let _buildsLoaded = $state(false);

export const dataStore = {
  get statuses() {
    return _statuses;
  },
  get statusLoaded() {
    return _statusLoaded;
  },
  get starting() {
    return _starting;
  },
  get error() {
    return _error;
  },
  get channel() {
    return _channel;
  },
  get builds() {
    return _builds;
  },
  get buildsLoaded() {
    return _buildsLoaded;
  },
  /** The channels the catalogs can browse right now. */
  get browsable() {
    return _statuses.filter(browsable).map((s) => s.channel);
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

/**
 * Run the load waterfall for a channel (sub-second when cached, up to ~45s
 * on a cold parse — stages show as the backend's `data-load` activity).
 */
export async function startLoad(channel: string): Promise<void> {
  _error = "";
  _starting = { ..._starting, [channel]: true };
  const result = await commands.dataLoad(channel);
  const { [channel]: _done, ...rest } = _starting;
  _starting = rest;
  if (result.status === "ok") {
    _statuses = _statuses.map((s) => (s.channel === result.data.channel ? result.data : s));
  } else {
    _error = result.error.message;
  }
}

/** Delete a channel's cached build — or, with `null`, the whole cache
 *  including the archive. The next load is a full parse. */
export async function wipe(channel: string | null): Promise<void> {
  _error = "";
  const result = await commands.dataWipe(channel);
  if (result.status === "ok") await Promise.all([loadStatus(), loadBuilds()]);
  else _error = result.error.message;
}

/** Delete one archived build. */
export async function deleteBuild(key: string): Promise<void> {
  _error = "";
  const result = await commands.dataDeleteBuild(key);
  if (result.status === "ok") await loadBuilds();
  else _error = result.error.message;
}

/** Refresh the build list (current builds + archive). */
export async function loadBuilds(): Promise<void> {
  const result = await commands.dataBuilds();
  if (result.status === "ok") _builds = result.data;
  else _error = result.error.message;
  _buildsLoaded = true;
}

/** `data:changed` catch-up for the build list — only once something has
 *  shown it, so a session that never opens Game data never pays for it. */
export function refreshBuildsIfShown(): void {
  if (_buildsLoaded) void loadBuilds();
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
