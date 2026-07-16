<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import Switch from "$lib/components/Switch.svelte";
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

  function stateLabel(state: string): string {
    switch (state) {
      case "up-to-date":
        return "Patched · current";
      case "stale":
        return "Stale — re-patch pending";
      case "foreign":
        return "Paused — modified by another tool";
      default:
        return "Not patched";
    }
  }
</script>

<h1>Text Patching</h1>
<p class="muted">
  Enriches Star Citizen's own text — component grades, illegal-goods markers,
  weapon stats. Patches re-apply automatically after game updates while
  Starlume is running.
</p>

{#if langpatchStore.error}
  <p class="error">{langpatchStore.error}</p>
{/if}

{#if overview}
  <section>
    <h2>Installs</h2>
    {#each overview.installs as install (install.channel_key)}
      <div class="install-row">
        <Switch
          checked={install.selected}
          onchange={(v) => toggleChannel(install.channel_key, v)}
        >
          <strong>{install.channel}</strong>
          <span class="muted">{install.version}</span>
        </Switch>
        {#if install.selected}
          <span
            class="pill"
            class:good={install.state === "up-to-date"}
            class:warn={install.state === "stale" || install.state === "foreign"}
          >
            {stateLabel(install.state)}
          </span>
          <span class="row-buttons">
            {#if install.state === "foreign"}
              <button
                disabled={langpatchStore.busy}
                onclick={() => applyLangpatch(install.channel_key)}
              >
                Take over
              </button>
            {:else}
              <button
                disabled={langpatchStore.busy}
                onclick={() => applyLangpatch(install.channel_key)}
              >
                Re-apply now
              </button>
            {/if}
            {#if install.state !== "unpatched"}
              <button
                disabled={langpatchStore.busy}
                onclick={() => removeLangpatch(install.channel_key)}
              >
                Remove
              </button>
            {/if}
          </span>
        {/if}
      </div>
    {:else}
      <p class="muted">No Star Citizen installation found.</p>
    {/each}
    <Switch
      checked={overview.auto_patch}
      onchange={(v) => mutate((u) => (u.auto_patch = v))}
    >
      Keep patches up to date automatically (re-patch after game updates)
    </Switch>
  </section>

  <section>
    <h2>Patchers</h2>
    {#each overview.patchers as patcher (patcher.id)}
      <div class="patcher">
        <Switch
          checked={patcher.enabled}
          onchange={(v) => togglePatcher(patcher.id, v)}
        >
          <strong>{patcher.name}</strong>
          {#if patcher.uses_replace_ops && overview.language_pack}
            <span class="pill warn" title="Replaces whole values — overwrites language-pack text for its keys">
              overwrites pack text
            </span>
          {/if}
        </Switch>
        <p class="muted patcher-desc">{patcher.description}</p>
        {#if patcher.enabled}
          {#each patcher.options as option (option.id)}
            <div class="option-row">
              {#if option.kind.type === "Bool"}
                <Switch
                  checked={(patcher.values[option.id] ?? option.default) === "true"}
                  onchange={(v) => setOption(patcher.id, option.id, v ? "true" : "false")}
                >
                  {option.label}
                </Switch>
              {:else if option.kind.type === "Choice"}
                <label>
                  <span>{option.label}</span>
                  <select
                    value={patcher.values[option.id] ?? option.default}
                    onchange={(e) => setOption(patcher.id, option.id, e.currentTarget.value)}
                  >
                    {#each option.kind.choices as choice (choice.value)}
                      <option value={choice.value}>{choice.label}</option>
                    {/each}
                  </select>
                </label>
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
      Optional community translation (file path or URL, e.g. a German
      global.ini). It overlays the base text before enrichment; enrichment
      rides on top. URLs are cached, so offline re-patches keep working.
    </p>
    <div class="row-buttons">
      <input
        type="text"
        placeholder="C:\path\to\global.ini or https://github.com/…/global.ini"
        bind:value={packInput}
      />
      <button
        disabled={langpatchStore.busy}
        onclick={() => mutate((u) => (u.language_pack = packInput.trim() || null))}
      >
        Save
      </button>
    </div>
  </section>
{:else if !langpatchStore.error}
  <p class="muted">Loading…</p>
{/if}

<style>
  .install-row {
    display: flex;
    align-items: center;
    gap: var(--space-3, 12px);
    flex-wrap: wrap;
  }
  .patcher {
    margin-bottom: var(--space-4, 16px);
  }
  .patcher-desc {
    margin: 2px 0 6px;
  }
  .option-row {
    margin-left: var(--space-5, 24px);
  }
  .option-row label {
    display: flex;
    align-items: center;
    gap: var(--space-3, 12px);
  }
  input[type="text"] {
    flex: 1;
    min-width: 280px;
  }
</style>
