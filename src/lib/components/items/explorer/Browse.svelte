<script lang="ts">
  // One container's contents (the home storage every patch refills):
  // sub-containers first, then the in-game categories. A big container opens
  // on an overview card per category; a category has sub filters, armour
  // folds into sets (or pieces), weapons can narrow to kitted ones, and
  // resources fold per material.
  import {
    armourSets,
    categoryOf,
    CATEGORIES,
    isFull,
    isKitted,
    SUBCATEGORIES,
    subcategoryOf,
    weightOf,
    type ArmourSet,
    type Category,
    type Weight,
  } from "../model/classify";
  import { materials } from "../model/resources";
  import { stackIdentical } from "../model/stack";
  import { isFolder, visible, type ItemNode, type Node } from "../model/tree";
  import ChipButton from "./ChipButton.svelte";
  import FolderTile from "./FolderTile.svelte";
  import ItemCard from "./ItemCard.svelte";
  import ResourceCard from "./ResourceCard.svelte";
  import SectionHead from "./SectionHead.svelte";
  import SetCard from "./SetCard.svelte";
  import { CATEGORY_ICON, fmtN, type BrowseFilters } from "./ui";

  let {
    view,
    showBricked,
    keep,
    selected,
    filters,
    onfilter,
    onselect,
    onopen,
  }: {
    view: Node;
    showBricked: boolean;
    keep: (n: ItemNode) => boolean;
    selected: Node | null;
    filters: BrowseFilters;
    onfilter: (patch: Partial<BrowseFilters>) => void;
    onselect: (n: Node) => void;
    onopen: (n: Node) => void;
  } = $props();

  /** Past this many units a container opens on the category overview. */
  const BIG = 60;
  const WEIGHTS: Weight[] = ["Light", "Medium", "Heavy", "Other"];
  const WEIGHT_GROUPS: Weight[] = ["Heavy", "Medium", "Light", "Other"];

  const kids = $derived(view.kids.filter((k) => visible(k, showBricked)));
  const folders = $derived(kids.filter(isFolder));
  const items = $derived(kids.filter((k): k is ItemNode => k.type === "item" && !isFolder(k) && keep(k)));
  const byCat = $derived.by(() => {
    const m = new Map<Category, ItemNode[]>();
    for (const x of items) {
      const c = categoryOf(x);
      const l = m.get(c);
      if (l) l.push(x);
      else m.set(c, [x]);
    }
    return m;
  });
  const cats = $derived(CATEGORIES.filter((c) => byCat.has(c)));
  const cat = $derived<Category | "">(byCat.has(filters.cat as Category) ? (filters.cat as Category) : "");
  const sum = (list: ItemNode[]) => list.reduce((s, x) => s + x.item.stack, 0);
  /** Materials for resources (boxes and pieces don't add up), units else. */
  const count = (list: ItemNode[], c: Category) => (c === "Resources" ? materials(list).length : sum(list));
  const big = $derived(sum(items) > BIG);
  const pick = (c: string) => onfilter({ cat: c, sub: "", w: "", kit: "" });

  // The open category, narrowed.
  const all = $derived(cat ? (byCat.get(cat) ?? []) : []);
  const order = $derived(cat ? (SUBCATEGORIES[cat] ?? []) : []);
  const weights = $derived(cat === "Armour" ? WEIGHTS.filter((w) => all.some((x) => weightOf(x) === w)) : []);
  const narrowed = $derived(
    all.filter(
      (x) =>
        (!filters.w || weightOf(x) === filters.w) &&
        (!filters.sub || subcategoryOf(x, cat as Category) === filters.sub) &&
        (!filters.kit || isKitted(x)),
    ),
  );
  const subCount = (s: string) =>
    count(
      all.filter((x) => subcategoryOf(x, cat as Category) === s && (!filters.w || weightOf(x) === filters.w)),
      cat as Category,
    );
  const showSets = $derived(cat === "Armour" && filters.arm !== "pieces");
  const setGroups = $derived.by(() => {
    if (!showSets) return [];
    const sets = armourSets(narrowed);
    const full = (s: ArmourSet) => (isFull(s) ? 1 : 0);
    return WEIGHT_GROUPS.map((w) => ({
      w,
      sets: sets.filter((s) => s.weight === w).sort((a, b) => full(b) - full(a) || b.items.length - a.items.length),
    })).filter((g) => g.sets.length);
  });
  const sections = $derived(
    !filters.sub && order.length
      ? order.map((s) => ({ s, list: narrowed.filter((x) => subcategoryOf(x, cat as Category) === s) })).filter((g) => g.list.length)
      : [{ s: "", list: narrowed }],
  );

  function overview(c: Category) {
    const list = byCat.get(c) ?? [];
    const subs = new Map<string, number>();
    for (const x of list) {
      const s = subcategoryOf(x, c);
      if (s && !subs.has(s)) subs.set(s, count(list.filter((y) => subcategoryOf(y, c) === s), c));
    }
    const notes: string[] = [];
    if (c === "Weapons") {
      const k = list.filter(isKitted).length;
      if (k) notes.push(`${k} kitted`);
    }
    if (c === "Armour") {
      const f = armourSets(list).filter(isFull).length;
      if (f) notes.push(`${f} full sets`);
    }
    if (c === "Resources") {
      const ms = materials(list);
      notes.push(`${ms.filter((m) => m.best >= 500).length} of ${ms.length} at Q500+`);
    }
    const crafted = list.filter((x) => x.item.crafted).length;
    if (crafted) notes.push(`${crafted} crafted`);
    return { n: count(list, c), subs: [...subs.entries()].sort((a, b) => b[1] - a[1]).slice(0, 4), notes };
  }

  const isOn = (nodes: Node[]) => !!selected && nodes.includes(selected);
  const folderHeading = $derived(
    view.type === "place" && view.level === "root"
      ? "Places"
      : view.type === "place" && view.level === "body"
        ? "Locations"
        : view.type === "place" && view.level === "station"
          ? "Areas"
          : folders.some((f) => f.type === "ship")
            ? "Ships & containers"
            : "Containers",
  );
</script>

{#snippet cards(list: ItemNode[], c: Category)}
  <div class="grid" class:wide={c === "Weapons"}>
    {#if c === "Resources"}
      {#each materials(list).sort((a, b) => b.best - a.best) as m (m.name)}
        <ResourceCard {m} on={isOn(m.nodes)} onclick={() => onselect(m.nodes[0])} />
      {/each}
    {:else}
      {#each stackIdentical(list) as s (s.key)}
        <ItemCard stack={s} category={c} on={isOn(s.nodes)} onclick={() => onselect(s.node)} />
      {/each}
    {/if}
  </div>
{/snippet}

{#if folders.length}
  <SectionHead title={folderHeading} n={folders.length} />
  <div class="tiles">
    {#each folders as f (f.id)}
      <FolderTile node={f} bricked={showBricked} on={selected === f} onclick={() => onopen(f)} />
    {/each}
  </div>
{/if}

{#if cats.length && (cats.length > 1 || big)}
  <div class="cat-bar">
    <ChipButton on={!cat} n={sum(items)} onclick={() => pick("")}>All</ChipButton>
    {#each cats as c (c)}
      {@const Icon = CATEGORY_ICON[c]}
      <ChipButton on={cat === c} n={count(byCat.get(c) ?? [], c)} onclick={() => pick(c)}><Icon size={12} />{c}</ChipButton>
    {/each}
  </div>
{/if}

{#if !cats.length}
  {#if !folders.length}<p class="empty">Empty.</p>{/if}
{:else if !cat && big}
  <div class="overview">
    {#each cats as c (c)}
      {@const o = overview(c)}
      {@const Icon = CATEGORY_ICON[c]}
      <button type="button" class="ov" onclick={() => pick(c)}>
        <span class="ov-top"><Icon size={16} /><span class="ov-name">{c}</span><span class="ov-n">{fmtN(o.n)}</span></span>
        {#if o.subs.length}
          <span class="ov-subs">
            {#each o.subs as [s, k] (s)}<span><span>{s}</span><b>{fmtN(k)}</b></span>{/each}
          </span>
        {/if}
        {#if o.notes.length}<span class="ov-note">{o.notes.join(" · ")}</span>{/if}
      </button>
    {/each}
  </div>
{:else if !cat}
  {#each cats as c (c)}
    <SectionHead title={c} n={count(byCat.get(c) ?? [], c)} />
    {@render cards(byCat.get(c) ?? [], c)}
  {/each}
{:else}
  <div class="sub-bar">
    {#if order.length}<ChipButton on={!filters.sub} onclick={() => onfilter({ sub: "" })}>All</ChipButton>{/if}
    {#each order.filter((s) => subCount(s) > 0) as s (s)}
      <ChipButton on={filters.sub === s} n={subCount(s)} onclick={() => onfilter({ sub: filters.sub === s ? "" : s })}
        >{s}</ChipButton
      >
    {/each}
    {#if weights.length}<span class="sep"></span>{/if}
    {#each weights as w (w)}
      <ChipButton on={filters.w === w} onclick={() => onfilter({ w: filters.w === w ? "" : w })}
        >{w === "Other" ? "Flight & other" : w}</ChipButton
      >
    {/each}
    {#if cat === "Armour"}
      <span class="sep"></span>
      <ChipButton on={filters.arm !== "pieces"} onclick={() => onfilter({ arm: "" })}>Sets</ChipButton>
      <ChipButton on={filters.arm === "pieces"} onclick={() => onfilter({ arm: "pieces" })}>Pieces</ChipButton>
    {/if}
    {#if cat === "Weapons"}
      <span class="sep"></span>
      <ChipButton on={!!filters.kit} n={all.filter(isKitted).length} onclick={() => onfilter({ kit: filters.kit ? "" : "1" })}
        >Kitted</ChipButton
      >
    {/if}
  </div>
  {#if !narrowed.length}
    <p class="empty">Nothing here.</p>
  {:else if showSets}
    {#each setGroups as g (g.w)}
      <SectionHead title={g.w === "Other" ? "Flight, racing & other" : `${g.w} armour`} n={g.sets.length} />
      <div class="grid">
        {#each g.sets as s (s.name)}
          <SetCard
            set={s}
            on={isOn(s.items)}
            onclick={() => onselect(s.items.find((n) => n.item.kind.item_type === "Char_Armor_Torso") ?? s.items[0])}
          />
        {/each}
      </div>
    {/each}
  {:else}
    {#each sections as g (g.s)}
      {#if g.s}<SectionHead title={g.s} n={count(g.list, cat)} />{/if}
      {@render cards(g.list, cat)}
    {/each}
  {/if}
{/if}

<style>
  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(176px, 1fr));
    gap: 6px;
    margin-top: 4px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 6px;
    margin-top: 6px;
  }
  .grid.wide {
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
  }
  .cat-bar {
    position: sticky;
    top: 0;
    z-index: 2;
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 10px;
    padding: 8px 0;
    border-bottom: 1px solid var(--border-soft);
    background: var(--bg);
  }
  .sub-bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    padding: 8px 0 2px;
  }
  .sep {
    width: 1px;
    height: 18px;
    margin: 0 4px;
    background: var(--border);
  }
  .overview {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(190px, 1fr));
    gap: 8px;
    margin-top: 10px;
  }
  .ov {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-radius: 9px;
    background: color-mix(in srgb, var(--bg-raised) 40%, transparent);
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .ov:hover {
    background: var(--bg-raised);
    border-color: var(--accent-border);
  }
  .ov-top {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-dim);
  }
  .ov-name {
    flex: 1;
    font-family: var(--font-display);
    font-weight: 700;
    font-size: 0.95rem;
    color: var(--text-strong);
  }
  .ov-n {
    font-family: var(--font-mono);
    font-size: 0.9rem;
    color: var(--accent);
  }
  .ov-subs {
    display: flex;
    flex-direction: column;
    gap: 1px;
    font-size: 0.72rem;
    color: var(--text-dim);
  }
  .ov-subs > span {
    display: flex;
  }
  .ov-subs b {
    margin-left: auto;
    font-weight: 400;
    font-family: var(--font-mono);
    color: var(--text);
  }
  .ov-note {
    font-size: 0.7rem;
    color: var(--accent);
  }
  .empty {
    padding: 14px 2px;
    font-size: 0.84rem;
    color: var(--text-dim);
  }
</style>
