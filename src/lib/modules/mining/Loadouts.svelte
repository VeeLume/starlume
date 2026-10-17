<script lang="ts">
  // Saved loadouts: yours, and your crew's in each player's monogram hue
  // (the calculator's support suggestions come from these). The one on the
  // calculator is ringed; a crew mate on the rock can be taken off again.
  import type { LoadoutView, MiningCatalogView, SavedLoadoutView } from "$lib/bindings";
  import Monogram from "$lib/components/Monogram.svelte";
  import NowCard from "$lib/components/NowCard.svelte";
  import { nameHue } from "$lib/monogram";
  import { addSupport, miningStore, removeSupport, setLoadout } from "$lib/state/mining.svelte";
  import { lasersLine, namesOf, ownLasers, sameLoadout } from "./format";
  import LoadoutDialog from "./LoadoutDialog.svelte";

  let { catalog, loadout }: { catalog: MiningCatalogView; loadout: LoadoutView } = $props();

  let editing = $state<SavedLoadoutView | null>(null);
  let adding = $state<"mine" | "crew" | null>(null);

  const names = $derived(namesOf(catalog));
  const saved = $derived(miningStore.state?.saved ?? []);
  const mine = $derived(saved.filter((s) => s.owner === null));
  const crew = $derived(
    saved.filter((s) => s.owner !== null).sort((a, b) => (a.owner ?? "").localeCompare(b.owner ?? "")),
  );
  const own = $derived<LoadoutView>({ ...loadout, lasers: ownLasers(loadout) });
  const aboard = $derived(new Set(loadout.lasers.flatMap((l) => (l.crew === null ? [] : [l.crew]))));
  const modulesOf = (l: LoadoutView) => l.lasers.flatMap((x) => x.modules.map((m) => names.get(m) ?? "Unknown"));
</script>

{#snippet card(s: SavedLoadoutView)}
  {@const cur = s.owner === null ? sameLoadout(s.loadout, own) : aboard.has(s.id)}
  {@const mods = modulesOf(s.loadout)}
  <div class="cell" class:cur>
    <NowCard
      hue={s.owner ? nameHue(s.owner) : "var(--hue-sky)"}
      chip={s.owner ? undefined : "● YOURS"}
      version={s.name}
    >
      {#snippet top()}
        {#if s.owner}<span class="who"><Monogram name={s.owner} size={26} />{s.owner}</span>{/if}
        {#if cur}<span class="hue-chip on">on the calculator</span>{/if}
      {/snippet}
      <span class="lasers">{lasersLine(s.loadout, names)}</span>
      <div class="tags">
        {#each mods as m, i (i)}<span class="tag">{m}</span>{/each}
        {#if !mods.length}<span class="tag faint">no modules</span>{/if}
      </div>
      {#snippet action()}
        {#if s.owner}
          {#if cur}
            <button class="btn" onclick={() => void removeSupport(s.id, s.owner)}>Take off the rock</button>
          {:else}
            <button class="btn" onclick={() => void addSupport(s.id)}>Add as support</button>
          {/if}
        {:else if !cur}
          <button class="btn" onclick={() => void setLoadout({ ...s.loadout, gadget: loadout.gadget, lead: null })}>Use</button>
        {/if}
        <button class="btn" onclick={() => (editing = s)}>Edit</button>
      {/snippet}
    </NowCard>
  </div>
{/snippet}

<div class="lo">
  <section>
    <p class="label">Yours</p>
    <div class="grid">
      {#each mine as s (s.id)}{@render card(s)}{/each}
      <button class="add" onclick={() => (adding = "mine")}>+ Loadout</button>
    </div>
  </section>
  <section>
    <p class="label">Crew</p>
    <div class="grid">
      {#each crew as s (s.id)}{@render card(s)}{/each}
      <button class="add" onclick={() => (adding = "crew")}>+ Crew loadout</button>
    </div>
  </section>
  <section>
    <p class="label">Later</p>
    <div class="plans">
      <article class="plan">
        <h4>Find a loadout</h4>
        <div class="ghost" aria-hidden="true">
          <span class="select">Ore</span><span class="select">Place</span><span class="select">Ship</span>
          <button class="btn" disabled tabindex="-1">Find</button>
        </div>
        <p>Lasers and modules ranked by how many of those rocks they crack.</p>
      </article>
      <article class="plan">
        <h4>Ship fit</h4>
        <p>Only lasers that fit the ship. A ship from My Items brings its fitted parts.</p>
      </article>
    </div>
  </section>
</div>

<LoadoutDialog
  open={editing !== null || adding !== null}
  onclose={() => {
    editing = null;
    adding = null;
  }}
  {catalog}
  saved={editing}
  crew={adding === "crew"}
/>

<style>
  .lo {
    display: flex;
    flex-direction: column;
    gap: 22px;
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .label {
    margin: 0;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: 14px;
    align-items: stretch;
  }
  .cell {
    display: flex;
    border-radius: var(--radius-lg);
  }
  .cell.cur {
    box-shadow: 0 0 0 1px var(--accent-border);
  }
  .who {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-weight: var(--weight-semibold);
    font-size: 0.9rem;
  }
  .hue-chip.on {
    --hue: var(--accent);
    margin-left: 6px;
  }
  .lasers {
    font-size: 0.92rem;
    font-weight: var(--weight-medium);
    color: var(--text-dim);
  }
  .tag.faint {
    color: var(--text-faint);
  }
  .add {
    min-height: 150px;
    border: 1px dashed var(--border);
    border-radius: var(--radius-lg);
    background: transparent;
    font: inherit;
    font-size: 0.875rem;
    color: var(--text-dim);
    cursor: pointer;
  }
  .add:hover {
    border-color: var(--accent);
    color: var(--text);
  }
  .plans {
    display: grid;
    grid-template-columns: minmax(0, 1.4fr) minmax(0, 1fr);
    gap: 14px;
  }
  .plan {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 14px 16px;
    border-radius: var(--radius-lg);
    border: 1px dashed var(--border);
  }
  .plan h4 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.1rem;
    letter-spacing: 0.02em;
    color: var(--text-dim);
  }
  .plan p {
    margin: 0;
    font-size: 0.86rem;
    color: var(--text-faint);
  }
  .ghost {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
    opacity: 0.5;
  }
  .ghost .select {
    min-width: 90px;
    font-size: var(--text-small);
  }
  @media (max-width: 760px) {
    .plans {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
