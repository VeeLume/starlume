<script lang="ts">
  // One branch of the place tree. A branch opened stays open: navigating
  // only ever opens the path to the target; only the caret closes.
  import { Clock, TriangleAlert } from "lucide-svelte";
  import type { SvelteSet } from "svelte/reactivity";
  import { offSaved } from "../model/loadout";
  import type { Node } from "../model/tree";
  import TreeNode from "./TreeNode.svelte";
  import { fmtN, nodeIcon } from "./ui";

  let {
    node,
    depth,
    open,
    forceOpen,
    isAt,
    count,
    kidsOf,
    ongo,
  }: {
    node: Node;
    depth: number;
    open: SvelteSet<string>;
    /** A search is on: every branch with hits shows. */
    forceOpen: boolean;
    isAt: (n: Node) => boolean;
    count: (n: Node) => number;
    kidsOf: (n: Node) => Node[];
    ongo: (n: Node) => void;
  } = $props();

  const root = $derived(node.type === "place" && node.level === "root");
  const kids = $derived(kidsOf(node));
  const expanded = $derived(forceOpen || root || open.has(node.id));
  const c = $derived(count(node));
  const Icon = $derived(nodeIcon(node));
  const warn = $derived(
    node.type === "ship" ? (node.ship.rental_expiry ? "clock" : offSaved(node.ship) ? "off" : null) : null,
  );

  function toggle(e: MouseEvent) {
    e.stopPropagation();
    if (root) return;
    if (open.has(node.id)) open.delete(node.id);
    else open.add(node.id);
  }
</script>

<div
  class="tnode"
  class:on={isAt(node)}
  style:padding-left="{6 + depth * 13}px"
  role="treeitem"
  aria-selected={isAt(node)}
  aria-expanded={kids.length ? expanded : undefined}
  tabindex="-1"
  onclick={() => ongo(node)}
  onkeydown={(e) => e.key === "Enter" && ongo(node)}
>
  <button type="button" class="caret" tabindex="-1" aria-label={expanded ? "Collapse" : "Expand"} onclick={toggle}
    >{kids.length ? (expanded ? "▾" : "▸") : ""}</button
  >
  <Icon size={13} />
  <span class="name">{node.name}</span>
  {#if warn === "clock"}<Clock size={11} class="warn" />{:else if warn === "off"}<TriangleAlert size={11} class="warn" />{/if}
  {#if c > 0 || node.type !== "ship"}<span class="n">{fmtN(c)}</span>{/if}
</div>
{#if expanded}
  {#each kids as k (k.id)}
    <TreeNode node={k} depth={depth + 1} {open} {forceOpen} {isAt} {count} {kidsOf} {ongo} />
  {/each}
{/if}

<style>
  .tnode {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 6px;
    border-radius: 6px;
    font-size: 0.82rem;
    color: var(--text);
    cursor: pointer;
  }
  .tnode:hover {
    background: var(--bg-raised);
  }
  .tnode.on {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    color: var(--text-strong);
  }
  .tnode :global(svg) {
    flex: 0 0 auto;
    opacity: 0.75;
  }
  .tnode :global(.warn) {
    color: var(--warn);
    opacity: 1;
  }
  .caret {
    flex: 0 0 11px;
    width: 11px;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--text-faint);
    font-size: 0.68rem;
    text-align: center;
    cursor: pointer;
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .n {
    font-family: var(--font-mono);
    font-size: 0.68rem;
    color: var(--text-faint);
  }
  .on .n {
    color: var(--text-dim);
  }
</style>
