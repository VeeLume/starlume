// Comm-links — app framework beside the patch notes (decision 2026-10-15):
// the cached newest entries of RSI's comm-link listing, for the News page.
// A local read; the refresh runs Rust-side (commlinks.rs) and announces
// itself with `commlinks:changed` (frontend rule 5).

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { commands, type CommLinksOverview } from "$lib/bindings";

let _overview = $state<CommLinksOverview | null>(null);
let _error = $state("");

export const commLinksStore = {
  get overview() {
    return _overview;
  },
  get error() {
    return _error;
  },
};

export async function loadCommLinks(): Promise<void> {
  _overview = await commands.commLinksOverview();
}

let _watchers = 0;
let _unlisten: Promise<UnlistenFn> | null = null;

/** Keep the store current while a surface shows it. Returns the stop function. */
export function watchCommLinks(): () => void {
  if (_watchers++ === 0) {
    _unlisten = listen("commlinks:changed", () => void loadCommLinks());
    void loadCommLinks();
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

/** Read now (online-gated Rust-side). */
export async function refreshCommLinks(): Promise<void> {
  const result = await commands.commLinksRefresh();
  _error = result.status === "error" ? result.error.message : "";
}

export async function openCommLink(url: string): Promise<void> {
  const result = await commands.commLinksOpen(url);
  _error = result.status === "error" ? result.error.message : "";
}
