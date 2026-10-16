<script lang="ts">
  // The solve: the verdict for the loadout as given, the charge bar with what
  // to do at each level, and what would do better — gadgets, module kits for
  // the one laser, one support Prospector. "Limit" is the heaviest breakable
  // mass over the rock's: at 100 % and above it cracks. "Clean" is our player
  // model's share of clean cracks over 100 attempts, not the game's label.
  import type { EvalView, MiningCatalogView, MiningReportView } from "$lib/bindings";
  import ChargeBar from "./ChargeBar.svelte";
  import GuideNotes from "./GuideNotes.svelte";

  let { report, catalog }: { report: MiningReportView; catalog: MiningCatalogView } = $props();

  const names = $derived(
    new Map<string, string>([
      ...catalog.lasers.map((l) => [l.id, l.name] as [string, string]),
      ...catalog.modules.map((m) => [m.id, m.name] as [string, string]),
      ...catalog.gadgets.map((g) => [g.id, g.name] as [string, string]),
    ]),
  );
  const name = (id: string) => names.get(id) ?? "Unknown";
  const kit = (ids: string[]) => (ids.length ? ids.map(name).join(", ") : "no modules");

  const pct = (v: number, digits = 0) => `${(v * 100).toFixed(digits)} %`;
  const yours = $derived(report.yours);

  function verdict(e: EvalView): { text: string; tone: "good" | "warn" | "bad" } {
    if (!e.difficulty) return { text: `Can't crack — ${(e.need - e.power).toFixed(0)}/s short`, tone: "bad" };
    const d = e.difficulty;
    const time = d.medianTimeS === null ? "" : `, ~${d.medianTimeS.toFixed(0)} s`;
    return {
      text: `${pct(d.controlled)} clean${time}`,
      tone: d.controlled >= 0.8 ? "good" : d.controlled >= 0.4 ? "warn" : "bad",
    };
  }
</script>

{#snippet evalCells(e: EvalView)}
  {@const v = verdict(e)}
  <td class="num" class:short={e.margin < 1}>{pct(e.margin)}</td>
  <td><span class="status-pill {v.tone}"><i></i>{v.text}</span></td>
  <td class="num dim">{e.difficulty ? pct(e.difficulty.overcharged) : "—"}</td>
{/snippet}

{#snippet head(first: string)}
  <thead>
    <tr><th>{first}</th><th class="num">Limit</th><th>Our player model</th><th class="num">Overcharge</th></tr>
  </thead>
{/snippet}

<section class="card">
  <p class="dim rock">
    {report.rock.mass.toFixed(0)} t · {report.rock.scu.toFixed(2)} SCU · bare rock R {pct(report.rock.resistance, 1)} · I
    {report.rock.instability.toFixed(2)}
    {#if report.rock.gadgetResistance !== report.rock.resistance || report.rock.gadgetInstability !== report.rock.instability}
      · with the gadget R {pct(report.rock.gadgetResistance, 1)} · I {report.rock.gadgetInstability.toFixed(2)}
    {/if}
  </p>

  {#if yours}
    {@const e = yours.eval}
    {@const v = verdict(e)}
    <div class="verdict">
      <span
        class="status-pill {v.tone}"
        title="Simulated with the laser's charge metered evenly: optimistic on high-instability rocks"
        ><i></i>{e.margin >= 1 ? "Cracks" : "Too heavy"} · {v.text}</span
      >
      <span class="dim small">
        Beam on, the scanner should read R {yours.scannerResistancePct.toFixed(0)} % · I {yours.scannerInstability.toFixed(2)}
        — if it doesn't, check the scan.
      </span>
    </div>
    <div class="stat-tiles">
      <div class="stat-tile"><b>{pct(e.margin)}</b><span>limit</span></div>
      <div class="stat-tile"><b>{pct(e.resistance, 1)}</b><span>resistance</span></div>
      <div class="stat-tile"><b>{e.instability.toFixed(1)}</b><span>instability</span></div>
      <div class="stat-tile"><b>{pct(e.windowHigh - e.windowLow, 1)}</b><span>window</span></div>
    </div>
    {#if yours.guide.balance !== null}
      <div class="guide">
        <ChargeBar guide={yours.guide} />
        <GuideNotes guide={yours.guide} />
      </div>
    {/if}
  {:else}
    <p class="dim">Add a laser to see whether it cracks.</p>
  {/if}
</section>

{#if report.gadgets.length}
  <section>
    <h3 class="label">Gadget, with your loadout</h3>
    <table class="data">
      {@render head("Gadget")}
      <tbody>
        {#each report.gadgets as g (g.gadget ?? "")}
          <tr><td>{g.gadget ? name(g.gadget) : "No gadget"}</td>{@render evalCells(g.eval)}</tr>
        {/each}
      </tbody>
    </table>
  </section>
{/if}

{#if report.passiveKits.length || report.activeKits.length}
  <section>
    <h3 class="label">Modules, gadget as set</h3>
    <table class="data">
      {@render head("Modules")}
      <tbody>
        {#each report.passiveKits as k (k.modules.join())}
          <tr><td>{kit(k.modules)}</td>{@render evalCells(k.eval)}</tr>
        {/each}
        {#each report.activeKits as k (k.modules.join())}
          <tr>
            <td>
              {kit(k.modules)} <span class="badge accent">active</span>
              {#if k.runsOutS !== null}<span class="dim small"> runs out after {k.runsOutS.toFixed(0)} s</span>{/if}
            </td>
            {@render evalCells(k.eval)}
          </tr>
        {/each}
      </tbody>
    </table>
  </section>
{/if}

{#if report.support.length}
  <section>
    <h3 class="label">Plus one Prospector</h3>
    <p class="dim small">
      Your loadout and gadget unchanged. Two ships double the instability wave — inferred, not traced, so read crew
      difficulty as approximate.
    </p>
    <table class="data">
      {@render head("Support laser")}
      <tbody>
        {#each report.support as s (s.laser + s.modules.join())}
          <tr><td>{name(s.laser)} <span class="dim">· {kit(s.modules)}</span></td>{@render evalCells(s.eval)}</tr>
        {/each}
      </tbody>
    </table>
  </section>
{/if}

<style>
  section {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  h3 {
    margin: 0;
  }
  .rock {
    margin: 0;
  }
  .verdict {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
  }
  .small {
    font-size: var(--text-small);
  }
  .num {
    text-align: right;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .short {
    color: var(--bad);
  }
  .guide {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-3);
    align-items: flex-start;
  }
  .guide > :global(.notes) {
    flex: 1 1 260px;
  }
  /* Room for "4631/6060" and the labels: wrap to a second row instead. */
  .stat-tile {
    flex-basis: 108px;
  }
</style>
