// Session Log (session): one game run as a timeline and a wallet track.
// Rust half: crates/mod-session (model, reducer) + src-tauri/src/session.rs
// (feeds, storage, commands). Reads only what the Game.log and telemetry
// opt-ins already read; it switches neither on (Settings → Game data).
// Unreleased: see registry.ts.

import { History } from "lucide-svelte";
import type { ModuleDescriptor } from "../types";

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
  nav: [{ href: "/session", label: "Session Log", icon: History }],
};
