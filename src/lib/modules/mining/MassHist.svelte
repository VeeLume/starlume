<script lang="ts">
  // A rock type's mass distribution from its rolls, each bar in the class
  // the calculator's ship has at that mass. The handle is a line through
  // the bars with a knob under them and the mass above it; dragging rolls
  // the rock closest to that mass. Lightest, Median and Heaviest snap.
  import type { MassBucketView } from "$lib/bindings";
  import { CLASSES, int } from "./format";

  let {
    buckets,
    value,
    onmass,
    onsnap,
  }: {
    buckets: MassBucketView[];
    value: number;
    onmass: (mass: number) => void;
    onsnap: (pick: "small" | "average" | "large") => void;
  } = $props();

  const top = $derived(buckets.at(-1)?.to ?? 1);
  const most = $derived(Math.max(1, ...buckets.map((b) => b.count)));
  const at = (m: number) => `${(Math.min(Math.max(m, 0), top) / top) * 100}%`;
  const hueOf = (c: string | null) => (c ? (CLASSES.find((x) => x.id === c)?.hue ?? "var(--text-faint)") : "var(--text-faint)");
  const on = $derived(buckets.findIndex((b) => value >= b.from && value < b.to));
</script>

<div class="hist">
  <div class="plot">
    <span class="bubble" style:left={at(value)}>{int(value)} t</span>
    <div class="bars">
      {#each buckets as b, i (i)}
        <i class:on={i === on} style:height="{(b.count / most) * 100}%" style:--hue={hueOf(b.class)}></i>
      {/each}
    </div>
    <span class="base"></span>
    <span class="hand" style:left={at(value)}><i></i></span>
    <input
      type="range"
      min="0"
      max={top}
      step={top / 400}
      value={Math.min(value, top)}
      aria-label="Rock mass"
      oninput={(e) => onmass(Number(e.currentTarget.value))}
    />
  </div>
  <div class="snaps">
    <button onclick={() => onsnap("small")}>Lightest</button>
    <button onclick={() => onsnap("average")}>Median</button>
    <button onclick={() => onsnap("large")}>Heaviest</button>
  </div>
</div>

<style>
  .hist {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .plot {
    position: relative;
    height: 112px;
  }
  .bubble {
    position: absolute;
    top: 0;
    z-index: 2;
    transform: translateX(-50%);
    padding: 1px 6px;
    border-radius: 4px;
    font-family: var(--font-mono);
    font-size: 0.72rem;
    white-space: nowrap;
    color: var(--text-strong);
    background: var(--bg);
    border: 1px solid var(--border);
    pointer-events: none;
  }
  .bars {
    position: absolute;
    left: 0;
    right: 0;
    top: 24px;
    height: 66px;
    display: flex;
    align-items: flex-end;
    gap: 2px;
  }
  .bars i {
    flex: 1;
    display: block;
    min-height: 2px;
    border-radius: 2px 2px 0 0;
    background: color-mix(in srgb, var(--hue) 70%, transparent);
    opacity: 0.55;
    transition: opacity var(--transition-fast);
  }
  .bars i.on {
    opacity: 1;
    box-shadow: 0 0 0 1px var(--text-strong);
  }
  .base {
    position: absolute;
    left: 0;
    right: 0;
    top: 96px;
    height: 2px;
    border-radius: 1px;
    background: var(--border);
  }
  .hand {
    position: absolute;
    top: 22px;
    z-index: 1;
    width: 0;
    height: 80px;
    pointer-events: none;
  }
  .hand::before {
    content: "";
    position: absolute;
    left: -1px;
    top: 0;
    width: 2px;
    height: 74px;
    border-radius: 1px;
    background: var(--text-strong);
    box-shadow: 0 0 0 1px var(--bg-raised);
  }
  .hand i {
    position: absolute;
    left: -8px;
    top: 66px;
    width: 16px;
    height: 16px;
    box-sizing: border-box;
    border-radius: 50%;
    background: var(--text-strong);
    border: 3px solid var(--bg-raised);
    box-shadow: 0 0 0 1px var(--border);
  }
  input {
    position: absolute;
    left: 0;
    right: 0;
    top: 20px;
    width: 100%;
    height: 88px;
    margin: 0;
    opacity: 0;
    cursor: ew-resize;
  }
  .plot:has(input:focus-visible) .bubble {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .snaps {
    display: flex;
    justify-content: space-between;
  }
  .snaps button {
    padding: 2px 0;
    border: 0;
    background: none;
    font: inherit;
    font-size: 0.72rem;
    color: var(--text-dim);
    cursor: pointer;
  }
  .snaps button:hover {
    color: var(--text);
    text-decoration: underline;
  }
</style>
