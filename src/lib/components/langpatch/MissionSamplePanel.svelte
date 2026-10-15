<script lang="ts">
  // Mission Enhancer's live preview on the Text Patching page (the detail
  // side of its list-detail layout): one mission at a time, as the
  // current patch settings render it. The backend picks the missions so
  // every Mission Enhancer option shows in one of them, per build rather
  // than per config — toggling an option shows its effect on text already
  // on screen. Re-reads after each save lands (`savedRev`) and on every
  // mount, the cached sample shown meanwhile.
  import { onMount } from "svelte";
  import { ChevronLeft, ChevronRight } from "lucide-svelte";
  import { Button } from "@veelume/ui";
  import RichText from "$lib/components/catalog/RichText.svelte";
  import { ensureChannel } from "$lib/state/data.svelte";
  import { langpatchStore, loadPreviewSample } from "$lib/state/langpatch.svelte";
  import { splitAdded } from "./preview";

  /** The patcher whose options the picks showcase. */
  const PATCHER = "mission_enhancer";

  let channel = $state<string | null>(null);
  let resolved = $state(false);
  let index = $state(0);

  const sample = $derived(langpatchStore.sample);
  const current = $derived(sample && sample.length > 0 ? sample[index % sample.length] : null);
  // The game's own text, shortened unless opened; what the patch appended
  // set off below it (design pass).
  const split = $derived(current ? splitAdded(current.description.original, current.description.patched) : null);
  let baseOpen = $state(false);

  // Option id → label, and whether it is on right now (an off option still
  // names itself, dimmed — the mission is where it would show).
  const patcher = $derived(langpatchStore.overview?.patchers.find((p) => p.id === PATCHER));
  function optionLabel(id: string): string {
    return patcher?.options.find((o) => o.id === id)?.label ?? id;
  }
  function optionOn(id: string): boolean {
    if (!patcher?.enabled) return false;
    const option = patcher.options.find((o) => o.id === id);
    const value = patcher.values[id] ?? option?.default;
    return option?.kind.type === "Bool" ? value === "true" : value !== "off";
  }

  function step(by: number) {
    if (!sample || sample.length === 0) return;
    index = (index + by + sample.length) % sample.length;
  }

  onMount(() => {
    void (async () => {
      channel = await ensureChannel();
      resolved = true;
      if (channel) await loadPreviewSample(channel);
    })();
  });

  // Re-read after each saved config edit (not on the optimistic overview
  // change — the backend reads the config from disk).
  let seenRev = langpatchStore.savedRev;
  $effect(() => {
    const rev = langpatchStore.savedRev;
    if (rev === seenRev) return;
    seenRev = rev;
    if (channel) void loadPreviewSample(channel);
  });
</script>

<div class="sample">
  <div class="head">
    <span class="label">Preview</span>
    {#if sample && sample.length > 0}
      <span class="count">{(index % sample.length) + 1} / {sample.length}</span>
      <Button variant="ghost" size="icon" title="Previous mission" disabled={sample.length < 2} onclick={() => step(-1)}>
        <ChevronLeft size={16} />
      </Button>
      <Button variant="ghost" size="icon" title="Next mission" disabled={sample.length < 2} onclick={() => step(1)}>
        <ChevronRight size={16} />
      </Button>
    {/if}
  </div>

  {#if langpatchStore.sampleError}
    <p class="text-sm text-destructive">{langpatchStore.sampleError}</p>
  {/if}

  {#if resolved && !channel}
    <p class="muted">No game data yet.</p>
  {:else if !sample}
    {#if !langpatchStore.sampleError}<p class="muted">Loading…</p>{/if}
  {:else if !current}
    <p class="muted">No mission shows these settings.</p>
  {:else}
    <!-- The contracts screen's Details column: its width in characters, so
         wrapping looks like the game's. Only this panel scrolls. -->
    <div class="panel">
      <div class="mission-title"><RichText text={current.title.patched} /></div>
      <div class="body">
        <p class="showcases">
          Shows
          {#each current.showcases as id, i (id)}
            <span class:off={!optionOn(id)} title={optionOn(id) ? undefined : "Off"}>{optionLabel(id)}</span
            >{#if i < current.showcases.length - 1}{" · "}{/if}
          {/each}
        </p>
        {#if split?.added}
          <button
            type="button"
            class="base"
            class:open={baseOpen}
            title={baseOpen ? "Shorten" : "Show the whole description"}
            onclick={() => (baseOpen = !baseOpen)}
          >
            <RichText text={split.base} />
          </button>
          <div class="added">
            <span class="added-by">Added by the patch</span>
            <RichText text={split.added} />
          </div>
        {:else}
          <RichText text={current.description.patched} />
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .sample {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    flex: 1;
    min-height: 0;
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .label {
    flex: 1;
    font-family: var(--font-display);
    font-size: 0.75rem;
    color: var(--text-faint);
    text-transform: var(--label-transform);
    letter-spacing: var(--label-tracking);
  }
  .count {
    font-size: var(--text-small);
    color: var(--text-faint);
    font-variant-numeric: tabular-nums;
  }
  .panel {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    max-width: 62ch;
    border-radius: var(--radius-lg);
    background: var(--bg-sunk);
    border: 1px solid var(--border);
    overflow: hidden;
  }
  .mission-title {
    padding: var(--space-3) var(--space-4);
    border-bottom: 1px solid var(--border);
    font-weight: var(--weight-semibold);
    color: var(--text-strong);
  }
  .body {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: var(--space-3) var(--space-4);
    font-size: var(--text-control);
  }
  .showcases {
    margin: 0 0 var(--space-3);
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .showcases .off {
    text-decoration: line-through;
  }
  .base {
    display: block;
    width: 100%;
    text-align: left;
    color: var(--text-dim);
    cursor: pointer;
  }
  .base:not(.open) {
    max-height: 4.5em;
    overflow: hidden;
    -webkit-mask-image: linear-gradient(180deg, #000 40%, transparent);
    mask-image: linear-gradient(180deg, #000 40%, transparent);
  }
  .added {
    margin-top: var(--space-3);
    padding-top: var(--space-3);
    border-top: 1px dashed var(--border);
  }
  .added-by {
    display: block;
    margin-bottom: var(--space-1);
    font-size: var(--text-small);
    color: var(--text-faint);
  }
</style>
