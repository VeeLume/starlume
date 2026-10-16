// The frontend module registry. Order here is presentation order everywhere
// (onboarding picker, settings list, sidebar groups).
//
// Adding a module: create `src/lib/modules/<id>/descriptor.ts` exporting a
// `ModuleDescriptor`, import it here, done — the onboarding picker, the
// sidebar, and the settings module list all pick it up. See types.ts for the
// Rust half of the recipe.

import type { ModuleDescriptor } from "./types";
import { langpatchModule } from "./langpatch/descriptor";
import { contractsModule } from "./contracts/descriptor";
import { itemsModule } from "./items/descriptor";
import { devtrackerModule } from "./devtracker/descriptor";
import { miningModule } from "./mining/descriptor";
import { SHOW_UNRELEASED } from "$lib/unreleased";

export const moduleRegistry: ModuleDescriptor[] = [
  // Unreleased ones are dev-build only — see $lib/unreleased (mirrors
  // modules::registry() in Rust, same order).
  devtrackerModule,
  langpatchModule,
  itemsModule,
  ...(SHOW_UNRELEASED ? [miningModule, contractsModule] : []),
];

export function moduleById(id: string): ModuleDescriptor | undefined {
  return moduleRegistry.find((m) => m.id === id);
}
