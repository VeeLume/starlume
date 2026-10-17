<script lang="ts">
  // Places as a tree: each system's planets with their moons (the game's
  // parent links), the fields, which stand for many places at once, and the
  // system's other places — its Lagrange points, which hang off the star —
  // folded into one group. A planet without ship rocks of its own is a
  // plain parent. A search shows the matches with their parents, opened.
  import { untrack } from "svelte";
  import { createExpansion } from "@veelume/ui";
  import type { PlaceNodeView, PlaceSystemView } from "$lib/bindings";

  let {
    systems,
    selected,
    onselect,
  }: { systems: PlaceSystemView[]; selected: string; onselect: (node: PlaceNodeView) => void } = $props();

  let q = $state("");
  const open = createExpansion("many");

  const query = $derived(q.trim().toLowerCase());
  function keep(n: PlaceNodeView): PlaceNodeView | null {
    const children = n.children.map(keep).filter((c): c is PlaceNodeView => c !== null);
    if (n.name.toLowerCase().includes(query)) return n;
    return children.length ? { ...n, children } : null;
  }
  const shown = $derived(
    query
      ? systems
          .map((s) => ({ ...s, nodes: s.nodes.map(keep).filter((n): n is PlaceNodeView => n !== null) }))
          .filter((s) => s.nodes.length)
      : systems,
  );

  // A system's places outside any planet (no children, not a field) are its
  // Lagrange points: one group instead of a long flat list.
  const loose = (n: PlaceNodeView) => n.kind === "place" && n.children.length === 0;
  const groupKey = (system: string) => `lagrange:${system}`;

  // The selected place's parents open, so it shows.
  $effect(() => {
    const path = (nodes: PlaceNodeView[], up: string[]): string[] | null => {
      for (const n of nodes) {
        if (n.id === selected) return up;
        const p = path(n.children, [...up, n.id]);
        if (p) return p;
      }
      return null;
    };
    const ids = systems.flatMap((s) => path(s.nodes, []) ?? []);
    const groups = systems.filter((s) => s.nodes.some((n) => loose(n) && n.id === selected)).map((s) => groupKey(s.name));
    untrack(() => [...ids, ...groups].forEach((id) => open.open(id)));
  });

  function click(n: PlaceNodeView) {
    if (n.areas.length) {
      onselect(n);
      if (n.children.length) open.open(n.id);
    } else open.toggle(n.id);
  }
</script>

{#snippet node(n: PlaceNodeView, depth: number)}
  {@const expanded = n.children.length > 0 && (!!query || open.has(n.id))}
  <div class="node" class:kid={depth > 0}>
    {#if n.children.length}
      <button class="car" onclick={() => open.toggle(n.id)} aria-label={expanded ? "Close" : "Open"}
        >{expanded ? "▾" : "▸"}</button
      >
    {:else}
      <span class="car"></span>
    {/if}
    <button class="pl" class:on={n.id === selected} class:parent={!n.areas.length} onclick={() => click(n)}>
      <span class="nm">{n.name}</span>
      {#if n.kind === "field" && n.count > 0}
        <span class="c">{n.count}</span>
      {:else if n.children.length && !expanded}
        <span class="c">{n.children.length}</span>
      {/if}
    </button>
  </div>
  {#if expanded}
    <div class="kids">
      {#each n.children as c (c.id)}{@render node(c, depth + 1)}{/each}
    </div>
  {/if}
{/snippet}

<input class="input search" type="search" placeholder="Find a place" aria-label="Find a place" bind:value={q} />
<div class="tree">
  {#each shown as s (s.name)}
    {@const points = s.nodes.filter(loose)}
    {@const grouped = points.length > 1}
    {@const groupOpen = !!query || open.has(groupKey(s.name))}
    <div class="grp">{s.name}</div>
    {#each s.nodes.filter((n) => !grouped || !loose(n)) as n (n.id)}{@render node(n, 0)}{/each}
    {#if grouped}
      <div class="node">
        <button class="car" onclick={() => open.toggle(groupKey(s.name))} aria-label={groupOpen ? "Close" : "Open"}
          >{groupOpen ? "▾" : "▸"}</button
        >
        <button class="pl parent" onclick={() => open.toggle(groupKey(s.name))}>
          <span class="nm">Lagrange points</span>
          {#if !groupOpen}<span class="c">{points.length}</span>{/if}
        </button>
      </div>
      {#if groupOpen}
        <div class="kids">
          {#each points as n (n.id)}{@render node(n, 1)}{/each}
        </div>
      {/if}
    {/if}
  {/each}
  {#if query && !shown.length}<p class="dim none">No place matches.</p>{/if}
</div>

<style>
  .search {
    width: 100%;
  }
  .tree {
    display: flex;
    flex-direction: column;
  }
  .grp {
    font-family: var(--font-display);
    font-size: 0.7rem;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-faint);
    padding: 12px 8px 4px;
  }
  .node {
    display: grid;
    grid-template-columns: 14px minmax(0, 1fr);
    gap: 4px;
    align-items: center;
  }
  .car {
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-faint);
    font-size: 0.7rem;
    cursor: pointer;
  }
  .pl {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
    min-width: 0;
    padding: 5px 8px;
    border: 0;
    border-radius: 6px;
    background: none;
    font: inherit;
    font-size: 0.875rem;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .kid .pl {
    font-size: 0.85rem;
    padding-block: 4px;
    color: var(--text-dim);
  }
  .pl:hover {
    background: color-mix(in srgb, var(--text) 4%, transparent);
  }
  .pl.parent {
    color: var(--text-dim);
  }
  .pl.on {
    background: var(--accent-fill);
    color: var(--text-strong);
  }
  .nm {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .c {
    font-family: var(--font-mono);
    font-size: 0.72rem;
    color: var(--text-faint);
  }
  .kids {
    margin-left: 14px;
    padding-left: 6px;
    border-left: 1px solid var(--border);
    display: flex;
    flex-direction: column;
  }
  .none {
    margin: 8px;
    font-size: var(--text-small);
  }
</style>
