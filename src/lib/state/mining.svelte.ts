// Mining state — the calculator. The catalog (lasers, modules, gadgets,
// elements) is the build's cooked reference data; the loadout on the
// calculator and the saved ones are the module's persisted state (Rust-side,
// frontend rule 6); the scanned rock being typed in is session state that
// lives here so a navigation keeps it (frontend rule 1). Nothing here
// touches the network.

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  commands,
  type LoadoutView,
  type MiningCatalogView,
  type MiningReportView,
  type MiningStateView,
} from "$lib/bindings";

/** One composition row as typed: strings, so a half-typed number stays. */
export interface RockRow {
  element: string;
  percent: string;
  quality: string;
}

export interface RockDraft {
  mass: string;
  rows: RockRow[];
}

const emptyRow = (): RockRow => ({ element: "", percent: "", quality: "" });

let _catalog = $state.raw<MiningCatalogView | null>(null);
let _catalogChannel: string | null = null;
let _catalogError = $state("");
let _state = $state.raw<MiningStateView | null>(null);
let _rock = $state<RockDraft>({ mass: "", rows: [emptyRow()] });
let _report = $state.raw<MiningReportView | null>(null);
let _solving = $state(false);
let _solveError = $state("");
// Guards against an older solve landing after a newer request.
let _solveReq = 0;

export const miningStore = {
  get catalog() {
    return _catalog;
  },
  get catalogError() {
    return _catalogError;
  },
  /** The calculator's loadout and the saved ones; null until loaded. */
  get state() {
    return _state;
  },
  /** The rock being typed in (mutable: the editor binds to it). */
  get rock() {
    return _rock;
  },
  get report() {
    return _report;
  },
  get solving() {
    return _solving;
  },
  get solveError() {
    return _solveError;
  },
};

/** Load `channel`'s catalog. Cache-first: the last one stays on screen while
 *  a reload runs (frontend rule 2). */
export async function loadMiningCatalog(channel: string): Promise<void> {
  if (channel !== _catalogChannel) {
    _catalog = null;
    _report = null;
    _catalogChannel = channel;
  }
  const result = await commands.miningCatalog(channel);
  if (channel !== _catalogChannel) return;
  if (result.status === "ok") {
    _catalog = result.data;
    _catalogError = "";
  } else {
    _catalogError = result.error.message;
  }
}

export async function loadMiningState(): Promise<void> {
  _state = await commands.miningState();
}

/** Make `loadout` the calculator's (persisted, so it opens with it next time). */
export async function setLoadout(loadout: LoadoutView): Promise<void> {
  // Optimistic: the editor reads the store.
  if (_state) _state = { ..._state, current: loadout };
  const result = await commands.miningSetCurrent(loadout);
  if (result.status === "ok") _state = result.data;
}

export async function saveLoadout(
  name: string,
  loadout: LoadoutView,
): Promise<string | null> {
  const result = await commands.miningSaveLoadout(name, loadout);
  if (result.status === "ok") {
    _state = result.data;
    return null;
  }
  return result.error.message;
}

export async function deleteLoadout(id: string): Promise<void> {
  const result = await commands.miningDeleteLoadout(id);
  if (result.status === "ok") _state = result.data;
}

export function addRockRow(): void {
  _rock.rows.push(emptyRow());
}

export function removeRockRow(i: number): void {
  _rock.rows.splice(i, 1);
  if (_rock.rows.length === 0) _rock.rows.push(emptyRow());
}

export function clearRock(): void {
  // In place: the editor holds the draft object.
  _rock.mass = "";
  _rock.rows = [emptyRow()];
  _report = null;
  _solveError = "";
}

/** Locale-tolerant number: "20.998", "20 998" and "20,998" are all fine for
 *  a mass; "6,22" is a decimal comma. */
export function parseNumber(s: string, thousands = false): number | null {
  let t = s.trim().replace(/[%\s t]/g, "");
  if (!t) return null;
  if (thousands) t = t.replace(/[.,](?=\d{3}(\D|$))/g, "");
  const n = Number(t.replace(",", "."));
  return Number.isFinite(n) ? n : null;
}

/** Solve the drafted rock against the calculator's loadout; a no-op until
 *  the rock has a mass and an element. */
export async function solveRock(channel: string): Promise<void> {
  const loadout = _state?.current;
  const mass = parseNumber(_rock.mass, true);
  const entries = _rock.rows.flatMap((r) => {
    const percent = parseNumber(r.percent);
    if (!r.element || percent === null || percent <= 0) return [];
    const q = parseNumber(r.quality);
    return [
      {
        element: r.element,
        percent,
        quality: q === null ? null : Math.round(q),
      },
    ];
  });
  const mine = ++_solveReq;
  if (!loadout || mass === null || mass <= 0 || entries.length === 0) {
    _report = null;
    _solveError = "";
    _solving = false;
    return;
  }
  _solving = true;
  const result = await commands.miningSolve(channel, loadout, {
    mass,
    entries,
  });
  if (mine !== _solveReq) return;
  _solving = false;
  if (result.status === "ok") {
    _report = result.data;
    _solveError = "";
  } else {
    _solveError = result.error.message;
  }
}

/** Backend push: the game data changed (a cook landed, a wipe). */
export async function listenForMiningChanges(
  channel: () => string | null,
): Promise<UnlistenFn> {
  return listen("data:changed", () => {
    const c = channel();
    if (c) void loadMiningCatalog(c);
  });
}
