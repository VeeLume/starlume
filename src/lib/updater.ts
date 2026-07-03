// Auto-update: check GitHub Releases, prompt, install, relaunch.
// Called on startup from the root layout; `interactive` is for a manual
// "Check for updates" button in Settings.
import { check } from "@tauri-apps/plugin-updater";
import { ask, message } from "@tauri-apps/plugin-dialog";
import { relaunch } from "@tauri-apps/plugin-process";

// NOTE: deliberately NOT gated by the online master switch — update checks are
// the one documented exception to the online-policy invariant (see CLAUDE.md):
// a legitimate app function with no ToS implications, and keeping even
// offline-mode users on current (security-)fixed builds matters more.
export async function checkForUpdates(interactive = false): Promise<void> {
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
