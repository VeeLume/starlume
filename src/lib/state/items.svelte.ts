// My Items state — the inventory tier (gRPC-sourced, cached on disk by
// svc-dossier) and the finder joined from it. The cached reads are free and
// ungated; the refresh is the ToS-grey network call the backend gates via
// require_grpc("items").

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { commands, type FinderView, type InventorySummaryView } from "$lib/bindings";
import { buildTree, type Tree } from "$lib/components/items/model/tree";

let _summary = $state<InventorySummaryView | null>(null);
// Raw, not deep: the finder is replaced whole, and the tree it derives is a
// cross-linked graph (parent pointers) a deep proxy would only slow down.
let _finder = $state.raw<FinderView | null>(null);
let _tree = $state.raw<Tree | null>(null);
let _finderChannel: string | null = null;
let _loading = $state(false);
let _refreshing = $state(false);
let _error = $state("");
let _finderError = $state("");
// Guards against an older finder response landing after a newer request.
let _finderReq = 0;

export const itemsStore = {
  get summary() {
    return _summary;
  },
  /** The finder over the cached inventory; null until loaded or fetched. */
  get finder() {
    return _finder;
  },
  /** The explorer's containment tree over the finder; null with it. */
  get tree() {
    return _tree;
  },
  /** A finder load is in flight (the cached one stays on screen). */
  get loading() {
    return _loading;
  },
  get refreshing() {
    return _refreshing;
  },
  /** The last refresh's error (gate, launcher session, network). */
  get error() {
    return _error;
  },
  /** The last finder load's error (game data not loaded). */
  get finderError() {
    return _finderError;
  },
};

/** Load the cached inventory counts (no network, no gate). */
export async function loadItems(): Promise<void> {
  _summary = await commands.itemsSummary();
}

/** Join the cached inventory with `channel`'s game data. Cache-first: the
 *  last finder stays on screen while this runs (frontend rule 2). */
export async function loadFinder(channel: string): Promise<void> {
  if (channel !== _finderChannel) {
    _finder = null;
    _tree = null;
    _finderChannel = channel;
  }
  const mine = ++_finderReq;
  _loading = true;
  const result = await commands.itemsFinder(channel);
  if (mine !== _finderReq) return;
  _loading = false;
  if (result.status === "ok") {
    _finder = result.data;
    _tree = result.data ? buildTree(result.data.items, result.data.ships) : null;
    _finderError = "";
  } else {
    _finderError = result.error.message;
  }
}

/** Fetch live from CIG's backend (gated — errors when online / gRPC / the
 *  items feature aren't all on, or the launcher session is stale). The
 *  `items:changed` push reloads the finder. */
export async function refreshItems(): Promise<void> {
  _error = "";
  _refreshing = true;
  const result = await commands.itemsRefresh();
  _refreshing = false;
  if (result.status === "ok") {
    _summary = result.data;
  } else {
    _error = result.error.message;
  }
}

/** Backend push: a fetch finished, or the game data changed. */
export async function listenForItemsChanges(channel: () => string | null): Promise<UnlistenFn> {
  const reload = () => {
    void loadItems();
    const c = channel();
    if (c) void loadFinder(c);
  };
  const offItems = await listen("items:changed", reload);
  const offData = await listen("data:changed", reload);
  return () => {
    offItems();
    offData();
  };
}
