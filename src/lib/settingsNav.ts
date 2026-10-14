// Settings categories as data (the kit scaffold's contract): adding a
// setting is one entry here plus one small page under /settings/<id>.

import { Blocks, Database, Globe, SlidersHorizontal, UserRound } from "lucide-svelte";
import type { SettingsCategory } from "@veelume/ui";
import { SHOW_UNRELEASED } from "$lib/unreleased";

export const settingsCategories: SettingsCategory[] = [
  {
    id: "general",
    label: "General",
    description: "Tray behavior, startup, updates",
    icon: SlidersHorizontal,
    path: "/settings/general",
  },
  {
    id: "game-data",
    label: "Game data",
    description: "Installs, cached builds, the patch archive",
    icon: Database,
    path: "/settings/game-data",
  },
  {
    id: "modules",
    label: "Modules",
    description: "Enable or disable feature modules",
    icon: Blocks,
    path: "/settings/modules",
  },
  {
    id: "online",
    label: "Online & privacy",
    description: "Network master switch, game-services consent",
    icon: Globe,
    path: "/settings/online",
  },
  // Server + Discord serve only the unreleased friends/community layer.
  ...(SHOW_UNRELEASED
    ? [
        {
          id: "account",
          label: "Account",
          description: "Server URL, Discord sign-in",
          icon: UserRound,
          path: "/settings/account",
        },
      ]
    : []),
];
