<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  // Layout (design pass, 2026-10-15): list and detail, so the page fits the
  // window and nothing but a preview scrolls. Left, never scrolling: the
  // install in brief (its hue, the state in one sentence, the actions), the
  // patchers as rows with a one-line summary, the language pack. Right: the
  // selected patcher — its options, then what they change (Mission
  // Enhancer's live preview; fixed examples for the others). Calm: neutral
  // surfaces, violet only on controls.
  import { Button, Segmented, Switch, storedValue } from "@veelume/ui";
  import { AlertTriangle, Cpu, Crosshair, Puzzle, Ruler, Target } from "lucide-svelte";
  import type { LangpatchInstallView } from "$lib/bindings";
  import { channelHue } from "$lib/channels";
  import { installChangelist, installVersion } from "$lib/home/version";
  import { postAge } from "$lib/modules/devtracker/time";
  import { gameTag } from "$lib/components/langpatch/preview";
  import { patcherSummary } from "$lib/components/langpatch/examples";
  import PatcherExample from "$lib/components/langpatch/PatcherExample.svelte";
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

  // The selected patcher — a per-device view preference, so it survives a
  // restart (never the URL: it is not a browse state).
  const selectedId = storedValue<string>(
    "starlume.langpatch.patcher",
    "mission_enhancer",
    (v): v is string => typeof v === "string",
  );
  const selected = $derived(
    overview?.patchers.find((p) => p.id === selectedId.value) ?? overview?.patchers[0] ?? null,
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
    if (!install.selected) return { lead: "", rest: "Not patched by Starlume." };
    switch (install.state) {
      case "up-to-date":
        return {
          lead: "",
          rest: `Patched${install.patched_at ? ` ${postAge(Date.parse(install.patched_at))}` : ""}.`,
        };
      case "stale":
        return { lead: "Re-patch pending.", rest: "The game or your settings changed." };
      case "foreign":
        return { lead: "Paused.", rest: "Another tool changed global.ini, so Starlume left it alone." };
      default:
        return { lead: "", rest: "Not patched yet." };
    }
  }
</script>

<div class="lp-page">
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

    <div class="cols">
      <div class="left">
        {#each overview.installs as install (install.channel_key)}
          <!-- Live work beats the stored state: a re-patch in flight (or held
               back — SC running, pack downloading, the cook it waits on) says
               what is happening instead of a bare "Stale". -->
          {@const work = install.selected
            ? (activityFor("langpatch", install.channel_key) ??
              (install.state === "stale" ? activityFor("data-load", install.channel_key) : undefined))
            : undefined}
          {@const text = stateText(install)}
          <article class="install" class:off={!install.selected} style:--hue={channelHue(install.channel_key)}>
            <div class="install-top">
              <span class="hue-chip">{install.channel.toUpperCase()}</span>
              <span class="version" title={installChangelist(install.version) ? `build ${installChangelist(install.version)}` : undefined}>
                {installVersion(install.version)}
              </span>
              <span class="grow"></span>
              <Switch
                label="Patch {install.channel}"
                checked={install.selected}
                onchange={(v) => toggleChannel(install.channel_key, v)}
              />
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
                  title={install.state === "foreign" ? "Patch it with your settings again" : undefined}
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

        <div>
          <h2 class="label">Patchers</h2>
          <div class="plist" role="listbox" aria-label="Patchers">
            {#each overview.patchers as patcher (patcher.id)}
              {@const Icon = PATCHER_ICONS[patcher.id] ?? Puzzle}
              <!-- The row selects; its switch toggles without selecting. -->
              <div
                class="pitem"
                class:sel={selected?.id === patcher.id}
                role="option"
                tabindex="0"
                aria-selected={selected?.id === patcher.id}
                onclick={() => (selectedId.value = patcher.id)}
                onkeydown={(e) => (e.key === "Enter" || e.key === " ") && (selectedId.value = patcher.id)}
              >
                <Icon size={17} />
                <span class="nm">
                  <b>{patcher.name}</b>
                  <span>{patcherSummary(patcher)}</span>
                </span>
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <span onclick={(e) => e.stopPropagation()}>
                  <Switch label={patcher.name} checked={patcher.enabled} onchange={(v) => togglePatcher(patcher.id, v)} />
                </span>
              </div>
            {/each}
          </div>
        </div>

        <div class="pack">
          <input
            class="input"
            type="text"
            placeholder="Language pack (optional)"
            title="A community translation: path or URL of a global.ini (e.g. German). It overlays the base text before enrichment; URLs are cached, so offline re-patches keep working."
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
      </div>

      {#if selected}
        {@const Icon = PATCHER_ICONS[selected.id] ?? Puzzle}
        {@const toggles = selected.options.filter((o) => o.kind.type === "Bool")}
        {@const choices = selected.options.filter((o) => o.kind.type === "Choice")}
        <section class="detail" class:off={!selected.enabled}>
          <div class="detail-head">
            <Icon size={20} />
            <div>
              <h3>
                {selected.name}
                {#if selected.uses_replace_ops && overview.language_pack}
                  <span class="replaces" title="Replaces whole values — overwrites language-pack text for its keys">
                    overwrites pack text
                  </span>
                {/if}
              </h3>
              <p>{selected.description}</p>
            </div>
            <Switch label={selected.name} checked={selected.enabled} onchange={(v) => togglePatcher(selected.id, v)} />
          </div>

          {#if selected.options.length > 0}
            <div class="opts">
              {#if toggles.length}
                <div class="checks">
                  {#each toggles as option (option.id)}
                    {@const tag = gameTag(option.description)}
                    <label class="check opt-check" title={option.description}>
                      <input
                        type="checkbox"
                        disabled={!selected.enabled}
                        checked={(selected.values[option.id] ?? option.default) === "true"}
                        onchange={(e) => setOption(selected.id, option.id, e.currentTarget.checked ? "true" : "false")}
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
                      value={selected.values[option.id] ?? option.default}
                      disabled={!selected.enabled}
                      onchange={(v) => setOption(selected.id, option.id, v)}
                    />
                  </div>
                {/if}
              {/each}
            </div>
          {/if}

          {#if selected.id === "mission_enhancer"}
            <MissionSamplePanel />
          {:else}
            <div class="example">
              <span class="label">{selected.id === "label_fixes" ? "What it shortens" : "Example"}</span>
              <PatcherExample patcher={selected} />
            </div>
          {/if}
        </section>
      {/if}
    </div>
  {:else if !langpatchStore.error}
    <p class="muted">Loading…</p>
  {/if}
</div>

<style>
  /* The page fits the window (main's padding on both sides); only the
   * preview inside the detail scrolls. A narrow window stacks and scrolls. */
  .lp-page {
    display: flex;
    flex-direction: column;
    gap: var(--space-5);
    height: calc(100vh - 2 * var(--content-pad));
    min-height: 0;
  }
  .top {
    display: flex;
    justify-content: flex-end;
  }
  .switch-row {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    font-size: var(--text-control);
    color: var(--text-dim);
  }
  .cols {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 330px minmax(0, 1fr);
    gap: var(--space-8);
  }
  .left {
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
    min-height: 0;
    overflow: auto;
  }
  .grow {
    flex: 1;
  }
  .label {
    display: block;
    margin: 0 0 6px;
    font-family: var(--font-display);
    font-size: 0.75rem;
    font-weight: var(--weight-normal);
    color: var(--text-faint);
    text-transform: var(--label-transform);
    letter-spacing: var(--label-tracking);
  }

  /* The install in brief: a channel card like Home's. */
  .install {
    --hue: var(--text-dim);
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px 14px;
    border-radius: var(--radius-lg);
    background: linear-gradient(
      135deg,
      color-mix(in srgb, var(--hue) 10%, var(--bg-raised)),
      var(--bg-raised) 60%
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
    gap: 10px;
  }
  .version {
    font-family: var(--font-display);
    font-weight: var(--weight-bold);
    font-size: 1.3rem;
    line-height: 1;
    color: color-mix(in srgb, var(--hue) 55%, var(--text-strong));
  }
  .why {
    margin: 0;
    font-size: var(--text-control);
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

  /* Patchers: rows that select. */
  .plist {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .pitem {
    display: grid;
    grid-template-columns: 17px minmax(0, 1fr) auto;
    gap: 12px;
    align-items: center;
    padding: 8px 10px;
    border-radius: var(--radius);
    border: 1px solid transparent;
    cursor: pointer;
  }
  .pitem:hover {
    background: color-mix(in srgb, var(--text) 4%, transparent);
  }
  .pitem:focus-visible {
    outline: 2px solid var(--accent);
  }
  .pitem.sel {
    background: color-mix(in srgb, var(--accent) 10%, var(--bg-raised));
    border-color: color-mix(in srgb, var(--accent) 30%, var(--border));
  }
  .pitem :global(svg) {
    color: var(--text-faint);
  }
  .nm {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .nm b {
    font-weight: var(--weight-semibold);
    color: var(--text-strong);
  }
  .nm span {
    font-size: var(--text-small);
    color: var(--text-faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .pack {
    display: flex;
    gap: 8px;
    margin-top: auto;
  }
  .pack input {
    flex: 1;
    min-width: 0;
  }

  /* The selected patcher. */
  .detail {
    display: flex;
    flex-direction: column;
    gap: var(--space-6);
    min-height: 0;
    padding: 16px 18px;
    border-radius: var(--radius-lg);
    background: var(--bg-raised);
    border: 1px solid var(--border);
  }
  .detail-head {
    display: grid;
    grid-template-columns: 20px minmax(0, 1fr) auto;
    gap: 12px;
    align-items: start;
  }
  .detail-head :global(svg) {
    margin-top: 2px;
    color: var(--text-faint);
  }
  h3 {
    margin: 0;
    font-family: var(--font-body);
    font-size: 1.1rem;
    font-weight: var(--weight-semibold);
    color: var(--text-strong);
  }
  .detail-head p {
    margin: 2px 0 0;
    font-size: var(--text-control);
    color: var(--text-dim);
  }
  .replaces {
    margin-left: 8px;
    font-size: var(--text-small);
    font-weight: var(--weight-normal);
    color: var(--warn);
  }
  .detail.off .opts {
    opacity: 0.55;
  }
  .opts {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .checks {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 8px 20px;
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
    gap: 12px;
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
  .example {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  @media (max-width: 900px) {
    .lp-page {
      height: auto;
    }
    .cols {
      grid-template-columns: minmax(0, 1fr);
    }
    .left {
      overflow: visible;
    }
    .choice-row {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
