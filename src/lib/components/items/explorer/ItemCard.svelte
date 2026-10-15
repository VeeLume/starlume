<script lang="ts">
  // One stack of identical items. A weapon shows its kit as one row per
  // slot (bright = added to stock, grey = stock, plain = stock unknown);
  // armour its weight class; crafted items their quality and top stat.
  import type { Category } from "../model/classify";
  import { kitOf, shortPartName, SLOT_NAME, weightOf, type Slot } from "../model/classify";
  import { avgIngredientQuality, statDeltas } from "../model/crafted";
  import type { Stack } from "../model/stack";
  import Card from "./Card.svelte";
  import Weight from "./Weight.svelte";
  import Quality from "./Quality.svelte";
  import Tag from "./Tag.svelte";
  import Delta from "./Delta.svelte";
  import { CATEGORY_ICON, fmtN } from "./ui";

  let { stack, category, on, onclick }: { stack: Stack; category: Category; on: boolean; onclick: () => void } =
    $props();

  const n = $derived(stack.node);
  const item = $derived(n.item);
  const weapon = $derived(item.kind.item_type === "WeaponPersonal" && category === "Weapons");
  const kit = $derived(weapon ? kitOf(n) : []);
  const rows = $derived<Slot[]>(["OPT", "BRL", "UND", ...(kit.some((k) => k.slot === "UTL") ? (["UTL"] as Slot[]) : [])]);
  const mag = $derived(kit.find((k) => k.slot === "MAG"));
  const weight = $derived(category === "Armour" ? weightOf(n) : "Other");
  const crafted = $derived(item.crafted);
  const q = $derived(crafted ? avgIngredientQuality(crafted) : null);
  const top = $derived(crafted ? statDeltas(crafted).sort((a, b) => Math.abs(b.pct) - Math.abs(a.pct))[0] : null);
  const sized = $derived(category === "Ship parts" || weapon);
</script>

<Card icon={CATEGORY_ICON[category]} {on} crafted={!!crafted} {onclick}>
  {#snippet name()}{item.kind.name}{/snippet}
  {#snippet right()}{#if stack.n > 1}×{fmtN(stack.n)}{/if}{/snippet}
  {#if weapon}
    <span class="kit">
      {#each rows as s (s)}
        {@const k = kit.find((x) => x.slot === s)}
        <span class="slot">{s}</span>
        <span
          class="part {k ? k.state : 'empty'}"
          title={k ? `${SLOT_NAME[s]}: ${k.part.item.kind.name} (${k.state})` : `${SLOT_NAME[s]}: empty`}
          >{k ? shortPartName(k.part.item.kind.name) : "—"}</span
        >
      {/each}
    </span>
  {/if}
  <span class="meta">
    <Weight {weight} />
    {#if q != null}<Quality {q} prefix="avg Q" title="Average ingredient quality (the data records none for the item)" />{/if}
    {#if top}<Delta d={top} />{/if}
    {#if weapon}
      <span class="faint num" title={mag?.part.item.kind.name}
        >{mag ? `MAG ${mag.part.item.ammo != null ? `${mag.part.item.ammo} rds` : mag.state}` : "no mag"}</span
      >
    {/if}
    {#if sized && item.kind.size}<span class="faint num">S{item.kind.size}</span>{/if}
    {#if item.ammo != null}<span class="faint num">{item.ammo} rds</span>{/if}
    {#if item.ownership === "rented"}<Tag label="rented" tone="warning" />{/if}
    {#if item.ownership === "bricked"}<Tag label="bricked" tone="destructive" />{/if}
  </span>
</Card>

<style>
  .kit {
    display: grid;
    grid-template-columns: 24px minmax(0, 1fr);
    gap: 1px 6px;
    font-size: 0.7rem;
    line-height: 1.35;
  }
  .slot {
    font-family: var(--font-mono);
    font-size: 0.56rem;
    letter-spacing: 0.04em;
    color: var(--text-faint);
    padding-top: 2px;
  }
  .part {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .part.added {
    color: var(--text-strong);
    font-weight: var(--weight-medium);
  }
  .part.stock {
    color: var(--text-dim);
  }
  .part.fitted {
    color: var(--text);
  }
  .part.empty {
    color: var(--text-faint);
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    font-size: 0.66rem;
  }
  .faint {
    color: var(--text-faint);
  }
  .num {
    font-family: var(--font-mono);
  }
</style>
