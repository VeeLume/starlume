<script lang="ts">
  // An armour set opened (UX board sketch 7): the recipe of one piece in one
  // colourway (a piece's colourways share one recipe; the owned mark is per
  // blueprint). Design pass 8 folds the colourway-by-piece grid, which took
  // the whole first screen, into the recipe's header: a piece switch and a
  // colourway list that says which you own and how many you own of each.
  import { Segmented, Select } from "@veelume/ui";
  import { craftingStore } from "$lib/state/crafting.svelte";
  import { PIECE_SHORT, colourways, type CraftSet } from "./browse";
  import RecipeSheet from "./RecipeSheet.svelte";

  let { set, channel }: { set: CraftSet; channel: string } = $props();

  const grid = $derived(colourways(set));
  /** The piece and colourway picked; by default the first piece in its
   *  first colourway. */
  let piece = $state<string | null>(null);
  let colour = $state<string | null>(null);
  const shownPiece = $derived(piece && grid.pieces.includes(piece) ? piece : (grid.pieces[0] ?? null));
  const at = $derived(grid.pieces.indexOf(shownPiece ?? ""));
  /** The colourways the piece comes in. */
  const rows = $derived(grid.rows.filter((r) => r.cells[at]));
  const shownColour = $derived(colour && rows.some((r) => r.name === colour) ? colour : (rows[0]?.name ?? null));
  const cell = $derived(rows.find((r) => r.name === shownColour)?.cells[at] ?? null);
  const ownedHere = $derived(rows.filter((r) => craftingStore.owns(r.cells[at]!.variant.guid)).length);

  /** How many pieces of a colourway you own, for the list. */
  function ownedOf(r: (typeof grid.rows)[number]): string {
    const have = r.cells.filter((c) => c && craftingStore.owns(c.variant.guid)).length;
    const of = r.cells.filter(Boolean).length;
    if (have === 0) return "";
    return of === 1 ? "owned" : `${have} of ${of} owned`;
  }
</script>

{#if cell}
  <RecipeSheet family={cell.family} {channel} pick={cell.variant.guid} single>
    {#snippet picker()}
      {#if grid.pieces.length > 1}
        <Segmented
          options={grid.pieces.map((p) => ({ value: p, label: PIECE_SHORT[p] ?? p }))}
          value={shownPiece ?? ""}
          onchange={(p) => (piece = p)}
        />
      {/if}
      <Select
        class="pick"
        label="Colourway"
        options={rows.map((r) => ({ value: r.name, label: r.name, detail: ownedOf(r) }))}
        value={shownColour ?? ""}
        onchange={(c) => (colour = c)}
      />
      <span class="count">{ownedHere} of {rows.length}</span>
    {/snippet}
  </RecipeSheet>
{/if}

<style>
  .count {
    flex: 0 0 auto;
    font-size: var(--text-small);
    color: var(--text-faint);
    white-space: nowrap;
  }
</style>
