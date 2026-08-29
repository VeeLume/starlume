<script lang="ts">
  import { Settings, Switch } from "@veelume/ui";
  import { moduleRegistry } from "$lib/modules/registry";
  import { settingsStore, applySettings, loadSettings } from "$lib/state/settings.svelte";
  import { onMount } from "svelte";

  const settings = $derived(settingsStore.current);
  let error = $state("");

  onMount(() => void loadSettings());

  async function toggleModule(id: string, on: boolean) {
    error = "";
    const current = settings?.enabled_modules ?? [];
    const next = on ? [...current, id] : current.filter((m) => m !== id);
    const err = await applySettings({ enabled_modules: next });
    if (err) error = err.message;
  }
</script>

<Settings.Page title="Modules">
  {#if settings}
    <Settings.Section>
      {#if moduleRegistry.length === 0}
        <p class="text-sm text-muted-foreground">
          No feature modules are available in this build yet.
        </p>
      {:else}
        {#each moduleRegistry as m (m.id)}
          <Settings.Row label={m.name} hint={m.description}>
            <Switch
              label={m.name}
              checked={settings.enabled_modules.includes(m.id)}
              onchange={(v) => toggleModule(m.id, v)}
            />
          </Settings.Row>
        {/each}
      {/if}
    </Settings.Section>

    {#if error}
      <p class="text-sm text-destructive">{error}</p>
    {/if}
  {/if}
</Settings.Page>
