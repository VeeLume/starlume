// Settings categories as data (the kit scaffold's contract): adding a
// setting is one entry here plus one small page under /settings/<id>.

import { Blocks, Globe, SlidersHorizontal, UserRound } from "lucide-svelte";
import type { SettingsCategory } from "@veelume/ui";

export const settingsCategories: SettingsCategory[] = [
  {
    id: "general",
    label: "General",
    description: "Tray behavior, startup, game data, updates",
    icon: SlidersHorizontal,
    path: "/settings/general",
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
  {
    id: "account",
    label: "Account",
    description: "Server URL, Discord sign-in",
    icon: UserRound,
    path: "/settings/account",
  },
];
