<script lang="ts">
  // What would do better than the ship as it is: the best three changes as
  // rows, every alternative the solve tried behind "All alternatives".
  // Support comes from your crew's loadouts first, so a row says who to
  // bring.
  import { Dialog } from "@veelume/ui";
  import type { LoadoutView, MiningCatalogView, MiningReportView } from "$lib/bindings";
  import { alternatives, byOutcome, improves } from "./alternatives";
  import HelpRow from "./HelpRow.svelte";

  let { report, loadout, catalog }: { report: MiningReportView; loadout: LoadoutView; catalog: MiningCatalogView } =
    $props();

  let all = $state(false);

  const alts = $derived(alternatives(report, loadout, catalog));
  const yours = $derived(report.yours?.eval ?? null);
  const best = $derived(
    alts
      .filter((a) => improves(a.eval, yours))
      .sort(byOutcome)
      .slice(0, 3),
  );
  const groups = $derived(
    (["Gadgets", "Modules", "Crew", "Support"] as const)
      .map((g) => ({ g, rows: alts.filter((a) => a.group === g).sort(byOutcome) }))
      .filter((x) => x.rows.length),
  );
</script>

<section class="card help">
  <p class="label">Would help</p>
  {#if best.length}
    <div class="rows">
      {#each best as a (a.key)}<HelpRow alt={a} />{/each}
    </div>
  {:else}
    <p class="dim none">Nothing does better.</p>
  {/if}
  {#if alts.length}
    <button class="link" onclick={() => (all = true)}>All alternatives ›</button>
  {/if}
</section>

<Dialog open={all} onclose={() => (all = false)} title="All alternatives" class="max-w-2xl!">
  <div class="all">
    {#each groups as x (x.g)}
      <p class="label">{x.g}</p>
      <div class="rows">
        {#each x.rows as a (a.key)}<HelpRow alt={a} onapply={() => (all = false)} />{/each}
      </div>
    {/each}
  </div>
</Dialog>

<style>
  .help {
    display: flex;
    flex-direction: column;
    gap: 10px;
    background: var(--bg-raised);
  }
  .label {
    margin: 0;
  }
  .rows {
    display: flex;
    flex-direction: column;
  }
  .none {
    margin: 0;
  }
  .link {
    align-self: flex-start;
    padding: 0;
    border: 0;
    background: none;
    font: inherit;
    font-size: var(--text-small);
    color: var(--accent);
    cursor: pointer;
  }
  .link:hover {
    text-decoration: underline;
  }
  .all {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 16px 16px;
    overflow: auto;
  }
  .all .label:not(:first-child) {
    margin-top: 8px;
  }
</style>
