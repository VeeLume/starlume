<script lang="ts">
  // Acceleration per direction, normal against boosted (dashed), with the
  // figures beside it; hovering a row lights its axis. Six axes around a
  // hexagon: forward up, retro down, the sides and up/down between.
  import type { AccelView } from "$lib/bindings";

  let { normal, boosted }: { normal: AccelView; boosted: AccelView } = $props();

  const AXES: [keyof AccelView, string, number][] = [
    ["forward", "Forward", 0],
    ["up", "Up", 60],
    ["right", "Strafe right", 120],
    ["backward", "Retro", 180],
    ["down", "Down", 240],
    ["left", "Strafe left", 300],
  ];

  let on = $state<number | null>(null);
  const top = $derived(Math.max(4, Math.ceil(Math.max(...AXES.map(([k]) => boosted[k])))));
  const r = $derived(85 / top);
  const pt = (g: number, deg: number) => [100 + Math.sin((deg * Math.PI) / 180) * g * r, 100 - Math.cos((deg * Math.PI) / 180) * g * r];
  const poly = (fn: (k: keyof AccelView, deg: number) => number[]) =>
    AXES.map(([k, , d]) => fn(k, d).map((n) => n.toFixed(1)).join(",")).join(" ");
</script>

<div class="rose">
  <svg viewBox="0 0 200 200" role="img" aria-label="Acceleration by direction, normal and boosted">
    <polygon class="ring" points={poly((_, d) => pt(top / 2, d))}></polygon>
    <polygon class="ring" points={poly((_, d) => pt(top, d))}></polygon>
    {#each AXES as [, , d], i (d)}
      {@const [x, y] = pt(top, d)}
      <line class="ax" class:on={on === i} x1="100" y1="100" x2={x} y2={y}></line>
    {/each}
    <polygon class="lo" points={poly((k, d) => pt(normal[k], d))}></polygon>
    <polygon class="hi" points={poly((k, d) => pt(boosted[k], d))}></polygon>
  </svg>
  <div class="tbl">
    <span class="h">G</span><span class="h r">normal</span><span class="h r">boost</span>
    {#each AXES as [k, label], i (k)}
      <span class="k" class:on={on === i} onmouseenter={() => (on = i)} onmouseleave={() => (on = null)} role="presentation"
        >{label}</span
      ><span class="v">{normal[k].toFixed(2)}</span><span class="v">{boosted[k].toFixed(2)}</span>
    {/each}
  </div>
</div>

<style>
  .rose {
    display: grid;
    grid-template-columns: 150px minmax(0, 1fr);
    gap: 12px;
    align-items: center;
  }
  svg {
    width: 100%;
    height: auto;
  }
  .ring {
    fill: none;
    stroke: var(--border);
  }
  .ax {
    stroke: var(--border);
  }
  .ax.on {
    stroke: var(--text-dim);
    stroke-width: 1.5;
  }
  .lo {
    fill: color-mix(in srgb, var(--accent) 18%, transparent);
    stroke: color-mix(in srgb, var(--accent) 55%, white);
    stroke-width: 2;
    stroke-linejoin: round;
  }
  .hi {
    fill: none;
    stroke: color-mix(in srgb, var(--accent) 80%, black);
    stroke-width: 2;
    stroke-dasharray: 5 3;
    stroke-linejoin: round;
  }
  .tbl {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 54px 54px;
    gap: 2px 10px;
    font-size: 0.82rem;
  }
  .h {
    font: 10px var(--font-mono);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-faint);
  }
  .r,
  .v {
    text-align: right;
  }
  .v {
    font-family: var(--font-mono);
    color: var(--text-strong);
  }
  .k {
    color: var(--text-dim);
    cursor: default;
  }
  .k.on {
    color: var(--text-strong);
  }
</style>
