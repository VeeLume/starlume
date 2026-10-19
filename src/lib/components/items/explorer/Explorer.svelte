<script lang="ts">
  // The explorer, three views over one tree: Items (containers browse by
  // category), Ships (where every ship is; a ship opens on its loadout) and
  // Resources (every material below the place, by quality).
  //
  // Layout (design pass, 2026-10-15): one wide pane, two at most. The
  // breadcrumb navigates — each crumb's dropdown lists its neighbours, the
  // trailing "Inside" what the place holds, and Places opens the whole tree
  // as a popover. The place name, filters and category bar span the full
  // width and never move; the item sheet (Detail) opens beside the cards
  // only while something is picked, as tall as they are (✕ or Esc).
  import { untrack } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import { Button, Popup } from "@veelume/ui";
  import { Globe, X } from "lucide-svelte";
  import Chevron from "$lib/components/Chevron.svelte";
  import { categoryOf } from "../model/classify";
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
  import ItemCard from "./ItemCard.svelte";
  import ResourceList from "./ResourceList.svelte";
  import SectionHead from "./SectionHead.svelte";
  import ShipView from "./ShipView.svelte";
  import TreeNode from "./TreeNode.svelte";
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

  // The navigation popovers: Places (the tree), or one crumb's menu.
  let placesOpen = $state(false);
  let menuFor = $state<string | null>(null);

  // ── What counts ──────────────────────────────────────────────────────────
  const matches = (n: Node) =>
    n.type !== "place" && (n.name.toLowerCase().includes(q) || (n.type === "item" && categoryOf(n).toLowerCase().includes(q)));
  const keepItem = (x: ItemNode) => visible(x, bricked) && (!crafted || !!x.item.crafted) && (!q || matches(x));
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
  /** Open a place (and select something in it). A container keeps no
   *  filters from the last one. Opening a ship selects it (its sheet is
   *  the ship's); a place opens with nothing picked, the sheet closed. */
  function go(folder: Node, item?: Node) {
    placesOpen = false;
    menuFor = null;
    const sel = item ?? (folder.type === "ship" ? folder : null);
    browse.setMany({ at: folder.id, sel: sel?.id ?? "", cat: "", sub: "", w: "", kit: "" });
  }
  const select = (n: Node) => browse.set("sel", n.id);
  const closeSheet = () => browse.set("sel", "");
  function onkeydown(e: KeyboardEvent) {
    if (e.key !== "Escape" || placesOpen || menuFor) return;
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
  const filtering = $derived(!!q || crafted);
  const viewNode = $derived(soleArea(at) ?? at);
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
  const crumbs = $derived(ancestors(at));
  /** A crumb's dropdown: its neighbours (the parent's places). */
  const menuOf = (k: number) => (k > 0 ? kidsOf(crumbs[k - 1]) : []);
  const inside = $derived(kidsOf(at));
  const summary = $derived(
    shipsMode
      ? `${fmtN(count(at))} ${count(at) === 1 ? "ship" : "ships"}`
      : resMode
        ? `${fmtN(count(at))} ${count(at) === 1 ? "material" : "materials"}`
        : `${fmtN(count(at))} ${count(at) === 1 ? "item" : "items"}`,
  );
  const browsing = $derived(!resMode && !shipsMode && at.type !== "ship" && !filtering);
  const craftedCount = $derived(units(tree.root, (x) => visible(x, bricked) && !!x.item.crafted));
  const placeholder = $derived(shipsMode ? "Find a ship…" : resMode ? "Find a material…" : "Find an item, anywhere…");
</script>

<svelte:window {onkeydown} />

{#snippet placeMenu(nodes: Node[], label: string)}
  <div class="menu" role="menu" aria-label={label}>
    {#each nodes as n (n.id)}
      {@const Icon = nodeIcon(n)}
      <button type="button" class="menu-row" class:cur={n === at} role="menuitem" onclick={() => go(n)}>
        <Icon size={14} /><span class="menu-name">{n.name}</span><span class="menu-n">{fmtN(count(n))}</span>
      </button>
    {/each}
  </div>
{/snippet}

<div class="explorer">
  <div class="navrow">
    <div class="anchor">
      <Button variant="outline" onclick={() => ((placesOpen = !placesOpen), (menuFor = null))}>
        <Globe size={14} /> Places
      </Button>
      <Popup open={placesOpen} onclose={() => (placesOpen = false)} label="Places" class="places-pop">
        <div role="tree">
          <TreeNode node={tree.root} depth={0} {open} forceOpen={!!q} {isAt} {count} {kidsOf} ongo={(n) => go(n)} />
        </div>
      </Popup>
    </div>
    <nav class="crumbs" aria-label="Where">
      {#each crumbs as c, k (c.id)}
        {#if k > 0}<span class="sep">›</span>{/if}
        {@const menu = menuOf(k)}
        <span class="anchor crumb-wrap">
          {#if menu.length > 1}
            <!-- The whole crumb opens the places beside it (style guide: the
                 trigger is the hit area, never a lone chevron); the place
                 itself is the menu's first row, so going there stays one click. -->
            <button
              type="button"
              class="crumb"
              class:cur={c === at}
              class:open={menuFor === c.id}
              aria-haspopup="menu"
              aria-expanded={menuFor === c.id}
              title="Places beside {c.name}"
              onclick={() => ((menuFor = menuFor === c.id ? null : c.id), (placesOpen = false))}
            >
              {c.name}<Chevron open={menuFor === c.id} size={13} />
            </button>
            <Popup open={menuFor === c.id} onclose={() => (menuFor = null)} label="Beside {c.name}" class="menu-pop">
              {@render placeMenu([c, ...menu.filter((n) => n !== c)], `Beside ${c.name}`)}
            </Popup>
          {:else}
            <button type="button" class="crumb" class:cur={c === at} onclick={() => go(c)}>{c.name}</button>
          {/if}
        </span>
      {/each}
      {#if inside.length}
        <span class="sep">›</span>
        <span class="anchor crumb-wrap">
          <button
            type="button"
            class="crumb inside"
            onclick={() => ((menuFor = menuFor === "inside" ? null : "inside"), (placesOpen = false))}
          >
            Inside <Chevron open={menuFor === "inside"} size={13} />
          </button>
          <Popup open={menuFor === "inside"} onclose={() => (menuFor = null)} label="Inside {at.name}" class="menu-pop">
            {@render placeMenu(inside, `Inside ${at.name}`)}
          </Popup>
        </span>
      {/if}
    </nav>
    <span class="grow"></span>
    {#if view === "items"}
      <ChipButton on={crafted} n={craftedCount} onclick={() => browse.setMany({ crafted: crafted ? "" : "1", sel: "" })}>Crafted</ChipButton>
    {/if}
    {#if !resMode}
      <ChipButton on={bricked} onclick={() => browse.set("bricked", bricked ? "" : "1")}>Show bricked</ChipButton>
    {/if}
  </div>

  <div class="title-row">
    <h2>{at.name}</h2>
    <span class="summary">{summary}</span>
  </div>

  {#if browsing}
    {#key viewNode.id}
      <Browse
        part="bar"
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
          {@const stacks = stackIdentical(g.list)}
          <SectionHead title={pathLabel(g.c)} n={stacks.reduce((s, t) => s + t.n, 0)} onclick={() => go(g.c)}>
            {#snippet lead()}<Icon size={13} />{/snippet}
          </SectionHead>
          <div class="grid">
            {#each stacks as s (s.key)}
              <ItemCard stack={s} category={categoryOf(s.node)} on={!!shown && s.nodes.includes(shown as ItemNode)} onclick={() => select(s.node)} />
            {/each}
          </div>
        {/each}
      {:else}
        {#key viewNode.id}
          <Browse
            part="body"
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
  .anchor {
    position: relative;
  }
  .anchor :global(.places-pop) {
    width: 300px;
    max-height: 60vh;
    overflow-y: auto;
    padding: 6px;
    border-radius: var(--radius-lg);
    background: var(--bg-raised);
    border: 1px solid var(--border);
    box-shadow: var(--shadow-panel);
  }
  .anchor :global(.menu-pop) {
    min-width: 240px;
    max-height: 50vh;
    overflow-y: auto;
    padding: 4px;
    border-radius: var(--radius-lg);
    background: var(--bg-raised);
    border: 1px solid var(--border);
    box-shadow: var(--shadow-panel);
  }
  .crumbs {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 2px;
    min-width: 0;
  }
  .crumb-wrap {
    display: inline-flex;
    align-items: center;
  }
  .crumb {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 4px 6px 4px 8px;
    border: 0;
    border-radius: var(--radius);
    background: transparent;
    color: var(--text-dim);
    font: inherit;
    font-size: var(--text-control);
    cursor: pointer;
  }
  .crumb :global(.chev) {
    color: var(--text-faint);
  }
  .crumb:hover,
  .crumb.open {
    background: color-mix(in srgb, var(--text) 5%, transparent);
    color: var(--text);
  }
  .crumb.open :global(.chev) {
    color: var(--accent);
  }
  .crumb.cur {
    color: var(--text-strong);
    font-weight: var(--weight-semibold);
  }
  .crumb.inside {
    color: var(--text-faint);
  }
  .sep {
    color: var(--text-faint);
    padding: 0 2px;
  }
  .grow {
    flex: 1;
  }
  .menu {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .menu-row {
    display: grid;
    grid-template-columns: 14px minmax(0, 1fr) auto;
    gap: 8px;
    align-items: center;
    padding: 6px 8px;
    border: 0;
    border-radius: var(--radius);
    background: transparent;
    color: var(--text-dim);
    font: inherit;
    font-size: var(--text-control);
    text-align: left;
    cursor: pointer;
  }
  .menu-row:hover {
    background: color-mix(in srgb, var(--text) 5%, transparent);
    color: var(--text);
  }
  .menu-row.cur {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    color: var(--text-strong);
  }
  .menu-name {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .menu-n {
    font-family: var(--font-mono);
    font-size: var(--text-small);
    color: var(--text-faint);
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
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 8px;
    margin-top: 6px;
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
