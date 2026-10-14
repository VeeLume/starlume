<script lang="ts">
  // A section's kicker: "WEAPONS 81". With `onclick` it navigates (a search
  // group's container), else it only labels.
  import type { Snippet } from "svelte";
  import { fmtN } from "./ui";

  let {
    title,
    n,
    note,
    onclick,
    lead,
  }: { title: string; n?: number; note?: string; onclick?: () => void; lead?: Snippet } = $props();
</script>

{#if onclick}
  <button type="button" class="head link" {onclick}>
    {@render lead?.()}<span class="path">{title}</span>{#if n != null}<span class="n">{fmtN(n)}</span>{/if}
  </button>
{:else}
  <div class="head">
    {@render lead?.()}<span class="t">{title}</span>{#if note}<span class="note">{note}</span>{/if}{#if n != null}<span
        class="n">{fmtN(n)}</span
      >{/if}
  </div>
{/if}

<style>
  .head {
    display: flex;
    align-items: baseline;
    gap: 7px;
    margin-top: 12px;
    padding: 4px 2px;
    border: 0;
    background: transparent;
    color: var(--text-faint);
    font: inherit;
    text-align: left;
  }
  .head.link {
    cursor: pointer;
    border-radius: 5px;
  }
  .head.link:hover .path {
    color: var(--text-strong);
  }
  .t {
    font-family: var(--font-mono);
    font-size: 0.62rem;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-dim);
    white-space: nowrap;
  }
  .path {
    font-size: 0.78rem;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .note {
    font-size: 0.7rem;
    color: var(--text-faint);
  }
  .n {
    font-family: var(--font-mono);
    font-size: 0.66rem;
    color: var(--accent);
  }
</style>
