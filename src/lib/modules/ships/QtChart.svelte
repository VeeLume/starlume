<script lang="ts">
  // Quantum travel time against distance, from the drive's own travel
  // model (sc-vehicles: the game's spool and acceleration envelope), up to
  // 60 Gm; shaded where a jump is too short to reach cruise speed. Hover
  // anywhere for the time and fuel at that distance. `compare` draws a
  // second drive dashed (a picker's hovered option).
  import type { QuantumView } from "$lib/bindings";

  let { drive, compare = null }: { drive: QuantumView; compare?: QuantumView | null } = $props();

  const W = 720;
  const H = 230;
  const X = (gm: number) => 50 + (gm / 60) * 650;
  const top = $derived(Math.max(...drive.times.map(([, s]) => s), ...(compare?.times.map(([, s]) => s) ?? [0])));
  const yMax = $derived(Math.ceil(top / 60) * 60);
  const Y = (s: number) => 200 - (s / yMax) * 186;
  const line = (q: QuantumView) => q.times.map(([gm, s]) => `${X(gm).toFixed(1)},${Y(s).toFixed(1)}`).join(" ");
  /** Where the drive reaches cruise speed: the first distance after which
   *  time grows linearly. */
  const cruise = $derived.by(() => {
    const t = drive.times;
    const step = (i: number) => t[i + 1][1] - t[i][1];
    const last = step(t.length - 2);
    const i = t.findIndex((_, i) => i < t.length - 1 && Math.abs(step(i) - last) < 0.05);
    return i < 0 ? 0 : t[i][0];
  });
  const mmss = (s: number) => `${Math.floor(s / 60)}:${String(Math.round(s % 60)).padStart(2, "0")}`;
  const ticks = $derived(Array.from({ length: Math.floor(yMax / 60) }, (_, i) => (i + 1) * 60));

  let at = $state<number | null>(null);
  const hit = $derived(at === null ? null : drive.times[Math.min(drive.times.length - 1, Math.round(at * 2))]);

  function move(e: MouseEvent) {
    const box = (e.currentTarget as SVGElement).getBoundingClientRect();
    const x = ((e.clientX - box.left) / box.width) * W;
    at = x < 50 || x > 700 ? null : ((x - 50) / 650) * 60;
  }
</script>

<div class="qt">
  <svg viewBox="0 0 {W} {H}" role="img" aria-label="Quantum travel time against distance" onmousemove={move} onmouseleave={() => (at = null)}>
    <rect class="sh" x="50" y="14" width={X(cruise) - 50} height="186"></rect>
    <line class="ax" x1="50" y1="200" x2="700" y2="200"></line>
    <line class="ax" x1="50" y1="14" x2="50" y2="200"></line>
    {#each ticks as s (s)}
      <line class="grid" x1="50" y1={Y(s)} x2="700" y2={Y(s)}></line>
      <text x="44" y={Y(s) + 3} text-anchor="end">{mmss(s)}</text>
    {/each}
    {#each [0, 10, 20, 30, 40, 50, 60] as gm (gm)}
      <text x={X(gm)} y="218" text-anchor="middle">{gm}{gm === 60 ? " Gm" : ""}</text>
    {/each}
    {#if compare}<polyline class="cmp" points={line(compare)}></polyline>{/if}
    <polyline class="ln" points={line(drive)}></polyline>
    {#if hit}
      <line class="cross" x1={X(hit[0])} y1="14" x2={X(hit[0])} y2="200"></line>
      <circle class="pt" cx={X(hit[0])} cy={Y(hit[1])} r="5"></circle>
    {/if}
  </svg>
  <p class="read">
    {#if hit}
      {hit[0]} Gm in {mmss(hit[1])}, burning {(hit[0] * drive.fuelPerGm).toFixed(2)} SCU{hit[0] < cruise ? ", short of cruise speed" : ""}
    {:else}
      Shaded: jumps too short to reach cruise speed ({cruise} Gm). Hover for any distance.
    {/if}
  </p>
</div>

<style>
  svg {
    display: block;
    width: 100%;
    height: auto;
  }
  text {
    font: 10px var(--font-mono);
    fill: var(--text-faint);
  }
  .sh {
    fill: color-mix(in srgb, var(--text-faint) 10%, transparent);
  }
  .ax {
    stroke: var(--border);
  }
  .grid {
    stroke: var(--border);
    stroke-dasharray: 2 4;
  }
  .ln {
    fill: none;
    stroke: var(--accent);
    stroke-width: 2;
    stroke-linejoin: round;
  }
  .cmp {
    fill: none;
    stroke: var(--hue-apricot);
    stroke-width: 2;
    stroke-dasharray: 5 4;
  }
  .cross {
    stroke: var(--text-dim);
    stroke-dasharray: 3 3;
  }
  .pt {
    fill: var(--accent);
    stroke: var(--bg-raised);
    stroke-width: 2;
  }
  .read {
    margin: 4px 0 0;
    min-height: 18px;
    font: 0.8rem var(--font-mono);
    color: var(--text-dim);
  }
</style>
