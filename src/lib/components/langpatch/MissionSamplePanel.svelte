<script lang="ts">
  // The Text Patching page's preview pane: one mission at a time, as the
  // current patch settings render it. The backend picks the missions so
  // every Mission Enhancer option shows in one of them, per build rather
  // than per config — toggling an option shows its effect on text already
  // on screen. Re-reads after each save lands (`savedRev`) and on every
  // mount, the cached sample shown meanwhile.
  import { onMount } from "svelte";
  import { ChevronLeft, ChevronRight, PanelRightClose } from "lucide-svelte";
  import { Button } from "@veelume/ui";
  import RichText from "$lib/components/catalog/RichText.svelte";
  import { ensureChannel } from "$lib/state/data.svelte";
  import { langpatchStore, loadPreviewSample } from "$lib/state/langpatch.svelte";

  let { oncollapse }: { oncollapse: () => void } = $props();

  /** The patcher whose options the picks showcase. */
  const PATCHER = "mission_enhancer";

  let channel = $state<string | null>(null);
  let resolved = $state(false);
  let index = $state(0);

  const sample = $derived(langpatchStore.sample);
  const current = $derived(sample && sample.length > 0 ? sample[index % sample.length] : null);

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

<aside class="sample">
  <div class="sample-head">
    <h2>Mission preview</h2>
    {#if langpatchStore.sampleBusy && sample}
      <span class="muted text-sm">Updating…</span>
    {/if}
    <span class="hide">
      <Button variant="ghost" size="icon" title="Hide preview" onclick={oncollapse}>
        <PanelRightClose size={16} />
      </Button>
    </span>
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
    <div class="stepper">
      <Button
        variant="ghost"
        size="icon"
        title="Previous mission"
        disabled={sample.length < 2}
        onclick={() => step(-1)}
      >
        <ChevronLeft size={16} />
      </Button>
      <p class="mission-title"><RichText text={current.title.patched} /></p>
      <Button
        variant="ghost"
        size="icon"
        title="Next mission"
        disabled={sample.length < 2}
        onclick={() => step(1)}
      >
        <ChevronRight size={16} />
      </Button>
    </div>

    <p class="showcases text-sm">
      <span class="muted">Showcases</span>
      {#each current.showcases as id, i (id)}
        <span class:off={!optionOn(id)} title={optionOn(id) ? undefined : "Off"}
          >{optionLabel(id)}</span
        >{#if i < current.showcases.length - 1}<span class="muted"> · </span>{/if}
      {/each}
      <span class="muted count">{(index % sample.length) + 1} / {sample.length}</span>
    </p>

    <!-- The contracts screen's Details column: its width in characters, so
         wrapping looks like the game's. -->
    <div class="panel">
      <RichText text={current.description.patched} />
    </div>
  {/if}
</aside>

<style>
  .sample {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
  .sample-head {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }
  /* The hide button stays on the pane's right edge. */
  .hide {
    margin-left: auto;
  }
  .stepper {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    max-width: 62ch;
  }
  .mission-title {
    flex: 1;
    margin: 0;
    text-align: center;
    font-weight: var(--weight-medium);
  }
  .showcases {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    column-gap: var(--space-1);
    max-width: 62ch;
    margin: 0;
  }
  .showcases .off {
    color: var(--text-dim);
    text-decoration: line-through;
  }
  .count {
    margin-left: auto;
  }
  .panel {
    max-width: 62ch;
    padding: var(--space-3);
    border: 1px solid var(--border-soft);
    border-radius: var(--radius-md);
  }
</style>
