<script lang="ts">
  // The detail pane: what the selection is and where. Never moves, never
  // disappears; a material gets its own read (ResourceDetail).
  import { Dot, Paperclip } from "lucide-svelte";
  import { kitOf } from "../model/classify";
  import { avgIngredientQuality, statDeltas } from "../model/crafted";
  import { offSaved } from "../model/loadout";
  import {
    ancestors,
    container,
    isFolder,
    pathLabel,
    stowed,
    units,
    visible,
    type ItemNode,
    type Node,
    type Tree,
  } from "../model/tree";
  import Delta from "./Delta.svelte";
  import Quality from "./Quality.svelte";
  import ResourceDetail from "./ResourceDetail.svelte";
  import Tag from "./Tag.svelte";
  import { fmtN, fmtTime, nodeIcon, SOURCE } from "./ui";
  import { categoryOf } from "../model/classify";

  let {
    node,
    tree,
    showBricked,
    here,
    ongo,
  }: {
    node: Node | null;
    tree: Tree;
    showBricked: boolean;
    /** The stack the selection stands for in the open container. */
    here: ItemNode[];
    ongo: (folder: Node, select?: Node) => void;
  } = $props();

  const item = $derived(node?.type === "item" ? node : null);
  const crafted = $derived(item?.item.crafted ?? null);
  const kit = $derived(item?.item.kind.item_type === "WeaponPersonal" ? kitOf(item) : []);
  const attached = $derived(
    item && item.item.kind.item_type !== "WeaponPersonal"
      ? item.kids.filter((k): k is ItemNode => k.type === "item" && !isFolder(k) && visible(k, showBricked))
      : [],
  );
  const chain = $derived(
    node ? ancestors(node).filter((x) => !(x.type === "place" && (x.level === "root" || x.level === "system"))) : [],
  );
  const elsewhere = $derived.by(() => {
    if (!item) return [];
    const m = new Map<string, { c: Node; n: number; first: ItemNode }>();
    for (const x of tree.byKind.get(item.item.kind.guid) ?? []) {
      if (x === item || here.includes(x) || !visible(x, showBricked)) continue;
      const c = container(x);
      if (!c) continue;
      const e = m.get(c.id) ?? { c, n: 0, first: x };
      e.n += x.item.stack;
      m.set(c.id, e);
    }
    return [...m.values()].sort((a, b) => b.n - a.n).slice(0, 6);
  });
  const inside = $derived.by(() => {
    if (!node || node.type === "item" || (node.type === "place" && node.level === "group")) return [];
    const cats = new Map<string, number>();
    const mats = new Set<string>();
    const walk = (x: Node) => {
      for (const k of x.kids) {
        if (k.type === "item" && visible(k, showBricked)) {
          if (k.item.resource) mats.add(k.item.resource.name);
          else cats.set(categoryOf(k), (cats.get(categoryOf(k)) ?? 0) + k.item.stack);
        }
        if (k.type !== "ship" || node.type !== "ship") walk(k);
      }
    };
    walk(node);
    if (mats.size) cats.set("Resources", mats.size);
    return [...cats.entries()].sort((a, b) => Number(a[0] === "Other") - Number(b[0] === "Other") || b[1] - a[1]);
  });
  const subtitle = $derived(
    !node
      ? ""
      : node.type === "place"
        ? node.sub
        : node.type === "ship"
          ? `Ship${node.ship.kind.size ? ` · S${node.ship.kind.size}` : ""}`
          : `${categoryOf(node)}${node.item.kind.size && ["Ship parts", "Weapons"].includes(categoryOf(node)) ? ` · S${node.item.kind.size}` : ""}`,
  );
</script>

{#if !node}
  <p class="dim">Select an item to see where it is and what it is.</p>
{:else if item?.item.resource}
  <ResourceDetail node={item} {tree} {ongo} />
{:else}
  <div class="head">
    <div class="titles">
      <h3>{node.name}</h3>
      <p class="sub">{subtitle}{here.length > 1 ? ` · ${here.reduce((s, x) => s + x.item.stack, 0)} here` : ""}</p>
    </div>
    {#if crafted}{@const q = avgIngredientQuality(crafted)}{#if q != null}<Quality {q} prefix="avg Q" />{/if}{/if}
  </div>
  {#if item}
    <div class="tags">
      {#if item.item.fitted && container(item)?.type === "ship"}<Tag label="fitted" />{/if}
      {#if item.item.ownership === "rented"}<Tag label="rented" tone="warning" />{/if}
      {#if item.item.ownership === "bricked"}<Tag label="bricked" tone="destructive" />{/if}
      {#if item.item.ammo != null}<Tag label={`${item.item.ammo} rds`} />{/if}
    </div>
  {/if}

  {#if node.type === "ship"}
    {@const s = node.ship}
    {@const off = offSaved(s)}
    <h4>Ship</h4>
    <dl>
      {#if SOURCE[s.source]}<dt>Source</dt><dd>{SOURCE[s.source]}</dd>{/if}
      {#if s.rental_expiry}<dt>Rental</dt><dd class="warn">ends {fmtTime(s.rental_expiry)}</dd>{/if}
      <dt>Fitted vs saved</dt>
      <dd>
        {#if off == null}<span class="dim">no saved loadout read</span>{:else if off}<span class="warn">{off} differ</span>{:else}<span
            class="good">match</span
          >{/if}
      </dd>
      {#if !node.virtual}
        <dt>Fitted parts</dt><dd class="mono">{units(node, (x) => x.item.fitted && visible(x, showBricked))}</dd>
        <dt>Stored on board</dt><dd class="mono">{stowed(node, (x) => visible(x, showBricked)).reduce((n, x) => n + x.item.stack, 0)}</dd>
      {/if}
    </dl>
  {/if}

  {#if crafted}
    <h4>Crafted</h4>
    <div class="deltas">{#each statDeltas(crafted) as d (d.name)}<Delta {d} />{/each}</div>
    <p class="ingredients">
      {#each crafted.ingredients as g, i (i)}<span>{g.name ?? "?"} Q{g.quality}</span>{/each}
    </p>
  {/if}

  {#if node.type !== "place" || node.level !== "root"}
    <h4>Where</h4>
    <div class="where">
      {#each chain as x, k (x.id)}
        {@const Icon = x === node ? Dot : nodeIcon(x)}
        <button
          type="button"
          class="where-row"
          class:self={x === node}
          style:padding-left="{k * 12 + 4}px"
          disabled={x === node}
          onclick={() => ongo(isFolder(x) ? x : (container(x) ?? x), x)}><Icon size={12} />{x.name}</button
        >
      {/each}
    </div>
  {/if}

  {#if kit.length || attached.length}
    <h4>Attached</h4>
    {#each kit as k (k.part.id)}
      <div class="att">
        <span class="slot">{k.slot}</span>
        <span class="att-name" class:added={k.state === "added"}>{k.part.item.kind.name}</span>
        {#if k.part.item.ammo != null}<span class="mono dim">{k.part.item.ammo} rds</span>{/if}
        {#if k.state === "stock"}<span class="dim">stock</span>{:else if k.state === "added"}<Tag label="added" tone="primary" />{/if}
      </div>
    {/each}
    {#each attached as a (a.id)}
      <div class="att"><Paperclip size={12} /><span class="att-name">{a.item.kind.name}</span></div>
    {/each}
  {/if}

  {#if elsewhere.length}
    <h4>Also elsewhere</h4>
    {#each elsewhere as e (e.c.id)}
      <button type="button" class="link" onclick={() => ongo(e.c, e.first)}
        ><span class="mono n">{e.n}×</span><span>{pathLabel(e.c)}</span></button
      >
    {/each}
  {/if}

  {#if inside.length}
    <h4>Inside</h4>
    {#each inside as [c, k] (c)}
      <div class="fact"><span class="dim">{c}</span><span class="mono">{fmtN(k)}{#if c === "Resources"}<span class="dim"> materials</span>{/if}</span></div>
    {/each}
  {/if}
{/if}

<style>
  .head {
    display: flex;
    align-items: flex-start;
    gap: 8px;
  }
  .titles {
    flex: 1;
    min-width: 0;
  }
  h3 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.05rem;
    font-weight: 700;
    line-height: 1.25;
    color: var(--text-strong);
  }
  .sub {
    margin: 3px 0 0;
    font-size: 0.76rem;
    color: var(--text-dim);
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 8px;
  }
  h4 {
    margin: 14px 0 6px;
    font-family: var(--font-mono);
    font-size: 0.6rem;
    font-weight: 400;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--text-faint);
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 3px 10px;
    margin: 0;
    font-size: 0.8rem;
  }
  dt {
    color: var(--text-dim);
  }
  dd {
    margin: 0;
    text-align: right;
  }
  .deltas {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .ingredients {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 10px;
    margin: 6px 0 0;
    font-family: var(--font-mono);
    font-size: 0.7rem;
    color: var(--text-dim);
  }
  .where {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .where-row,
  .link {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 2px 4px;
    border: 0;
    border-radius: 5px;
    background: transparent;
    color: var(--text-dim);
    font: inherit;
    font-size: 0.8rem;
    text-align: left;
    cursor: pointer;
  }
  .where-row:hover:not(.self),
  .link:hover {
    color: var(--text);
    background: var(--bg-raised);
  }
  .where-row.self {
    color: var(--text-strong);
    cursor: default;
  }
  .link .n {
    width: 26px;
    text-align: right;
    color: var(--text);
  }
  .att {
    display: flex;
    align-items: baseline;
    gap: 6px;
    padding: 2px 0;
    font-size: 0.8rem;
    color: var(--text-faint);
  }
  .slot {
    flex: 0 0 26px;
    font-family: var(--font-mono);
    font-size: 0.6rem;
  }
  .att-name {
    flex: 1;
    min-width: 0;
    color: var(--text-dim);
  }
  .att-name.added {
    color: var(--accent);
  }
  .fact {
    display: flex;
    justify-content: space-between;
    padding: 2px 0;
    font-size: 0.8rem;
  }
  .dim {
    color: var(--text-dim);
  }
  .warn {
    color: var(--warn);
  }
  .good {
    color: var(--good);
  }
  .mono {
    font-family: var(--font-mono);
  }
</style>
