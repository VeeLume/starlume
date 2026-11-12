<script lang="ts">
  // One row of a kind (UX board sketch 16): a model with the variants you hold
  // named in it, or one thing you care about (crafted, kitted) with its
  // details on a second line: quality, what crafting changed, the parts on
  // it. The picture slot holds the category icon until item renders exist.
  import { kitOf, shortPartName, weightOf, type Category } from "../model/classify";
  import { avgIngredientQuality, statDeltas } from "../model/crafted";
  import { variantsText, type ModelRow } from "../model/rows";
  import Delta from "./Delta.svelte";
  import Quality from "./Quality.svelte";
  import Tag from "./Tag.svelte";
  import Weight from "./Weight.svelte";
  import { CATEGORY_ICON, fmtN } from "./ui";

  let {
    row,
    category,
    on,
    where,
    onclick,
  }: {
    row: ModelRow;
    category: Category;
    on: boolean;
    /** Where it sits, for lists that span containers. */
    where?: string;
    onclick: () => void;
  } = $props();

  const Icon = $derived(CATEGORY_ICON[category]);
  const n = $derived(row.node);
  const crafted = $derived(row.cared ? n.item.crafted : null);
  const q = $derived(crafted ? avgIngredientQuality(crafted) : null);
  const top = $derived(crafted ? statDeltas(crafted).sort((a, b) => Math.abs(b.pct) - Math.abs(a.pct))[0] : null);
  const parts = $derived(
    row.cared && n.item.kind.item_type === "WeaponPersonal"
      ? kitOf(n)
          .filter((k) => k.slot !== "MAG")
          .map((k) => shortPartName(k.part.item.kind.name))
      : [],
  );
  const words = $derived(variantsText(row));
  const bricked = $derived(row.nodes.some((x) => x.item.ownership === "bricked"));
  const rented = $derived(row.nodes.some((x) => x.item.ownership === "rented"));
</script>

<button type="button" class="row" class:on class:cared={row.cared} {onclick}>
  <span class="pic"><Icon size={16} /></span>
  <span class="main">
    <span class="line">
      <span class="name">{row.label}</span>
      {#if row.n > 1}<span class="n">×{fmtN(row.n)}</span>{/if}
      {#if category === "Armour"}<Weight weight={weightOf(n)} />{/if}
      {#if bricked}<Tag label="bricked" tone="destructive" />{/if}
      {#if rented}<Tag label="rented" tone="warning" />{/if}
    </span>
    {#if row.cared}
      <span class="detail">
        {#if q != null}<Quality {q} prefix="avg Q" title="Average ingredient quality" />{/if}
        {#if top}<Delta d={top} />{/if}
        {#if parts.length}<span class="parts">{parts.join(" · ")}</span>{/if}
      </span>
    {:else if words}
      <span class="variants" title={words}>{words}</span>
    {/if}
  </span>
  {#if where}<span class="where" title={where}>{where}</span>{/if}
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
  .row.cared {
    min-height: 56px;
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
  .row.cared .pic {
    height: 44px;
  }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .line {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--text-control);
    font-weight: var(--weight-semibold);
    color: var(--text-strong);
  }
  .n {
    flex: none;
    font-family: var(--font-mono);
    font-size: var(--text-small);
    color: var(--text-dim);
  }
  .variants,
  .parts {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--text-small);
    color: var(--text-dim);
  }
  .parts {
    color: var(--text);
  }
  .detail {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    font-size: 0.7rem;
  }
  .where {
    flex: 0 1 220px;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--text-small);
    color: var(--text-dim);
    text-align: right;
  }
</style>
