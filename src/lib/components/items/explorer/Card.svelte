<script lang="ts">
  // The browse card frame: an icon slot (the in-game icons land here), the
  // name with an optional count or amount on the right, then the card's own
  // lines. Every item, set and material card shares this geometry.
  import type { Snippet } from "svelte";
  import type { Box } from "lucide-svelte";

  let {
    icon,
    name,
    right,
    on = false,
    crafted = false,
    onclick,
    children,
  }: {
    icon: typeof Box;
    name: Snippet;
    right?: Snippet;
    on?: boolean;
    crafted?: boolean;
    onclick: () => void;
    children?: Snippet;
  } = $props();
  const Icon = $derived(icon);
</script>

<button type="button" class="item-card" class:on class:crafted {onclick}>
  <span class="ico"><Icon size={16} /></span>
  <span class="body">
    <span class="head">
      <span class="name">{@render name()}</span>
      {#if right}<span class="right">{@render right()}</span>{/if}
    </span>
    {@render children?.()}
  </span>
</button>

<style>
  .item-card {
    display: flex;
    gap: 11px;
    min-width: 0;
    padding: 10px 11px;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--bg-raised);
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
    transition:
      background 90ms,
      border-color 90ms;
  }
  .item-card:hover {
    border-color: color-mix(in srgb, var(--text) 18%, var(--border));
  }
  .item-card.on {
    border-color: color-mix(in srgb, var(--accent) 55%, var(--border));
    background: color-mix(in srgb, var(--accent) 7%, var(--bg-raised));
  }
  .ico {
    flex: 0 0 auto;
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
    color: var(--text-dim);
    background: var(--bg);
  }
  .body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .head {
    display: flex;
    gap: 6px;
    align-items: flex-start;
  }
  .name {
    flex: 1;
    min-width: 0;
    font-size: 0.88rem;
    font-weight: 600;
    color: var(--text-strong);
    line-height: 1.25;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .right {
    font-family: var(--font-mono);
    font-size: 0.74rem;
    color: var(--text-strong);
    white-space: nowrap;
  }
</style>
