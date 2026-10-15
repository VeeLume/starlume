<script lang="ts">
  // The quality mix: ten cells, the bands of 100 on 0–1000, the stock line
  // at 500. A cell's brightness is that band's share of THIS material, so
  // every row reads on its own and no scale is shared or needed (amounts are
  // the numbers beside it). Bands under the quality filter fade.
  import type { ItemNode } from "../model/tree";
  import { fmtAmount, isRated, qualityMix } from "../model/resources";

  let { list, bulk, min = 0, height = 14 }: { list: ItemNode[]; bulk: boolean; min?: number; height?: number } =
    $props();

  const cells = $derived(qualityMix(list));
  const heat = (band: number, share: number) =>
    `color-mix(in srgb, ${band >= 5 ? "var(--hue-quality)" : "var(--text-dim)"} ${Math.round(22 + 78 * share)}%, transparent)`;
</script>

{#if isRated(list)}
  <span class="mix">
    <span class="cells" style:height="{height}px">
      {#each cells as c, i (i)}
        <span
          class="cell"
          class:empty={!c.amount}
          class:under={min > 0 && (i + 1) * 100 <= min}
          style:background={c.amount ? heat(i, c.share) : undefined}
          title={`Q ${i * 100}–${i * 100 + 99}: ${c.amount ? `${fmtAmount(bulk, c.amount)} · ${Math.round(c.share * 100)}% of this material` : "none"}`}
        ></span>
      {/each}
    </span>
    <span class="axis"><span>0</span><span>500</span><span>1000</span></span>
  </span>
{:else}
  <span class="none">no quality</span>
{/if}

<style>
  .mix {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .cells {
    position: relative;
    display: grid;
    grid-template-columns: repeat(10, 1fr);
    gap: 2px;
  }
  .cells::after {
    content: "";
    position: absolute;
    left: calc(50% - 0.5px);
    top: -2px;
    bottom: -2px;
    width: 1px;
    background: var(--text-faint);
  }
  .cell {
    border-radius: 2px;
    border: 1px solid transparent;
  }
  .cell.empty {
    border-color: var(--border-soft);
  }
  .cell.under {
    opacity: 0.3;
  }
  .axis {
    display: flex;
    justify-content: space-between;
    font-family: var(--font-mono);
    font-size: 0.52rem;
    color: var(--text-faint);
    line-height: 1;
  }
  .none {
    font-size: 0.68rem;
    color: var(--text-faint);
  }
</style>
