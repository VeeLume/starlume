// LIVE patch notes — app framework (decision 2026-10-15): the cached notes
// from RSI's patch-note pages, for Home's up-to-date check and the Patch
// Notes page. A local read; the refresh runs Rust-side (patchnotes.rs) and
// announces itself with `patchnotes:changed` (frontend rule 5). Surfaces
// subscribe with `watchPatchNotes()` and share one listener.

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { commands, type PatchNotesOverview, type ReleaseView } from "$lib/bindings";

let _overview = $state<PatchNotesOverview | null>(null);
let _refreshing = $state(false);
let _error = $state("");

export const patchNotesStore = {
  get overview() {
    return _overview;
  },
  /** The newest LIVE release the notes describe. */
  get latestLive(): ReleaseView | null {
    return _overview?.releases.find((r) => r.channel === "LIVE") ?? null;
  },
  get refreshing() {
    return _refreshing;
  },
  get error() {
    return _error;
  },
};

export async function loadPatchNotes(): Promise<void> {
  _overview = await commands.patchNotesOverview();
}

let _watchers = 0;
let _unlisten: Promise<UnlistenFn> | null = null;

/** Keep the store current while a surface shows it. Returns the stop function. */
export function watchPatchNotes(): () => void {
  if (_watchers++ === 0) {
    _unlisten = listen("patchnotes:changed", () => void loadPatchNotes());
    void loadPatchNotes();
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

/** Read now (online-gated Rust-side). The change event reloads the store. */
export async function refreshPatchNotes(): Promise<void> {
  if (_refreshing) return;
  _refreshing = true;
  const result = await commands.patchNotesRefresh();
  _error = result.status === "error" ? result.error.message : "";
  _refreshing = false;
}

export async function openPatchNote(url: string): Promise<void> {
  const result = await commands.patchNotesOpen(url);
  _error = result.status === "error" ? result.error.message : "";
}
