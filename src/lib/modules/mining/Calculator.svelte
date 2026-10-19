<script lang="ts">
  // The calculator, in two columns that end level: the scan and the gadget
  // (what's on the rock) beside your ship, the outcome and the range (how
  // you fly it); how to crack it across the width. A head or slot on the
  // ship card opens the part picker over the left column, so the ship and
  // the outcome stay in view and follow each pick. The solve (mod-mining)
  // runs when the rock, the tried mass or the loadout changes; the page
  // owns that trigger.
  import type { LoadoutView, MiningCatalogView } from "$lib/bindings";
  import { miningStore, miningUi, onCalculator, setOwnLasers } from "$lib/state/mining.svelte";
  import GadgetCard from "./GadgetCard.svelte";
  import HowCard from "./HowCard.svelte";
  import OutcomeCard from "./OutcomeCard.svelte";
  import PartPicker from "./PartPicker.svelte";
  import RangeCard from "./RangeCard.svelte";
  import RockDialog from "./RockDialog.svelte";
  import ScanCard from "./ScanCard.svelte";
  import ShipCard from "./ShipCard.svelte";
  import { ownLasers } from "./format";

  let { catalog, channel, loadout }: { catalog: MiningCatalogView; channel: string; loadout: LoadoutView } = $props();

  let editRock = $state(false);
  /** The open part picker (`L<head>`, `S<head>:<slot>`); page-local. */
  let open = $state("");

  const report = $derived(miningStore.report);
  const own = $derived(ownLasers(loadout));
  const distance = $derived(own.find((l) => l.distanceM !== null)?.distanceM ?? null);
</script>

<div class="calc" class:stale={miningStore.solving}>
  <div class="col">
    {#if open}
      <div class="over">
        <PartPicker
          {catalog}
          {channel}
          target={open}
          lasers={own}
          build={onCalculator}
          onpick={(next) => {
            open = "";
            void setOwnLasers(next);
          }}
          onclose={() => (open = "")}
        />
      </div>
    {/if}
    <ScanCard {catalog} {report} onedit={() => (editRock = true)} onroll={() => (miningUi.view = "find")} />
    {#if report?.yours}<GadgetCard {catalog} {report} {loadout} />{/if}
  </div>
  <div class="col">
    <ShipCard {catalog} {channel} {loadout} {report} {open} onopen={(k) => (open = open === k ? "" : k)} />
    {#if report}<OutcomeCard {report} />{/if}
    {#if report?.yours}<RangeCard yours={report.yours} {distance} />{/if}
  </div>
  {#if miningStore.solveError}
    <p class="error wide">{miningStore.solveError}</p>
  {:else if report?.yours}
    <div class="wide"><HowCard {catalog} {report} yours={report.yours} {loadout} /></div>
  {/if}
</div>

<RockDialog open={editRock} onclose={() => (editRock = false)} {catalog} />

<style>
  .calc {
    container-type: inline-size;
    display: grid;
    grid-template-columns: minmax(0, 1.5fr) minmax(0, 1fr);
    gap: 16px;
    align-items: stretch;
  }
  .col {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
  }
  /* The cards keep their height; the last one in each column fills it, so
     both columns end level. */
  .calc .col > :global(*) {
    flex: none;
  }
  .calc .col > :global(:last-child) {
    flex: 1 1 auto;
  }
  .over {
    position: absolute;
    inset: 0 0 auto 0;
    z-index: 4;
    flex: none;
  }
  .wide {
    grid-column: 1 / -1;
    display: flex;
    min-width: 0;
  }
  .stale .wide,
  .stale :global(.outcome) {
    opacity: 0.6;
    transition: opacity var(--transition-fast);
  }
  @container (width < 900px) {
    .calc {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
