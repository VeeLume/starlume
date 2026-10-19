<script lang="ts">
  // Onboarding: the tray-app settings worth deciding up front (they were a
  // tip on the last page). The same settings as Settings → General and
  // Game data; autostart registers with Windows in `update_settings`.
  import { Settings, Switch } from "@veelume/ui";
  import { settingsStore, applySettings } from "$lib/state/settings.svelte";

  const settings = $derived(settingsStore.current);
  let error = $state("");

  async function apply(patch: Parameters<typeof applySettings>[0]) {
    error = "";
    const err = await applySettings(patch);
    if (err) error = err.message;
  }
</script>

{#if settings}
  <Settings.Section>
    <Settings.Row label="Start with Windows" hint="Starts hidden in the tray.">
      <Switch label="Start with Windows" checked={settings.autostart} onchange={(v) => apply({ autostart: v })} />
    </Settings.Row>
    <Settings.Row label="Closing the window keeps Starlume in the tray">
      <Switch
        label="Close to tray"
        checked={settings.close_to_tray}
        onchange={(v) => apply({ close_to_tray: v })}
      />
    </Settings.Row>
    <Settings.Row
      label="Notifications while hidden"
      hint="Windows notifications for game-ops posts, finished patches and errors."
    >
      <Switch
        label="Notifications while hidden"
        checked={settings.native_notifications}
        onchange={(v) => apply({ native_notifications: v })}
      />
    </Settings.Row>
    <Settings.Row
      label="Prepare game data at startup"
      hint="Reads a new game build once after each patch, so pages open at once."
    >
      <Switch
        label="Prepare game data at startup"
        checked={settings.auto_load_game_data}
        onchange={(v) => apply({ auto_load_game_data: v })}
      />
    </Settings.Row>
  </Settings.Section>
  {#if error}<p class="error">{error}</p>{/if}
{/if}
