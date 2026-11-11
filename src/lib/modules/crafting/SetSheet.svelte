<script lang="ts">
  // An armour set opened (UX board sketch 7): its colourways against its
  // pieces, which of them you own, and below, the recipe of the cell you
  // pick (a piece's colourways share one recipe; the marks are per cell).
  import { craftingStore } from "$lib/state/crafting.svelte";
  import { PIECE_SHORT, colourways, type CraftSet } from "./browse";
  import RecipeSheet from "./RecipeSheet.svelte";

  let { set, channel }: { set: CraftSet; channel: string } = $props();

  const grid = $derived(colourways(set));
  /** The picked cell; by default the first piece of the first colourway. */
  let chosen = $state<{ family: string; variant: string } | null>(null);
  const fallback = $derived.by(() => {
    for (const r of grid.rows) for (const c of r.cells) if (c) return { family: c.family.key, variant: c.variant.guid };
    return null;
  });
  const pick = $derived(chosen && set.families.some((f) => f.key === chosen!.family) ? chosen : fallback);
  const family = $derived(set.families.find((f) => f.key === pick?.family) ?? null);
</script>

<div class="set">
  <header>
    <h2>{set.name}</h2>
    <p class="sub">
      {set.weight === "Flight & other" ? "Flight and other" : `${set.weight} armour`} · {set.blueprints} blueprints
      {#if set.owned}· <span class="good">{set.owned} owned</span>{/if}
    </p>
  </header>

  <div class="cw" style="--cols: {grid.pieces.length}">
    <span class="h">Colourway</span>
    {#each grid.pieces as p (p)}<span class="h">{PIECE_SHORT[p]}</span>{/each}
    {#each grid.rows as r (r.name)}
      <span class="name">{r.name}</span>
      {#each r.cells as c, i (i)}
        {#if c}
          {@const own = craftingStore.owns(c.variant.guid)}
          <button
            type="button"
            class="cell"
            class:own
            class:on={pick?.variant === c.variant.guid}
            title="{c.variant.name}: {own ? 'you own this blueprint' : 'you don’t own this blueprint'}"
            onclick={() => (chosen = { family: c.family.key, variant: c.variant.guid })}>{own ? "owned" : "lack"}</button
          >
        {:else}
          <span class="cell none" title="No such colourway for this piece">—</span>
        {/if}
      {/each}
    {/each}
  </div>

  {#if family}
    <RecipeSheet {family} {channel} pick={pick?.variant ?? null} />
  {/if}
</div>

<style>
  .set {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.4rem;
    color: var(--text-strong);
  }
  .sub {
    margin: 2px 0 0;
    font-size: var(--text-small);
    color: var(--text-dim);
  }
  .good {
    color: var(--good);
  }
  .cw {
    display: grid;
    grid-template-columns: minmax(0, 1fr) repeat(var(--cols), 58px);
    gap: 2px 4px;
    align-items: center;
    font-size: var(--text-small);
  }
  .h {
    padding-bottom: 4px;
    border-bottom: 1px solid var(--border);
    font-family: var(--font-mono);
    font-size: var(--text-badge);
    letter-spacing: var(--label-tracking);
    text-transform: uppercase;
    color: var(--text-faint);
  }
  .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text);
  }
  .cell {
    height: 24px;
    border: 0;
    border-radius: var(--radius-chip);
    background: transparent;
    color: var(--text-dim);
    font: inherit;
    font-size: var(--text-small);
    cursor: pointer;
  }
  .cell:hover {
    background: color-mix(in srgb, var(--text) 6%, transparent);
  }
  .cell.own {
    color: var(--good);
    background: color-mix(in srgb, var(--good) 12%, transparent);
  }
  .cell.on {
    box-shadow: inset 0 0 0 1px var(--accent);
    color: var(--text-strong);
  }
  .cell.none {
    display: grid;
    place-items: center;
    color: var(--text-faint);
    cursor: default;
  }
</style>
