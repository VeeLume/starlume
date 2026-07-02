// The frontend module registry. Order here is presentation order everywhere
// (onboarding picker, settings list, sidebar groups).
//
// Adding a module: create `src/lib/modules/<id>/descriptor.ts` exporting a
// `ModuleDescriptor`, import it here, done — the onboarding picker, the
// sidebar, and the settings module list all pick it up. See types.ts for the
// Rust half of the recipe.

import type { ModuleDescriptor } from "./types";

export const moduleRegistry: ModuleDescriptor[] = [
  // mod-cargo lands first (see the migration order in the design doc).
];

export function moduleById(id: string): ModuleDescriptor | undefined {
  return moduleRegistry.find((m) => m.id === id);
}
