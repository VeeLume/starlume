// First-launch onboarding visibility (the Hearth pattern).
//
// The overlay is rendered by the root layout when `onboarding.open` is true.
// On app start we open it once if `onboarding_completed` is false; Settings
// can re-open it. Finishing or skipping marks the setting.

import { loadSettings } from "$lib/state/settings.svelte";

let _open = $state(false);
let _checked = false;

export const onboarding = {
  get open() {
    return _open;
  },
};

/** On app start, open onboarding if it hasn't been completed. Runs once. */
export async function maybeStartOnboarding() {
  if (_checked) return;
  _checked = true;
  const settings = await loadSettings();
  if (!settings.onboarding_completed) _open = true;
}

/** Re-open onboarding (e.g. from Settings). */
export function openOnboarding() {
  _open = true;
}

/** Close the overlay — the wizard marks `onboarding_completed` itself. */
export function closeOnboarding() {
  _open = false;
}
