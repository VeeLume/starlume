<script lang="ts">
  // The one arrow for anything that opens or expands (decision 2026-10-18):
  // pointing down for what opens below, turning up while open; pointing
  // right for a row that expands in place, turning down when open. No
  // ▾ ▸ text glyphs. Takes the colour of its text.
  import { ChevronDown } from "lucide-svelte";

  let { open = false, dir = "down", size = 14 }: { open?: boolean; dir?: "down" | "right"; size?: number } = $props();
</script>

<span class="chev" class:right={dir === "right"} class:open aria-hidden="true"><ChevronDown {size} strokeWidth={1.75} /></span>

<style>
  .chev {
    display: inline-flex;
    flex: none;
    transition: transform var(--transition-fast, 0.15s);
  }
  .chev :global(svg) {
    display: block;
  }
  .chev.open {
    transform: rotate(180deg);
  }
  .chev.right {
    transform: rotate(-90deg);
  }
  .chev.right.open {
    transform: none;
  }
</style>
