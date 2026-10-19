// The unofficial-API opt-in: any RSI/CIG API Starlume is not an official
// client of — CIG's game-services backend (gRPC) and RSI's Spectrum. One
// master switch (persisted as `grpc_enabled`, the name predates Spectrum) plus
// a per-feature allow-list (`grpc_features`); the backend gate is
// `AppState::require_unofficial`. Shared by Settings → Online, onboarding and
// the What's-new guide.

import type { AppError } from "$lib/bindings";
import { settingsStore, applySettings } from "$lib/state/settings.svelte";

/** The one-time consent shown before the master switch first turns on. */
export const UNOFFICIAL_CONSENT = {
  title: "Unofficial APIs consent",
  message:
    "These features read your data from RSI and CIG services Starlume is not an " +
    "official client of — the game-services backend and Spectrum — using your " +
    "launcher session. That sits in a ToS-grey area. Calls are read-only, paced " +
    "to stay light, and made only for the features you enable. " +
    "Your account, your risk.\n\nAllow unofficial API calls?",
} as const;

/** The same consent, for a page that shows it in place instead of a
 *  dialog (onboarding, a What's-new page): the text sits beside the
 *  feature switches, and turning one on is the consent. `names` are the
 *  modules that read through these APIs. */
export function unofficialTerms(names: string[]): string {
  const who = names.length > 1 ? `${names.slice(0, -1).join(", ")} and ${names.at(-1)} read` : `${names[0]} reads`;
  return (
    `${who} your data from RSI and CIG services Starlume is not an ` +
    "official client of, with your launcher session. That is a grey area of " +
    "their terms. Calls are read-only, paced to stay light, and made only " +
    "for what you allow. Your account, your risk."
  );
}

/** Allow or withhold one unofficial-API feature. Allowing turns the master
 *  switch on too (the backend records the consent on its first enable);
 *  withholding leaves the master as it is, for the other features. */
export function allowFeature(id: string, on: boolean): Promise<AppError | null> {
  const current = settingsStore.current?.grpc_features ?? [];
  const features = on ? [...current.filter((f) => f !== id), id] : current.filter((f) => f !== id);
  return applySettings(on ? { grpc_features: features, grpc_enabled: true } : { grpc_features: features });
}
