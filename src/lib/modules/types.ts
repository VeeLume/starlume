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
import type { NavIcon } from "@veelume/ui";

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
  /** Icon component (lucide-svelte) — the kit NavRail contract. */
  icon: NavIcon;
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

/** A card on Home. Cards answer "can I play" / "what is going on" at a
 *  glance and link to their feature page for the rest. A card reads stores
 *  only: Home never fetches and never starts polling (data-lifecycle rules
 *  1–2; the tray idle footprint). Loading its store's cache on mount is
 *  fine — that is a local read. */
export interface HomeCard {
  /** Unique within the owner (module, or the shell). */
  id: string;
  /** Card heading; also the label in Home's customize mode. */
  title: string;
  component: Component;
  /** Shown until the user hides it. Default true. */
  defaultVisible?: boolean;
  /** Spans the full row of the card grid. */
  wide?: boolean;
  /** The feature page; the card header links there. */
  href?: string;
  /** Draws its own frame and heading (Game status's hero); Home adds only
   *  the customize controls. */
  frameless?: boolean;
  /** A short count beside the heading ("29 new"); read reactively, so it
   *  may read a store. `null` = none. */
  badge?: () => string | null;
}

/** Props of a module's per-install decoration on the Game status card. */
export interface InstallStatusProps {
  /** Lowercase channel key ("live"), the key activity records carry. */
  channelKey: string;
}

/** A module's contribution to each install row of Home's Game status card
 *  ("can I play?"): a compact state for that channel, e.g. Text Patching's
 *  patched / re-patch pending. Render nothing when there is nothing to say. */
export interface InstallStatus {
  /** Short column label ("Text"). */
  label: string;
  component: Component<InstallStatusProps>;
}

/** A card a module adds to the News page's top row, beside the LIVE
 *  card (the page is the app's: LIVE notes are reference data). E.g. the dev
 *  tracker's PTU cycle from Spectrum. Render it in `NowCard` so the row reads
 *  alike; the component reads its own store. */
export interface PatchNotesCard {
  /** Unique within the module. */
  id: string;
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
  /** Cards on Home, rendered while enabled; the user orders and hides them. */
  homeCards?: HomeCard[];
  /** A per-install decoration on Home's Game status card, while enabled. */
  installStatus?: InstallStatus;
  /** Cards in the News page's top row, while enabled. */
  patchNotes?: PatchNotesCard[];
}
