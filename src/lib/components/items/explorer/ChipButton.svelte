<script lang="ts">
  // A filter chip: category bar, sub filters, the quality steps.
  import type { Snippet } from "svelte";
  import { fmtN } from "./ui";

  let { on = false, n, onclick, children }: { on?: boolean; n?: number; onclick: () => void; children: Snippet } =
    $props();
</script>

<button type="button" class="chip" class:on aria-pressed={on} {onclick}>
  {@render children()}{#if n != null}<span class="n">{fmtN(n)}</span>{/if}
</button>

<style>
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 24px;
    padding: 0 9px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: transparent;
    color: var(--text-dim);
    font: inherit;
    font-size: 0.74rem;
    cursor: pointer;
  }
  .chip:hover {
    color: var(--text);
  }
  .chip.on {
    color: var(--accent);
    border-color: var(--accent-border);
    background: var(--accent-fill);
  }
  .n {
    font-family: var(--font-mono);
    font-size: 0.64rem;
    opacity: 0.8;
  }
</style>
