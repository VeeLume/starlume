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
  type CaptureKind,
  type CaptureView,
  type ClassSharesView,
  type ComfortView,
  type EvalView,
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
  /** On a planet or moon (true) or in space (false); null: not known (rated
   *  as in space). The beam's distance drifts differently. */
  surface: boolean | null;
}

/** Where the rock on the calculator came from. */
export interface RockSource {
  /** "Most common", "Heaviest", … */
  pick: string;
  rockType: string;
  place: string;
  /** The rock type's heaviest roll (the finder's mass bars): where the
   *  "what if it weighs" axis ends. Null for a typed or read scan. */
  heaviest?: number | null;
}

export type MiningView = "calc" | "find" | "lo" | "crew";

const emptyRow = (): RockRow => ({ element: "", percent: "", quality: "" });

let _catalog = $state.raw<MiningCatalogView | null>(null);
let _catalogChannel: string | null = null;
let _catalogError = $state("");
let _state = $state.raw<MiningStateView | null>(null);
let _rock = $state<RockDraft>({ mass: "", rows: [emptyRow()], surface: null });
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
// The last scan capture (a button press or a picked screenshot) and its
// reading; the image itself stays Rust-side (frontend rule 6).
let _capture = $state.raw<CaptureView | null>(null);
let _capturing = $state<CaptureKind | null>(null);
let _captureError = $state("");
/** The HUD reader has its data (scan.bin); null until checked. */
let _scanReady = $state<boolean | null>(null);
let _generating = $state(false);
let _rollReq = 0;
let _generateError = $state("");
// "What if it weighs": a mass tried instead of the scanned one.
let _massOverride = $state<number | null>(null);
let _band = $state.raw<MassBandView | null>(null);
let _bandReq = 0;
// What the shown band and class table were computed for: while the inputs
// differ (a newer request still running) the views dim instead of passing
// an old answer off as the current one.
let _bandFor = $state("");
/** A rock type at an area with the loadout's class shares (the finder's rows). */
export type RockClassesView = { area: string; rockType: string; classes: ClassSharesView };

// The finder's class split: the calculator's loadout's comfort solve, for
// the loadout it was built for.
let _classComfort = $state.raw<ComfortView[] | null>(null);
let _classKey = "";
let _classFor = $state("");
let _classReq = 0;

// ── Comfort solves (design pass 5) ──
// Every rock that spawns, per loadout: the Loadouts tab's scores and comfort
// zones, the finder's class split. A solve takes a few seconds, so results
// stay for the session (and on disk Rust-side, per build) and run one at a
// time, the urgent ones (the sheet, the calculator's loadout) first.
const COMFORT_KEEP = 96;
let _comfort = $state.raw(new Map<string, ComfortView[]>());
let _comfortFailed = new Set<string>();
type ComfortJob = { key: string; channel: string; loadout: LoadoutView; done: ((c: ComfortView[] | null) => void)[] };
const _comfortQueue: ComfortJob[] = [];
let _comfortRunning: ComfortJob | null = null;
let _comfortBusy = $state(false);
let _comfortRunningKey = $state("");

/** What the views have selected (session state). */
export const miningUi = $state({
  /** What Loadouts measures on: the calculator's rock, or every rock that
   *  spawns (`measureOre`: one ore's; `measureSys`: one system's). */
  measure: "all" as "rock" | "all",
  measureOre: "",
  measureSys: "all",
  /** This session's try-outs per saved loadout (Loadouts' "Tried this
   *  session"), and the ones kept (their heads as JSON). */
  tries: {} as Record<string, LaserInputView[][]>,
  kept: [] as string[],
  view: "calc" as MiningView,
  /** Loadouts' part overlay: every option measured, or the stat table. */
  partView: "measured" as "measured" | "stats",
  /** The finder browses by place or by ore. */
  mode: "ore" as "place" | "ore",
  ore: "",
  /** The finder's place node and, within it, the area and rock type. */
  place: "",
  area: "",
  rockType: "",
  pick: "common",
  /** The place quality rolls at (Roll at); the picked place or the area's
   *  first when it is not one of the area's places. */
  at: "",
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
  /** The band shown was computed for another rock or loadout (the current
   *  one is still being worked out). */
  get bandStale() {
    return _band !== null && bandInputs() !== _bandFor;
  },
  /** Each rock type at each area with the loadout's class shares (from the
   *  calculator's loadout's comfort solve). */
  get classTable(): RockClassesView[] | null {
    if (!_classComfort || !_areas) return null;
    const by = new Map(_classComfort.map((c) => [c.dist, c.classes] as const));
    return _areas.areas.flatMap((a) =>
      a.rocks.flatMap((r) => {
        const classes: ClassSharesView | undefined = by.get(r.dist);
        return classes ? [{ area: a.id, rockType: r.rockType, classes }] : [];
      }),
    );
  },
  /** The class table shown is for another loadout. */
  get classTableStale() {
    return _classComfort !== null && _state !== null && classInputs(_state.current) !== _classFor;
  },
  /** A comfort solve is running. */
  get comfortBusy() {
    return _comfortBusy;
  },
  /** The key ([`comfortKey`]) of the solve running now; "" when idle. */
  get comfortRunning() {
    return _comfortRunningKey;
  },
  /** The last scan capture and what was read from it. */
  get capture() {
    return _capture;
  },
  /** The kind of capture under way; null when idle. */
  get capturing() {
    return _capturing;
  },
  get captureError() {
    return _captureError;
  },
  /** Read scan works: the HUD reader's data is on this machine. */
  get scanReady() {
    return _scanReady;
  },
};

/** Check once whether the HUD reader has its data (a stat, no read). */
export async function loadScanReady(): Promise<void> {
  _scanReady ??= await commands.miningScanReady();
}

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
  _source = { ...source, heaviest: g.masses.at(-1)?.to ?? null };
  _massOverride = null;
  _rock.mass = String(Math.round(g.mass));
  _rock.surface = g.surface;
  _rock.rows = g.entries.map((e) => ({
    element: e.element,
    percent: e.percent.toFixed(2),
    quality: e.quality === null ? "" : String(e.quality),
  }));
  miningUi.view = "calc";
}

/** A reading fills the calculator only when every value checked out: one
 *  wrong value is worse than none. */
export function readingAccepted(c: CaptureView | null): boolean {
  const r = c?.rock;
  return (
    !!r &&
    r.found &&
    r.unsure.length === 0 &&
    r.mass !== null &&
    r.rows.length > 0 &&
    r.rows.every((w) => w.percent !== null && (w.inert || w.element !== null))
  );
}

/** Put a capture's reading on the calculator: the mass and every row that
 *  read (an unknown ore keeps its numbers with the element left to pick).
 *  The scan card calls it for an accepted reading; "Type it in" for one
 *  that wasn't, to correct by hand. */
export function useCapture(c: CaptureView): void {
  const r = c.rock;
  if (!r || !r.found) return;
  const rows = r.rows
    .filter((w) => !w.inert && (w.element !== null || w.percent !== null))
    .map((w) => ({
      element: w.element ?? "",
      percent: w.percent === null ? "" : w.percent.toFixed(2),
      quality: w.quality === null ? "" : String(w.quality),
    }));
  _massOverride = null;
  _rock.mass = r.mass === null ? "" : String(Math.round(r.mass));
  _rock.rows = rows.length ? rows : [emptyRow()];
  const at = new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  _source = { pick: "Read from the game", rockType: r.rockName ?? "", place: at };
}

async function runCapture(kind: CaptureKind, call: () => ReturnType<typeof commands.miningCapture>): Promise<void> {
  _capturing = kind;
  _captureError = "";
  const result = await call();
  _capturing = null;
  if (result.status === "ok") {
    _capture = result.data;
    if (kind === "rock" && readingAccepted(result.data)) useCapture(result.data);
  } else {
    _captureError = result.error.message;
  }
}

/** Put the last capture away (its reading was typed in, or dismissed). */
export function dismissCapture(): void {
  _capture = null;
  _captureError = "";
}

/** Capture the game window now and read it as `kind`. */
export function captureScan(channel: string, kind: CaptureKind): Promise<void> {
  return runCapture(kind, () => commands.miningCapture(channel, kind));
}

/** Read a screenshot file as `kind`. */
export function readScreenshot(channel: string, kind: CaptureKind, path: string): Promise<void> {
  return runCapture(kind, () => commands.miningReadScreenshot(channel, kind, path));
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
  // Before try-outs the calculator didn't remember which saved loadout it
  // came from: the one it matches.
  if (_state && _state.basedOn === null) {
    const own = ownLasers(_state.current);
    const match = _state.saved.find((s) => s.owner === null && sameLasers(s.loadout.lasers, own));
    if (match) await setLoadout(_state.current, match.id);
  }
}

/** Make `loadout` the calculator's (persisted, so it opens with it next
 *  time); `basedOn` is the saved loadout your part of it came from (kept
 *  when left out). */
export async function setLoadout(loadout: LoadoutView, basedOn: string | null = _state?.basedOn ?? null): Promise<void> {
  // Optimistic: the cards read the store.
  if (_state) _state = { ..._state, current: loadout, basedOn };
  const result = await commands.miningSetCurrent(loadout, basedOn);
  if (result.status === "ok") _state = result.data;
}

// ── The try-out ─────────────────────────────────────────────────────────────
// Edits on the calculator change the current loadout only; the saved one it
// came from (`basedOn`) changes on Save.

const ownLasers = (l: LoadoutView) => l.lasers.filter((x) => x.crew === null);

/** The same lasers, modules and ships (the gadget belongs to the rock, the
 *  distance to the moment). */
export function sameLasers(a: LaserInputView[], b: LaserInputView[]): boolean {
  const key = (ls: LaserInputView[]) => JSON.stringify(ls.map((l) => [l.laser, l.modules, l.owner]));
  return key(a) === key(b);
}

/** Lasers as they are saved: no distance, no crew, every head on (who is
 *  in the seats is the moment's, like the distance). */
const asSaved = (ls: LaserInputView[]): LoadoutView => ({
  lasers: ls.map((l) => ({ ...l, distanceM: null, crew: null, off: false })),
  gadget: null,
  lead: null,
});

/** `own` as your part of the calculator's loadout: the crew mates kept on
 *  ships after yours, the gadget and your beam distance kept. `keepOff`
 *  carries the heads holding fire over by position (a saved loadout coming
 *  in has every head on), when the ship has as many heads. At least one of
 *  your heads fires. */
function withCrew(own: LaserInputView[], current: LoadoutView, keepOff = false): LoadoutView {
  const now = ownLasers(current);
  const distance = now.find((l) => l.distanceM !== null)?.distanceM ?? null;
  const crewLasers = current.lasers.filter((l) => l.crew !== null);
  const after = Math.max(-1, ...own.map((l) => l.owner)) + 1;
  const base = Math.min(...crewLasers.map((l) => l.owner), Infinity);
  const carry = keepOff && now.length === own.length;
  let off = own.map((l, i) => (carry ? now[i].off : l.off));
  if (off.every(Boolean)) off = off.map(() => false);
  return {
    gadget: current.gadget,
    lead: null,
    lasers: [
      ...own.map((l, i) => ({ ...l, crew: null, distanceM: distance, off: off[i] })),
      ...crewLasers.map((l) => ({ ...l, owner: after + (l.owner - base) })),
    ],
  };
}

/** `own` as your part of the calculator's loadout, to compare on the rock
 *  (a picker option; with `keepOff`, the saved loadout behind a try-out,
 *  with today's heads holding fire). */
export function onCalculator(own: LaserInputView[], keepOff = false): LoadoutView {
  const current = _state?.current ?? { lasers: [], gadget: null, lead: null };
  return withCrew(own, current, keepOff);
}

/** How many heads and slots of `now` differ from `saved`. */
export function changesFrom(saved: LaserInputView[], now: LaserInputView[]): number {
  let n = 0;
  for (let i = 0; i < Math.max(saved.length, now.length); i++) {
    const a = saved[i];
    const b = now[i];
    if (!a || !b) {
      n++;
      continue;
    }
    if (a.laser !== b.laser) n++;
    for (let s = 0; s < Math.max(a.modules.length, b.modules.length); s++) if (a.modules[s] !== b.modules[s]) n++;
  }
  return n;
}

/** Replace your lasers on the calculator (a try-out until saved). */
export function setOwnLasers(own: LaserInputView[]): Promise<void> {
  const current = _state?.current;
  return current ? setLoadout(withCrew(own, current)) : Promise.resolve();
}

/** Put a saved loadout of yours on the calculator, crew mates and the
 *  heads holding fire kept. */
export function useLoadout(id: string): Promise<void> {
  const saved = _state?.saved.find((s) => s.id === id);
  const current = _state?.current;
  if (!saved || !current) return Promise.resolve();
  return setLoadout(withCrew(saved.loadout.lasers, current, true), id);
}

/** Switch your head at `index` (among your own lasers) on or off: a seat
 *  nobody is in today. Not a change to the loadout, so no try-out. */
export function setHeadOff(index: number, off: boolean): Promise<void> {
  const current = _state?.current;
  if (!current) return Promise.resolve();
  return setOwnLasers(ownLasers(current).map((l, i) => (i === index ? { ...l, off } : l)));
}

/** Drop the try-out: back to the saved loadout. */
export function revertTryOut(): Promise<void> {
  const id = _state?.basedOn;
  return id ? useLoadout(id) : Promise.resolve();
}

/** Write the try-out into the saved loadout it came from. */
export async function saveTryOut(): Promise<string | null> {
  const saved = _state?.saved.find((s) => s.id === _state?.basedOn);
  if (!saved || !_state) return null;
  return saveLoadout(saved.id, saved.name, null, asSaved(ownLasers(_state.current)));
}

/** Keep the try-out as a new loadout of yours (the old one stays). */
export async function saveTryOutAs(name: string): Promise<string | null> {
  if (!_state) return null;
  const current = _state.current;
  const error = await saveLoadout(null, name, null, asSaved(ownLasers(current)));
  if (error) return error;
  const made = _state.saved.find((s) => s.owner === null && s.name.toLowerCase() === name.trim().toLowerCase());
  if (made) await setLoadout(current, made.id);
  return null;
}

/** A loadout as it is saved, from lasers being edited elsewhere. */
export const savedForm = (lasers: LaserInputView[]): LoadoutView => asSaved(lasers);

// ── Comparing loadouts on the rock ──────────────────────────────────────────
// The picker's options, the loadout list and a try-out against its saved
// loadout, all on the calculator's rock (at the tried mass). Cached per
// rock and loadouts, so reopening a picker or the list doesn't re-run it.

const _compared = new Map<string, Promise<(EvalView | null)[] | null>>();
const COMPARE_CACHE = 80;

/** How each loadout does on the calculator's rock; null without a rock. */
export function compareOnRock(channel: string, loadouts: LoadoutView[]): Promise<(EvalView | null)[] | null> {
  const rock = draftRock();
  if (!rock || !loadouts.length) return Promise.resolve(null);
  const at = { ...rock, mass: _massOverride ?? rock.mass };
  const key = JSON.stringify([channel, at, loadouts.map((l) => [l.lasers.map((x) => [x.laser, x.modules, x.owner, x.distanceM, x.off]), l.gadget])]);
  const hit = _compared.get(key);
  if (hit) return hit;
  const run = commands.miningCompare(channel, at, loadouts).then((r) => (r.status === "ok" ? r.data : null));
  _compared.set(key, run);
  if (_compared.size > COMPARE_CACHE) _compared.delete(_compared.keys().next().value as string);
  return run;
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

/** Your lasers' beam distance to the rock (null: within optimal range);
 *  crew mates' ships keep theirs. */
export async function setDistance(meters: number | null): Promise<void> {
  const current = _state?.current;
  if (!current) return;
  await setLoadout({
    ...current,
    lasers: current.lasers.map((l) => (l.crew === null ? { ...l, distanceM: meters } : l)),
  });
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

const bandInputs = () => JSON.stringify([_state?.current ?? null, draftRock(), _source?.heaviest ?? null]);
const classInputs = (l: LoadoutView) => JSON.stringify([l.lasers.map((x) => [x.laser, x.modules, x.owner, x.off]), l.gadget]);

/** The class band for the drafted rock and the calculator's loadout. */
export async function loadMassBand(channel: string): Promise<void> {
  const loadout = _state?.current;
  const rock = draftRock();
  const inputs = bandInputs();
  const mine = ++_bandReq;
  if (!loadout || !loadout.lasers.length || !rock) {
    _band = null;
    return;
  }
  const result = await commands.miningMassBand(channel, loadout, rock, _source?.heaviest ?? null);
  if (mine !== _bandReq) return;
  _band = result.status === "ok" ? result.data : null;
  _bandFor = inputs;
}

/** The finder's class table for the calculator's loadout (cached per
 *  loadout: it rolls every rock type at every area). */
export async function loadClassTable(channel: string): Promise<void> {
  const loadout = _state?.current;
  if (!loadout || !loadout.lasers.length) {
    _classComfort = null;
    _classKey = "";
    return;
  }
  const inputs = classInputs(loadout);
  const key = JSON.stringify([channel, inputs]);
  if (key === _classKey) return;
  _classKey = key;
  const mine = ++_classReq;
  const result = await ensureComfort(channel, loadout, true);
  if (mine !== _classReq) return;
  _classComfort = result;
  _classFor = inputs;
}

/** The loadout as a comfort solve sees it: its lasers' parts, crew and
 *  range, and the gadget. */
export const comfortKey = (channel: string, l: LoadoutView) =>
  JSON.stringify([channel, l.lasers.map((x) => [x.laser, x.modules, x.owner, x.crew !== null, x.distanceM, x.off]), l.gadget]);

/** The loadout's comfort solve, when done (reactive); null while it runs or
 *  when it failed. */
export function comfortOf(channel: string, loadout: LoadoutView): ComfortView[] | null {
  return _comfort.get(comfortKey(channel, loadout)) ?? null;
}

/** Ask for a loadout's comfort solve; `urgent` goes before the waiting
 *  ones (the sheet and the calculator's loadout before a sweep's options). */
export function requestComfort(channel: string, loadout: LoadoutView, urgent = false): void {
  void ensureComfort(channel, loadout, urgent);
}

/** The comfort solve, once it is done. */
export function ensureComfort(channel: string, loadout: LoadoutView, urgent = false): Promise<ComfortView[] | null> {
  const key = comfortKey(channel, loadout);
  const have = _comfort.get(key);
  if (have) return Promise.resolve(have);
  if (_comfortFailed.has(key) || !loadout.lasers.length) return Promise.resolve(null);
  return new Promise((resolve) => {
    if (_comfortRunning?.key === key) {
      _comfortRunning.done.push(resolve);
      return;
    }
    const at = _comfortQueue.findIndex((j) => j.key === key);
    const job = at >= 0 ? _comfortQueue.splice(at, 1)[0] : { key, channel, loadout, done: [] };
    job.done.push(resolve);
    if (urgent) _comfortQueue.unshift(job);
    else _comfortQueue.push(job);
    void drainComfort();
  });
}

/** Drop the waiting solves `keep` says no to (a closed sweep's options). */
export function dropComfort(keep: (key: string) => boolean): void {
  for (let i = _comfortQueue.length - 1; i >= 0; i--) {
    const job = _comfortQueue[i];
    if (keep(job.key)) continue;
    _comfortQueue.splice(i, 1);
    job.done.forEach((d) => d(null));
  }
}

async function drainComfort(): Promise<void> {
  if (_comfortRunning) return;
  _comfortBusy = true;
  while (_comfortQueue.length) {
    const job = _comfortQueue.shift()!;
    _comfortRunning = job;
    _comfortRunningKey = job.key;
    const result = await commands.miningComfort(job.channel, job.loadout);
    _comfortRunning = null;
    _comfortRunningKey = "";
    if (result.status === "ok") {
      const next = new Map(_comfort);
      next.set(job.key, result.data);
      while (next.size > COMFORT_KEEP) next.delete(next.keys().next().value as string);
      _comfort = next;
      job.done.forEach((d) => d(result.data));
    } else {
      _comfortFailed.add(job.key);
      job.done.forEach((d) => d(null));
    }
  }
  _comfortBusy = false;
}

/** A new build's data: the session's solves were for the old one. */
function forgetComfort(): void {
  _comfort = new Map();
  _comfortFailed = new Set();
  _classKey = "";
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

/** Where the rock is (the Range card's switch); the solve follows. */
export function setSurface(surface: boolean | null): void {
  _rock.surface = surface;
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
function draftRock(): {
  mass: number;
  entries: { element: string; percent: number; quality: number | null }[];
  surface: boolean | null;
} | null {
  const mass = parseNumber(_rock.mass, true);
  const entries = _rock.rows.flatMap((r) => {
    const percent = parseNumber(r.percent);
    if (!r.element || percent === null || percent <= 0) return [];
    const q = parseNumber(r.quality);
    return [{ element: r.element, percent, quality: q === null ? null : Math.round(q) }];
  });
  return mass === null || mass <= 0 || entries.length === 0 ? null : { mass, entries, surface: _rock.surface };
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
    // New game data: the solves were for the old build.
    forgetComfort();
    const c = channel();
    if (c) {
      void loadMiningCatalog(c);
      void loadMiningAreas(c);
    }
  });
}
