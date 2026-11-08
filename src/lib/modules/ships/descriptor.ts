// Ships (ships) — the loadout manager: every port of a ship and what it
// holds, the power network as the ship's power screen shows it, and what
// else fits a port. Rust half: crates/mod-ships (slots, power, figures) +
// src-tauri/src/ships.rs (commands) over svc-data's cooked ship index.

import { Rocket } from "lucide-svelte";
import type { ModuleDescriptor } from "../types";

export const shipsModule: ModuleDescriptor = {
  id: "ships",
  name: "Ships",
  description:
    "Fit your ships: every port and what fits it, the power network as the ship's power screen shows it, and how a swap changes the ship.",
  pitch: [
    "Every port, what it holds and what else fits",
    "Power, cooling and signatures as the power screen shows them",
    "How a swap changes the ship before you buy it",
    "Shop and player-market prices from UEX, when you're online",
  ],
  needsGame: true,
  nav: [{ href: "/ships", label: "Ships", icon: Rocket }],
};
