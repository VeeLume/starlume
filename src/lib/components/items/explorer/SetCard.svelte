<script lang="ts">
  // An armour set: its pieces by name (owned ones plain, missing ones
  // faint), a count toward the full set (the pieces the set has in the game
  // data) that becomes "Full set", and crafted quality. The weight class is
  // the group heading Browse puts above the cards (design pass).
  import { Shield } from "lucide-svelte";
  import { isFull, PIECES, type ArmourSet } from "../model/classify";
  import { avgIngredientQuality } from "../model/crafted";
  import Card from "./Card.svelte";
  import Quality from "./Quality.svelte";

  let { set, on, onclick }: { set: ArmourSet; on: boolean; onclick: () => void } = $props();

  const shown = $derived(PIECES.filter((p) => set.have.has(p) || set.full.includes(p)));
  const bestQ = $derived(
    Math.max(-1, ...set.items.map((n) => (n.item.crafted ? (avgIngredientQuality(n.item.crafted) ?? -1) : -1))),
  );
</script>

<Card icon={Shield} {on} crafted={bestQ >= 0} {onclick}>
  {#snippet name()}{set.name}{/snippet}
  {#snippet right()}
    {#if isFull(set)}<span class="full">Full set</span>{:else if set.full.length > 1}<span
        class="of"
        title={set.full.join(", ")}>{set.got}/{set.full.length}</span
      >{/if}
  {/snippet}
  <span class="pieces">
    {#each shown as p (p)}
      <span class="piece" class:have={set.have.has(p)}
        >{p}{(set.have.get(p) ?? 0) > 1 ? ` ×${set.have.get(p)}` : ""}</span
      >
    {/each}
  </span>
  {#if bestQ >= 0}
    <span class="meta"><Quality q={bestQ} prefix="avg Q" /><span class="crafted">crafted</span></span>
  {/if}
</Card>

<style>
  .of {
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .full {
    font-size: var(--text-small);
    color: var(--good);
    white-space: nowrap;
  }
  .pieces {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 0;
    font-size: var(--text-small);
  }
  .piece {
    color: var(--text-faint);
    opacity: 0.65;
  }
  .piece.have {
    opacity: 1;
    color: var(--text-dim);
  }
  .piece + .piece::before {
    content: "·";
    margin: 0 6px;
    color: var(--text-faint);
  }
  .crafted {
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .meta {
    display: flex;
    gap: 6px;
    align-items: center;
  }
</style>
