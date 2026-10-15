// Dev Tracker (devtracker) — CIG staff posts from Spectrum, chat included,
// filtered by content. Rust half: crates/mod-devtracker (filter, author
// modes, read state) + src-tauri/src/devtracker.rs (poll, gate, commands)
// over svc-spectrum.

import { Radio } from "lucide-svelte";
import type { ModuleDescriptor } from "../types";
import PulseCard from "./PulseCard.svelte";

export const devtrackerModule: ModuleDescriptor = {
  id: "devtracker",
  name: "Dev Tracker",
  description:
    "CIG staff posts from Spectrum, chat included, filtered down to game news: patches, PTU waves, maintenance, known issues.",
  icon: "📡",
  nav: [{ href: "/devtracker", label: "Dev Tracker", icon: Radio }],
  homeCards: [{ id: "pulse", title: "Dev Tracker", component: PulseCard, href: "/devtracker" }],
};
