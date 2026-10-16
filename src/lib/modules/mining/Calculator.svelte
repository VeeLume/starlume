<script lang="ts">
  // The calculator: the scanned rock and your ship side by side, then how to
  // fly it beside the outcome and what would help. The solve (mod-mining)
  // runs when the rock or the loadout changes; the page owns that trigger.
  import type { LoadoutView, MiningCatalogView } from "$lib/bindings";
  import { miningStore, miningUi } from "$lib/state/mining.svelte";
  import ChargeCard from "./ChargeCard.svelte";
  import LoadoutDialog from "./LoadoutDialog.svelte";
  import OutcomeCard from "./OutcomeCard.svelte";
  import RockDialog from "./RockDialog.svelte";
  import ScanCard from "./ScanCard.svelte";
  import ShipCard from "./ShipCard.svelte";
  import WouldHelp from "./WouldHelp.svelte";

  let { catalog, loadout }: { catalog: MiningCatalogView; loadout: LoadoutView } = $props();

  let editRock = $state(false);
  let editShip = $state(false);

  const report = $derived(miningStore.report);
</script>

<div class="calc">
  <div class="row top">
    <ScanCard {catalog} {report} onedit={() => (editRock = true)} onroll={() => (miningUi.view = "find")} />
    <ShipCard {catalog} {loadout} onedit={() => (editShip = true)} />
  </div>

  {#if miningStore.solveError}
    <p class="error">{miningStore.solveError}</p>
  {:else if report}
    <div class="row" class:stale={miningStore.solving}>
      {#if report.yours}
        <ChargeCard guide={report.yours.guide} />
      {:else}
        <section class="card empty"><p class="dim">Add a laser to see how it flies.</p></section>
      {/if}
      <div class="col">
        <OutcomeCard {report} />
        <WouldHelp {report} {loadout} {catalog} />
      </div>
    </div>
  {/if}
</div>

<RockDialog open={editRock} onclose={() => (editRock = false)} {catalog} />
<LoadoutDialog open={editShip} onclose={() => (editShip = false)} {catalog} live />

<style>
  .calc {
    container-type: inline-size;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .row {
    display: grid;
    grid-template-columns: minmax(0, 1.5fr) minmax(0, 1fr);
    gap: 16px;
    align-items: start;
  }
  .row.top {
    display: flex;
    flex-wrap: wrap;
    align-items: stretch;
  }
  .col {
    display: flex;
    flex-direction: column;
    gap: 14px;
    min-width: 0;
  }
  .empty {
    background: var(--bg-raised);
  }
  .empty p {
    margin: 0;
  }
  .stale {
    opacity: 0.6;
    transition: opacity var(--transition-fast);
  }
  @container (width < 900px) {
    .row {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
