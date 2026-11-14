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
  type SessionStateView,
  type SessionSummary,
} from "$lib/bindings";

/** Past sessions kept in memory at once: the ones just looked at. */
const OPENED_KEPT = 4;

let _state = $state<SessionStateView | null>(null);
let _history = $state<SessionSummary[] | null>(null);
let _opened = $state<Record<string, Session | null>>({});
let _error = $state("");

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

let _watchers = 0;
let _unlisten: Promise<UnlistenFn> | null = null;
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
    window.addEventListener("focus", refresh);
    refresh();
  }
  let stopped = false;
  return () => {
    if (stopped) return;
    stopped = true;
    if (--_watchers === 0) {
      void _unlisten?.then((u) => u());
      _unlisten = null;
      window.removeEventListener("focus", refresh);
    }
  };
}
