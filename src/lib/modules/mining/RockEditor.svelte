<script lang="ts">
  // The scan, as the scanner panel lists it: mass, then one row per
  // composition line (element, percent, quality). Inert is the remainder and
  // is left out. The draft lives in the mining store, so it survives a
  // navigation; editing a rolled rock makes it a typed one.
  import { Select } from "@veelume/ui";
  import { Plus, X } from "lucide-svelte";
  import type { MiningCatalogView } from "$lib/bindings";
  import { addRockRow, clearRock, markTyped, miningStore, parseNumber, removeRockRow } from "$lib/state/mining.svelte";

  let { catalog }: { catalog: MiningCatalogView } = $props();

  const rock = miningStore.rock;
  const elements = $derived([{ value: "", label: "—" }, ...catalog.elements.map((e) => ({ value: e.id, label: e.name }))]);
  const total = $derived(rock.rows.reduce((sum, r) => sum + (parseNumber(r.percent) ?? 0), 0));
</script>

<div class="editor" oninput={markTyped}>
<label class="mass">
  <span class="label">Mass</span>
  <input class="input" inputmode="numeric" placeholder="20998" bind:value={rock.mass} aria-label="Mass in tonnes" />
  <span class="dim">t</span>
</label>

<div class="rows">
  <div class="row head label">
    <span>Element</span><span>%</span><span>Quality</span><span></span>
  </div>
  {#each rock.rows as row, i (i)}
    <div class="row">
      <Select
        label="Element"
        options={elements}
        value={row.element}
        onchange={(v) => {
          row.element = v;
          markTyped();
        }}
      />
      <input class="input" inputmode="decimal" placeholder="43.65" bind:value={row.percent} aria-label="Percent" />
      <input class="input" inputmode="numeric" placeholder="—" bind:value={row.quality} aria-label="Quality" />
      <button class="btn subtle icon" onclick={() => removeRockRow(i)} title="Remove row" aria-label="Remove row">
        <X size={14} />
      </button>
    </div>
  {/each}
</div>

<div class="foot">
  <button class="btn subtle add" onclick={addRockRow}><Plus size={14} /> Add element</button>
  <span class="dim small" class:error={total > 100.05}>inert {Math.max(0, 100 - total).toFixed(2)}%</span>
  <button class="btn subtle" onclick={clearRock}>Clear</button>
</div>
</div>

<style>
  .mass {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin-bottom: var(--space-2);
  }
  .mass .input {
    width: 9rem;
  }
  .rows {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 5.5rem 5rem auto;
    gap: 6px;
    align-items: center;
  }
  .row .input {
    min-width: 0;
  }
  .head {
    padding: 0 2px;
  }
  .icon {
    padding: 4px 6px;
    display: inline-flex;
  }
  .foot {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin-top: 6px;
  }
  .foot .small {
    margin-left: auto;
  }
  .add {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .small {
    font-size: var(--text-small);
  }
</style>
