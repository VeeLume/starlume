// Discord — Rich Presence. Rust half: crates/svc-presence (what a moment
// shows, the Discord pipe) + src-tauri/src/presence.rs (the worker, fed by
// the Session Log's live session).

import { MessageCircle } from "lucide-svelte";
import type { ModuleDescriptor } from "../types";

export const discordModule: ModuleDescriptor = {
  id: "discord",
  name: "Discord",
  description: "Show what you are doing in the game on your Discord profile: your ship, your contract, where you are.",
  pitch: [
    "Your ship's picture and your contract on your profile",
    "You choose how much of where you are it says",
    "Needs the Session Log; nothing goes out until you switch it on",
  ],
  needsGame: true,
  nav: [{ href: "/discord", label: "Discord", icon: MessageCircle, section: "companion" }],
};
