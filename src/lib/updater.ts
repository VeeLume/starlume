// Auto-update: check GitHub Releases, prompt, install, relaunch.
// Called on startup from the root layout; `interactive` is for a manual
// "Check for updates" button in Settings.
import { check } from "@tauri-apps/plugin-updater";
import { ask, message } from "@tauri-apps/plugin-dialog";
import { relaunch } from "@tauri-apps/plugin-process";
import { settingsStore } from "$lib/state/settings.svelte";

export async function checkForUpdates(interactive = false): Promise<void> {
  // Honor the online master switch — the update check is a network call too.
  if (settingsStore.current && !settingsStore.current.online_enabled) {
    if (interactive) {
      await message("Online features are disabled (Settings → Online).", {
        title: "Updater",
      });
    }
    return;
  }
  try {
    const update = await check();
    if (update) {
      const install = await ask(
        `Starlume ${update.version} is available (you have ${update.currentVersion}). Install now?`,
        { title: "Update available", kind: "info" },
      );
      if (install) {
        await update.downloadAndInstall();
        await relaunch();
      }
    } else if (interactive) {
      await message("You're on the latest version.", { title: "No update" });
    }
  } catch (e) {
    // Offline or GitHub hiccup — never block the app on the updater.
    console.error("update check failed:", e);
    if (interactive) {
      await message(`Update check failed: ${e}`, { title: "Updater", kind: "warning" });
    }
  }
}
