// Mining state. The catalog (lasers, modules, gadgets, elements), the areas
// (where each rock type spawns) and the place tree are the build's cooked
// reference data; the loadout on the calculator and the saved loadouts,
// yours and your crew's, are the module's persisted state (Rust-side,
// frontend rule 6); the scanned rock being typed in and what the finder has
// selected are session state that lives here so a navigation keeps them
// (frontend rule 1). Nothing here touches the network.

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  commands,
  type GeneratedView,
  type LaserInputView,
  type GenerateRequestView,
  type LoadoutView,
  type MassBandView,
  type MiningAreasView,
  type MiningCatalogView,
  type MiningReportView,
  type MiningStateView,
  type PlaceSystemView,
  type RockClassesView,
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

/** Where the rock on the calculator came from. */
export interface RockSource {
  /** "Most common", "Heaviest", … */
  pick: string;
  rockType: string;
  place: string;
}

export type MiningView = "calc" | "find" | "lo";

const emptyRow = (): RockRow => ({ element: "", percent: "", quality: "" });

let _catalog = $state.raw<MiningCatalogView | null>(null);
let _catalogChannel: string | null = null;
let _catalogError = $state("");
let _state = $state.raw<MiningStateView | null>(null);
let _rock = $state<RockDraft>({ mass: "", rows: [emptyRow()] });
let _source = $state.raw<RockSource | null>(null);
let _report = $state.raw<MiningReportView | null>(null);
let _solving = $state(false);
let _solveError = $state("");
// Guards against an older solve landing after a newer request.
let _solveReq = 0;
let _areas = $state.raw<MiningAreasView | null>(null);
let _places = $state.raw<PlaceSystemView[] | null>(null);
let _areasChannel: string | null = null;
let _areasError = $state("");
let _rolled = $state.raw<GeneratedView | null>(null);
// What the roll on show was asked for (the finder's selection as a key),
// so a revisit shows it instead of rolling again.
let _rolledKey = $state("");
let _generating = $state(false);
let _rollReq = 0;
let _generateError = $state("");
// "What if it weighs": a mass tried instead of the scanned one.
let _massOverride = $state<number | null>(null);
let _band = $state.raw<MassBandView | null>(null);
let _bandReq = 0;
// The finder's class table, for the loadout it was built for.
let _classTable = $state.raw<RockClassesView[] | null>(null);
let _classKey = "";
let _classReq = 0;

/** What the views have selected (session state). */
export const miningUi = $state({
  view: "calc" as MiningView,
  /** The finder browses by place or by ore. */
  mode: "ore" as "place" | "ore",
  ore: "",
  /** The finder's place node and, within it, the area and rock type. */
  place: "",
  area: "",
  rockType: "",
  pick: "common",
  /** The calculator's gadget and crew row is folded to one line each. */
  fold: false,
  /** Whose steps How to crack it shows (an operator index); -1: everyone. */
  me: -1,
});

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
  get source() {
    return _source;
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
  get places() {
    return _places;
  },
  get areasError() {
    return _areasError;
  },
  /** The finder's last roll, before it goes to the calculator. */
  get rolled() {
    return _rolled;
  },
  get rolledKey() {
    return _rolledKey;
  },
  get generating() {
    return _generating;
  },
  get generateError() {
    return _generateError;
  },
  /** The mass tried on the slider; null: the scanned one. */
  get massOverride() {
    return _massOverride;
  },
  /** The loadout's class along the mass axis for the scanned composition. */
  get band() {
    return _band;
  },
  /** Each rock type at each area with the loadout's class shares. */
  get classTable() {
    return _classTable;
  },
};

/** Load the generator's areas and place tree for `channel` (cache-first,
 *  frontend rule 2). */
export async function loadMiningAreas(channel: string): Promise<void> {
  if (channel !== _areasChannel) {
    _areas = null;
    _places = null;
    _areasChannel = channel;
  }
  const [areas, places] = await Promise.all([commands.miningAreas(channel), commands.miningPlaces(channel)]);
  if (channel !== _areasChannel) return;
  if (areas.status === "ok") {
    _areas = areas.data;
    _areasError = "";
  } else {
    _areasError = areas.error.message;
  }
  if (places.status === "ok") _places = places.data;
}

/** Roll a rock in the finder (it shows there before it goes to the
 *  calculator); `key` names the selection it is for. */
export async function rollRock(channel: string, request: GenerateRequestView, key: string): Promise<void> {
  _generating = true;
  _generateError = "";
  const mine = ++_rollReq;
  const loadout = _state?.current ?? null;
  const result = await commands.miningGenerate(channel, request, loadout && loadout.lasers.length ? loadout : null);
  if (mine !== _rollReq) return;
  _generating = false;
  _rolledKey = key;
  if (result.status === "ok") {
    _rolled = result.data;
  } else {
    _rolled = null;
    _generateError = result.error.message;
  }
}

/** Put the finder's roll on the calculator (which solves it). */
export function useRolled(source: RockSource): void {
  const g = _rolled;
  if (!g) return;
  _source = source;
  _massOverride = null;
  _rock.mass = String(Math.round(g.mass));
  _rock.rows = g.entries.map((e) => ({
    element: e.element,
    percent: e.percent.toFixed(2),
    quality: e.quality === null ? "" : String(e.quality),
  }));
  miningUi.view = "calc";
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
  const state = await commands.miningState();
  _state = state;
  // Crew mates added before lasers carried their crew loadout's id: match
  // them by name, so taking them off still works (and a name alone means
  // a MOLE seat from now on).
  const stale = state.current.lasers.some((l) => l.who !== null && l.crew === null);
  if (stale) {
    const byOwner = new Map(state.saved.filter((s) => s.owner !== null).map((s) => [s.owner, s.id]));
    const lasers = state.current.lasers.map((l) =>
      l.who !== null && l.crew === null && byOwner.has(l.who) ? { ...l, crew: byOwner.get(l.who) ?? null } : l,
    );
    if (lasers.some((l, i) => l !== state.current.lasers[i])) await setLoadout({ ...state.current, lasers });
  }
}

/** Make `loadout` the calculator's (persisted, so it opens with it next time). */
export async function setLoadout(loadout: LoadoutView): Promise<void> {
  // Optimistic: the cards read the store.
  if (_state) _state = { ..._state, current: loadout };
  const result = await commands.miningSetCurrent(loadout);
  if (result.status === "ok") _state = result.data;
}

/** Save a loadout, yours (`owner` null) or a crew mate's; `id` edits one. */
export async function saveLoadout(
  id: string | null,
  name: string,
  owner: string | null,
  loadout: LoadoutView,
): Promise<string | null> {
  const result = await commands.miningSaveLoadout(id, name, owner, loadout);
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

/** Bring a crew loadout along: its lasers join the calculator's loadout as
 *  one more ship, flown by its owner. */
export async function addSupport(id: string): Promise<void> {
  const saved = _state?.saved.find((s) => s.id === id);
  const current = _state?.current;
  if (!saved || !current) return;
  const ship = Math.max(-1, ...current.lasers.map((l) => l.owner)) + 1;
  const base = Math.min(...saved.loadout.lasers.map((l) => l.owner), 0);
  await setLoadout({
    ...current,
    lasers: [
      ...current.lasers,
      ...saved.loadout.lasers.map((l) => ({ ...l, owner: ship + (l.owner - base), who: saved.owner, crew: saved.id })),
    ],
  });
}

/** Take a crew mate's ship off the rock again: the lasers that came from
 *  that crew loadout (older loadouts without the id: by the name). */
export async function removeSupport(crew: string | null, who: string | null): Promise<void> {
  const current = _state?.current;
  if (!current) return;
  const gone = (l: LaserInputView) => (crew !== null ? l.crew === crew : l.crew === null && l.who === who && who !== null);
  const kept = current.lasers.filter((l) => !gone(l));
  const lead = current.lead !== null && current.lead < current.lasers.length && gone(current.lasers[current.lead]) ? null : current.lead;
  // A lead index past a removed laser shifts with it.
  const shifted = lead === null ? null : kept.indexOf(current.lasers[lead]);
  await setLoadout({ ...current, lasers: kept, lead: shifted !== null && shifted >= 0 ? shifted : null });
}

/** Make the laser at `index` the crew's lead. */
export async function setLead(index: number): Promise<void> {
  const current = _state?.current;
  if (current) await setLoadout({ ...current, lead: index });
}

/** Try the rock at another mass (null: back to the scanned one). */
export function setMassOverride(mass: number | null): void {
  _massOverride = mass;
}

/** The class band for the drafted rock and the calculator's loadout. */
export async function loadMassBand(channel: string): Promise<void> {
  const loadout = _state?.current;
  const rock = draftRock();
  const mine = ++_bandReq;
  if (!loadout || !loadout.lasers.length || !rock) {
    _band = null;
    return;
  }
  const result = await commands.miningMassBand(channel, loadout, rock);
  if (mine !== _bandReq) return;
  _band = result.status === "ok" ? result.data : null;
}

/** The finder's class table for the calculator's loadout (cached per
 *  loadout: it rolls every rock type at every area). */
export async function loadClassTable(channel: string): Promise<void> {
  const loadout = _state?.current;
  if (!loadout || !loadout.lasers.length) {
    _classTable = null;
    _classKey = "";
    return;
  }
  const key = JSON.stringify([channel, loadout.lasers.map((l) => [l.laser, l.modules, l.owner]), loadout.gadget]);
  if (key === _classKey) return;
  _classKey = key;
  const mine = ++_classReq;
  const result = await commands.miningClassTable(channel, loadout);
  if (mine !== _classReq) return;
  _classTable = result.status === "ok" ? result.data : null;
}

export function addRockRow(): void {
  _rock.rows.push(emptyRow());
}

export function removeRockRow(i: number): void {
  _rock.rows.splice(i, 1);
  if (_rock.rows.length === 0) _rock.rows.push(emptyRow());
}

/** A typed scan replaces a rolled one. */
export function markTyped(): void {
  _source = null;
  _massOverride = null;
}

export function clearRock(): void {
  // In place: the editor holds the draft object.
  _rock.mass = "";
  _rock.rows = [emptyRow()];
  _report = null;
  _solveError = "";
  _source = null;
  _massOverride = null;
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

/** The drafted rock as the solver takes it, at its scanned mass; null
 *  until it has a mass and an element. */
function draftRock(): { mass: number; entries: { element: string; percent: number; quality: number | null }[] } | null {
  const mass = parseNumber(_rock.mass, true);
  const entries = _rock.rows.flatMap((r) => {
    const percent = parseNumber(r.percent);
    if (!r.element || percent === null || percent <= 0) return [];
    const q = parseNumber(r.quality);
    return [{ element: r.element, percent, quality: q === null ? null : Math.round(q) }];
  });
  return mass === null || mass <= 0 || entries.length === 0 ? null : { mass, entries };
}

/** Solve the drafted rock (at the tried mass, when one is set) against the
 *  calculator's loadout; a no-op until the rock has a mass and an element. */
export async function solveRock(channel: string): Promise<void> {
  const loadout = _state?.current;
  const rock = draftRock();
  const mine = ++_solveReq;
  if (!loadout || !rock) {
    _report = null;
    _solveError = "";
    _solving = false;
    return;
  }
  _solving = true;
  const result = await commands.miningSolve(channel, loadout, {
    ...rock,
    mass: _massOverride ?? rock.mass,
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
export async function listenForMiningChanges(channel: () => string | null): Promise<UnlistenFn> {
  return listen("data:changed", () => {
    const c = channel();
    if (c) {
      void loadMiningCatalog(c);
      void loadMiningAreas(c);
    }
  });
}
