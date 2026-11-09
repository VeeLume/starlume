// Ships state. The ship list is the build's cooked reference data; which
// ship is open, its edits and the power settings are session state that
// lives here so a navigation keeps them (frontend rule 1); the page for the
// open ship (slots, power network, figures) is computed Rust-side per
// request, newest answer wins. Only the store pictures touch the network,
// through their gated refresh command.

import { convertFileSrc } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  commands,
  type CandidateView,
  type ShipDraftView,
  type EditView,
  type LevelView,
  type ShipLoadoutsView,
  type ShipMarketView,
  type ShipOwnedView,
  type ShipPricesView,
  type ShipScene,
  type ShipSavedLoadoutView,
  type QtPlacesView,
  type ModeView,
  type ShipPageView,
  type ShipRowView,
  type ShipsHomeView,
} from "$lib/bindings";

let _ships = $state.raw<ShipRowView[] | null>(null);
let _shipsChannel: string | null = null;
let _shipsError = $state("");

let _page = $state.raw<ShipPageView | null>(null);
let _pageKey = "";
let _pageError = $state("");
let _pageSeq = 0;

/** What the page has open (session state). */
export const shipsUi = $state({
  /** The open ship's GUID; "" until picked. */
  ship: "",
  /** Swaps per ship, on top of its stock loadout. */
  edits: {} as Record<string, EditView[]>,
  mode: "scm" as ModeView,
  /** The Build card shows the ship's picture rather than its holo. */
  picture: false,
  /** Port paths (`/`-joined) switched off, per ship. */
  off: {} as Record<string, string[]>,
  /** Power columns set by hand, per ship, in the order they were set. */
  levels: {} as Record<string, LevelView[]>,
  /** Per ship: the saved loadout the edits are a try-out against; null:
   *  stock. */
  basedOn: {} as Record<string, string | null>,
});

let _saved = $state.raw<Record<string, ShipSavedLoadoutView[]>>({});

/** The player's saved loadouts, per ship GUID, by name. */
export const shipsLoadouts = {
  of(ship: string): ShipSavedLoadoutView[] {
    return _saved[ship] ?? [];
  },
};

export const shipsStore = {
  get ships() {
    return _ships;
  },
  get shipsError() {
    return _shipsError;
  },
  get page() {
    return _page;
  },
  get pageError() {
    return _pageError;
  },
  /** The open ship as it comes from the factory, under the same power
   *  settings, for the changes the edits make; null while it has none. */
  get stock() {
    return _stock;
  },
};

let _stock = $state.raw<ShipPageView | null>(null);
let _stockKey = "";
let _stockSeq = 0;

let _home = $state.raw<ShipsHomeView | null>(null);

export const shipsHome = {
  get view() {
    return _home;
  },
};

/** The page with no ship open: your ships, new ships, recent ones. Reads
 *  the inventory snapshot already on disk: no network. */
export async function loadHome(channel: string): Promise<void> {
  const result = await commands.shipsHome(channel);
  if (result.status === "ok") _home = result.data;
}

let _places = $state.raw<QtPlacesView | null>(null);
let _placesChannel: string | null = null;

/** Where quantum travel goes (the build's waypoints) and the default
 *  routes. Cooked data; loaded once per channel. */
export const shipsPlaces = {
  get view() {
    return _places;
  },
};

export async function loadPlaces(channel: string, force = false): Promise<void> {
  if (!force && channel === _placesChannel && _places) return;
  const result = await commands.shipsPlaces(channel);
  if (result.status === "ok") {
    _places = result.data;
    _placesChannel = channel;
  }
}

let _images = $state.raw<Map<string, string>>(new Map());

/** Store pictures by hull GUID, as webview URLs (downloaded files). */
export const shipsImages = {
  get byGuid() {
    return _images;
  },
  /** The large pictures fetched so far: only for ships shown as a picture. */
  get large() {
    return _wallpapers;
  },
};

let _wallpapers = $state.raw<Map<string, string>>(new Map());
const _wallpapersAsked = new Set<string>();

/** `ship`'s large picture, ready to draw before the page swaps to it. Asked
 *  again next time when it couldn't be had (offline, say). */
export async function loadWallpaper(channel: string, ship: string): Promise<void> {
  if (_wallpapersAsked.has(ship)) return;
  _wallpapersAsked.add(ship);
  const result = await commands.shipsWallpaper(channel, ship);
  if (result.status !== "ok" || !result.data) {
    _wallpapersAsked.delete(ship);
    return;
  }
  const url = convertFileSrc(result.data);
  const image = new Image();
  image.src = url;
  await image.decode().catch(() => undefined);
  _wallpapers = new Map(_wallpapers).set(ship, url);
}

/** The pictures on disk (no network), then a refresh of the store list if
 *  it is a week old: online-gated Rust-side, silent when offline. A
 *  finished refresh pushes `ships:images`, which reloads. */
export async function loadImages(channel: string): Promise<void> {
  const result = await commands.shipsImages(channel);
  if (result.status === "ok") _images = new Map(result.data.map((i) => [i.guid, convertFileSrc(i.path)]));
}

export function refreshImages(): void {
  void commands.shipsImagesRefresh(false);
}

let _market = $state.raw<Map<string, ShipMarketView>>(new Map());

/** Every ship's prices for the catalogue's cards: in game (UEX) and on
 *  the pledge store. */
export const shipsMarket = {
  of(ship: string): ShipMarketView | null {
    return _market.get(ship) ?? null;
  },
};

/** The catalogue's prices from the cached UEX and store lists (no
 *  network). Cache-first: the last ones stay while these load. */
export async function loadMarket(channel: string): Promise<void> {
  const result = await commands.shipsMarket(channel);
  if (result.status === "ok") _market = new Map(result.data.map((m) => [m.ship, m]));
}

let _prices = $state.raw<Record<string, ShipPricesView>>({});

/** Where each ship is sold and rented (UEX's cached community prices). */
export const shipsPrices = {
  of(ship: string): ShipPricesView | null {
    return _prices[ship] ?? null;
  },
};

/** `ship`'s buy and rent prices from the cached UEX lists (no network).
 *  Cache-first: the last answer stays while this one loads. */
export async function loadShipPrices(channel: string, ship: string): Promise<void> {
  const result = await commands.shipsPrices(channel, ship);
  if (result.status === "ok") _prices = { ..._prices, [ship]: result.data };
}

/** Open `ship` and remember it for "Recently opened". The first time this
 *  session, its saved loadouts and the draft it was left with come back
 *  from disk. */
export function openShip(ship: string): void {
  shipsUi.ship = ship;
  closePicker();
  void commands.shipsOpened(ship);
  void loadLoadouts(ship);
}

// ── Loadouts ──────────────────────────────────────────────────────────
// The edits and power settings are a draft Rust keeps per ship (frontend
// rule 6: a restart must not lose them): every change is written through,
// a moment later. A draft is a try-out against the saved loadout it came
// from, or stock, until saved.

/** Ships whose draft came back from disk this session: only theirs may be
 *  written, so a change made before the file was read can never overwrite
 *  the draft in it. */
const _hydrated = new Set<string>();

async function loadLoadouts(ship: string): Promise<void> {
  const result = await commands.shipsLoadouts(ship);
  if (result.status !== "ok") return;
  _saved = { ..._saved, [ship]: result.data.saved };
  // The file is the truth on the first open this session; after that the
  // session's state is the newer one.
  if (!_hydrated.has(ship)) adopt(ship, result.data);
}

/** Take `ship`'s loadouts and draft as the file has them. */
function adopt(ship: string, view: ShipLoadoutsView): void {
  _hydrated.add(ship);
  _saved = { ..._saved, [ship]: view.saved };
  const d = view.draft;
  shipsUi.basedOn[ship] = d?.basedOn ?? null;
  shipsUi.edits[ship] = d?.edits ?? [];
  shipsUi.off[ship] = d?.off ?? [];
  shipsUi.levels[ship] = d?.levels ?? [];
}

function draftOf(ship: string): ShipDraftView {
  return $state.snapshot({
    basedOn: shipsUi.basedOn[ship] ?? null,
    edits: shipsUi.edits[ship] ?? [],
    off: shipsUi.off[ship] ?? [],
    levels: shipsUi.levels[ship] ?? [],
  });
}

const _pending = new Map<string, ReturnType<typeof setTimeout>>();

/** Write `ship`'s draft through, once a burst of changes settles; not
 *  before its draft was read (see `_hydrated`). */
function persist(ship: string): void {
  if (!_hydrated.has(ship)) return;
  clearTimeout(_pending.get(ship));
  _pending.set(
    ship,
    setTimeout(() => {
      _pending.delete(ship);
      void commands.shipsSetDraft(ship, draftOf(ship));
    }, 300),
  );
}

/** What a list of swaps does in the end, per port (a later swap of the
 *  same port replaces an earlier one). */
function finalSwaps(edits: EditView[]): Map<string, string | null> {
  return new Map(edits.map((e) => [e.path.join("/"), e.item]));
}

let _owned = $state.raw<Record<string, ShipOwnedView>>({});

/** Your ships of each class and their loadouts in the game (the inventory
 *  My Items last read). */
export const shipsOwned = {
  of(ship: string): ShipOwnedView | null {
    return _owned[ship] ?? null;
  },
};

/** `ship`'s copies you own and their insured and live loadouts, from the
 *  cached inventory (no network). */
export async function loadOwned(channel: string, ship: string): Promise<void> {
  const result = await commands.shipsOwned(channel, ship);
  if (result.status === "ok") _owned = { ..._owned, [ship]: result.data };
}

/** A loadout in the game as a draft's base: `insured:<urn>` or
 *  `live:<urn>`. Read-only: saving it makes a new loadout. */
export const inGame = (id: string | null | undefined) => !!id && (id.startsWith("insured:") || id.startsWith("live:"));

/** What a draft of `ship` is measured against: the saved loadout it came
 *  from, or one in the game (no power settings: the game has none). */
function baseOf(ship: string): { edits: EditView[]; off: string[]; levels: LevelView[] } | null {
  const id = shipsUi.basedOn[ship];
  if (!id) return null;
  if (inGame(id)) {
    const [kind, urn] = [id.slice(0, id.indexOf(":")), id.slice(id.indexOf(":") + 1)];
    const o = shipsOwned.of(ship)?.ships.find((s) => s.urn === urn);
    const edits = kind === "insured" ? o?.insured : o?.live;
    return edits ? { edits, off: [], levels: [] } : null;
  }
  return shipsLoadouts.of(ship).find((s) => s.id === id) ?? null;
}

/** How many ports and power settings `ship`'s draft changes against its
 *  base (a saved loadout, one in the game, or stock). */
export function changesOf(ship: string): number {
  const base = baseOf(ship);
  const a = finalSwaps(base?.edits ?? []);
  const b = finalSwaps(shipsUi.edits[ship] ?? []);
  let n = 0;
  for (const k of new Set([...a.keys(), ...b.keys()])) if (a.get(k) !== b.get(k)) n++;
  const offA = new Set(base?.off ?? []);
  const offB = new Set(shipsUi.off[ship] ?? []);
  for (const p of new Set([...offA, ...offB])) if (offA.has(p) !== offB.has(p)) n++;
  const lvA = new Map((base?.levels ?? []).map((l) => [l.column, l.pips]));
  const lvB = new Map((shipsUi.levels[ship] ?? []).map((l) => [l.column, l.pips]));
  for (const c of new Set([...lvA.keys(), ...lvB.keys()])) if (lvA.get(c) !== lvB.get(c)) n++;
  return n;
}

/** Save the open ship's draft: into the loadout it is based on, or with
 *  `asNew` as a new one named `name`. An error message, or null. */
export async function saveLoadout(channel: string, name: string, asNew: boolean): Promise<string | null> {
  const ship = shipsUi.ship;
  // A loadout in the game is read-only: saving makes a new one.
  const based = shipsUi.basedOn[ship] ?? null;
  const id = asNew || inGame(based) ? null : based;
  const clash = shipsLoadouts.of(ship).find((s) => s.name.trim().toLowerCase() === name.trim().toLowerCase() && s.id !== id);
  if (clash) return `There is a loadout named ${clash.name} already.`;
  clearTimeout(_pending.get(ship));
  const result = await commands.shipsSaveLoadout(channel, ship, id, name, draftOf(ship));
  if (result.status !== "ok") return result.error.message;
  _saved = { ..._saved, [ship]: result.data.saved };
  shipsUi.basedOn[ship] = result.data.draft?.basedOn ?? null;
  return null;
}

/** Open a saved loadout, one in the game (`insured:<urn>`, `live:<urn>`)
 *  or stock (null) on the open ship; its draft goes. */
export async function openLoadout(id: string | null): Promise<void> {
  const ship = shipsUi.ship;
  clearTimeout(_pending.get(ship));
  if (inGame(id)) {
    shipsUi.basedOn[ship] = id;
    const base = baseOf(ship);
    shipsUi.edits[ship] = base ? $state.snapshot(base.edits) : [];
    shipsUi.off[ship] = [];
    shipsUi.levels[ship] = [];
    _hydrated.add(ship);
    await commands.shipsSetDraft(ship, draftOf(ship));
    return;
  }
  const result = await commands.shipsOpenLoadout(ship, id);
  if (result.status === "ok") adopt(ship, result.data);
}

/** Rename a saved loadout of the open ship; its parts and the draft stay.
 *  An error message, or null. */
export async function renameLoadout(id: string, name: string): Promise<string | null> {
  const ship = shipsUi.ship;
  const clash = shipsLoadouts.of(ship).find((s) => s.name.trim().toLowerCase() === name.trim().toLowerCase() && s.id !== id);
  if (clash) return `There is a loadout named ${clash.name} already.`;
  const result = await commands.shipsRenameLoadout(ship, id, name);
  if (result.status !== "ok") return result.error.message;
  _saved = { ..._saved, [ship]: result.data.saved };
  return null;
}

/** Forget a saved loadout; what is open keeps its edits, against stock. */
export async function deleteLoadout(id: string): Promise<void> {
  const ship = shipsUi.ship;
  clearTimeout(_pending.get(ship));
  // The draft as it is now goes first, so the delete keeps its edits.
  if (_hydrated.has(ship)) await commands.shipsSetDraft(ship, draftOf(ship));
  const result = await commands.shipsDeleteLoadout(ship, id);
  if (result.status === "ok") adopt(ship, result.data);
}

/** Back to the ships home page. */
export function closeShip(): void {
  shipsUi.ship = "";
  closePicker();
}

/** Load `channel`'s ship list. Cache-first: the last list stays on screen
 *  while a reload runs (frontend rule 2). */
export async function loadShips(channel: string): Promise<void> {
  if (channel !== _shipsChannel) {
    _ships = null;
    _page = null;
    _pageKey = "";
    _shipsChannel = channel;
  }
  const result = await commands.shipsList(channel);
  if (channel !== _shipsChannel) return;
  if (result.status === "ok") {
    _ships = result.data;
    _shipsError = "";
  } else {
    _shipsError = result.error.message;
  }
}

/** Compute the open ship's page. The newest request wins; the last page
 *  stays on screen meanwhile. */
export async function loadPage(channel: string): Promise<void> {
  const ship = shipsUi.ship;
  if (!ship) return;
  const loadout = { ship, edits: shipsUi.edits[ship] ?? [] };
  const power = { mode: shipsUi.mode, off: shipsUi.off[ship] ?? [], levels: shipsUi.levels[ship] ?? [] };
  void loadStock(channel, ship, loadout.edits.length > 0, power);
  const key = JSON.stringify([channel, loadout, power]);
  if (key === _pageKey && _page) return;
  const seq = ++_pageSeq;
  const result = await commands.shipsView(channel, loadout, power);
  if (seq !== _pageSeq) return;
  if (result.status === "ok") {
    _page = result.data;
    _pageKey = key;
    _pageError = "";
  } else {
    _pageError = result.error.message;
  }
}

let _scene = $state.raw<{ ship: string; key: string; scene: ShipScene } | null>(null);
let _sceneSeq = 0;

/** The open ship as its holo draws it, fitted parts included. */
export const shipsScene = {
  of(ship: string): ShipScene | null {
    return _scene?.ship === ship ? _scene.scene : null;
  },
};

/** Assemble the open ship for its holo; the last scene stays while the next
 *  loads, the newest request wins. A failure leaves the picture showing. */
export async function loadScene(channel: string): Promise<void> {
  const ship = shipsUi.ship;
  if (!ship) return;
  const loadout = { ship, edits: shipsUi.edits[ship] ?? [] };
  const key = JSON.stringify([channel, loadout]);
  if (key === _scene?.key) return;
  const seq = ++_sceneSeq;
  const result = await commands.shipsScene(channel, loadout);
  if (seq !== _sceneSeq) return;
  if (result.status === "ok") _scene = { ship, key, scene: result.data };
  else console.warn("no ship holo:", result.error.message);
}

/** The stock page beside the edited one (spviewer's "+2,938 DPS"): only
 *  while there are edits, newest request wins. */
async function loadStock(
  channel: string,
  ship: string,
  edited: boolean,
  power: { mode: ModeView; off: string[]; levels: LevelView[] },
): Promise<void> {
  const seq = ++_stockSeq;
  if (!edited) {
    _stock = null;
    _stockKey = "";
    return;
  }
  const loadout = { ship, edits: [] };
  const key = JSON.stringify([channel, loadout, power]);
  if (key === _stockKey && _stock) return;
  const result = await commands.shipsView(channel, loadout, power);
  if (seq !== _stockSeq) return;
  if (result.status === "ok") {
    _stock = result.data;
    _stockKey = key;
  }
}

let _candidates = $state.raw<CandidateView[] | null>(null);
let _candidatesError = $state("");
let _candidatesSeq = 0;

/** The port the picker is open on, and the ports the pick also goes to
 *  (a turret's other guns); null: closed. */
export const shipsPicker = $state({ open: null as { targets: string[][] } | null });

export const shipsCandidates = {
  get list() {
    return _candidates;
  },
  get error() {
    return _candidatesError;
  },
};

/** Open the picker on `targets[0]` and load what fits it. */
export async function openPicker(channel: string, targets: string[][]): Promise<void> {
  const ship = shipsUi.ship;
  shipsPicker.open = { targets };
  _candidates = null;
  _candidatesError = "";
  const seq = ++_candidatesSeq;
  const loadout = { ship, edits: shipsUi.edits[ship] ?? [] };
  const power = { mode: shipsUi.mode, off: shipsUi.off[ship] ?? [], levels: shipsUi.levels[ship] ?? [] };
  const result = await commands.shipsCandidates(channel, loadout, power, targets);
  if (seq !== _candidatesSeq) return;
  if (result.status === "ok") _candidates = result.data;
  else _candidatesError = result.error.message;
}

export function closePicker(): void {
  shipsPicker.open = null;
  _candidatesSeq++;
}

/** Fit `item` on every target port; null empties them. */
export function pick(targets: string[][], item: string | null): void {
  const ship = shipsUi.ship;
  const edits = shipsUi.edits[ship] ?? [];
  shipsUi.edits[ship] = [...edits, ...targets.map((path) => ({ path, item }))];
  persist(ship);
  closePicker();
}

/** Switch a part, a turret or a whole system on or off. */
export function toggleOff(paths: string[]): void {
  const ship = shipsUi.ship;
  const now = new Set(shipsUi.off[ship] ?? []);
  const allOff = paths.every((p) => now.has(p));
  for (const p of paths) {
    if (allOff) now.delete(p);
    else now.add(p);
  }
  shipsUi.off[ship] = [...now];
  persist(ship);
}

/** Set a power column to `pips`; a later setting of the same column
 *  replaces the earlier one. */
export function setLevel(column: string, pips: number): void {
  const ship = shipsUi.ship;
  // A raise takes only the free pips, as in game; kept as what it got, so
  // an old ask past them can't later claim pips a newer setting freed.
  const power = _page?.ship === ship ? _page.power : null;
  const col = power?.columns.find((c) => c.id === column);
  if (power && col && pips > col.level) pips = Math.min(pips, Math.floor(col.level + Math.max(power.supply - power.used, 0)));
  const levels = (shipsUi.levels[ship] ?? []).filter((l) => l.column !== column);
  shipsUi.levels[ship] = [...levels, { column, pips }];
  persist(ship);
}

/** Back to the power-on distribution: every item on, no hand-set pips. */
export function resetPower(): void {
  const ship = shipsUi.ship;
  shipsUi.off[ship] = [];
  shipsUi.levels[ship] = [];
  persist(ship);
}

/** Backend push: the game data changed (a cook landed, a wipe), or new
 *  store pictures arrived. */
export async function listenForShipChanges(channel: () => string | null): Promise<UnlistenFn> {
  const data = await listen("data:changed", () => {
    const c = channel();
    if (c) {
      _pageKey = "";
      void loadShips(c);
      void loadPage(c);
      void loadImages(c);
      void loadPlaces(c, true);
    }
  });
  // A store refresh: new pictures, and the store prices with them.
  const images = await listen("ships:images", () => {
    const c = channel();
    if (c) {
      void loadImages(c);
      void loadMarket(c);
    }
  });
  // New UEX prices: the open ship's, and the open picker's list.
  const prices = await listen("uex:changed", () => {
    const c = channel();
    if (!c) return;
    for (const ship of Object.keys(_prices)) void loadShipPrices(c, ship);
    void loadMarket(c);
    if (shipsPicker.open) void openPicker(c, shipsPicker.open.targets);
  });
  return () => {
    data();
    images();
    prices();
  };
}
