<script lang="ts">
  // A ship is one view, no tabs: its loadout cards, then what is stored on
  // board (grouped by locker or cargo container), then the ships it carries.
  import { Package, Rocket } from "lucide-svelte";
  import type { ItemKindView } from "$lib/bindings";
  import { categoryOf } from "../model/classify";
  import { stackIdentical } from "../model/stack";
  import { container, stowed, visible, type ItemNode, type Node, type ShipNode, type Tree } from "../model/tree";
  import FolderTile from "./FolderTile.svelte";
  import ItemCard from "./ItemCard.svelte";
  import LoadoutCards from "./LoadoutCards.svelte";
  import SectionHead from "./SectionHead.svelte";

  let {
    ship,
    tree,
    showBricked,
    selected,
    onselect,
    ongo,
  }: {
    ship: ShipNode;
    tree: Tree;
    showBricked: boolean;
    selected: Node | null;
    onselect: (n: Node) => void;
    ongo: (n: Node) => void;
  } = $props();

  const stored = $derived(stowed(ship, (x) => visible(x, showBricked)));
  const groups = $derived.by(() => {
    const m = new Map<string, { c: Node; list: ItemNode[] }>();
    for (const x of stored) {
      const c = container(x) ?? ship;
      const g = m.get(c.id);
      if (g) g.list.push(x);
      else m.set(c.id, { c, list: [x] });
    }
    return [...m.values()];
  });
  const carried = $derived(ship.kids.filter((k): k is ShipNode => k.type === "ship" && visible(k, showBricked)));
  const selectedGuid = $derived(selected?.type === "item" ? selected.item.kind.guid : null);

  /** A loadout line selects the part: fitted on this ship, else the
   *  saved one wherever it is. */
  function selectPart(kind: ItemKindView) {
    const onShip: ItemNode[] = [];
    const walk = (x: Node) =>
      x.kids.forEach((k) => {
        if (k.type === "ship") return;
        if (k.type === "item") onShip.push(k);
        walk(k);
      });
    walk(ship);
    const hit =
      onShip.find((x) => x.item.kind.guid === kind.guid) ??
      (tree.byKind.get(kind.guid) ?? []).find((x) => x.item.ownership !== "bricked");
    if (hit) onselect(hit);
  }
</script>

<LoadoutCards {ship} {tree} {selectedGuid} onselect={selectPart} {ongo} />

{#if !ship.virtual}
  <SectionHead title="Stored on board" n={stored.reduce((s, x) => s + x.item.stack, 0)}>
    {#snippet lead()}<Package size={13} />{/snippet}
  </SectionHead>
  {#if !stored.length}<p class="dim">Nothing stored on board.</p>{/if}
  {#each groups as g (g.c.id)}
    {#if g.c !== ship}<p class="holder">{g.c.name}</p>{/if}
    <div class="grid">
      {#each stackIdentical(g.list) as s (s.key)}
        <ItemCard stack={s} category={categoryOf(s.node)} on={!!selected && s.nodes.includes(selected as ItemNode)} onclick={() => onselect(s.node)} />
      {/each}
    </div>
  {/each}
{/if}

{#if carried.length}
  <SectionHead title="Carried ships" n={carried.length}>
    {#snippet lead()}<Rocket size={13} />{/snippet}
  </SectionHead>
  <div class="ships">
    {#each carried as c (c.id)}
      <FolderTile node={c} bricked={showBricked} on={selected === c} onclick={() => ongo(c)} />
    {/each}
  </div>
{/if}

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 6px;
    margin-top: 6px;
  }
  .ships {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 6px;
    margin-top: 6px;
  }
  .holder {
    margin: 8px 2px 0;
    font-size: 0.76rem;
    color: var(--text-dim);
  }
  .dim {
    font-size: 0.8rem;
    color: var(--text-faint);
  }
</style>
