<script lang="ts">
  // The finder's mass scale under a card's header, aligned with every
  // row's mass bar below it (the bar takes the row's width less its
  // figures, which this leaves room for).
  import { MASS_SCALE, massAt } from "./format";
</script>

<span class="ax">
  <span class="sc">
    {#each MASS_SCALE.ticks as [m, l] (m)}<span style:left="{massAt(m)}%">{l}</span>{/each}
  </span>
  <span class="lg">mass · mostly</span>
</span>

<style>
  .ax {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 10px;
    font-family: var(--font-mono);
    font-size: 0.66rem;
    color: var(--text-faint);
  }
  .sc {
    position: relative;
    height: 12px;
  }
  .sc span {
    position: absolute;
    transform: translateX(-50%);
    white-space: nowrap;
  }
  .lg {
    width: 180px;
    text-align: right;
  }
</style>
