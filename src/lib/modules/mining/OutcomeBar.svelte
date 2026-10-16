<script lang="ts">
  // One setup's simulated tries as one bar: clean cracks green, overcharges
  // red, the rest (timed out) left empty. Empty when it can't break.
  import type { DifficultyView } from "$lib/bindings";

  let { d, height = 10 }: { d: DifficultyView | null; height?: number } = $props();
</script>

<span class="out" style:height="{height}px">
  {#if d}
    <i class="ok" style:width="{d.controlled * 100}%"></i>
    <i class="ov" style:width="{d.overcharged * 100}%"></i>
  {/if}
</span>

<style>
  .out {
    display: flex;
    gap: 2px;
    min-width: 0;
    border-radius: 5px;
    overflow: hidden;
    background: color-mix(in srgb, var(--text) 6%, transparent);
  }
  i {
    display: block;
    height: 100%;
  }
  .ok {
    background: color-mix(in srgb, var(--good) 70%, transparent);
  }
  .ov {
    background: color-mix(in srgb, var(--bad) 60%, transparent);
  }
</style>
