<script lang="ts">
  // The explorer, three views over one tree: Items (containers browse by
  // category), Ships (where every ship is; a ship opens on its loadout) and
  // Resources (every material below the place, by quality).
  //
  // Layout (design passes 2026-10-15 and 2026-11-11): one wide pane, two at
  // most. One field is the scope line (ScopeField): the place, the kind and
  // a sub kind as words, then free text; it replaced the breadcrumb, the
  // Places button and the category bar. Every place leads with your gear
  // (Gear). The item sheet (Detail) opens beside the cards only while
  // something is picked, as tall as they are (✕ or Esc).
  import { untrack } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import { X } from "lucide-svelte";
  import { categoryOf, subcategoryOf, type Category } from "../model/classify";
  import { itemsBelow } from "../model/gear";
  import { materials } from "../model/resources";
  import { stackIdentical } from "../model/stack";
  import {
    ancestors,
    areaOf,
    container,
    isFolder,
    pathLabel,
    shipsIn,
    stowed,
    units,
    visible,
    type ItemNode,
    type Node,
    type PlaceNode,
    type ShipNode,
    type Tree,
  } from "../model/tree";
  import type { ItemsBrowse } from "./browse";
  import Browse from "./Browse.svelte";
  import ChipButton from "./ChipButton.svelte";
  import Detail from "./Detail.svelte";
  import FolderTile from "./FolderTile.svelte";
  import Gear from "./Gear.svelte";
  import { areasBelow } from "../model/gear";
  import ItemRow from "./ItemRow.svelte";
  import { modelRows } from "../model/rows";
  import ResourceList from "./ResourceList.svelte";
  import SectionHead from "./SectionHead.svelte";
  import ScopeField from "./ScopeField.svelte";
  import ShipView from "./ShipView.svelte";
  import { fmtN, nodeIcon, type View } from "./ui";

  let { tree, browse }: { tree: Tree; browse: ItemsBrowse } = $props();

  const v = $derived(browse.values);
  const view = $derived<View>(v.view === "ships" || v.view === "res" ? v.view : "items");
  const shipsMode = $derived(view === "ships");
  const resMode = $derived(view === "res");
  const at = $derived<Node>(tree.byId.get(v.at) ?? tree.root);
  const sel = $derived<Node | null>(tree.byId.get(v.sel) ?? null);
  const q = $derived(v.q.trim().toLowerCase());
  const crafted = $derived(v.crafted === "1");
  const bricked = $derived(v.bricked === "1");
  const min = $derived(Number(v.min) || 0);

  // Tree branches opened stay open; going somewhere opens its path.
  const open = new SvelteSet<string>();
  $effect(() => {
    const path = ancestors(at).map((a) => a.id);
    untrack(() => path.forEach((id) => open.add(id)));
  });

  // ── What counts ──────────────────────────────────────────────────────────
  const matches = (n: Node) =>
    n.type !== "place" && (n.name.toLowerCase().includes(q) || (n.type === "item" && categoryOf(n).toLowerCase().includes(q)));
  /** The kind word narrows everything; the sub kind word too where the
   *  page is a list of hits (a container's own sub filters do it there, and
   *  must not lose their other options). */
  const cat = $derived(v.cat as Category | "");
  const atFlat = $derived(at.type === "place" && (at.level === "root" || at.level === "system" || at.level === "body"));
  const subInKeep = $derived(!!v.sub && (atFlat || !!q || crafted));
  const baseKeep = (x: ItemNode) => visible(x, bricked) && (!crafted || !!x.item.crafted);
  const keepItem = (x: ItemNode) =>
    baseKeep(x) &&
    (!cat || categoryOf(x) === cat) &&
    (!subInKeep || subcategoryOf(x, cat as Category) === v.sub) &&
    (!q || matches(x));
  const keepShip = (x: ShipNode) => visible(x, bricked) && (!q || matches(x));
  const keepRes = (x: ItemNode) => !!x.item.resource && (!q || matches(x));
  function resBelow(n: Node): ItemNode[] {
    const out: ItemNode[] = [];
    const walk = (x: Node) => {
      if (x.type === "item" && keepRes(x)) out.push(x);
      x.kids.forEach(walk);
    };
    walk(n);
    return out;
  }
  const count = $derived(
    shipsMode
      ? (n: Node) => shipsIn(n, keepShip).filter((s) => s !== n).length
      : resMode
        ? (n: Node) => new Set(resBelow(n).filter((x) => x.item.resource!.quality >= min).map((x) => x.item.resource!.name)).size
        : (n: Node) => units(n, keepItem),
  );

  /** A location or planet whose only visible child is one area (only
   *  Storage, say) skips that level and opens straight on the area. */
  function soleArea(n: Node): PlaceNode | null {
    if (n.type !== "place" || (n.level !== "station" && n.level !== "body")) return null;
    const vis = n.kids.filter((k) => count(k) > 0);
    return vis.length === 1 && vis[0].type === "place" && vis[0].level === "area" ? vis[0] : null;
  }
  const insideShip = (n: Node) => ancestors(n).some((a) => a.type === "ship");
  function kidsOf(n: Node): Node[] {
    const ks = n === tree.root && shipsMode ? [...tree.root.kids, tree.away, tree.unspawned] : (soleArea(n) ?? n).kids;
    return ks.filter((k) => {
      if (!isFolder(k)) return false;
      if (shipsMode) return k.type === "ship" ? keepShip(k) || shipsIn(k, keepShip).length > 0 : n.type !== "ship" && count(k) > 0;
      if (k.type === "place" && k.level === "group") return false;
      // Inside a ship the tree shows what holds stowed items (lockers,
      // cargo) and carried ships; mounts and turrets live in the loadout.
      if (!resMode && insideShip(k) && k.type !== "ship") return stowed(k, keepItem).length > 0;
      return count(k) > 0;
    });
  }
  const isAt = (n: Node) => at === n || soleArea(n) === at;

  // ── Navigation ───────────────────────────────────────────────────────────
  /** Open a place (and select something in it). The kind words in the
   *  field stay (they are visible, so going elsewhere with "Weapons" in it
   *  shows that place's weapons); a container's own toggles reset. Opening
   *  a ship selects it (its sheet is the ship's); a place opens with
   *  nothing picked, the sheet closed. */
  function go(folder: Node, item?: Node, extra: Record<string, string> = {}) {
    const sel = item ?? (folder.type === "ship" ? folder : null);
    browse.setMany({ at: folder.id, sel: sel?.id ?? "", w: "", kit: "", ...extra });
  }
  // The field's picks replace the typed text, in the same URL write.
  const fieldGo = (n: Node) => go(n, undefined, { q: "" });
  /** A thing picked in the field: where it sits, with it selected. */
  const openThing = (n: ItemNode) => go(container(n) ?? tree.root, n, { q: "" });
  const select = (n: Node) => browse.set("sel", n.id);
  const closeSheet = () => browse.set("sel", "");
  function onkeydown(e: KeyboardEvent) {
    if (e.key !== "Escape") return;
    const t = e.target as HTMLElement | null;
    if (t?.closest("input, textarea, select")) return;
    if (sel) closeSheet();
  }
  function setView(next: View) {
    if (next === view) return;
    const leaveGroup = next !== "ships" && ancestors(at).some((a) => a.type === "place" && a.level === "group");
    browse.setMany({ view: next, sel: "", ...(leaveGroup ? { at: "" } : {}) });
  }

  // ── The body ─────────────────────────────────────────────────────────────
  const viewNode = $derived(soleArea(at) ?? at);
  /** A place that holds nothing of its own (Everywhere, a system, a
   *  planet) lists the areas below it, biggest first: the storage that
   *  holds most things is one click away, not four (design pass
   *  2026-11-11). */
  const flat = $derived(
    viewNode.type === "place" && (viewNode.level === "root" || viewNode.level === "system" || viewNode.level === "body"),
  );
  /** Hits instead of browsing: text, Crafted, or a kind across places. */
  const filtering = $derived(!!q || crafted || (!!cat && flat));
  const resList = $derived(resMode ? resBelow(viewNode) : []);
  const resMulti = $derived(new Set(resList.map((x) => areaOf(x)?.id)).size > 1);
  const shipGroups = $derived.by(() => {
    if (!shipsMode || at.type === "ship") return [];
    const pool = at === tree.root ? [tree.root, tree.away, tree.unspawned] : [at];
    const found = pool.flatMap((p) => shipsIn(p, keepShip));
    const m = new Map<string, { c: Node; list: ShipNode[] }>();
    for (const s of found) {
      const c = container(s) ?? tree.root;
      const g = m.get(c.id);
      if (g) g.list.push(s);
      else m.set(c.id, { c, list: [s] });
    }
    const group = (c: Node) => (c.type === "place" && c.level === "group" ? 1 : 0);
    return [...m.values()].sort((a, b) => group(a.c) - group(b.c) || b.list.length - a.list.length);
  });
  const hits = $derived.by(() => {
    if (resMode || shipsMode || at.type === "ship" || !filtering) return [];
    const out: ItemNode[] = [];
    const walk = (x: Node) => {
      if (x.type === "item" && keepItem(x)) out.push(x);
      x.kids.forEach(walk);
    };
    at.kids.forEach(walk);
    const m = new Map<string, { c: Node; list: ItemNode[] }>();
    for (const h of out) {
      const c = container(h) ?? tree.root;
      const g = m.get(c.id);
      if (g) g.list.push(h);
      else m.set(c.id, { c, list: [h] });
    }
    return [...m.values()].sort((a, b) => b.list.length - a.list.length);
  });

  /** What the sheet shows: the selection while it is in view; nothing
   *  (the sheet closed) otherwise. */
  const shown = $derived.by<Node | null>(() => {
    if (!sel) return null;
    if (resMode) {
      return sel.type === "item" && sel.item.resource && resList.some((x) => x.item.resource!.name === sel.item.resource!.name)
        ? sel
        : null;
    }
    if (filtering && !shipsMode && at.type !== "ship") {
      return hits.some((g) => g.list.includes(sel as ItemNode)) ? sel : null;
    }
    return sel;
  });
  const here = $derived(
    shown?.type === "item" && shown.parent
      ? (stackIdentical(shown.parent.kids.filter((k): k is ItemNode => k.type === "item" && k.name === shown.name && visible(k, bricked))).find((s) =>
          s.nodes.includes(shown as ItemNode),
        )?.nodes ?? [])
      : [],
  );
  const summary = $derived(
    shipsMode
      ? `${fmtN(count(at))} ${count(at) === 1 ? "ship" : "ships"}`
      : resMode
        ? `${fmtN(count(at))} ${count(at) === 1 ? "material" : "materials"}`
        : `${fmtN(count(at))} ${count(at) === 1 ? "item" : "items"}`,
  );
  const browsing = $derived(!resMode && !shipsMode && at.type !== "ship" && !filtering);
  const areas = $derived(flat ? areasBelow(viewNode, count) : []);
  const AREAS_FIRST = 10;
  let areasOpen = $state(false);
  /** "New Babbage · microTech": where an area is, when areas from several
   *  locations sit side by side. */
  function areaSub(a: Node): string | undefined {
    const up = ancestors(a)
      .filter((x) => x !== a && !(x.type === "place" && (x.level === "root" || x.level === "system")))
      .map((x) => x.name)
      .reverse();
    return up.length ? up.join(" · ") : undefined;
  }
  const craftedCount = $derived(units(tree.root, (x) => visible(x, bricked) && !!x.item.crafted));

  // What the field counts and offers: the place's items of every kind, the
  // ones within its kind words, every item for hits elsewhere, and the
  // biggest areas anywhere.
  const fieldKinds = $derived(view === "items" ? itemsBelow(viewNode, baseKeep) : []);
  const fieldHere = $derived(
    cat
      ? fieldKinds.filter((x) => categoryOf(x) === cat && (!v.sub || subcategoryOf(x, cat as Category) === v.sub))
      : fieldKinds,
  );
  const fieldAll = $derived(view === "items" ? itemsBelow(tree.root, baseKeep) : []);
  const fieldAreas = $derived(areasBelow(tree.root, count));
</script>

<svelte:window {onkeydown} />

<div class="explorer">
  <div class="navrow">
    <ScopeField
      {tree}
      {view}
      {at}
      q={v.q}
      cat={v.cat}
      sub={v.sub}
      {crafted}
      kindsFrom={fieldKinds}
      here={fieldHere}
      all={fieldAll}
      areas={fieldAreas}
      {craftedCount}
      {open}
      {count}
      {kidsOf}
      {isAt}
      ongo={fieldGo}
      onopen={openThing}
      onkind={(c, s) => browse.setMany({ cat: c, sub: s, w: "", kit: "", sel: "", q: "" })}
      oncrafted={(on) => browse.setMany({ crafted: on ? "1" : "", sel: "", q: "" })}
      onq={(text) => browse.set("q", text)}
    />
    {#if !resMode}
      <ChipButton on={bricked} onclick={() => browse.set("bricked", bricked ? "" : "1")}>Show bricked</ChipButton>
    {/if}
  </div>

  <div class="title-row">
    <h2>{at.name}</h2>
    <span class="summary">{summary}</span>
  </div>

  <div class="body-row" class:with-sheet={!!shown}>
    <section class="content">
      {#if resMode}
        {#if resList.length}
          <ResourceList
            list={resList}
            selected={shown}
            multi={resMulti}
            {min}
            sort={v.sort}
            onselect={select}
            onmin={(m) => browse.set("min", String(m))}
            onsort={(s) => browse.set("sort", s)}
          />
        {:else}
          <p class="empty">No resources here.</p>
        {/if}
      {:else if at.type === "ship"}
        <ShipView ship={at} {tree} showBricked={bricked} selected={sel} onselect={select} ongo={(n) => go(n)} />
      {:else if shipsMode}
        {#if !shipGroups.length}<p class="empty">No ships match.</p>{/if}
        {#each shipGroups as g (g.c.id)}
          {@const Icon = nodeIcon(g.c)}
          <SectionHead title={pathLabel(g.c)} n={g.list.length} onclick={() => go(g.c)}>
            {#snippet lead()}<Icon size={13} />{/snippet}
          </SectionHead>
          <div class="ships">
            {#each g.list as s (s.id)}
              <FolderTile node={s} bricked={bricked} on={sel === s} onclick={() => go(s)} />
            {/each}
          </div>
        {/each}
      {:else if filtering}
        {#if !hits.length}<p class="empty">Nothing here matches.</p>{/if}
        {#each hits as g (g.c.id)}
          {@const Icon = nodeIcon(g.c)}
          <SectionHead title={pathLabel(g.c)} n={g.list.reduce((s, x) => s + x.item.stack, 0)} onclick={() => go(g.c)}>
            {#snippet lead()}<Icon size={13} />{/snippet}
          </SectionHead>
          <div class="rows">
            {#each modelRows(g.list) as r (r.key)}
              <ItemRow
                row={r}
                category={categoryOf(r.node)}
                on={!!shown && r.nodes.includes(shown as ItemNode)}
                onclick={() => select(r.node)}
              />
            {/each}
          </div>
        {/each}
      {:else}
        {#if !v.cat}
          <Gear view={viewNode} keep={keepItem} selected={sel} onselect={select} />
        {/if}
        {#if flat}
          <SectionHead title="Where the rest is" n={areas.length} />
          <div class="tiles">
            {#each areasOpen ? areas : areas.slice(0, AREAS_FIRST) as a (a.area.id)}
              <FolderTile node={a.area} sub={areaSub(a.area)} bricked={bricked} on={false} onclick={() => go(a.area)} />
            {/each}
          </div>
          {#if areas.length > AREAS_FIRST}
            <button type="button" class="more" onclick={() => (areasOpen = !areasOpen)}>
              {areasOpen ? "Show fewer" : `Show all ${fmtN(areas.length)}`}
            </button>
          {/if}
        {:else}
        {#key viewNode.id}
          <Browse
            view={viewNode}
            showBricked={bricked}
            keep={keepItem}
            selected={sel}
            filters={{ cat: v.cat, sub: v.sub, w: v.w, arm: v.arm, kit: v.kit }}
            onfilter={(patch) => browse.setMany(patch)}
            onselect={select}
            onopen={(n) => go(n)}
          />
        {/key}
        {/if}
      {/if}
    </section>
    {#if shown}
      <aside class="sheet" aria-label="Details">
        <button type="button" class="close" title="Close (Esc)" aria-label="Close the details" onclick={closeSheet}>
          <X size={16} />
        </button>
        <Detail node={shown} {tree} showBricked={bricked} {here} ongo={go} />
      </aside>
    {/if}
  </div>
</div>

<style>
  .explorer {
    display: flex;
    flex-direction: column;
    gap: 12px;
    flex: 1;
    min-height: 0;
  }
  .navrow {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .title-row {
    display: flex;
    align-items: baseline;
    gap: 14px;
    flex-wrap: wrap;
  }
  h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.6rem;
    font-weight: var(--weight-bold);
    line-height: 1.1;
    color: var(--text-strong);
  }
  .summary {
    font-size: var(--text-control);
    color: var(--text-dim);
  }
  .body-row {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 16px;
  }
  .body-row.with-sheet {
    grid-template-columns: minmax(0, 1fr) minmax(320px, 400px);
  }
  .content {
    min-width: 0;
    min-height: 0;
    overflow-y: auto;
    padding-right: 4px;
    scrollbar-width: thin;
  }
  .sheet {
    position: relative;
    min-height: 0;
    overflow-y: auto;
    padding: 16px 18px;
    border-radius: var(--radius-lg);
    background: var(--bg-raised);
    border: 1px solid var(--border);
    scrollbar-width: thin;
  }
  .close {
    position: absolute;
    top: 12px;
    right: 12px;
    display: grid;
    place-items: center;
    padding: 4px;
    border: 0;
    border-radius: var(--radius);
    background: transparent;
    color: var(--text-faint);
    cursor: pointer;
  }
  .close:hover {
    color: var(--text);
    background: color-mix(in srgb, var(--text) 6%, transparent);
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(190px, 1fr));
    gap: 6px;
    margin-top: 4px;
  }
  .more {
    margin-top: 6px;
    padding: 2px;
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
  .rows {
    display: flex;
    flex-direction: column;
    margin-top: 2px;
  }
  .ships {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
    gap: 8px;
    margin-top: 6px;
  }
  .empty {
    padding: 18px 2px;
    font-size: 0.85rem;
    color: var(--text-dim);
  }
  @media (max-width: 900px) {
    .body-row.with-sheet {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
