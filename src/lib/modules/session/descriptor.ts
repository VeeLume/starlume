// Session Log (session): one game run as a timeline and a wallet track.
// Rust half: crates/mod-session (model, reducer) + src-tauri/src/session.rs
// (feeds, storage, commands). Reads only what the Game.log and telemetry
// opt-ins already read; its setup step (Setup.svelte) offers both, and
// Settings → Game data keeps them.

import { History } from "lucide-svelte";
import type { ModuleDescriptor } from "../types";
import Setup from "./Setup.svelte";

export const sessionModule: ModuleDescriptor = {
  id: "session",
  name: "Session Log",
  description:
    "What happened each time you played: shards, places, missions, purchases, and what it did to your aUEC balance.",
  pitch: [
    "Every session as a timeline: shards, places, missions, purchases",
    "Your aUEC balance at the start and now, and what changed it",
    "Past sessions kept on this computer only",
  ],
  needsGame: true,
  setup: {
    title: "Set up the Session Log",
    lead: "What it reads. Both are off until you turn them on.",
    component: Setup,
  },
  nav: [{ href: "/session", label: "Session Log", icon: History, section: "companion" }],
};
