// The RSI launcher, for Home's Game status card (launcher.rs): whether it is
// installed, whether the game runs, and the account's game library (which
// channels it may play, at which build) for the "update needed" hint.
// Status is local; the library is an unofficial API, gated Rust-side —
// when the gate is closed or the session stale, the hint is just absent.

import { commands, type LauncherStatusView, type LibraryChannelView } from "$lib/bindings";

let _status = $state<LauncherStatusView | null>(null);
let _library = $state<LibraryChannelView[] | null>(null);
let _error = $state("");

export const launcherStore = {
  get status() {
    return _status;
  },
  /** `null` = not read (gate closed, session stale, or not yet). */
  get library() {
    return _library;
  },
  get error() {
    return _error;
  },
};

/** Re-read the status, and the library (cached an hour Rust-side). */
export async function loadLauncher(): Promise<void> {
  _status = await commands.launcherStatus();
  const lib = await commands.launcherLibrary();
  _library = lib.status === "ok" ? lib.data : null;
}

/** Start the launcher, or bring it forward. */
export async function openLauncher(): Promise<void> {
  const result = await commands.launcherOpen();
  _error = result.status === "error" ? result.error.message : "";
}

/** The library's build for a channel, `undefined` when the library wasn't
 *  read, `null` when this account can't play that channel right now. */
export function libraryLabel(library: LibraryChannelView[] | null, channel: string): string | null | undefined {
  if (!library) return undefined;
  return library.find((c) => c.id.toLowerCase() === channel.toLowerCase())?.version_label ?? null;
}
