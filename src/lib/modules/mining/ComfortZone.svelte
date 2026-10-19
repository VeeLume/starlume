<script lang="ts">
  // A loadout's comfort zone: every rock you'd meet as the outline, by mass
  // on the finder's log scale, height for how often, coloured by the class
  // the loadout has there — green where it is comfortable, grey where it
  // cannot break, apricot where it has too much power. `mirror` draws a
  // second loadout below the line on the same rocks (try-out above, saved
  // below). `top` is the shared scale when several are drawn together.
  import { BIN_HUES, BINS, STACK } from "./comfort";
  import { MASS_SCALE, massAt } from "./format";

  let {
    bins,
    mirror = null,
    top,
    height = 40,
    axis = false,
    label = "Rocks by mass, coloured by class",
  }: {
    bins: number[][];
    mirror?: number[][] | null;
    top: number;
    height?: number;
    axis?: boolean;
    label?: string;
  } = $props();

  const W = 520;
  const bw = W / BINS;
  const H = $derived(mirror ? height * 2 + 1 : height);
  const rects = (b: number[][], up: boolean, base: number) =>
    b.flatMap((bin, j) => {
      let y = 0;
      return STACK.flatMap((c) => {
        const h = (bin[c] / top) * (height - 2);
        if (h < 0.15) return [];
        const r = { x: j * bw, y: up ? base - y - h : base + y, h, fill: BIN_HUES[c] };
        y += h;
        return [r];
      });
    });
  const all = $derived([...rects(bins, true, height), ...(mirror ? rects(mirror, false, height + 1) : [])]);
</script>

<div class="cz">
  <svg viewBox="0 0 {W} {H}" preserveAspectRatio="none" style:height="{H}px" role="img" aria-label={label}>
    {#each MASS_SCALE.ticks as [m] (m)}
      <line x1={(massAt(m) / 100) * W} x2={(massAt(m) / 100) * W} y1="0" y2={H} class="gl" />
    {/each}
    <line x1="0" x2={W} y1={height + 0.5} y2={height + 0.5} class="base" />
    {#each all as r, i (i)}
      <rect x={r.x} y={r.y} width={bw - 0.6} height={r.h} style:fill="color-mix(in srgb, {r.fill} 75%, transparent)" />
    {/each}
  </svg>
  {#if axis}
    <div class="ax">
      {#each MASS_SCALE.ticks as [m, l] (m)}<span style:left="{massAt(m)}%">{l}</span>{/each}
    </div>
  {/if}
</div>

<style>
  .cz {
    min-width: 0;
  }
  svg {
    display: block;
    width: 100%;
  }
  .gl {
    stroke: color-mix(in srgb, var(--text) 9%, transparent);
  }
  .base {
    stroke: var(--border);
  }
  .ax {
    position: relative;
    height: 14px;
    font-family: var(--font-mono);
    font-size: 0.62rem;
    color: var(--text-faint);
  }
  .ax span {
    position: absolute;
    top: 2px;
    transform: translateX(-50%);
    white-space: nowrap;
  }
</style>
