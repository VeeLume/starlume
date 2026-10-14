// My Items (items) — the player's items and ships: where they are, what is
// fitted to what, and the saved loadouts. Rust half: src-tauri/src/items.rs
// (orchestration) over svc-dossier's inventory tier.

import { Package } from "lucide-svelte";
import type { ModuleDescriptor } from "../types";

export const itemsModule: ModuleDescriptor = {
  id: "items",
  name: "My Items",
  description:
    "Find any item or ship you own: where it is, what it is fitted to, and your saved loadouts.",
  icon: "▣",
  nav: [{ href: "/items", label: "My Items", icon: Package }],
};
