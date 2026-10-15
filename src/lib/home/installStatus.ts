// The enabled modules' per-install decorations on the Game status card —
// its own file so the card can import it without a cycle through cards.ts.

import type { InstallStatus } from "$lib/modules/types";
import { moduleRegistry } from "$lib/modules/registry";

export function installStatuses(enabledModules: string[]): (InstallStatus & { key: string })[] {
  return moduleRegistry
    .filter((d) => d.installStatus && enabledModules.includes(d.id))
    .map((d) => ({ ...d.installStatus!, key: d.id }));
}
