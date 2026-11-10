// Crafting: the blueprint catalog with what you own and want, and each
// recipe's materials and stats. Rust half: svc-data's catalog (reference
// data), crates/mod-crafting (your marks), src-tauri/src/crafting.rs (IPC).

import { Hammer } from "lucide-svelte";
import type { ModuleDescriptor } from "../types";

export const craftingModule: ModuleDescriptor = {
  id: "crafting",
  name: "Crafting",
  description: "Every blueprint, which ones you own and want, and what a recipe makes at the quality of the materials you hold.",
  pitch: [
    "Every blueprint, with the ones you own and want marked",
    "Set each material's quality and see what the item comes out as",
    "What you hold towards a recipe, and which missions grant it",
  ],
  needsGame: true,
  unofficial: [
    { feature: "blueprints", required: false },
    { feature: "items", required: false },
  ],
  nav: [{ href: "/crafting", label: "Crafting", icon: Hammer }],
};
