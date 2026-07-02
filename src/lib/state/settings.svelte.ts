// Shared settings state — one snapshot for the whole frontend so the sidebar
// (enabled modules), the settings page, and onboarding never drift apart.

import { commands, type AppSettings, type AppError } from "$lib/bindings";

let _current = $state<AppSettings | null>(null);

export const settingsStore = {
  get current() {
    return _current;
  },
};

/** Load the snapshot once (idempotent — later calls refresh it). */
export async function loadSettings(): Promise<AppSettings> {
  _current = await commands.getSettings();
  return _current;
}

/**
 * Apply a partial change on top of the current snapshot. Returns the error
 * (if any) so callers can surface it; the store keeps the confirmed state.
 */
export async function applySettings(
  patch: Partial<AppSettings>,
): Promise<AppError | null> {
  const base = _current ?? (await loadSettings());
  const result = await commands.updateSettings({ ...base, ...patch });
  if (result.status === "ok") {
    _current = result.data;
    return null;
  }
  return result.error;
}
