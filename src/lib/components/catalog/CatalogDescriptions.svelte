<script lang="ts">
  import RichText from "./RichText.svelte";

  // Stacked labeled prose blocks for the longer text of a CatalogRow
  // expansion. Primary reads full-color; secondary blocks pass muted.
  interface DescBlock {
    label?: string;
    text: string;
    muted?: boolean;
  }

  // `flush` drops the built-in gutter/measure so the block can sit inside a
  // layout column (e.g. .catalog-cols); the column then controls width.
  // `rich` renders SC mission text (\n + <EMn> tags) via RichText.
  let {
    blocks = [],
    flush = false,
    rich = false,
  }: { blocks?: DescBlock[]; flush?: boolean; rich?: boolean } = $props();
</script>

<div class="descriptions" class:flush>
  {#each blocks as b, i (i)}
    <div>
      {#if b.label}<span class="label">{b.label}</span>{/if}
      <p class:muted={b.muted}>{#if rich}<RichText text={b.text} />{:else}{b.text}{/if}</p>
    </div>
  {/each}
</div>

<style>
  .descriptions {
    margin: 6px 10px 12px 52px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    max-width: 72ch;
  }
  .descriptions.flush {
    margin: 0;
    max-width: none;
  }
  .label {
    display: block;
    font-family: var(--font-mono);
    font-size: 0.6rem;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-faint);
    margin-bottom: 4px;
  }
  p {
    margin: 0;
    font-size: 0.83rem;
    line-height: 1.6;
    color: var(--text);
    white-space: pre-wrap;
  }
  p.muted {
    color: var(--text-dim);
  }
</style>
