// Shared Star Citizen state — install scan + RSI account. The RSI account is
// the app's PRIMARY identity (who you are in the game); Discord (authStore)
// is a secondary connector for the community/guild layer. The sidebar banner
// and the Me page both read from here so a verify anywhere updates everywhere.

import { commands, type ScStatus, type RsiAccount } from "$lib/bindings";

let _status = $state<ScStatus | null>(null);
let _loaded = $state(false);

export const scStore = {
  get status() {
    return _status;
  },
  /** The primary RSI identity, if recognized. */
  get account(): RsiAccount | null {
    return _status?.account ?? null;
  },
  get loaded() {
    return _loaded;
  },
};

/** Local scan (installs + launcher-handle recognition). No network. */
export async function loadSc(): Promise<ScStatus | null> {
  const result = await commands.scStatus();
  if (result.status === "ok") _status = result.data;
  _loaded = true;
  return _status;
}

/**
 * Verify the account against the RSI public profile (online-gated). Captures
 * the immutable anchors and updates the shared account in place.
 */
export async function verifyAccount(handle: string): Promise<string | null> {
  const result = await commands.verifyRsiAccount(handle);
  if (result.status === "ok") {
    if (_status) _status = { ..._status, account: result.data };
    return null;
  }
  return result.error.message;
}
