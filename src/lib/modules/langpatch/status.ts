// Install patch states → one badge each (the StatusBadge map contract).
// The Text Patching page's install table (Home's pill words them on its own).

import type { StatusMap } from "@veelume/ui";

export const installStateMap: StatusMap = {
  "up-to-date": { label: () => "Patched · current", tone: "primary" },
  stale: { label: () => "Stale — re-patch pending", tone: "warning" },
  foreign: { label: () => "Paused — modified by another tool", tone: "warning" },
  unpatched: { label: () => "Not patched", tone: "neutral" },
};
