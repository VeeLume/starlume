<script lang="ts">
  // The explorer: the place tree · what the open place holds · a detail
  // pane that never moves. One surface, three views over the same tree:
  // Items (containers browse by category), Ships (where every ship is; a
  // ship opens on its loadout) and Resources (every material below the
  // place, by quality). No inline expanders.
  import { untrack } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import { Surface, storedValue } from "@veelume/ui";
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
  import { nodeIcon, type View } from "./ui";

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

  // The detail pane's docked state and width: per-device preferences.
  const detailOpen = storedValue<boolean>("starlume.items.detailOpen", true, (x): x is boolean => typeof x === "boolean");
  const detailWidth = storedValue<number | null>(
    "starlume.items.detailWidth",
    null,
    (x): x is number | null => x === null || typeof x === "number",
  );

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
   *  filters from the last one. */
  function go(folder: Node, item?: Node) {
    browse.setMany({ at: folder.id, sel: (item ?? folder).id, cat: "", sub: "", w: "", kit: "" });
  }
  const select = (n: Node) => browse.set("sel", n.id);
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

  /** What the detail pane shows: the selection, else the answer the body
   *  leads with (the first search hit, the best material). */
  const shown = $derived.by<Node | null>(() => {
    if (resMode) {
      if (sel?.type === "item" && sel.item.resource && resList.some((x) => x.item.resource!.name === sel.item.resource!.name)) return sel;
      return materials(resList, min).sort((a, b) => b.best - a.best)[0]?.nodes[0] ?? null;
    }
    if (filtering && !shipsMode && at.type !== "ship") {
      const all = hits.flatMap((g) => g.list);
      if (sel && all.includes(sel as ItemNode)) return sel;
      return hits.length ? stackIdentical(hits[0].list)[0].node : null;
    }
    return sel ?? at;
  });
  const here = $derived(
    shown?.type === "item" && shown.parent
      ? (stackIdentical(shown.parent.kids.filter((k): k is ItemNode => k.type === "item" && k.name === shown.name && visible(k, bricked))).find((s) =>
          s.nodes.includes(shown as ItemNode),
        )?.nodes ?? [])
      : [],
  );
  const crumbs = $derived(ancestors(at));
  const craftedCount = $derived(units(tree.root, (x) => visible(x, bricked) && !!x.item.crafted));
  const placeholder = $derived(shipsMode ? "Find a ship…" : resMode ? "Find a material…" : "Find an item, anywhere…");
</script>

<div class="explorer">
  <div class="tools">
    <input
      class="input search"
      type="search"
      value={v.q}
      placeholder={placeholder}
      aria-label={placeholder}
      oninput={(e) => browse.set("q", e.currentTarget.value)}
    />
    {#if view === "items"}
      <ChipButton on={crafted} n={craftedCount} onclick={() => browse.setMany({ crafted: crafted ? "" : "1", sel: "" })}>Crafted</ChipButton>
    {/if}
    {#if !resMode}
      <ChipButton on={bricked} onclick={() => browse.set("bricked", bricked ? "" : "1")}>Show bricked</ChipButton>
    {/if}
  </div>

  <Surface.Supporting
    class="explorer-panes"
    title="Details"
    open={detailOpen.value}
    onopen={(next) => (detailOpen.value = next)}
    width={detailWidth.value}
    onresize={(next) => (detailWidth.value = next)}
  >
    {#snippet main()}
      <div class="main">
        <nav class="tree" aria-label="Places">
          <div role="tree">
            <TreeNode node={tree.root} depth={0} {open} forceOpen={!!q} {isAt} {count} {kidsOf} ongo={(n) => go(n)} />
          </div>
        </nav>
        <section class="content">
          <div class="crumbs">
            {#each crumbs as c, k (c.id)}
              {#if k > 0}<span class="sep">›</span>{/if}
              <button type="button" class="crumb" class:cur={c === at} onclick={() => go(c)}>{c.name}</button>
            {/each}
          </div>
          <div class="body">
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
          </div>
        </section>
      </div>
    {/snippet}
    {#snippet pane()}
      <Detail node={shown} {tree} showBricked={bricked} {here} ongo={go} />
    {/snippet}
  </Surface.Supporting>
</div>

<style>
  .explorer {
    display: flex;
    flex-direction: column;
    gap: 10px;
    flex: 1;
    min-height: 0;
  }
  .tools {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .search {
    flex: 1;
    min-width: 0;
  }
  .explorer :global(.explorer-panes) {
    flex: 1;
    min-height: 0;
  }
  .main {
    display: flex;
    gap: 10px;
    height: 100%;
    min-height: 0;
  }
  .tree {
    flex: 0 0 228px;
    overflow-y: auto;
    padding: 5px;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
  }
  .content {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
  }
  .crumbs {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    min-height: 45px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--border);
    font-size: 0.82rem;
  }
  .crumb {
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--text-dim);
    font: inherit;
    cursor: pointer;
  }
  .crumb:hover {
    color: var(--text);
  }
  .crumb.cur {
    font-family: var(--font-display);
    font-size: 0.95rem;
    font-weight: 700;
    color: var(--text-strong);
  }
  .sep {
    color: var(--text-faint);
  }
  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 0 10px 12px;
  }
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
  .empty {
    padding: 18px 2px;
    font-size: 0.85rem;
    color: var(--text-dim);
  }
</style>
