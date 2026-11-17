// Plans (plans): loadouts and crafting projects as goals, the steps your
// inventory leaves open, and one materials list across all of them. Rust
// half: crates/mod-plans (model, ledger, steps, session) +
// src-tauri/src/plans.rs (gathering, IPC). Fetches nothing itself: it reads
// what Ships, Crafting, My Items and Mining already hold, so the unofficial
// features below are theirs (all optional: without a read of your inventory
// every step is still shown, as if you held nothing).

import { Target } from "lucide-svelte";
import type { ModuleDescriptor } from "../types";

export const plansModule: ModuleDescriptor = {
  id: "plans",
  name: "Plans",
  description:
    "Loadouts and crafting projects as goals, the next steps your inventory leaves open, and one materials list across all of them.",
  pitch: [
    "Set a goal: a saved loadout on its ship, or an item crafted at a quality",
    "The next steps come from what you hold: fit, buy, mine, earn, craft",
    "One materials list across every plan, and a next session to fly",
  ],
  needsGame: true,
  unofficial: [
    { feature: "items", required: false },
    { feature: "blueprints", required: false },
  ],
  nav: [{ href: "/plans", label: "Plans", icon: Target, section: "news" }],
};
