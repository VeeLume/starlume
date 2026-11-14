// Plans: the goals you set (a small file Rust-side) and the board derived
// from them. The board is never stored: Rust derives it on each read from
// the cached inventory, blueprints, saved loadouts, prices and game data,
// so this store only mirrors it (frontend rules 1, 6). Pages render the
// cached board and refresh behind it (rule 2); `plans:changed` and the
// events of what the board is derived from re-read it (rule 5), and a focus
// re-read catches up after a suspension.

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { commands, type LoadoutChoiceView, type Plan, type PlansBoardView } from "$lib/bindings";

let _plans = $state<Plan[] | null>(null);
let _board = $state<PlansBoardView | null>(null);
/** The channel `_board` was derived for. */
let _boardChannel = $state<string | null>(null);
let _loading = $state(false);
let _error = $state("");
let _choices = $state<LoadoutChoiceView[]>([]);

export const plansStore = {
  /** Your plans in order; null until first read. */
  get plans() {
    return _plans;
  },
  /** The board of `channel`, or null when none was derived for it yet. */
  board(channel: string | null): PlansBoardView | null {
    return channel && channel === _boardChannel ? _board : null;
  },
  /** A board is being derived and none is cached to show meanwhile. */
  get loading() {
    return _loading && !_board;
  },
  get error() {
    return _error;
  },
  /** The saved loadouts "New plan" offers. */
  get choices() {
    return _choices;
  },
  /** The plan for this blueprint, if it is planned. */
  craftOf(blueprint: string): Plan | undefined {
    return _plans?.find((p) => p.goal.kind === "craft" && p.goal.blueprint === blueprint);
  },
  /** The plan for this saved loadout, if it is planned. */
  loadoutOf(loadout: string): Plan | undefined {
    return _plans?.find((p) => p.goal.kind === "loadout" && p.goal.loadout === loadout);
  },
};

/** The plans themselves: one small file read, no game data needed. */
export async function loadPlans(): Promise<void> {
  _plans = await commands.plansList();
}

let _boardRun = 0;

/** Derive the board again. The cached one stays up until the new one is in. */
export async function loadBoard(channel: string): Promise<void> {
  const run = ++_boardRun;
  _loading = true;
  const result = await commands.plansBoard(channel);
  if (run !== _boardRun) return;
  _loading = false;
  if (result.status === "ok") {
    _board = result.data;
    _boardChannel = channel;
    _error = "";
  } else {
    _error = result.error.message;
  }
}

export async function loadChoices(channel: string): Promise<void> {
  const result = await commands.plansLoadoutChoices(channel);
  if (result.status === "ok") _choices = result.data;
}

type Changed = Awaited<ReturnType<typeof commands.plansRemove>>;

/** Take a mutation's answer: the new list, or its error. The board follows
 *  through `plans:changed` wherever a page watches it. */
function settle(result: Changed): boolean {
  if (result.status === "ok") {
    _plans = result.data;
    _error = "";
    return true;
  }
  _error = result.error.message;
  return false;
}

export async function addCraftPlan(blueprint: string, count = 1, quality = 0): Promise<boolean> {
  return settle(await commands.plansAddCraft(blueprint, count, quality));
}

export async function addLoadoutPlan(loadout: string): Promise<boolean> {
  return settle(await commands.plansAddLoadout(loadout));
}

export async function removePlan(id: string): Promise<boolean> {
  return settle(await commands.plansRemove(id));
}

export async function focusPlan(id: string): Promise<boolean> {
  return settle(await commands.plansFocus(id));
}

export async function setCraftPlan(id: string, count: number, quality: number): Promise<boolean> {
  return settle(await commands.plansSetCraft(id, count, quality));
}

/** What the board is derived from: a change to any of these re-derives it. */
const SOURCES = ["plans:changed", "items:changed", "blueprints:changed", "data:changed", "uex:changed"];

let _watchers = 0;
let _unlisten: Promise<UnlistenFn>[] = [];
let _channelOf: () => string | null = () => null;
const refresh = () => {
  void loadPlans();
  const channel = _channelOf();
  if (channel) void loadBoard(channel);
};

/** Keep plans and board current while a page shows them. Returns the stop
 *  function. */
export function watchPlans(channel: () => string | null): () => void {
  _channelOf = channel;
  if (_watchers++ === 0) {
    _unlisten = SOURCES.map((name) => listen(name, refresh));
    window.addEventListener("focus", refresh);
  }
  let stopped = false;
  return () => {
    if (stopped) return;
    stopped = true;
    if (--_watchers === 0) {
      for (const u of _unlisten) void u.then((off) => off());
      _unlisten = [];
      window.removeEventListener("focus", refresh);
    }
  };
}
