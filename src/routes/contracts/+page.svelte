<script lang="ts">
  // Contracts: sub pages on the Ships page's tab row (a view switch is
  // navigation, never the Segmented input). Catalog is slice 1; Standing
  // needs the unofficial standing read (slice 2) and stays visibly off
  // until then. Contracts shows only its own concern: next steps and plan
  // content belong to Plans, which links here.
  import Catalog from "$lib/modules/contracts/Catalog.svelte";
</script>

<div class="contracts">
  <div class="tabs" role="tablist" aria-label="Contracts">
    <button type="button" role="tab" class="tab active" aria-selected="true">Catalog</button>
    <button type="button" role="tab" class="tab" aria-selected="false" disabled title="Your rank with each faction, read from the game">
      Standing
    </button>
  </div>
  <Catalog />
</div>

<style>
  .contracts {
    display: flex;
    flex-direction: column;
    gap: 16px;
    height: 100%;
    min-height: 0;
  }
  .tabs {
    margin-bottom: 0;
  }
</style>
