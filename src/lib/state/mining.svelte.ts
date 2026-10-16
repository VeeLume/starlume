// Mining state — the calculator. The catalog (lasers, modules, gadgets,
// elements) is the build's cooked reference data; the loadout on the
// calculator and the saved ones are the module's persisted state (Rust-side,
// frontend rule 6); the scanned rock being typed in is session state that
// lives here so a navigation keeps it (frontend rule 1). The rock
// generator's areas (where each rock type spawns) are cooked reference data
// too; a generated rock lands in the same draft. Nothing here touches the
// network.

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  commands,
  type GeneratedView,
  type GenerateRequestView,
  type LoadoutView,
  type MiningAreasView,
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
let _areas = $state.raw<MiningAreasView | null>(null);
let _areasChannel: string | null = null;
let _areasError = $state("");
/** The last generated rock, with what was asked for (a label). */
let _generated = $state.raw<{ label: string; rock: GeneratedView } | null>(
  null,
);
let _generating = $state(false);
let _generateError = $state("");

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
  /** The rock generator's areas; null until loaded. */
  get areas() {
    return _areas;
  },
  get areasError() {
    return _areasError;
  },
  get generated() {
    return _generated;
  },
  get generating() {
    return _generating;
  },
  get generateError() {
    return _generateError;
  },
};

/** Load the generator's areas for `channel` (cache-first, frontend rule 2).
 *  The backend starts the once-per-build places pass if it is missing. */
export async function loadMiningAreas(channel: string): Promise<void> {
  if (channel !== _areasChannel) {
    _areas = null;
    _areasChannel = channel;
  }
  const result = await commands.miningAreas(channel);
  if (channel !== _areasChannel) return;
  if (result.status === "ok") {
    _areas = result.data;
    _areasError = "";
  } else {
    _areasError = result.error.message;
  }
}

/** Roll a rock and put it in the scan editor (which re-solves). */
export async function generateRock(
  channel: string,
  request: GenerateRequestView,
  label: string,
): Promise<void> {
  _generating = true;
  _generateError = "";
  const loadout = _state?.current ?? null;
  const result = await commands.miningGenerate(
    channel,
    request,
    loadout && loadout.lasers.length ? loadout : null,
  );
  _generating = false;
  if (result.status !== "ok") {
    _generateError = result.error.message;
    return;
  }
  const g = result.data;
  _generated = { label, rock: g };
  _rock.mass = String(Math.round(g.mass));
  _rock.rows = g.entries.map((e) => ({
    element: e.element,
    percent: e.percent.toFixed(2),
    quality: e.quality === null ? "" : String(e.quality),
  }));
}

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
  _generated = null;
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
  const offData = await listen("data:changed", () => {
    const c = channel();
    if (c) {
      void loadMiningCatalog(c);
      void loadMiningAreas(c);
    }
  });
  // The once-per-build places pass finished: areas get their place names.
  const offPlaces = await listen("mining:places", () => {
    const c = channel();
    if (c) void loadMiningAreas(c);
  });
  return () => {
    offData();
    offPlaces();
  };
}
