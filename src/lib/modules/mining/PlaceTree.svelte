<script lang="ts">
  // Places as a tree: each system's planets with their moons and Lagrange
  // points (design pass 4: the game names a point by its planet, HUR L1,
  // and the data parents it to the star, so the backend puts it back), each
  // point with its rest stop; the fields, which stand for many places at
  // once, beside them. With a loadout every place carries the ship's score
  // there. A planet without ship rocks of its own is a plain parent. A
  // search (a place, a rest stop or the game's code, PYR3 L3) shows the
  // matches with their parents, opened.
  import { untrack } from "svelte";
  import { createExpansion } from "@veelume/ui";
  import type { PlaceNodeView, PlaceSystemView } from "$lib/bindings";
  import Chevron from "$lib/components/Chevron.svelte";

  let {
    systems,
    selected,
    score = () => null,
    onselect,
  }: {
    systems: PlaceSystemView[];
    selected: string;
    /** The ship's good share at a place (0-100), when known. */
    score?: (node: PlaceNodeView) => number | null;
    onselect: (node: PlaceNodeView) => void;
  } = $props();

  let q = $state("");
  const open = createExpansion("many");

  const query = $derived(q.trim().toLowerCase());
  const matches = (n: PlaceNodeView) =>
    n.name.toLowerCase().includes(query) || (n.restStop ?? "").toLowerCase().includes(query);
  function keep(n: PlaceNodeView): PlaceNodeView | null {
    const children = n.children.map(keep).filter((c): c is PlaceNodeView => c !== null);
    if (matches(n)) return n;
    return children.length ? { ...n, children } : null;
  }
  const shown = $derived(
    query
      ? systems
          .map((s) => ({ ...s, nodes: s.nodes.map(keep).filter((n): n is PlaceNodeView => n !== null) }))
          .filter((s) => s.nodes.length)
      : systems,
  );

  /** `HUR L1` → `L1`. */
  const short = (name: string) => name.split(" ").pop() ?? name;
  /** `HUR-L1 Green Glade Station` → `Green Glade Station`: the point is the row. */
  const stop = (name: string) => name.replace(/^[A-Z]{3}-L\d\s+/, "");

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
    untrack(() => ids.forEach((id) => open.open(id)));
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
  {@const s = n.areas.length ? score(n) : null}
  <div class="node" class:kid={depth > 0}>
    {#if n.children.length}
      <button class="car" onclick={() => open.toggle(n.id)} aria-label={expanded ? "Close" : "Open"}
        ><Chevron dir="right" open={expanded} size={13} /></button
      >
    {:else}
      <span class="car"></span>
    {/if}
    <button class="pl" class:on={n.id === selected} class:parent={!n.areas.length} onclick={() => click(n)}>
      {#if n.kind === "lpoint"}
        <span class="nm"><span class="lp">{short(n.name)}</span>{#if n.restStop}<small>{stop(n.restStop)}</small>{/if}</span>
      {:else}
        <span class="nm">{n.name}</span>
      {/if}
      {#if s !== null}
        <span class="sc" class:w={s < 65} class:b={s < 40}>{s}</span>
      {:else if n.kind === "field" && n.count > 0}
        <span class="c">{n.count}</span>
      {:else if n.children.length && !expanded}
        <span class="c">{n.children.length}</span>
      {/if}
    </button>
  </div>
  {#if expanded}
    <div class="kids">
      {#each n.children as c, i (c.id)}
        {#if c.kind === "lpoint" && n.children[i - 1]?.kind !== "lpoint"}<p class="lh">Lagrange points</p>{/if}
        {@render node(c, depth + 1)}
      {/each}
    </div>
  {/if}
{/snippet}

<input
  class="input search"
  type="search"
  placeholder="Place, rest stop or PYR3 L3"
  aria-label="Find a place"
  bind:value={q}
/>
<div class="tree">
  {#each shown as s (s.name)}
    <div class="grp">{s.name}</div>
    {#each s.nodes as n (n.id)}{@render node(n, 0)}{/each}
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
    font-family: var(--font-mono);
    font-size: 0.7rem;
    letter-spacing: var(--label-tracking);
    text-transform: var(--label-transform);
    color: var(--text-faint);
    padding: 12px 8px 4px;
  }
  .lh {
    margin: 6px 0 2px;
    padding-left: 22px;
    font-family: var(--font-mono);
    font-size: 0.62rem;
    letter-spacing: var(--label-tracking);
    text-transform: var(--label-transform);
    color: var(--text-faint);
  }
  .node {
    display: grid;
    grid-template-columns: 14px minmax(0, 1fr);
    gap: 4px;
    align-items: center;
  }
  .car {
    display: inline-flex;
    justify-content: center;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-faint);
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
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .lp {
    font-family: var(--font-mono);
    font-size: 0.8rem;
  }
  .nm small {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .c,
  .sc {
    font-family: var(--font-mono);
    font-size: 0.72rem;
    color: var(--text-faint);
  }
  .sc {
    font-weight: var(--weight-semibold);
    color: var(--good);
  }
  .sc.w {
    color: var(--warn);
  }
  .sc.b {
    color: var(--bad);
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
