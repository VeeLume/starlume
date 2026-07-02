// Shared auth state + the auth-changed listener. The sidebar account block
// and the settings page both read from here.

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { commands, type AuthStatus, type Profile } from "$lib/bindings";

let _current = $state<AuthStatus | null>(null);
let _profile = $state<Profile | null>(null);

export const authStore = {
  get current() {
    return _current;
  },
  get profile() {
    return _profile;
  },
};

export async function loadAuth(): Promise<AuthStatus> {
  _current = await commands.authStatus();
  if (_current.logged_in) {
    // Best-effort: the sidebar falls back to the generic badge when the
    // server is unreachable. A 401 clears the token backend-side and emits
    // auth-changed, which re-runs this and lands in the else branch.
    const result = await commands.fetchProfile();
    _profile = result.status === "ok" ? result.data : null;
  } else {
    _profile = null;
  }
  return _current;
}

/** Start listening for backend auth changes. Returns the unlisten handle. */
export async function listenForAuthChanges(): Promise<UnlistenFn> {
  return listen("auth-changed", () => void loadAuth());
}
