<script lang="ts">
  // A rock type's masses on the finder's shared log scale: the spread from
  // lightest to heaviest, the most common mass marked, the figures beside
  // it. Every row uses the same scale, so a 1k rock and a 100k rock compare
  // at a glance, across places and ores.
  import type { MassRangeView } from "$lib/bindings";
  import { MASS_SCALE, kt, massAt } from "./format";

  let { mass }: { mass: MassRangeView } = $props();
</script>

<span class="ms">
  <span class="bar" aria-hidden="true">
    {#each MASS_SCALE.ticks as [m] (m)}<i class="gl" style:left="{massAt(m)}%"></i>{/each}
    <i class="sp" style:left="{massAt(mass.min)}%" style:width="{massAt(mass.max) - massAt(mass.min)}%"></i>
    <i class="md" style:left="{massAt(mass.mode)}%"></i>
  </span>
  <span class="lab">{kt(mass.min)}–{kt(mass.max)} t · mostly <b>{kt(mass.mode)}</b></span>
</span>

<style>
  .ms {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 10px;
    align-items: center;
  }
  .bar {
    position: relative;
    height: 10px;
  }
  .bar i {
    position: absolute;
  }
  .gl {
    top: 0;
    bottom: 0;
    width: 1px;
    background: color-mix(in srgb, var(--text) 8%, transparent);
  }
  .sp {
    top: 3px;
    height: 4px;
    border-radius: 2px;
    background: color-mix(in srgb, var(--hue-sand) 55%, transparent);
  }
  .md {
    top: 0;
    width: 3px;
    height: 10px;
    margin-left: -1px;
    border-radius: 1px;
    background: var(--hue-sand);
  }
  /* One width for every row's figures, so the bars line up with each other
     and with the card's axis (MassAxis). */
  .lab {
    width: 180px;
    font-family: var(--font-mono);
    font-size: 0.7rem;
    color: var(--text-faint);
    text-align: right;
    white-space: nowrap;
  }
  .lab b {
    font-weight: var(--weight-medium);
    color: var(--text-dim);
  }
</style>
