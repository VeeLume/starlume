<script lang="ts">
  import { Button, Settings, Switch } from "@veelume/ui";
  import { settingsStore, applySettings, loadSettings } from "$lib/state/settings.svelte";
  import { openOnboarding } from "$lib/state/onboarding.svelte";
  import { checkForUpdates } from "$lib/updater";
  import { onMount } from "svelte";

  const settings = $derived(settingsStore.current);
  let error = $state("");

  onMount(() => void loadSettings());

  async function apply(patch: Parameters<typeof applySettings>[0]) {
    error = "";
    const err = await applySettings(patch);
    if (err) error = err.message;
  }
</script>

<Settings.Page title="General">
  {#if settings}
    <Settings.Section title="Tray & startup">
      <Settings.Row label="Close to tray" hint="Closing the window keeps Starlume running in the background.">
        <Switch
          label="Close to tray"
          checked={settings.close_to_tray}
          onchange={(v) => apply({ close_to_tray: v })}
        />
      </Settings.Row>
      <Settings.Row label="Minimize to tray" hint="Minimize hides to the tray instead of the taskbar.">
        <Switch
          label="Minimize to tray"
          checked={settings.minimize_to_tray}
          onchange={(v) => apply({ minimize_to_tray: v })}
        />
      </Settings.Row>
      <Settings.Row label="Start minimized" hint="Launch straight to the tray, no window.">
        <Switch
          label="Start minimized"
          checked={settings.start_minimized}
          onchange={(v) => apply({ start_minimized: v })}
        />
      </Settings.Row>
      <Settings.Row label="Run at login" hint="Register Starlume to start with Windows.">
        <Switch
          label="Run at login"
          checked={settings.autostart}
          onchange={(v) => apply({ autostart: v })}
        />
      </Settings.Row>
    </Settings.Section>

    <Settings.Section title="System">
      <Settings.Row
        label="Windows notifications"
        hint="Native toasts while the window is hidden to the tray."
      >
        <Switch
          label="Windows notifications"
          checked={settings.native_notifications}
          onchange={(v) => apply({ native_notifications: v })}
        />
      </Settings.Row>
      <Settings.Row
        label="Load game data at startup"
        hint="Local only; the heavy parse runs only after a game patch."
      >
        <Switch
          label="Load game data at startup"
          checked={settings.auto_load_game_data}
          onchange={(v) => apply({ auto_load_game_data: v })}
        />
      </Settings.Row>
    </Settings.Section>

    <Settings.Section title="Maintenance">
      <Settings.Row label="Updates" hint="Update checks run at startup; check now on demand.">
        <Button variant="outline" onclick={() => checkForUpdates(true)}>Check for updates</Button>
      </Settings.Row>
      <Settings.Row label="Setup" hint="Re-run the first-launch onboarding flow.">
        <Button variant="outline" onclick={openOnboarding}>Re-run setup</Button>
      </Settings.Row>
    </Settings.Section>

    {#if error}
      <p class="text-sm text-destructive">{error}</p>
    {/if}
  {/if}
</Settings.Page>
