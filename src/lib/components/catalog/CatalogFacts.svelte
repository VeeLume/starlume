<script lang="ts">
  // Tight inline label/value pairs for the top of a CatalogRow expansion.
  // Pairs stay glued (label next to value) and wrap. See CatalogRow.
  interface Fact {
    k: string;
    v: string | number;
    mono?: boolean;
  }

  let { items = [] }: { items?: Fact[] } = $props();
</script>

<div class="facts">
  {#each items as f, i (i)}
    <span class="pair">
      <span class="k">{f.k}</span>
      <span class="v" class:mono={f.mono}>{f.v}</span>
    </span>
  {/each}
</div>

<style>
  .facts {
    margin: 2px 10px 10px 52px;
    border-left: 2px solid var(--border);
    padding: 8px 0 8px 16px;
    display: flex;
    flex-wrap: wrap;
    gap: 7px 26px;
    align-items: baseline;
  }
  .pair {
    display: inline-flex;
    align-items: baseline;
    gap: 7px;
  }
  .k {
    font-family: var(--font-mono);
    font-size: 0.6rem;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-faint);
  }
  .v {
    font-size: 0.8rem;
    color: var(--text);
  }
  .v.mono {
    font-family: var(--font-mono);
    font-size: 0.72rem;
  }
</style>
