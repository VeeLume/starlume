// Contracts (contracts) — the mission catalog first (v0.7.0 scope,
// 2026-11-18), then your standing, the path to a blueprint and your own
// mission times. Tracking active contracts, routes and cargo packing (the
// sc-cargo-planner successor) come later. The catalog reads svc-data's
// cooked missions; crates/mod-contracts stays a doc-only stub until a
// feature needs Rust-side state.

import { ScrollText } from "lucide-svelte";
import type { ModuleDescriptor } from "../types";

export const contractsModule: ModuleDescriptor = {
  id: "contracts",
  name: "Contracts",
  description:
    "Every mission in the game: what it gives, where it runs, what rank it needs and what you will fight.",
  pitch: [
    "Every mission with its pay, rep and blueprint pool",
    "The rank each one needs, on the faction's ladder",
    "Which missions grant a blueprint, and the odds",
  ],
  nav: [{ href: "/contracts", label: "Contracts", icon: ScrollText }],
};
