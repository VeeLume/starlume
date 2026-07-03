// Frontend module descriptors — the UI counterpart of the Rust `Module` trait.
//
// A feature module lands in two halves:
//   1. Rust: a `mod-*` crate + an entry in `src-tauri/src/modules.rs::registry()`
//      (commands, services, background tasks).
//   2. Frontend: a folder under `src/lib/modules/<id>/` exporting a
//      `ModuleDescriptor`, added to `registry.ts` (routes, nav entries,
//      onboarding steps).
// The `id` must match between the two halves.

import type { Component } from "svelte";

/** Props every module-contributed onboarding step receives. */
export interface OnboardingStepProps {
  /**
   * Gate the wizard's Next button. Steps start with Next enabled; call
   * `setCanContinue(false)` while required input is missing.
   */
  setCanContinue: (ok: boolean) => void;
}

/** One wizard step contributed by a module (shown only when it's selected). */
export interface OnboardingStep {
  /** Unique within the module. */
  id: string;
  /** Step heading shown in the wizard. */
  title: string;
  component: Component<OnboardingStepProps>;
}

/** A sidebar navigation entry contributed by a module (shown when enabled). */
export interface NavEntry {
  href: string;
  label: string;
  /** Small glyph/emoji rendered before the label. */
  icon: string;
}

/** A section a module contributes to the Me page — the unified place for
 *  per-module visibility/sharing settings ("what of mine does this module
 *  share, with whom"). Keep sections about the *user's* data exposure;
 *  module feature settings stay on the module's own pages. */
export interface MeSection {
  /** Unique within the module. */
  id: string;
  /** Section heading on the Me page. */
  title: string;
  component: Component;
}

export interface ModuleDescriptor {
  /** Must match the Rust `Module::id`. */
  id: string;
  name: string;
  description: string;
  /** Glyph/emoji for the onboarding picker and module lists. */
  icon: string;
  /** Sidebar entries, rendered while the module is enabled. */
  nav?: NavEntry[];
  /** Extra wizard steps, rendered while the module is selected in onboarding. */
  onboardingSteps?: OnboardingStep[];
  /** Visibility/sharing sections on the Me page, rendered while enabled. */
  meSections?: MeSection[];
}
