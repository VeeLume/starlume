<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  // Layout (design pass, 2026-10-15): calm on purpose — the install as a
  // channel card (its hue, the state in one sentence, the actions under
  // it), the patchers as a one-column list with their options indented
  // under them, the language pack on one line. Neutral surfaces; violet
  // only on controls.
  import { Button, Segmented, Surface, Switch, storedValue } from "@veelume/ui";
  import { AlertTriangle, Cpu, Crosshair, Puzzle, Ruler, Target } from "lucide-svelte";
  import type { LangpatchInstallView } from "$lib/bindings";
  import { channelHue } from "$lib/channels";
  import { installChangelist, installVersion } from "$lib/home/version";
  import { postAge } from "$lib/modules/devtracker/time";
  import { gameTag } from "$lib/components/langpatch/preview";
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

  // The mission preview pane's docked open state and dragged width —
  // per-device view preferences, so they survive a restart. (As a sheet on a
  // narrow window, the kit keeps its own open state.)
  const previewOpen = storedValue<boolean>(
    "starlume.langpatch.previewOpen",
    true,
    (v): v is boolean => typeof v === "boolean",
  );
  const previewWidth = storedValue<number | null>(
    "starlume.langpatch.previewWidth",
    null,
    (v): v is number | null => v === null || typeof v === "number",
  );

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

  const PATCHER_ICONS: Record<string, typeof Target> = {
    mission_enhancer: Target,
    component_grades: Cpu,
    illegal_goods: AlertTriangle,
    weapon_enhancer: Crosshair,
    label_fixes: Ruler,
  };

  /** The install's state in one sentence (the card's text). */
  function stateText(install: LangpatchInstallView): { lead: string; rest: string } {
    if (!install.selected) return { lead: "", rest: "Starlume doesn't patch this install." };
    switch (install.state) {
      case "up-to-date":
        return {
          lead: "",
          rest: `Patched with your settings${install.patched_at ? `, ${postAge(Date.parse(install.patched_at))}` : ""}.`,
        };
      case "stale":
        return { lead: "Re-patch pending.", rest: "The game or your settings changed since the last patch." };
      case "foreign":
        return {
          lead: "Paused.",
          rest: "Another tool changed global.ini, so Starlume left it alone. Take over to patch it with your settings again.",
        };
      default:
        return { lead: "", rest: "Not patched yet." };
    }
  }
</script>

<!-- The kit's supporting-pane layout: settings in the main pane, the
     mission preview beside it (a sheet over it on a narrow window). -->
<Surface.Supporting
  class="langpatch-panes"
  title="Mission preview"
  open={previewOpen.value}
  onopen={(next) => (previewOpen.value = next)}
  width={previewWidth.value}
  onresize={(next) => (previewWidth.value = next)}
>
{#snippet actions()}
  {#if langpatchStore.sampleBusy && langpatchStore.sample}
    <span class="muted text-sm">Updating…</span>
  {/if}
{/snippet}
{#snippet main()}
<div class="settings">
{#if langpatchStore.error}
  <p class="text-sm text-destructive">{langpatchStore.error}</p>
{/if}

{#if overview}
  <div class="top">
    <label class="switch-row">
      Keep patches up to date
      <Switch
        label="Keep patches up to date"
        checked={overview.auto_patch}
        onchange={(v) => mutate((u) => (u.auto_patch = v))}
      />
    </label>
  </div>

  <section class="installs">
    {#each overview.installs as install (install.channel_key)}
      <!-- Live work beats the stored state: a re-patch in flight (or held
           back — SC running, pack downloading, the cook it waits on) says
           what is happening instead of a bare "Stale". -->
      {@const work =
        install.selected
          ? (activityFor("langpatch", install.channel_key) ??
            (install.state === "stale" ? activityFor("data-load", install.channel_key) : undefined))
          : undefined}
      {@const text = stateText(install)}
      {@const build = installChangelist(install.version)}
      <article class="install" class:off={!install.selected} style:--hue={channelHue(install.channel_key)}>
        <div class="install-top">
          <span class="hue-chip">{install.channel.toUpperCase()}</span>
          <span class="version">{installVersion(install.version)}</span>
          {#if build}<span class="build">build {build}</span>{/if}
          <span class="grow"></span>
          <label class="switch-row">
            Patch {install.channel}
            <Switch
              label="Patch {install.channel}"
              checked={install.selected}
              onchange={(v) => toggleChannel(install.channel_key, v)}
            />
          </label>
        </div>
        <p class="why">
          {#if work}
            <span class="activity-note">⟳ {activityLabel(work)}</span>
          {:else}
            {#if text.lead}<b class:warn={install.state === "foreign"}>{text.lead}</b>{/if}
            {text.rest}
          {/if}
        </p>
        {#if install.selected}
          <div class="acts">
            <Button
              variant={install.state === "foreign" ? "primary" : "outline"}
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
                Remove patch
              </Button>
            {/if}
          </div>
        {/if}
      </article>
    {:else}
      <p class="muted">No Star Citizen installation found.</p>
    {/each}
  </section>

  <section>
    <h2 class="label">Patchers</h2>
    <div class="plist">
      {#each overview.patchers as patcher (patcher.id)}
        {@const Icon = PATCHER_ICONS[patcher.id] ?? Puzzle}
        {@const toggles = patcher.options.filter((o) => o.kind.type === "Bool")}
        {@const choices = patcher.options.filter((o) => o.kind.type === "Choice")}
        <div class="prow">
          <div class="prow-head">
            <Icon size={18} />
            <div class="prow-text">
              <h3>
                {patcher.name}
                {#if patcher.uses_replace_ops && overview.language_pack}
                  <span class="replaces" title="Replaces whole values — overwrites language-pack text for its keys">
                    overwrites pack text
                  </span>
                {/if}
              </h3>
              <p class="desc">{patcher.description}</p>
            </div>
            <Switch label={patcher.name} checked={patcher.enabled} onchange={(v) => togglePatcher(patcher.id, v)} />
          </div>
          {#if patcher.enabled && patcher.options.length > 0}
            <div class="opts">
              {#if toggles.length}
                <div class="checks">
                  {#each toggles as option (option.id)}
                    {@const tag = gameTag(option.description)}
                    <label class="check opt-check" title={option.description}>
                      <input
                        type="checkbox"
                        checked={(patcher.values[option.id] ?? option.default) === "true"}
                        onchange={(e) => setOption(patcher.id, option.id, e.currentTarget.checked ? "true" : "false")}
                      />
                      {option.label}
                      {#if tag}<span class="eg">{tag}</span>{/if}
                    </label>
                  {/each}
                </div>
              {/if}
              {#each choices as option (option.id)}
                {#if option.kind.type === "Choice"}
                  <div class="choice-row">
                    <span title={option.description}>{option.label}</span>
                    <Segmented
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
    </div>
  </section>

  <section>
    <h2 class="label">Language pack</h2>
    <div class="pack">
      <input
        class="input"
        type="text"
        placeholder="Path or URL of a community global.ini (optional)"
        title="A community translation, e.g. a German global.ini. It overlays the base text before enrichment; URLs are cached, so offline re-patches keep working."
        bind:value={packInput}
      />
      <Button
        variant="outline"
        disabled={langpatchStore.busy}
        onclick={() => mutate((u) => (u.language_pack = packInput.trim() || null))}
      >
        Use
      </Button>
    </div>
  </section>
{:else if !langpatchStore.error}
  <p class="muted">Loading…</p>
{/if}
</div>
{/snippet}
{#snippet pane()}
  <MissionSamplePanel />
{/snippet}
</Surface.Supporting>

<style>
  /* The panes own their padding, so main gives up its own: the layout then
   * fills main exactly and main never scrolls. */
  :global(main:has(> .langpatch-panes)) {
    padding: 0;
  }
  /* Centred in the main pane, capped: the preview takes a share of the
   * width, and long rows strand controls far from their labels. */
  .settings {
    container-type: inline-size;
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
    max-width: calc(46rem + 2 * var(--content-pad));
    margin-inline: auto;
    padding: var(--content-pad);
  }
  .top {
    display: flex;
    justify-content: flex-end;
    margin-bottom: calc(-1 * var(--space-5));
  }
  .switch-row {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    font-size: var(--text-control);
    color: var(--text-dim);
  }
  .grow {
    flex: 1;
  }
  .label {
    margin: 0 0 8px;
    font-family: var(--font-display);
    font-size: 0.75rem;
    font-weight: var(--weight-normal);
    color: var(--text-faint);
    text-transform: var(--label-transform);
    letter-spacing: var(--label-tracking);
  }

  /* The install: a channel card like Home's. */
  .installs {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .install {
    --hue: var(--text-dim);
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px 18px;
    border-radius: var(--radius-lg);
    background: linear-gradient(
      135deg,
      color-mix(in srgb, var(--hue) 10%, var(--bg-raised)),
      var(--bg-raised) 55%
    );
    border: 1px solid color-mix(in srgb, var(--hue) 24%, var(--border));
  }
  .install.off {
    background: var(--bg-raised);
    border-color: var(--border);
  }
  .install-top {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }
  .version {
    font-family: var(--font-display);
    font-weight: var(--weight-bold);
    font-size: 1.6rem;
    line-height: 1;
    color: color-mix(in srgb, var(--hue) 55%, var(--text-strong));
  }
  .build {
    font-family: var(--font-mono);
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .why {
    margin: 0;
    max-width: 62ch;
    color: var(--text-dim);
  }
  .why b {
    font-weight: var(--weight-semibold);
    color: var(--text-strong);
  }
  .why b.warn {
    color: var(--warn);
  }
  .acts {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  /* Patchers: one quiet column. */
  .plist {
    display: flex;
    flex-direction: column;
    border-top: 1px solid var(--border);
  }
  .prow {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 14px 2px;
    border-bottom: 1px solid var(--border);
  }
  .prow-head {
    display: grid;
    grid-template-columns: 18px minmax(0, 1fr) auto;
    gap: 14px;
    align-items: start;
  }
  .prow-head :global(svg) {
    margin-top: 2px;
    color: var(--text-faint);
  }
  .prow-text {
    min-width: 0;
  }
  h3 {
    margin: 0;
    font-family: var(--font-body);
    font-size: 0.95rem;
    font-weight: var(--weight-semibold);
    color: var(--text-strong);
  }
  .replaces {
    margin-left: 8px;
    font-size: var(--text-small);
    font-weight: var(--weight-normal);
    color: var(--warn);
  }
  .desc {
    margin: 2px 0 0;
    font-size: var(--text-control);
    color: var(--text-dim);
  }
  .opts {
    display: flex;
    flex-direction: column;
    gap: 14px;
    margin-left: 32px;
  }
  .checks {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px 24px;
  }
  .opt-check {
    gap: 9px;
    color: var(--text);
    white-space: normal;
  }
  .eg {
    font-family: var(--font-mono);
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .choice-row {
    display: grid;
    grid-template-columns: 150px minmax(0, 1fr);
    gap: 14px;
    align-items: center;
  }
  .choice-row > span {
    font-size: var(--text-control);
    color: var(--text-dim);
  }
  .choice-row :global([role="group"]) {
    justify-self: start;
    max-width: 100%;
    flex-wrap: wrap;
  }

  .pack {
    display: flex;
    gap: 8px;
  }
  .pack input {
    flex: 1;
    min-width: 0;
  }

  @container (width < 560px) {
    .checks,
    .choice-row {
      grid-template-columns: minmax(0, 1fr);
    }
    .opts {
      margin-left: 0;
    }
  }
</style>
