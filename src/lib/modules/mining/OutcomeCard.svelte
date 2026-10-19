<script lang="ts">
  // The outcome for your ship: our player model's simulated tries as one
  // bar, then cracks, overcharges, time, the ore the cracks kept and the
  // heaviest rock of this kind the ship breaks. "Crack" is the share that
  // lost no piece (the client drops pieces, and their ore, the fuller the
  // danger pool is at the crack), not the game's label.
  import type { MiningReportView } from "$lib/bindings";
  import { int, secs } from "./format";
  import OutcomeBar from "./OutcomeBar.svelte";

  let { report }: { report: MiningReportView } = $props();

  const e = $derived(report.yours?.eval ?? null);
  const d = $derived(e?.difficulty ?? null);
</script>

<section class="card outcome">
  {#if e}
    <p class="label">{d ? `${d.runs} simulated tries` : "Can't break"}</p>
    <OutcomeBar {d} />
    <div class="stat-tiles">
      {#if d}
        <div class="stat-tile"><b class="good">{Math.round(d.controlled * 100)}</b><span>crack</span></div>
        <div class="stat-tile"><b class="bad">{Math.round(d.overcharged * 100)}</b><span>overcharge</span></div>
        <div class="stat-tile"><b>{secs(d.medianTimeS)}</b><span>time</span></div>
        {#if e.oreKept !== null}
          <div class="stat-tile" title="The share of the rock's ore the cracks kept on average: an overcharge drops pieces and their ore">
            <b class:bad={e.oreKept < 0.95}>{Math.round(e.oreKept * 100)}%</b><span>ore kept</span>
          </div>
        {/if}
      {:else}
        <div class="stat-tile"><b class="bad">{int(e.need - e.power)}</b><span>power short</span></div>
      {/if}
      <div class="stat-tile"><b>{int(e.margin * report.rock.mass)} t</b><span>max mass</span></div>
    </div>
  {:else}
    <p class="label">No laser</p>
  {/if}
</section>

<style>
  .outcome {
    display: flex;
    flex-direction: column;
    gap: 10px;
    background: var(--bg-raised);
  }
  .label {
    margin: 0;
  }
  .good {
    color: var(--good);
  }
  .bad {
    color: var(--bad);
  }
</style>
