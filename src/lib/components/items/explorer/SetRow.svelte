<script lang="ts">
  // An armour set as one row, like Crafting's: its pieces as words in fixed
  // places (held plain, missing faint), "Full set" when the four are there,
  // crafted quality when a piece was crafted.
  import { Shield } from "lucide-svelte";
  import { isFull, PIECES, type ArmourSet } from "../model/classify";
  import { avgIngredientQuality } from "../model/crafted";
  import Quality from "./Quality.svelte";
  import Weight from "./Weight.svelte";
  import { fmtN } from "./ui";

  let { set, on, onclick }: { set: ArmourSet; on: boolean; onclick: () => void } = $props();

  const shown = $derived(PIECES.filter((p) => set.have.has(p) || set.full.includes(p)));
  const bestQ = $derived(
    Math.max(-1, ...set.items.map((n) => (n.item.crafted ? (avgIngredientQuality(n.item.crafted) ?? -1) : -1))),
  );
  const n = $derived(set.items.reduce((s, x) => s + x.item.stack, 0));
</script>

<button type="button" class="row" class:on {onclick}>
  <span class="pic"><Shield size={16} /></span>
  <span class="name">{set.name}</span>
  <Weight weight={set.weight} />
  <span class="pieces">
    {#each shown as p (p)}
      <span class="piece" class:have={set.have.has(p)}
        >{p}{(set.have.get(p) ?? 0) > 1 ? ` ×${set.have.get(p)}` : ""}</span
      >
    {/each}
  </span>
  {#if bestQ >= 0}<Quality q={bestQ} prefix="avg Q" />{/if}
  <span class="state">
    {#if isFull(set)}<span class="full">Full set</span>{:else if set.full.length > 1}{set.got}/{set.full.length}{/if}
  </span>
  <span class="n">{n > 1 ? `×${fmtN(n)}` : ""}</span>
</button>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    min-height: 42px;
    padding: 4px 10px 4px 6px;
    border: 0;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 55%, transparent);
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .row:hover {
    background: color-mix(in srgb, var(--text) 4%, transparent);
  }
  .row.on {
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .pic {
    flex: none;
    display: grid;
    place-items: center;
    width: 44px;
    height: 32px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text-dim);
  }
  .name {
    flex: 0 1 220px;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--text-control);
    font-weight: var(--weight-semibold);
    color: var(--text-strong);
  }
  .pieces {
    flex: 1;
    min-width: 0;
    display: flex;
    gap: 12px;
    overflow: hidden;
    white-space: nowrap;
    font-size: var(--text-small);
  }
  .piece {
    color: var(--text-faint);
    opacity: 0.6;
  }
  .piece.have {
    opacity: 1;
    color: var(--text-dim);
  }
  .state {
    flex: none;
    width: 64px;
    font-family: var(--font-mono);
    font-size: var(--text-small);
    color: var(--text-faint);
    text-align: right;
  }
  .full {
    color: var(--good);
  }
  .n {
    flex: none;
    width: 36px;
    font-family: var(--font-mono);
    font-size: var(--text-small);
    color: var(--text-dim);
    text-align: right;
  }
</style>
