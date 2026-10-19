// Text Patching (langpatch) — the sc-langpatch consolidation. Rust half:
// crates/mod-langpatch (engine) + src-tauri/src/langpatch.rs (orchestration).

import { Languages } from "lucide-svelte";
import type { ModuleDescriptor } from "../types";
import InstallStatus from "./InstallStatus.svelte";
import Setup from "./Setup.svelte";

export const langpatchModule: ModuleDescriptor = {
  id: "langpatch",
  name: "Text Patching",
  description:
    "Enrich Star Citizen's in-game text: component grades, illegal-goods markers, weapon stats — kept current automatically after game patches.",
  pitch: [
    "Blueprint rewards, cooldowns and crimestat risk in missions",
    "Size, tracking and stats on weapons, class and grade on components",
    "Re-patches after a game update, once the game is closed",
  ],
  needsGame: true,
  unofficial: [{ feature: "blueprints", required: false }],
  setup: {
    title: "Set up Text Patching",
    lead: "Which installs to patch, and which patchers.",
    component: Setup,
  },
  nav: [{ href: "/langpatch", label: "Text Patching", icon: Languages }],
  installStatus: { label: "Text", component: InstallStatus },
};
