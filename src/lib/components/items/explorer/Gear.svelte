<script lang="ts">
  // Your gear at a place, ahead of everything else it holds (decision 'My
  // Items opens on the few you care about…', 2026-11-11): what you crafted,
  // full armour sets, weapons kitted beyond stock. Each shelf shows its first
  // cards and opens to all of them; the opening is page-local (frontend
  // rule 7). Above an area the cards say where each thing is.
  import { SvelteSet } from "svelte/reactivity";
  import { categoryOf } from "../model/classify";
  import { gearOf, itemsBelow, whereLabel, whereOfAll } from "../model/gear";
  import type { ItemNode, Node } from "../model/tree";
  import ItemCard from "./ItemCard.svelte";
  import SectionHead from "./SectionHead.svelte";
  import SetCard from "./SetCard.svelte";
  import { fmtN } from "./ui";

  let {
    view,
    keep,
    selected,
    onselect,
  }: {
    view: Node;
    keep: (n: ItemNode) => boolean;
    selected: Node | null;
    onselect: (n: Node) => void;
  } = $props();

  /** Cards a shelf shows before "Show all": about one row, so the places
   *  below stay near the top. */
  const FIRST = 4;
  const opened = new SvelteSet<string>();

  const gear = $derived(gearOf(itemsBelow(view, keep)));
  /** Above an area, things come from several places: say which. */
  const spans = $derived(view.type === "place" && view.level !== "area");
  const isOn = (nodes: Node[]) => !!selected && nodes.includes(selected);
  const shown = <T,>(key: string, list: T[]) => (opened.has(key) ? list : list.slice(0, FIRST));
</script>

{#snippet more(key: string, n: number)}
  {#if n > FIRST}
    <button type="button" class="more" onclick={() => (opened.has(key) ? opened.delete(key) : opened.add(key))}>
      {opened.has(key) ? "Show fewer" : `Show all ${fmtN(n)}`}
    </button>
  {/if}
{/snippet}

{#if gear.crafted.length}
  {@const n = gear.crafted.reduce((s, x) => s + x.n, 0)}
  <SectionHead title="Crafted" {n} />
  <div class="grid">
    {#each shown("crafted", gear.crafted) as s (s.key)}
      <ItemCard
        stack={s}
        category={categoryOf(s.node)}
        on={isOn(s.nodes)}
        where={spans ? whereOfAll(s.nodes) : undefined}
        onclick={() => onselect(s.node)}
      />
    {/each}
  </div>
  {@render more("crafted", gear.crafted.length)}
{/if}

{#if gear.sets.length}
  <SectionHead title="Full armour sets" n={gear.sets.length} />
  <div class="grid">
    {#each shown("sets", gear.sets) as s (s.key + whereLabel(s.items[0]))}
      <SetCard
        set={s}
        on={isOn(s.items)}
        where={spans ? whereLabel(s.items[0]) : undefined}
        onclick={() => onselect(s.items.find((n) => n.item.kind.item_type === "Char_Armor_Torso") ?? s.items[0])}
      />
    {/each}
  </div>
  {@render more("sets", gear.sets.length)}
{/if}

{#if gear.kitted.length}
  {@const n = gear.kitted.reduce((s, x) => s + x.n, 0)}
  <SectionHead title="Kitted weapons" {n} />
  <div class="grid wide">
    {#each shown("kitted", gear.kitted) as s (s.key)}
      <ItemCard
        stack={s}
        category="Weapons"
        on={isOn(s.nodes)}
        where={spans ? whereOfAll(s.nodes) : undefined}
        onclick={() => onselect(s.node)}
      />
    {/each}
  </div>
  {@render more("kitted", gear.kitted.length)}
{/if}

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 6px;
    margin-top: 6px;
  }
  .grid.wide {
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  }
  .more {
    margin-top: 6px;
    padding: 2px 2px;
    border: 0;
    background: transparent;
    color: var(--accent);
    font: inherit;
    font-size: var(--text-small);
    cursor: pointer;
  }
  .more:hover {
    text-decoration: underline;
  }
</style>
