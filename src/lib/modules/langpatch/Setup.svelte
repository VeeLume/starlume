<script lang="ts">
  // Text Patching's setup (ModuleDescriptor.setup): which installs to
  // patch, re-patching after updates, and which patchers. An onboarding step
  // and the module's What's-new page; each patcher's options and preview
  // stay on the /langpatch page. Edits go through the store's queued save,
  // like the page's (the reconcile behind it waits for the module to be on).
  import { onMount } from "svelte";
  import { Settings, Switch } from "@veelume/ui";
  import type { OnboardingStepProps } from "../types";
  import {
    langpatchStore,
    loadLangpatch,
    saveLangpatchConfig,
    updateFromOverview,
  } from "$lib/state/langpatch.svelte";
  import { installVersion } from "$lib/home/version";

  // Typed for the step contract; nothing here is required.
  const {}: OnboardingStepProps = $props();

  const overview = $derived(langpatchStore.overview);

  onMount(() => void loadLangpatch());

  function setChannel(key: string, on: boolean) {
    if (!overview) return;
    const u = updateFromOverview(overview);
    u.channels = on ? [...u.channels.filter((c) => c !== key), key] : u.channels.filter((c) => c !== key);
    void saveLangpatchConfig(u);
  }

  function setAuto(on: boolean) {
    if (!overview) return;
    void saveLangpatchConfig({ ...updateFromOverview(overview), auto_patch: on });
  }

  function setPatcher(id: string, on: boolean) {
    if (!overview) return;
    const u = updateFromOverview(overview);
    u.patchers[id] = { options: {}, ...u.patchers[id], enabled: on };
    void saveLangpatchConfig(u);
  }
</script>

{#if overview}
  <div class="setup">
    <Settings.Section title="Installs">
      {#each overview.installs as i (i.channel_key)}
        <Settings.Row label={`${i.channel} ${installVersion(i.version)}`}>
          <Switch label={i.channel} checked={i.selected} onchange={(v) => setChannel(i.channel_key, v)} />
        </Settings.Row>
      {:else}
        <p class="dim">No Star Citizen install found. Text Patching starts once there is one.</p>
      {/each}
      <Settings.Row label="Re-patch after game updates" hint="Waits until the game is closed.">
        <Switch label="Re-patch after game updates" checked={overview.auto_patch} onchange={setAuto} />
      </Settings.Row>
    </Settings.Section>
    <Settings.Section
      title="Patchers"
      description="Each patcher's options and a preview are on the Text Patching page."
    >
      {#each overview.patchers as p (p.id)}
        <Settings.Row label={p.name} hint={p.description}>
          <Switch label={p.name} checked={p.enabled} onchange={(v) => setPatcher(p.id, v)} />
        </Settings.Row>
      {/each}
    </Settings.Section>
  </div>
{:else}
  <p class="dim">Loading…</p>
{/if}

<style>
  .setup {
    display: grid;
  }
</style>
