// Crafting's personal side: what you marked and what you hold. The catalog
// itself is reference data (catalog.svelte.ts); owned blueprints read from
// the game are blueprints.svelte.ts. A blueprint counts as owned from either.

import type { UnlistenFn } from "@tauri-apps/api/event";
import { listen } from "@tauri-apps/api/event";
import { commands, type CraftingMark, type CraftingMarksView } from "$lib/bindings";
import { stockIndex, type Stock } from "$lib/modules/crafting/model";
import { blueprintsStore } from "./blueprints.svelte";

let _owned = $state<Set<string>>(new Set());
let _wantBlueprint = $state<Set<string>>(new Set());
let _wantItem = $state<Set<string>>(new Set());
let _stock = $state<Stock>(new Map());
let _error = $state("");

function apply(marks: CraftingMarksView) {
  _owned = new Set(marks.owned);
  _wantBlueprint = new Set(marks.wantBlueprint);
  _wantItem = new Set(marks.wantItem);
}

export const craftingStore = {
  get stock() {
    return _stock;
  },
  get error() {
    return _error;
  },
  /** Read from the game: the mark can't be taken off by hand. */
  ownedInGame: (guid: string) => blueprintsStore.owns(guid),
  owns: (guid: string) => blueprintsStore.owns(guid) || _owned.has(guid),
  wantsBlueprint: (guid: string) => _wantBlueprint.has(guid),
  wantsItem: (guid: string) => _wantItem.has(guid),
};

export async function loadMarks(): Promise<void> {
  apply(await commands.craftingMarks());
}

export async function toggleMark(mark: CraftingMark, blueprint: string): Promise<void> {
  const result = await commands.craftingToggle(mark, blueprint);
  if (result.status === "ok") {
    _error = "";
    apply(result.data);
  } else {
    _error = result.error.message;
  }
}

/** What you hold, from the cached inventory: a local read. */
export async function loadStock(channel: string): Promise<void> {
  const result = await commands.craftingStock(channel);
  if (result.status === "ok") _stock = stockIndex(result.data);
}

/** Keep the stock current with a fresh inventory or a new build. */
export async function listenForStockChanges(channel: () => string | null): Promise<UnlistenFn> {
  const reload = () => {
    const c = channel();
    if (c) void loadStock(c);
  };
  const offItems = await listen("items:changed", reload);
  const offData = await listen("data:changed", reload);
  return () => {
    offItems();
    offData();
  };
}
