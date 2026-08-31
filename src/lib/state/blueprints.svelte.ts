// Owned-blueprint set (gRPC-sourced, cached). Personal account data behind
// svc-dossier: the cached read is free and ungated; the refresh is the
// ToS-grey network call the backend gates via require_grpc("blueprints").
//
// The store owns the set; catalog pages decorate by GUID (missions' blueprint
// pools), and the Me page owns the refresh affordance. App-level for now (like
// accounts) — migrates into the tracker module when it lands.

import { commands } from "$lib/bindings";

let _ownedIds = $state<Set<string>>(new Set());
let _fetchedAt = $state<number | null>(null);
let _refreshing = $state(false);
let _error = $state("");

export const blueprintsStore = {
  get count() {
    return _ownedIds.size;
  },
  /** Epoch seconds of the last fetch, or null when never fetched. */
  get fetchedAt() {
    return _fetchedAt;
  },
  get refreshing() {
    return _refreshing;
  },
  get error() {
    return _error;
  },
  /** Membership test — the decoration primitive. Reading this in a template
   *  tracks the set, so a refresh re-decorates every catalog live. */
  owns(guid: string): boolean {
    return _ownedIds.has(guid);
  },
};

/** Load the cached set (no network, no gate). Cheap — call on mount of any
 *  surface that decorates or displays ownership. */
export async function loadOwnedBlueprints(): Promise<void> {
  const owned = await commands.blueprintsOwned();
  _ownedIds = new Set(owned.blueprint_ids);
  _fetchedAt = owned.fetched_at;
}

/** Fetch live from CIG's backend (gated by require_grpc — errors when online
 *  / gRPC / the blueprints feature aren't all enabled, or the launcher
 *  session is stale). */
export async function refreshOwnedBlueprints(): Promise<void> {
  _error = "";
  _refreshing = true;
  const result = await commands.blueprintsRefresh();
  _refreshing = false;
  if (result.status === "ok") {
    _ownedIds = new Set(result.data.blueprint_ids);
    _fetchedAt = result.data.fetched_at;
  } else {
    _error = result.error.message;
  }
}
