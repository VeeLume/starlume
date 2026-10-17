<script lang="ts">
  // The calculator, in three bands: the scan beside your ship (or crew) and
  // the outcome; the gadget beside crew and loadout changes, a row that
  // folds to one line each; how to crack it across the width. The solve
  // (mod-mining) runs when the rock, the tried mass or the loadout
  // changes; the page owns that trigger.
  import type { LoadoutView, MiningCatalogView } from "$lib/bindings";
  import { miningStore, miningUi } from "$lib/state/mining.svelte";
  import ChangesCard from "./ChangesCard.svelte";
  import GadgetCard from "./GadgetCard.svelte";
  import HowCard from "./HowCard.svelte";
  import LoadoutDialog from "./LoadoutDialog.svelte";
  import OutcomeCard from "./OutcomeCard.svelte";
  import RockDialog from "./RockDialog.svelte";
  import ScanCard from "./ScanCard.svelte";
  import ShipCard from "./ShipCard.svelte";

  let { catalog, loadout }: { catalog: MiningCatalogView; loadout: LoadoutView } = $props();

  let editRock = $state(false);
  let editShip = $state(false);

  const report = $derived(miningStore.report);
</script>

<div class="calc" class:stale={miningStore.solving}>
  <div class="scan"><ScanCard {catalog} {report} onedit={() => (editRock = true)} onroll={() => (miningUi.view = "find")} /></div>
  <div class="right">
    <ShipCard {catalog} {loadout} {report} onedit={() => (editShip = true)} />
    {#if report}<OutcomeCard {report} />{/if}
  </div>
  {#if miningStore.solveError}
    <p class="error wide">{miningStore.solveError}</p>
  {:else if report?.yours}
    <div class="gad"><GadgetCard {catalog} {report} {loadout} /></div>
    <div class="help"><ChangesCard {report} {loadout} {catalog} /></div>
    <div class="wide"><HowCard {catalog} yours={report.yours} {loadout} /></div>
  {:else if report}
    <section class="card empty wide"><p class="dim">Add a laser to see how it flies.</p></section>
  {/if}
</div>

<RockDialog open={editRock} onclose={() => (editRock = false)} {catalog} />
<LoadoutDialog open={editShip} onclose={() => (editShip = false)} {catalog} live />

<style>
  .calc {
    container-type: inline-size;
    display: grid;
    grid-template-columns: minmax(0, 1.5fr) minmax(0, 1fr);
    grid-template-areas: "scan right" "gad help" "wide wide";
    gap: 16px;
    align-items: stretch;
  }
  .scan,
  .gad,
  .help,
  .right {
    display: flex;
    min-width: 0;
  }
  .scan {
    grid-area: scan;
  }
  .right {
    grid-area: right;
    flex-direction: column;
    gap: 14px;
  }
  .right > :global(:last-child) {
    flex: 1;
  }
  .gad {
    grid-area: gad;
  }
  .help {
    grid-area: help;
  }
  .wide {
    grid-column: 1 / -1;
    display: flex;
    min-width: 0;
  }
  .empty {
    background: var(--bg-raised);
  }
  .empty p {
    margin: 0;
  }
  .stale .gad,
  .stale .help,
  .stale .wide,
  .stale .right > :global(:last-child) {
    opacity: 0.6;
    transition: opacity var(--transition-fast);
  }
  @container (width < 900px) {
    .calc {
      grid-template-columns: minmax(0, 1fr);
      grid-template-areas: "scan" "right" "gad" "help" "wide";
    }
  }
</style>
