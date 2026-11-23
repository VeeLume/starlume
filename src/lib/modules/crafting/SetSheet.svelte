<script lang="ts">
  // An armour set opened (UX board sketch 7): the recipe of one piece in one
  // colourway (a piece's colourways share one recipe; the owned mark is per
  // blueprint). Design pass 8 folds the colourway-by-piece grid, which took
  // the whole first screen, into the recipe header's one picker: every
  // colourway under its piece, saying which you own. The header keeps one
  // layout for every item (Valerie, 2026-11-23), so a set brings no extra
  // controls; the title names the piece.
  import type { SelectOption } from "@veelume/ui";
  import { craftingStore } from "$lib/state/crafting.svelte";
  import { PIECE_SHORT, colourways, type CraftSet } from "./browse";
  import RecipeSheet from "./RecipeSheet.svelte";

  let { set, channel }: { set: CraftSet; channel: string } = $props();

  const grid = $derived(colourways(set));
  /** Every blueprint of the set, by piece then colourway. */
  const cells = $derived(
    grid.pieces.flatMap((piece, at) =>
      grid.rows.flatMap((r) => {
        const c = r.cells[at];
        return c ? [{ piece, colour: r.name, ...c }] : [];
      }),
    ),
  );
  let chosen = $state<string | null>(null);
  const cell = $derived(cells.find((c) => c.variant.guid === chosen) ?? cells[0] ?? null);
  const options = $derived<SelectOption[]>(
    cells.map((c) => ({
      value: c.variant.guid,
      label: c.colour,
      group: PIECE_SHORT[c.piece] ?? c.piece,
      detail: craftingStore.owns(c.variant.guid) ? "owned" : "",
    })),
  );
</script>

{#if cell}
  <RecipeSheet
    family={cell.family}
    {channel}
    pick={cell.variant.guid}
    single
    choices={{
      options,
      value: cell.variant.guid,
      onchange: (g) => (chosen = g),
      owned: `${set.owned} of ${set.blueprints} owned`,
    }}
  />
{/if}
