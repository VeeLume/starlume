// Mining (mining) — the calculator: a scanned rock against a loadout, with
// the verdict, our difficulty, the charge bar, and the gadget, modules or
// support that close the gap. Rust half: crates/mod-mining (catalog filters,
// solver) + src-tauri/src/mining.rs (commands, saved loadouts) over
// svc-data's cooked mining index.

import { Pickaxe } from "lucide-svelte";
import type { ModuleDescriptor } from "../types";

export const miningModule: ModuleDescriptor = {
  id: "mining",
  name: "Mining",
  description:
    "Will it crack? Type in a scanned rock and your loadout: breakability, difficulty, the charge bar, and the gadget, modules or support that close the gap.",
  icon: "⛏",
  nav: [{ href: "/mining", label: "Mining", icon: Pickaxe }],
};
