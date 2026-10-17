<script lang="ts">
  // A rock type's classes for the calculator's ship as one small stacked
  // bar, in power order: too much power, easy, manageable, hard, can't
  // break. The finder's rows carry it so no entry has to be opened.
  import type { ClassSharesView } from "$lib/bindings";
  import { CLASSES } from "./format";

  let { shares, height = 8 }: { shares: ClassSharesView; height?: number } = $props();
</script>

<span class="cbar" style:height="{height}px">
  {#each CLASSES as c (c.id)}
    {#if shares[c.id] > 0}<i style:width="{shares[c.id] * 100}%" style:--hue={c.hue}></i>{/if}
  {/each}
</span>

<style>
  .cbar {
    display: flex;
    gap: 1px;
    min-width: 0;
    border-radius: 3px;
    overflow: hidden;
    background: color-mix(in srgb, var(--text) 6%, transparent);
  }
  i {
    display: block;
    height: 100%;
    background: color-mix(in srgb, var(--hue) 70%, transparent);
  }
</style>
