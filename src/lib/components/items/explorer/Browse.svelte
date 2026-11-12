<script lang="ts">
  // One container's contents (the home storage every patch refills):
  // sub-containers first, then the in-game categories. A big container opens
  // on an overview card per category. Items list as rows (UX board sketch
  // 16): one row per model with the variants you hold named in it, what you
  // crafted or kitted first with its details (model/rows.ts). An open kind
  // has its sub kinds and toggles in a rail beside the rows that never moves;
  // an option with nothing in it dims, it never disappears. Armour folds into
  // sets (or pieces), resources fold per material. Containers show as tiles
  // only where the place holds no items of its own (a planet, a station).
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
  import { modelRows } from "../model/rows";
  import { isFolder, visible, type ItemNode, type Node } from "../model/tree";
  import FolderTile from "./FolderTile.svelte";
  import ItemRow from "./ItemRow.svelte";
  import ResourceCard from "./ResourceCard.svelte";
  import SectionHead from "./SectionHead.svelte";
  import SetRow from "./SetRow.svelte";
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
  /** Up to this many units (containers' contents included) a place shows
   *  everything inside it on one page: its loose items, then a section per
   *  container — a hangar's racks and cases without navigating into each. */
  const SMALL = 200;
  const WEIGHTS: Weight[] = ["Light", "Medium", "Heavy", "Other"];
  const WEIGHT_GROUPS: Weight[] = ["Heavy", "Medium", "Light", "Other"];

  const kids = $derived(view.kids.filter((k) => visible(k, showBricked)));
  const folders = $derived(kids.filter(isFolder));
  const loose = $derived(kids.filter((k): k is ItemNode => k.type === "item" && !isFolder(k) && keep(k)));
  /** Everything below a container, through nested containers (not into
   *  ships, not into a weapon's attachments). A container that is itself an
   *  item (a suit holding a backpack) counts as one too. */
  function below(n: Node): ItemNode[] {
    const out: ItemNode[] = [];
    const walk = (x: Node) => {
      for (const k of x.kids) {
        if (!visible(k, showBricked) || k.type === "ship") continue;
        if (k.type === "item" && keep(k)) out.push(k);
        if (isFolder(k)) walk(k);
      }
    };
    walk(n);
    return out;
  }
  const containers = $derived(folders.filter((f) => f.type !== "ship"));
  const ships = $derived(folders.filter((f) => f.type === "ship"));
  const nested = $derived(containers.map((c) => ({ c, list: below(c) })).filter((g) => g.list.length));
  const small = $derived(
    nested.length > 0 && loose.reduce((s, x) => s + x.item.stack, 0) + nested.reduce((s, g) => s + g.list.reduce((t, x) => t + x.item.stack, 0), 0) <= SMALL,
  );
  /** What the categories count: the loose items, plus the containers'
   *  contents when the place is small enough to show them all. */
  const items = $derived(small ? [...loose, ...nested.flatMap((g) => g.list)] : loose);
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
  const big = $derived(!small && sum(items) > BIG);
  const looseCats = $derived(CATEGORIES.filter((c) => loose.some((x) => categoryOf(x) === c)));
  const pick = (c: string) => onfilter({ cat: c, sub: "", w: "", kit: "" });

  // The open category, narrowed.
  const all = $derived(cat ? (byCat.get(cat) ?? []) : []);
  const order = $derived(cat ? (SUBCATEGORIES[cat] ?? []) : []);
  const weightCount = (w: Weight) =>
    sum(all.filter((x) => weightOf(x) === w && (!filters.sub || subcategoryOf(x, cat as Category) === filters.sub)));
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
    // Crafted, full sets and kitted weapons are the gear shelves above the
    // overview now; only what they don't say stays a note.
    const notes: string[] = [];
    if (c === "Resources") {
      const ms = materials(list);
      notes.push(`${ms.filter((m) => m.best >= 500).length} of ${ms.length} at Q500+`);
    }
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

{#snippet rows(list: ItemNode[], c: Category | null)}
  {#if c === "Resources"}
    <div class="grid">
      {#each materials(list).sort((a, b) => b.best - a.best) as m (m.name)}
        <ResourceCard {m} on={isOn(m.nodes)} onclick={() => onselect(m.nodes[0])} />
      {/each}
    </div>
  {:else}
    <div class="rows">
      {#each modelRows(list) as r (r.key)}
        <ItemRow row={r} category={categoryOf(r.node)} on={isOn(r.nodes)} onclick={() => onselect(r.node)} />
      {/each}
    </div>
  {/if}
{/snippet}

{#snippet railRow(label: string, n: number, on: boolean, onclick: () => void)}
  <button type="button" class="rail-row" class:on class:off={!n} disabled={!n && !on} aria-pressed={on} {onclick}
    ><span>{label}</span><b>{fmtN(n)}</b></button
  >
{/snippet}

{#if folders.length && !items.length}
  <SectionHead title={folderHeading} n={folders.length} />
  <div class="tiles">
    {#each folders as f (f.id)}
      <FolderTile node={f} bricked={showBricked} on={selected === f} onclick={() => onopen(f)} />
    {/each}
  </div>
{:else if ships.length && !cat}
  <SectionHead title="Ships" n={ships.length} />
  <div class="tiles">
    {#each ships as f (f.id)}
      <FolderTile node={f} bricked={showBricked} on={selected === f} onclick={() => onopen(f)} />
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
{:else if !cat && small}
  {#each looseCats as c (c)}
    {@const list = loose.filter((x) => categoryOf(x) === c)}
    <SectionHead title={c} n={count(list, c)} />
    {@render rows(list, c)}
  {/each}
  {#each nested as g (g.c.id)}
    <SectionHead title={g.c.name} n={sum(g.list)} onclick={() => onopen(g.c)} />
    {@render rows(g.list, null)}
  {/each}
{:else if !cat}
  {#each cats as c (c)}
    <SectionHead title={c} n={count(byCat.get(c) ?? [], c)} />
    {@render rows(byCat.get(c) ?? [], c)}
  {/each}
{:else}
  <div class="kind">
    <div class="kind-main">
      {#if !narrowed.length}
        <p class="empty">Nothing here.</p>
      {:else if showSets}
        {#each setGroups as g (g.w)}
          <SectionHead title={g.w === "Other" ? "Flight, racing & other" : `${g.w} armour`} n={g.sets.length} />
          <div class="rows">
            {#each g.sets as s (s.key)}
              <SetRow
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
          {@render rows(g.list, cat)}
        {/each}
      {/if}
    </div>
    <!-- The kind's own filters, beside the rows and never moving: every
         option stays, an empty one dims. -->
    <aside class="rail" aria-label="{cat} filters">
      {#if order.length}
        {@render railRow(
          "All",
          count(all.filter((x) => !filters.w || weightOf(x) === filters.w), cat),
          !filters.sub,
          () => onfilter({ sub: "" }),
        )}
        {#each order as s (s)}
          {@render railRow(s, subCount(s), filters.sub === s, () => onfilter({ sub: filters.sub === s ? "" : s }))}
        {/each}
      {/if}
      {#if cat === "Armour"}
        <span class="rail-sep"></span>
        {#each WEIGHTS as w (w)}
          {@render railRow(w === "Other" ? "Flight & other" : w, weightCount(w), filters.w === w, () =>
            onfilter({ w: filters.w === w ? "" : w }),
          )}
        {/each}
        <span class="rail-sep"></span>
        {@render railRow("As sets", armourSets(narrowed).length, filters.arm !== "pieces", () => onfilter({ arm: "" }))}
        {@render railRow("As pieces", sum(narrowed), filters.arm === "pieces", () => onfilter({ arm: "pieces" }))}
      {/if}
      {#if cat === "Weapons"}
        <span class="rail-sep"></span>
        {@render railRow("Kitted", all.filter(isKitted).length, !!filters.kit, () => onfilter({ kit: filters.kit ? "" : "1" }))}
      {/if}
    </aside>
  </div>
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
  .rows {
    display: flex;
    flex-direction: column;
    margin-top: 2px;
  }
  .kind {
    display: flex;
    gap: 20px;
    align-items: flex-start;
  }
  .kind-main {
    flex: 1;
    min-width: 0;
  }
  .rail {
    position: sticky;
    top: 0;
    flex: none;
    width: 170px;
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding-top: 12px;
  }
  .rail-row {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    padding: 4px 10px;
    border: 0;
    border-left: 2px solid var(--border);
    background: transparent;
    color: var(--text-dim);
    font: inherit;
    font-size: var(--text-control);
    text-align: left;
    cursor: pointer;
  }
  .rail-row:hover {
    color: var(--text);
    background: color-mix(in srgb, var(--text) 4%, transparent);
  }
  .rail-row.on {
    border-left-color: var(--accent);
    color: var(--text-strong);
  }
  .rail-row.off {
    opacity: 0.4;
    cursor: default;
  }
  .rail-row b {
    font-family: var(--font-mono);
    font-size: var(--text-small);
    font-weight: 400;
    color: var(--text-faint);
  }
  .rail-sep {
    height: 1px;
    margin: 8px 0;
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
    color: var(--text-strong);
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
    color: var(--text-faint);
  }
  .empty {
    padding: 14px 2px;
    font-size: 0.84rem;
    color: var(--text-dim);
  }
</style>
