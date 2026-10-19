// Dev Tracker (devtracker) — CIG staff posts from Spectrum, chat included,
// filtered by content. Rust half: crates/mod-devtracker (filter, author
// modes, read state) + src-tauri/src/devtracker.rs (poll, gate, commands)
// over svc-spectrum.

import { Radio } from "lucide-svelte";
import type { ModuleDescriptor } from "../types";
import PulseCard from "./PulseCard.svelte";
import CycleNow from "./CycleNow.svelte";
import HotfixDetail from "./HotfixDetail.svelte";
import Setup from "./Setup.svelte";
import { devtrackerStore } from "$lib/state/devtracker.svelte";

export const devtrackerModule: ModuleDescriptor = {
  id: "devtracker",
  name: "Dev Tracker",
  description:
    "CIG staff posts from Spectrum, chat included, filtered down to game news: patches, PTU waves, maintenance, known issues.",
  pitch: [
    "Forums and chat, filtered down to patches, PTU waves, maintenance and known issues",
    "Follow the staff you care about",
    "A notification when a server restart or a PTU wave is posted",
  ],
  unofficial: [{ feature: "devtracker", required: true }],
  setup: {
    title: "Set up the Dev Tracker",
    lead: "What should reach you while Starlume is in the tray.",
    component: Setup,
  },
  nav: [{ href: "/devtracker", label: "Dev Tracker", icon: Radio, section: "news" }],
  homeCards: [
    {
      id: "pulse",
      title: "Dev Tracker",
      component: PulseCard,
      href: "/devtracker",
      badge: () => {
        const n = devtrackerStore.overview?.unread ?? 0;
        return n ? `${n} new` : null;
      },
    },
  ],
  patchNotes: [{ id: "ptu", component: CycleNow }],
  installStatus: { label: "Hotfix", detail: HotfixDetail },
};
