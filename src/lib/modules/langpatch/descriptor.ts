// Text Patching (langpatch) — the sc-langpatch consolidation. Rust half:
// crates/mod-langpatch (engine) + src-tauri/src/langpatch.rs (orchestration).

import { Pilcrow } from "lucide-svelte";
import type { ModuleDescriptor } from "../types";
import InstallStatus from "./InstallStatus.svelte";

export const langpatchModule: ModuleDescriptor = {
  id: "langpatch",
  name: "Text Patching",
  description:
    "Enrich Star Citizen's in-game text: component grades, illegal-goods markers, weapon stats — kept current automatically after game patches.",
  icon: "¶",
  nav: [{ href: "/langpatch", label: "Text Patching", icon: Pilcrow }],
  installStatus: { label: "Text", component: InstallStatus },
};
