<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import {
    Button,
    RadioGroup,
    Settings,
    StatusBadge,
    Switch,
    storedValue,
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
  import { activityFor, activityLabel } from "$lib/state/activities.svelte";
  import MissionSamplePanel from "$lib/components/langpatch/MissionSamplePanel.svelte";

  const overview = $derived(langpatchStore.overview);

  // The mission preview pane's open/closed state on a wide window — a
  // per-device view preference, so it survives a restart.
  const previewOpen = storedValue<boolean>(
    "starlume.langpatch.previewOpen",
    true,
    (v): v is boolean => typeof v === "boolean",
  );

  // Below this width the two panes don't fit side by side: the preview turns
  // into a drawer over the settings, closed until asked for. Its state there
  // is transient, so narrowing the window never buries the settings.
  const NARROW_PX = 1024;
  let layoutWidth = $state(0);
  const narrow = $derived(layoutWidth > 0 && layoutWidth < NARROW_PX);
  let drawerOpen = $state(false);
  const open = $derived(narrow ? drawerOpen : previewOpen.value);

  function setOpen(next: boolean) {
    if (narrow) drawerOpen = next;
    else previewOpen.value = next;
  }

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

<!-- Two panes filling the content area: settings left, the mission preview
     right, each scrolling on its own. The preview collapses to a handle on
     the right border; on a narrow window it opens as a drawer instead. -->
<div
  class="panes"
  class:narrow
  class:collapsed={!open}
  bind:clientWidth={layoutWidth}
>
<div class="settings">
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
        <!-- Live work beats the stored state: a re-patch in flight (or held
             back — SC running, pack downloading, the cook it waits on) says
             what is happening instead of a bare "Stale". -->
        {@const work =
          activityFor("langpatch", install.channel_key) ??
          (install.state === "stale" ? activityFor("data-load", install.channel_key) : undefined)}
        <Settings.Row label={install.channel} hint={install.version}>
          <span class="flex flex-wrap items-center justify-end gap-2">
            {#if install.selected}
              {#if work}
                <span class="activity-note">⟳ {activityLabel(work)}</span>
              {:else}
                <StatusBadge status={install.state} map={stateMap} />
              {/if}
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
</div>

{#if open}
  <div class="preview">
    <MissionSamplePanel oncollapse={() => setOpen(false)} />
  </div>
{:else}
  <!-- On the right border, like the kit's list-collapse handle on the left:
       tall and narrow, reserving no column. -->
  <button
    type="button"
    class="dock-handle"
    title="Show mission preview"
    aria-label="Show mission preview"
    aria-expanded={false}
    onclick={() => setOpen(true)}
  >
    ‹
  </button>
{/if}
</div>

<style>
  /* The panes own the content padding, so main gives up its own: the layout
   * then fills main exactly, main never scrolls, and the preview pane meets
   * the right border. */
  :global(main:has(> .panes)) {
    padding: 0;
  }
  .panes {
    position: relative;
    display: flex;
    height: 100%;
  }
  /* Collapsed: a handle-wide gutter on the right, so the handle never sits
   * on the settings pane's scrollbar (the kit Split's rule, mirrored). */
  .panes.collapsed {
    padding-right: 12px;
  }
  /* The settings pane hugs its capped content (`.group`), and the preview
   * takes the rest — no dead gap between the two. */
  .settings {
    flex: 0 1 calc(52rem + 2 * var(--content-pad));
    min-width: 0;
    overflow-y: auto;
    padding: var(--content-pad);
  }
  .preview {
    flex: 1 1 0;
    min-width: 28rem;
    overflow-y: auto;
    padding: var(--content-pad);
    border-left: 1px solid var(--border);
    background: var(--bg);
  }
  /* Narrow: the preview is a drawer over the settings, not a column. */
  .panes.narrow .preview {
    position: absolute;
    inset-block: 0;
    right: 0;
    z-index: 10;
    width: min(36rem, 100%);
    min-width: 0;
    box-shadow: var(--shadow-panel);
  }
  .dock-handle {
    position: absolute;
    top: var(--content-pad);
    right: 0;
    display: grid;
    place-items: center;
    width: 12px;
    height: 64px;
    border: 1px solid var(--border);
    border-right: 0;
    border-radius: var(--radius) 0 0 var(--radius);
    background: var(--bg);
    color: var(--text-dim);
    font-size: var(--text-sm);
    transition:
      width 120ms,
      color 120ms;
  }
  .dock-handle:hover {
    width: 16px;
    color: var(--text);
  }
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
