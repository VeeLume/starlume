<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { Button, Segmented, StatusBadge, Switch, type StatusMap } from "@veelume/ui";
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
  <section>
    <h2>Installs</h2>
    {#each overview.installs as install (install.channel_key)}
      <div class="flex flex-wrap items-center gap-3">
        <Switch
          label={install.channel}
          checked={install.selected}
          onchange={(v) => toggleChannel(install.channel_key, v)}
        />
        <span><strong>{install.channel}</strong> <span class="muted">{install.version}</span></span>
        {#if install.selected}
          <StatusBadge status={install.state} map={stateMap} />
          <span class="flex gap-2">
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
          </span>
        {/if}
      </div>
    {:else}
      <p class="muted">No Star Citizen installation found.</p>
    {/each}
    <div class="flex items-center gap-3">
      <Switch
        label="Keep patches up to date automatically"
        checked={overview.auto_patch}
        onchange={(v) => mutate((u) => (u.auto_patch = v))}
      />
      <span>Keep patches up to date automatically (re-patch after game updates)</span>
    </div>
  </section>

  <section>
    <h2>Patchers</h2>
    {#each overview.patchers as patcher (patcher.id)}
      <div class="patcher">
        <div class="flex items-center gap-3">
          <Switch
            label={patcher.name}
            checked={patcher.enabled}
            onchange={(v) => togglePatcher(patcher.id, v)}
          />
          <strong>{patcher.name}</strong>
          {#if patcher.uses_replace_ops && overview.language_pack}
            <span title="Replaces whole values — overwrites language-pack text for its keys">
              <StatusBadge status="replaces" map={packBadgeMap} />
            </span>
          {/if}
        </div>
        <p class="muted patcher-desc">{patcher.description}</p>
        {#if patcher.enabled}
          {#each patcher.options as option (option.id)}
            <div class="option-row">
              {#if option.kind.type === "Bool"}
                <div class="flex items-center gap-3">
                  <Switch
                    label={option.label}
                    checked={(patcher.values[option.id] ?? option.default) === "true"}
                    onchange={(v) => setOption(patcher.id, option.id, v ? "true" : "false")}
                  />
                  <span>{option.label}</span>
                </div>
              {:else if option.kind.type === "Choice"}
                <div class="flex flex-wrap items-center gap-3">
                  <span>{option.label}</span>
                  <Segmented
                    options={option.kind.choices}
                    value={patcher.values[option.id] ?? option.default}
                    onchange={(v) => setOption(patcher.id, option.id, v)}
                  />
                </div>
              {/if}
            </div>
          {/each}
        {/if}
      </div>
    {/each}
  </section>

  <section>
    <h2>Language pack</h2>
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
  </section>
{:else if !langpatchStore.error}
  <p class="muted">Loading…</p>
{/if}

<style>
  section {
    margin-bottom: 24px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: flex-start;
  }
  .patcher {
    margin-bottom: var(--space-4, 16px);
  }
  .patcher-desc {
    margin: 2px 0 6px;
  }
  .option-row {
    margin-left: var(--space-5, 24px);
    margin-bottom: 6px;
  }
  .pack-input {
    flex: 1;
    min-width: 280px;
  }
</style>
