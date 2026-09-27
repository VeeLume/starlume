<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import {
    Button,
    RadioGroup,
    Settings,
    StatusBadge,
    Switch,
    type StatusMap,
  } from "@veelume/ui";
  import {
    langpatchStore,
    loadLangpatch,
    listenForLangpatchChanges,
    saveLangpatchConfig,
    updateFromOverview,
    applyLangpatch,
    removeLangpatch,
  } from "$lib/state/langpatch.svelte";

  const overview = $derived(langpatchStore.overview);

  let packInput = $state("");
  let unlisten: UnlistenFn | undefined;

  onMount(() => {
    void (async () => {
      unlisten = await listenForLangpatchChanges();
      await loadLangpatch();
      packInput = langpatchStore.overview?.language_pack ?? "";
    })();
  });

  onDestroy(() => unlisten?.());

  // Every edit sends the whole config (one IPC shape); the backend saves,
  // removes deselected channels, and kicks a reconcile.
  function mutate(edit: (u: ReturnType<typeof updateFromOverview>) => void) {
    if (!overview) return;
    const update = updateFromOverview(overview);
    edit(update);
    void saveLangpatchConfig(update);
  }

  function toggleChannel(key: string, on: boolean) {
    mutate((u) => {
      u.channels = on ? [...u.channels, key] : u.channels.filter((c) => c !== key);
    });
  }

  function togglePatcher(id: string, on: boolean) {
    mutate((u) => {
      const p = u.patchers[id] ?? { enabled: null, options: {} };
      u.patchers[id] = { ...p, enabled: on };
    });
  }

  function setOption(patcherId: string, optionId: string, value: string) {
    mutate((u) => {
      const p = u.patchers[patcherId] ?? { enabled: null, options: {} };
      u.patchers[patcherId] = {
        ...p,
        options: { ...p.options, [optionId]: value },
      };
    });
  }

  // Install patch states → one badge each (the StatusBadge map contract).
  const stateMap: StatusMap = {
    "up-to-date": { label: () => "Patched · current", tone: "primary" },
    stale: { label: () => "Stale — re-patch pending", tone: "warning" },
    foreign: { label: () => "Paused — modified by another tool", tone: "warning" },
    unpatched: { label: () => "Not patched", tone: "neutral" },
  };

  const packBadgeMap: StatusMap = {
    replaces: { label: () => "overwrites pack text", tone: "warning" },
  };
</script>

<h1>Text Patching</h1>
<p class="muted">
  Enriches Star Citizen's own text — component grades, illegal-goods markers, weapon stats.
  Patches re-apply automatically after game updates while Starlume is running.
</p>

{#if langpatchStore.error}
  <p class="text-sm text-destructive">{langpatchStore.error}</p>
{/if}

{#if overview}
  <!-- Cards group what belongs together; inside them Settings.Row puts the
       label + hint left and the control right, so every switch on the page
       hangs from one edge (the Settings pages' idiom). -->
  <section class="group">
    <h2>Installs</h2>
    <div class="card rows">
      {#each overview.installs as install (install.channel_key)}
        <Settings.Row label={install.channel} hint={install.version}>
          <span class="flex flex-wrap items-center justify-end gap-2">
            {#if install.selected}
              <StatusBadge status={install.state} map={stateMap} />
              <Button
                variant="outline"
                disabled={langpatchStore.busy}
                onclick={() => applyLangpatch(install.channel_key)}
              >
                {install.state === "foreign" ? "Take over" : "Re-apply now"}
              </Button>
              {#if install.state !== "unpatched"}
                <Button
                  variant="ghost"
                  disabled={langpatchStore.busy}
                  onclick={() => removeLangpatch(install.channel_key)}
                >
                  Remove
                </Button>
              {/if}
            {/if}
            <Switch
              label={install.channel}
              checked={install.selected}
              onchange={(v) => toggleChannel(install.channel_key, v)}
            />
          </span>
        </Settings.Row>
      {:else}
        <p class="muted">No Star Citizen installation found.</p>
      {/each}
      <Settings.Row
        label="Keep patches up to date"
        hint="Re-patch automatically after game updates."
      >
        <Switch
          label="Keep patches up to date"
          checked={overview.auto_patch}
          onchange={(v) => mutate((u) => (u.auto_patch = v))}
        />
      </Settings.Row>
    </div>
  </section>

  <section class="group">
    <h2>Patchers</h2>
    {#each overview.patchers as patcher (patcher.id)}
      <div class="card">
        <Settings.Row label={patcher.name} hint={patcher.description}>
          <span class="flex items-center gap-2">
            {#if patcher.uses_replace_ops && overview.language_pack}
              <span title="Replaces whole values — overwrites language-pack text for its keys">
                <StatusBadge status="replaces" map={packBadgeMap} />
              </span>
            {/if}
            <Switch
              label={patcher.name}
              checked={patcher.enabled}
              onchange={(v) => togglePatcher(patcher.id, v)}
            />
          </span>
        </Settings.Row>
        {#if patcher.enabled && patcher.options.length > 0}
          <div class="options rows">
            {#each patcher.options as option (option.id)}
              {#if option.kind.type === "Bool"}
                <Settings.Row label={option.label}>
                  <Switch
                    label={option.label}
                    checked={(patcher.values[option.id] ?? option.default) === "true"}
                    onchange={(v) => setOption(patcher.id, option.id, v ? "true" : "false")}
                  />
                </Settings.Row>
              {:else if option.kind.type === "Choice"}
                <!-- Radios, not Segmented: these choices carry long
                     descriptive labels, which a segmented bar squeezes into
                     one hard-to-scan line. -->
                <div class="choice">
                  <span class="text-sm font-medium">{option.label}</span>
                  <RadioGroup
                    options={option.kind.choices}
                    value={patcher.values[option.id] ?? option.default}
                    onchange={(v) => setOption(patcher.id, option.id, v)}
                  />
                </div>
              {/if}
            {/each}
          </div>
        {/if}
      </div>
    {/each}
  </section>

  <section class="group">
    <h2>Language pack</h2>
    <div class="card rows">
      <p class="muted">
        Optional community translation (file path or URL, e.g. a German global.ini). It
        overlays the base text before enrichment; enrichment rides on top. URLs are cached,
        so offline re-patches keep working.
      </p>
      <div class="flex w-full flex-wrap items-center gap-2">
        <input
          class="input pack-input"
          type="text"
          placeholder="C:\path\to\global.ini or https://github.com/…/global.ini"
          bind:value={packInput}
        />
        <Button
          variant="outline"
          disabled={langpatchStore.busy}
          onclick={() => mutate((u) => (u.language_pack = packInput.trim() || null))}
        >
          Save
        </Button>
      </div>
    </div>
  </section>
{:else if !langpatchStore.error}
  <p class="muted">Loading…</p>
{/if}

<style>
  /* Cap the width: Settings.Row pushes controls to the right edge, which on a
   * wide window strands them far from their labels. */
  .group {
    max-width: 52rem;
    margin-bottom: var(--space-8);
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
  .rows {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }
  /* A patcher's options sit under a hairline inside its card. */
  .options {
    margin-top: var(--space-3);
    padding-top: var(--space-3);
    border-top: 1px solid var(--border-soft);
  }
  /* A choice: its label on the row's left edge, the radios stacked under it. */
  .choice {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    padding: var(--space-2) 0;
  }
  .pack-input {
    flex: 1;
    min-width: 280px;
  }
</style>
