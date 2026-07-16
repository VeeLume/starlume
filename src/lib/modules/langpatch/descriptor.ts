// Text Patching (langpatch) — the sc-langpatch consolidation. Rust half:
// crates/mod-langpatch (engine) + src-tauri/src/langpatch.rs (orchestration).

import type { ModuleDescriptor } from "../types";

export const langpatchModule: ModuleDescriptor = {
  id: "langpatch",
  name: "Text Patching",
  description:
    "Enrich Star Citizen's in-game text: component grades, illegal-goods markers, weapon stats — kept current automatically after game patches.",
  icon: "¶",
  nav: [{ href: "/langpatch", label: "Text Patching", icon: "¶" }],
};
