<script lang="ts">
  // A patcher's fixed before/after examples (examples.ts), for the patchers
  // without a live preview. Follows the patcher's current options.
  import type { PatcherInfoView } from "$lib/bindings";
  import { patcherExample } from "./examples";

  let { patcher }: { patcher: PatcherInfoView } = $props();
  const example = $derived(patcherExample(patcher));
</script>

{#if example}
  <div class="example">
    <span class="head">{example.fromHead}</span><span></span><span class="head">Patched</span>
    {#each example.rows as row, i (i)}
      <span class="from">{row.from}</span>
      <span class="arrow" aria-hidden="true">→</span>
      <span class="to" class:em={row.emphasised}>{row.to}</span>
    {/each}
  </div>
  {#if example.note}<p class="note">{example.note}</p>{/if}
{/if}

<style>
  .example {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 20px minmax(0, 1fr);
    gap: 6px 10px;
    align-items: center;
    padding: 12px;
    border-radius: var(--radius-lg);
    background: var(--bg-sunk);
    border: 1px solid var(--border);
    font-size: var(--text-control);
  }
  .head {
    font-size: 0.68rem;
    color: var(--text-faint);
    text-transform: var(--label-transform);
    letter-spacing: 0.08em;
  }
  .from {
    color: var(--text-dim);
  }
  .arrow {
    text-align: center;
    color: var(--text-faint);
  }
  .to {
    font-family: var(--font-mono);
    color: var(--text-strong);
  }
  /* The game's own highlight, for an emphasised style. */
  .to.em {
    color: var(--em-link);
  }
  .note {
    margin: 0;
    font-size: var(--text-small);
    color: var(--text-faint);
  }
</style>
