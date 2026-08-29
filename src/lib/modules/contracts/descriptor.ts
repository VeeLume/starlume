// Contract Tracker (contracts) — the sc-cargo-planner successor with the
// wider scope decided 2026-08-29: all contract types, route planning at
// contract level, cargo packing as the hauling-specific view. Rust half:
// crates/mod-contracts (doc-only stub until the first feature lands).

import type { ModuleDescriptor } from "../types";

export const contractsModule: ModuleDescriptor = {
  id: "contracts",
  name: "Contract Tracker",
  description:
    "Track active contracts with per-objective progress, plan multi-contract routes — hauling gets cargo packing.",
  icon: "≣",
  nav: [{ href: "/contracts", label: "Contracts", icon: "≣" }],
};
