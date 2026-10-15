// Dev tracker state — the cached staff feed with the module's decisions
// (shown / filtered out, ops, unread) and the follow list. Everything here is
// a local read; the poll runs Rust-side (devtracker.rs) and announces itself
// with `devtracker:changed` (frontend rule 5). Surfaces subscribe with
// `watchDevtracker()` so the page and the Home card share one listener.

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { commands, type DevAuthorView, type DevtrackerOverview } from "$lib/bindings";

let _overview = $state<DevtrackerOverview | null>(null);
let _authors = $state<DevAuthorView[] | null>(null);
let _refreshing = $state(false);
let _error = $state("");

export const devtrackerStore = {
  get overview() {
    return _overview;
  },
  /** The follow list; null until a surface that shows it loaded it. */
  get authors() {
    return _authors;
  },
  /** A manual refresh is in flight. */
  get refreshing() {
    return _refreshing;
  },
  /** The last manual action's error (the poll's own lives in the overview). */
  get error() {
    return _error;
  },
};

export async function loadDevtracker(): Promise<void> {
  _overview = await commands.devtrackerOverview();
}

export async function loadAuthors(): Promise<void> {
  _authors = await commands.devtrackerAuthors();
}

// Ref-counted subscription: several surfaces share one listener.
let _watchers = 0;
let _unlisten: Promise<UnlistenFn> | null = null;

/** Keep the store current while a surface shows it: load the cache now and
 *  on every `devtracker:changed`. Returns the stop function. */
export function watchDevtracker(): () => void {
  if (_watchers++ === 0) {
    _unlisten = listen("devtracker:changed", () => {
      void loadDevtracker();
      if (_authors) void loadAuthors();
    });
    void loadDevtracker();
  }
  let stopped = false;
  return () => {
    if (stopped) return;
    stopped = true;
    if (--_watchers === 0) {
      void _unlisten?.then((u) => u());
      _unlisten = null;
    }
  };
}

function settle(result: { status: "ok" } | { status: "error"; error: { message: string } }) {
  _error = result.status === "error" ? result.error.message : "";
}

/** Fetch now (gated Rust-side). The change event reloads the store. */
export async function refreshDevtracker(): Promise<void> {
  if (_refreshing) return;
  _refreshing = true;
  settle(await commands.devtrackerRefresh());
  _refreshing = false;
}

export async function markRead(keys: string[]): Promise<void> {
  if (keys.length) settle(await commands.devtrackerMarkRead(keys));
}

export async function markAllRead(): Promise<void> {
  settle(await commands.devtrackerMarkAllRead());
}

export async function openPost(key: string): Promise<void> {
  settle(await commands.devtrackerOpenPost(key));
}

/** Open the cached Hotfix Central thread on Spectrum. */
export async function openHotfixCentral(): Promise<void> {
  settle(await commands.devtrackerOpenHotfix());
}

export async function setAuthorMode(id: string, mode: string): Promise<void> {
  // Optimistic: the segmented control reflects the click at once.
  if (_authors) _authors = _authors.map((a) => (a.id === id ? { ...a, mode } : a));
  settle(await commands.devtrackerSetAuthorMode(id, mode));
}

export async function follow(id: string, nickname: string): Promise<void> {
  settle(await commands.devtrackerFollow(id, nickname));
}

export async function unfollow(id: string): Promise<void> {
  settle(await commands.devtrackerUnfollow(id));
}

/** Follow by Spectrum handle (a gated lookup). Returns the error, if any,
 *  for the input it came from. */
export async function followHandle(handle: string): Promise<string | null> {
  const result = await commands.devtrackerFollowHandle(handle);
  return result.status === "error" ? result.error.message : null;
}

export async function resetAuthors(): Promise<void> {
  settle(await commands.devtrackerResetAuthors());
}

export async function setNotifyOps(on: boolean): Promise<void> {
  if (_overview) _overview = { ..._overview, notify_ops: on };
  settle(await commands.devtrackerSetNotifyOps(on));
}
