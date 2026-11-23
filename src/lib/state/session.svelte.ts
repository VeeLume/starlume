// The session log (session.rs): the current game session, the list of past
// ones, and the past sessions opened in this visit.
//
// The current session lives Rust-side (a suspended webview runs no JS); this
// store mirrors it (frontend rules 1, 6). Pages render from the cache and
// refresh in the background (rule 2). The backend announces changes with
// `session:changed` (rule 5); a focus re-read catches up after a suspension.
// Local only: no network, no gate. Its two sources have their own opt-ins
// (`gamelog_enabled`, `telemetry_enabled`), which this store only reports.

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  commands,
  type Session,
  type SessionNames,
  type SessionStateView,
  type SessionSummary,
} from "$lib/bindings";
import { mergeNames, nameQuery, unanswered } from "$lib/modules/session/model";

/** Past sessions kept in memory at once: the ones just looked at. */
const OPENED_KEPT = 4;

let _state = $state<SessionStateView | null>(null);
let _history = $state<SessionSummary[] | null>(null);
let _opened = $state<Record<string, Session | null>>({});
let _error = $state("");
// Display names by channel ("" = the default channel): what `session_names`
// has answered. A name is asked for once. When the game data changes the
// answers are kept on screen and asked again (rule 2: no blanking): one
// given without game data is a cleaned internal name and may do better.
let _names = $state<Record<string, SessionNames>>({});
/** Channels whose answers are older than the last game-data change. */
let _stale = $state<Record<string, boolean>>({});
// Your average minutes per contract record name, from the filed sessions:
// the Contracts catalog's time column. `null` = not read yet.
let _runMinutes = $state<Partial<Record<string, number>> | null>(null);
let _unlistenRuns: Promise<UnlistenFn> | null = null;

export const sessionStore = {
  /** `null` = not read yet. */
  get state() {
    return _state;
  },
  /** The live session, or the last one seen in this run of the app. */
  get current(): Session | null {
    return _state?.current ?? null;
  },
  /** Finished sessions, newest first; `null` = not read yet. */
  get history() {
    return _history;
  },
  /** A past session by id: `undefined` = not read yet, `null` = not kept. */
  opened(id: string): Session | null | undefined {
    return _opened[id];
  },
  /** The display names known for a session's channel; `null` = none yet. */
  names(session: Pick<Session, "channel">): SessionNames | null {
    return _names[session.channel ?? ""] ?? null;
  },
  /** Your average minutes per contract record name; empty while the module
   *  is off, `null` = not read yet. */
  get runMinutes() {
    return _runMinutes;
  },
  get error() {
    return _error;
  },
};

/** The current session: a memory read Rust-side. */
export async function loadSession(): Promise<void> {
  _state = await commands.sessionState();
}

/** The history list: one small file read. */
export async function loadHistory(): Promise<void> {
  const result = await commands.sessionHistory();
  if (result.status === "ok") {
    _history = result.data;
    _error = "";
  } else {
    _error = result.error.message;
  }
}

/** Read one past session (cached; a finished session does not change,
 *  except when its log is read again, which `session:changed` reports). */
export async function openSession(id: string): Promise<void> {
  if (id in _opened) return;
  const result = await commands.sessionGet(id);
  if (result.status !== "ok") {
    _error = result.error.message;
    return;
  }
  const kept = Object.entries(_opened).slice(-(OPENED_KEPT - 1));
  _opened = Object.fromEntries([...kept, [id, result.data]]);
  _error = "";
}

/** Read a past session's Game.log again and file what this version finds
 *  in it. `session:changed` then reloads the history and the open session.
 *  Returns what to tell the user, `""` when it worked. */
export async function rereadSession(id: string): Promise<string> {
  const result = await commands.sessionReread(id);
  if (result.status !== "ok") return result.error.message;
  return result.data ? "" : "Its Game.log is gone: the game keeps only so many old logs.";
}

/** Your measured times per contract: one small file read. A filed session
 *  (`session:changed` with history) can add runs, so it reloads then. */
export async function loadRunMinutes(): Promise<void> {
  _unlistenRuns ??= listen<{ history: boolean }>("session:changed", (event) => {
    if (event.payload.history) void loadRunMinutes();
  });
  _runMinutes = await commands.sessionRunMinutes();
}

const _asking = new Set<string>();

/** Ask Rust for the display names a session holds and the store lacks
 *  (all of them again when the game data changed since). A no-op when it has
 *  them all. */
export async function loadNames(session: Session): Promise<void> {
  const channel = session.channel ?? "";
  const all = nameQuery(session);
  const rest = unanswered(all, _stale[channel] ? null : _names[channel]);
  if (!rest) return;
  // One request per set of names: the page asks on every change.
  const ticket = `${channel}|${JSON.stringify(rest)}`;
  if (_asking.has(ticket)) return;
  _asking.add(ticket);
  try {
    const answer = await commands.sessionNames(rest);
    _names = { ..._names, [channel]: mergeNames(_names[channel], answer) };
    if (_stale[channel]) _stale = { ..._stale, [channel]: false };
  } finally {
    _asking.delete(ticket);
  }
}

/** The game data changed (loaded, wiped, a new build): every answer is asked
 *  again by the page that shows it; until then the old one stays. */
function staleNames() {
  _stale = Object.fromEntries(Object.keys(_names).map((channel) => [channel, true]));
}

let _watchers = 0;
let _unlisten: Promise<UnlistenFn> | null = null;
let _unlistenData: Promise<UnlistenFn> | null = null;
const refresh = () => {
  void loadSession();
  void loadHistory();
};

/** Keep the store current while a page shows it. Returns the stop function. */
export function watchSession(): () => void {
  if (_watchers++ === 0) {
    _unlisten = listen<{ history: boolean }>("session:changed", (event) => {
      void loadSession();
      if (event.payload.history) {
        // A filed session may replace one that is open here.
        _opened = {};
        void loadHistory();
      }
    });
    _unlistenData = listen("data:changed", staleNames);
    window.addEventListener("focus", refresh);
    // Names answered while the game data was not loaded: ask again.
    if (Object.values(_names).some((book) => !book.data_loaded)) staleNames();
    refresh();
  }
  let stopped = false;
  return () => {
    if (stopped) return;
    stopped = true;
    if (--_watchers === 0) {
      void _unlisten?.then((u) => u());
      _unlisten = null;
      void _unlistenData?.then((u) => u());
      _unlistenData = null;
      window.removeEventListener("focus", refresh);
    }
  };
}
