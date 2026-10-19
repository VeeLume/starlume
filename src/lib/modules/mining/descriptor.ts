// Mining (mining) — the calculator (a scanned rock against your ship: our
// difficulty, how to fly it, what would help), the rock finder (where rocks
// spawn, from the game's spawn rules) and your and your crew's loadouts.
// Rust half: crates/mod-mining (catalog filters, solver, generator) +
// src-tauri/src/mining.rs (commands, saved loadouts) over svc-data's cooked
// mining index.

import { Pickaxe } from "lucide-svelte";
import type { ModuleDescriptor } from "../types";

export const miningModule: ModuleDescriptor = {
  id: "mining",
  name: "Mining",
  description:
    "Will it crack? A scanned or rolled rock against your ship: difficulty, how to fly it, and the gadget, modules or crew that close the gap. Plus where each rock spawns.",
  pitch: [
    "Breakability, difficulty and how to fly it",
    "The gadget, modules or crew that close the gap",
    "Where each rock spawns, from the game's spawn rules",
  ],
  needsGame: true,
  nav: [{ href: "/mining", label: "Mining", icon: Pickaxe }],
};
