<script lang="ts">
  // Quantum routes as players fly them: per system, each planet to the
  // next one out and each jump point from its nearest planet, plus any
  // trip picked by hand. Distance is centre to centre less where travel
  // drops out at the destination; time comes from the drive's own curve
  // (sampled Rust-side, interpolated here), fuel from its burn per Gm.
  import { Segmented, Select, type SelectOption } from "@veelume/ui";
  import type { QtPlaceView, QuantumView } from "$lib/bindings";
  import { shipsPlaces } from "$lib/state/ships.svelte";

  let { drive }: { drive: QuantumView } = $props();

  const places = $derived(shipsPlaces.view?.places ?? []);
  const byGuid = $derived(new Map(places.map((p) => [p.guid, p])));
  const systems = $derived([...new Set(places.map((p) => p.system))]);
  let system = $state("");
  const shown = $derived(system && systems.includes(system) ? system : (systems[0] ?? ""));
  let from = $state("");
  let to = $state("");

  const routes = $derived(
    (shipsPlaces.view?.routes ?? [])
      .map(([a, b]) => [byGuid.get(a), byGuid.get(b)] as const)
      .filter((r): r is readonly [QtPlaceView, QtPlaceView] => !!r[0] && !!r[1] && r[0].system === shown),
  );
  const options = $derived<SelectOption[]>(
    places.filter((p) => p.system === shown).map((p) => ({ value: p.guid, label: p.name, group: p.kind })),
  );
  const custom = $derived.by(() => {
    const a = byGuid.get(from);
    const b = byGuid.get(to);
    return a && b && a !== b && a.system === b.system && a.system === shown ? ([a, b] as const) : null;
  });

  /** Seconds for `gm` from the drive's sampled curve. */
  function timeAt(gm: number): number {
    const pts = gm <= 60 ? drive.times : drive.far;
    if (pts.length < 2) return 0;
    let i = pts.findIndex((p) => p[0] >= gm);
    if (i < 0) i = pts.length - 1;
    if (i === 0) i = 1;
    const [x0, y0] = pts[i - 1];
    const [x1, y1] = pts[i];
    return y0 + ((y1 - y0) * (gm - x0)) / (x1 - x0);
  }

  function trip(a: QtPlaceView, b: QtPlaceView) {
    const d = Math.hypot(a.position[0] - b.position[0], a.position[1] - b.position[1], a.position[2] - b.position[2]);
    const gm = Math.max(d - b.arrival, 0) / 1e9;
    const fuel = gm * drive.fuelPerGm;
    return { gm, time: timeAt(gm), fuel, tank: drive.fuelCapacity > 0 ? fuel / drive.fuelCapacity : null };
  }

  const fmt = (n: number, d = 0) => n.toLocaleString("en-US", { maximumFractionDigits: d, minimumFractionDigits: d });
  const clock = (s: number) => {
    const m = Math.floor(s / 60);
    return m >= 60 ? `${Math.floor(m / 60)} h ${m % 60} min` : `${m} min ${String(Math.round(s % 60)).padStart(2, "0")} s`;
  };
</script>

{#snippet row(a: QtPlaceView, b: QtPlaceView, mine = false)}
  {@const t = trip(a, b)}
  <tr class:mine>
    <td>{a.name} to {b.name}</td>
    <td class="num">{fmt(t.gm, 2)} Gm</td>
    <td class="num">{clock(t.time)}</td>
    <td class="num dim">{fmt(t.fuel, 2)} SCU</td>
    <td class="num" class:bad={t.tank !== null && t.tank > 1}>{t.tank === null ? "—" : t.tank > 1 ? "over a tank" : `${Math.round(t.tank * 100)} %`}</td>
  </tr>
{/snippet}

<div class="routes">
  <div class="rh">
    <h4>Routes</h4>
    {#if systems.length > 1}
      <Segmented options={systems.map((s) => ({ value: s, label: s }))} value={shown} onchange={(v) => (system = v)} />
    {/if}
  </div>
  {#if places.length === 0}
    <p class="foot">No waypoints in this build's data; reload it in Settings → Game data.</p>
  {:else}
    <table>
      <thead><tr><th></th><th>distance</th><th>time</th><th>fuel</th><th>tank</th></tr></thead>
      <tbody>
        {#each routes as [a, b] (a.guid + b.guid)}{@render row(a, b)}{/each}
        {#if custom}{@render row(custom[0], custom[1], true)}{/if}
      </tbody>
    </table>
    <div class="pick">
      <Select label="From" options={options} value={from} onchange={(v) => (from = v)} />
      <Select label="To" options={options} value={to} onchange={(v) => (to = v)} />
    </div>
    <p class="foot">Centre to centre, less where travel drops out at the destination. Planets don't move in the data.</p>
  {/if}
</div>

<style>
  .routes {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }
  .rh {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }
  h4 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 0.74rem;
    font-weight: var(--weight-bold);
    letter-spacing: var(--label-tracking);
    text-transform: var(--label-transform);
    color: color-mix(in srgb, var(--hue-sky) 60%, var(--text-strong));
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--text-small);
  }
  th {
    font-size: 0.68rem;
    font-weight: 600;
    text-align: right;
    color: var(--text-faint);
    padding: 0 0 4px;
    border-bottom: 1px solid var(--border);
  }
  td {
    padding: 3px 0;
    color: var(--text);
  }
  td + td {
    padding-left: 10px;
  }
  tr.mine td {
    color: var(--text-strong);
    background: color-mix(in srgb, var(--hue-sky) 8%, transparent);
  }
  .num {
    text-align: right;
    font-family: var(--font-mono);
    color: var(--text-strong);
    white-space: nowrap;
  }
  .dim {
    color: var(--text-faint);
  }
  .bad {
    color: var(--bad);
  }
  .pick {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }
  .foot {
    margin: 0;
    font-size: 0.7rem;
    color: var(--text-faint);
  }
</style>
