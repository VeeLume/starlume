// My Items (items) — the player's items and ships: where they are, what is
// fitted to what, and the saved loadouts. Rust half: src-tauri/src/items.rs
// (orchestration) over svc-dossier's inventory tier.

import { Package } from "lucide-svelte";
import type { ModuleDescriptor } from "../types";
import SummaryCard from "./SummaryCard.svelte";

export const itemsModule: ModuleDescriptor = {
  id: "items",
  name: "My Items",
  description:
    "Find any item or ship you own: where it is, what it is fitted to, and your saved loadouts.",
  pitch: [
    "Where everything is, by system, planet and station",
    "Fitted, saved and stock loadouts side by side",
    "Crafted stats and resources by quality",
  ],
  needsGame: true,
  unofficial: [
    { feature: "items", required: true },
    { feature: "loadouts", required: false },
  ],
  nav: [{ href: "/items", label: "My Items", icon: Package }],
  homeCards: [{ id: "summary", title: "My Items", component: SummaryCard, href: "/items" }],
};
