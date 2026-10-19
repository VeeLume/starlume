// What's new: which guide is pending and marking it seen. Derived from
// settings and the registry (frontend rule 6: nothing here lives only in
// JS). A guide is pending when this build offers a module the user hasn't
// been offered (`modules_offered`, the frontend twin of
// `modules::not_yet_offered`) or when a release since `guide_seen` has
// changes in `$lib/guides`. Before onboarding finished nothing is pending:
// onboarding offers every module and records the running version.

import { getVersion } from "@tauri-apps/api/app";
import { goto } from "$app/navigation";
import { ingest } from "@veelume/ui";
import { commands } from "$lib/bindings";
import { moduleRegistry } from "$lib/modules/registry";
import type { ModuleDescriptor } from "$lib/modules/types";
import { guidesSince, type ReleaseGuide } from "$lib/guides";
import { compareVersions } from "$lib/home/version";
import { settingsStore, applySettings } from "./settings.svelte";

let _version = $state<string | null>(null);

export const guide = {
  /** The running app version; null until `loadVersion` resolved. */
  get version() {
    return _version;
  },
  /** Modules this build offers that the user hasn't been offered. */
  get newModules(): ModuleDescriptor[] {
    const s = settingsStore.current;
    if (!s?.onboarding_completed) return [];
    const offered = s.modules_offered ?? [];
    return moduleRegistry.filter((m) => !offered.includes(m.id));
  },
  /** Release guides since the last one seen, up to this version. */
  get newReleases(): ReleaseGuide[] {
    const s = settingsStore.current;
    if (!s?.onboarding_completed || !_version) return [];
    return guidesSince(s.guide_seen, _version);
  },
  get pending(): boolean {
    return this.newModules.length > 0 || this.newReleases.length > 0;
  },
};

export async function loadVersion(): Promise<string> {
  _version ??= await getVersion();
  return _version;
}

/** At startup, after the onboarding check: open a pending guide, once per
 *  run (the backend keeps the mark — a rebuilt window mounts again). */
export async function maybeOpenGuide(): Promise<void> {
  await loadVersion();
  if (!guide.pending) return;
  if (await commands.guideOpenDue()) void goto("/whats-new");
}

/** Done: the shown modules count as offered and the running version's
 *  changes as seen. */
export async function markGuideSeen(shown: string[]): Promise<void> {
  const s = settingsStore.current;
  const version = await loadVersion();
  const offered = new Set([...(s?.modules_offered ?? []), ...shown]);
  const seen = s?.guide_seen;
  await applySettings({
    modules_offered: [...offered],
    guide_seen: seen && compareVersions(seen, version) > 0 ? seen : version,
  });
}

/** Later: the guide waits in the bell. A guide with a new module already
 *  has the backend's startup entry (modules::announce_new); one with only
 *  changes gets its entry here, once per run (the keyed ingest dedupes). */
export function deferGuide(): void {
  if (guide.newModules.length > 0 || !_version) return;
  ingest({
    key: `guide-${_version}`,
    level: "info",
    title: `What's new in Starlume ${_version}`,
    action: { label: "What's new", href: "/whats-new" },
    source: "starlume",
  });
}

/** Every module this build offers — what onboarding offered. */
export function allModuleIds(): string[] {
  return moduleRegistry.map((m) => m.id);
}
