<script lang="ts">
  // An armour set: owned pieces as solid accent chips, missing ones faint
  // and dashed, and a count toward the full set (the pieces the set has in
  // the game data) that becomes the "full set" badge.
  import { Shield } from "lucide-svelte";
  import { isFull, PIECES, type ArmourSet } from "../model/classify";
  import { avgIngredientQuality } from "../model/crafted";
  import Card from "./Card.svelte";
  import Weight from "./Weight.svelte";
  import Quality from "./Quality.svelte";
  import Tag from "./Tag.svelte";

  let { set, on, onclick }: { set: ArmourSet; on: boolean; onclick: () => void } = $props();

  const shown = $derived(PIECES.filter((p) => set.have.has(p) || set.full.includes(p)));
  const bestQ = $derived(
    Math.max(-1, ...set.items.map((n) => (n.item.crafted ? (avgIngredientQuality(n.item.crafted) ?? -1) : -1))),
  );
</script>

<Card icon={Shield} {on} crafted={bestQ >= 0} {onclick}>
  {#snippet name()}{set.name}{/snippet}
  {#snippet right()}
    {#if isFull(set)}<Tag label="full set" tone="primary" />{:else if set.full.length > 1}<span
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
  {#if set.weight !== "Other" || bestQ >= 0}
    <span class="meta">
      <Weight weight={set.weight} />
      {#if bestQ >= 0}<Quality q={bestQ} prefix="avg Q" />{/if}
    </span>
  {/if}
</Card>

<style>
  .of {
    font-size: 0.64rem;
    color: var(--text-dim);
  }
  .pieces {
    display: flex;
    flex-wrap: wrap;
    gap: 3px;
  }
  .piece {
    font-size: 0.62rem;
    line-height: 1.65;
    padding: 0 6px;
    border-radius: 3px;
    border: 1px dashed var(--border);
    color: var(--text-faint);
    opacity: 0.6;
  }
  .piece.have {
    opacity: 1;
    border-style: solid;
    border-color: var(--accent-border);
    background: var(--accent-fill);
    color: var(--text-strong);
    font-weight: 600;
  }
  .meta {
    display: flex;
    gap: 6px;
    align-items: center;
  }
</style>
