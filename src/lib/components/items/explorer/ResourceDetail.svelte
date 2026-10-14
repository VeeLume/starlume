<script lang="ts">
  // A material, everywhere: totals, the amount per quality as bars to scale
  // with their numbers, and where it sits (the place on a quality row only
  // when the material is in more than one).
  import { Archive, User } from "lucide-svelte";
  import { amountOf, fmtAmount, materials, STOCK_QUALITY } from "../model/resources";
  import { areaOf, type ItemNode, type Node, type Tree } from "../model/tree";
  import Quality from "./Quality.svelte";

  let { node, tree, ongo }: { node: ItemNode; tree: Tree; ongo: (folder: Node, select?: Node) => void } = $props();

  const name = $derived(node.item.resource!.name);
  const all = $derived(tree.items.filter((x) => x.item.resource?.name === name));
  const m = $derived(materials(all)[0]);
  const rated = $derived(m.best > 0);
  const label = (a: Node | null) => (a?.type === "place" && a.area === "carried" ? "On you" : (a?.parent?.name ?? "?"));
  const rows = $derived.by(() => {
    const out = new Map<string, { q: number; area: Node | null; v: number }>();
    for (const x of all) {
      const area = areaOf(x);
      const k = `${x.item.resource!.quality}|${area?.id}`;
      const e = out.get(k) ?? { q: x.item.resource!.quality, area, v: 0 };
      e.v += amountOf(x);
      out.set(k, e);
    }
    return [...out.values()].sort((a, b) => b.q - a.q);
  });
  const max = $derived(Math.max(...rows.map((r) => r.v)));
  const multi = $derived(m.places.size > 1);
  const byPlace = $derived.by(() => {
    const out = new Map<string, { area: Node; v: number }>();
    for (const r of rows) {
      if (!r.area) continue;
      const e = out.get(r.area.id) ?? { area: r.area, v: 0 };
      e.v += r.v;
      out.set(r.area.id, e);
    }
    return [...out.values()].sort((a, b) => b.v - a.v);
  });
</script>

<div class="head">
  <div class="titles">
    <h3>{m.name}</h3>
    <p class="sub">Resource · {m.bulk ? `bulk, ${m.units} ${m.units === 1 ? "box" : "boxes"}` : "pieces"}{rated ? "" : " · no quality"}</p>
  </div>
  {#if rated}<Quality q={m.best} />{/if}
</div>
<div class="totals">
  <div><h4>Total</h4><span class="big">{fmtAmount(m.bulk, m.total)}</span></div>
  {#if rated}<div><h4>At Q{STOCK_QUALITY}+</h4><span class="big" class:accent={m.good > 0}>{fmtAmount(m.bulk, m.good)}</span></div>{/if}
</div>

{#if rated}
  <h4>By quality</h4>
  {#each rows as r, k (k)}
    <div class="qrow">
      <span><Quality q={r.q} /></span>
      <span class="track"><span class="bar" class:hi={r.q >= STOCK_QUALITY} style:width="{Math.max(3, (r.v / max) * 100)}%"></span></span>
      <span class="amt">{fmtAmount(m.bulk, r.v)}</span>
      {#if multi}<span class="at">{label(r.area)}</span>{/if}
    </div>
  {/each}
{/if}

<h4>{multi ? "By place" : "Where"}</h4>
{#each byPlace as e (e.area.id)}
  {@const Icon = e.area.type === "place" && e.area.area === "carried" ? User : Archive}
  <button type="button" class="link" onclick={() => ongo(e.area)}>
    <Icon size={12} /><span class="grow">{label(e.area)}<span class="dim">{` · ${e.area.name.toLowerCase()}`}</span></span>
    {#if multi}<span class="num">{fmtAmount(m.bulk, e.v)}</span>{/if}
  </button>
{/each}

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
    color: var(--text-strong);
  }
  .sub {
    margin: 3px 0 0;
    font-size: 0.76rem;
    color: var(--text-dim);
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
  .totals {
    display: flex;
    gap: 18px;
  }
  .totals h4 {
    margin: 12px 0 2px;
  }
  .big {
    font-family: var(--font-mono);
    font-size: 1.05rem;
    color: var(--text-strong);
  }
  .big.accent {
    color: var(--accent);
  }
  .qrow {
    display: grid;
    grid-template-columns: 52px minmax(0, 1fr) 72px;
    gap: 8px;
    align-items: center;
    padding: 3px 0;
    font-size: 0.78rem;
  }
  .track {
    display: block;
  }
  .bar {
    display: block;
    height: 8px;
    border-radius: 2px;
    background: var(--text-faint);
  }
  .bar.hi {
    background: var(--accent);
  }
  .amt {
    text-align: right;
    font-family: var(--font-mono);
    font-size: 0.74rem;
    color: var(--text);
  }
  .at {
    grid-column: 2 / 4;
    margin-top: -2px;
    font-size: 0.68rem;
    color: var(--text-dim);
  }
  .link {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 3px 4px;
    border: 0;
    border-radius: 5px;
    background: transparent;
    color: var(--text-dim);
    font: inherit;
    font-size: 0.78rem;
    text-align: left;
    cursor: pointer;
  }
  .link:hover {
    color: var(--text);
    background: var(--bg-raised);
  }
  .grow {
    flex: 1;
  }
  .dim {
    color: var(--text-faint);
  }
  .num {
    font-family: var(--font-mono);
    color: var(--text);
  }
</style>
