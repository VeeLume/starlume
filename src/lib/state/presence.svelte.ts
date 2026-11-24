// Discord Rich Presence: what Discord shows now and why (presence.rs).
// The Rust worker owns it; this mirrors it while Settings › Discord is open
// and re-reads on `presence:changed` (data-lifecycle rule 5).

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { commands, type PresenceView } from "$lib/bindings";

let _view = $state<PresenceView | null>(null);

export const presenceStore = {
  get view() {
    return _view;
  },
};

export async function loadPresence(): Promise<void> {
  _view = await commands.presenceState();
}

let _watchers = 0;
let _unlisten: Promise<UnlistenFn> | null = null;

/** Keep the store current while a surface shows it. Returns the stop function. */
export function watchPresence(): () => void {
  if (_watchers++ === 0) {
    _unlisten = listen("presence:changed", () => void loadPresence());
  }
  void loadPresence();
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
