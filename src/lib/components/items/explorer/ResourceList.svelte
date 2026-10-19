<script lang="ts">
  // The Resources view: every material below the selected place, one row
  // each, bulk (SCU) and hand-gathered pieces apart. The quality filter is
  // the crafting question ("how much do I have at Q700 or better?"): its
  // column answers it, and the strip fades the bands under it.
  import { materials, fmtAmount, QUALITY_STEPS, STOCK_QUALITY, type Material } from "../model/resources";
  import type { ItemNode, Node } from "../model/tree";
  import { Select } from "@veelume/ui";
  import ChipButton from "./ChipButton.svelte";
  import Quality from "./Quality.svelte";
  import QualityMix from "./QualityMix.svelte";
  import SectionHead from "./SectionHead.svelte";

  let {
    list,
    selected,
    multi,
    min,
    sort,
    onselect,
    onmin,
    onsort,
  }: {
    list: ItemNode[];
    selected: Node | null;
    /** The place covers more than one location: name where each sits. */
    multi: boolean;
    min: number;
    sort: string;
    onselect: (n: Node) => void;
    onmin: (q: number) => void;
    onsort: (s: string) => void;
  } = $props();

  const all = $derived(materials(list));
  const shown = $derived(materials(list, min));
  const order = (a: Material, b: Material) =>
    sort === "amount" ? b.total - a.total : sort === "name" ? a.name.localeCompare(b.name) : b.best - a.best;
  const groups = $derived(
    [
      { title: "Bulk", unit: "in SCU", list: shown.filter((m) => m.bulk) },
      { title: "Pieces", unit: "counted", list: shown.filter((m) => !m.bulk) },
    ].filter((g) => g.list.length),
  );
  const selName = $derived(selected?.type === "item" ? selected.item.resource?.name : undefined);
  const heat = (share: number, hi: boolean) =>
    `color-mix(in srgb, ${hi ? "var(--hue-quality)" : "var(--text-dim)"} ${Math.round(22 + 78 * share)}%, transparent)`;
</script>

<div class="rlist">
<div class="bar">
  <span class="qlabel">Quality</span>
  {#each QUALITY_STEPS as v (v)}
    <ChipButton on={min === v} n={materials(list, v).length} onclick={() => onmin(v)}>{v ? `Q${v}+` : "Any"}</ChipButton>
  {/each}
  <span class="grow"></span>
  <Select
    label="Sort materials"
    options={[
      { value: "q", label: "Best quality" },
      { value: "amount", label: "Amount" },
      { value: "name", label: "Name" },
    ]}
    value={sort}
    onchange={onsort}
  />
</div>
<p class="legend">
  Quality bands of 100 · brighter = more of that material
  <span class="key">{#each [0.1, 0.5, 1] as s (s)}<i style:background={heat(s, true)}></i>{/each}</span>
  · grey below 500 <span class="key"><i style:background={heat(0.7, false)}></i></span>
</p>

{#if !groups.length}
  <p class="empty">Nothing at Q{min} or better.</p>
{/if}
{#each groups as g (g.title)}
  <SectionHead title={g.title} note={g.unit} n={g.list.length} />
  <div class="row head">
    <span>Material</span><span class="mixcell">Quality mix</span><span>Best</span><span class="r">{min ? `Q${min}+` : `Q${STOCK_QUALITY}+`}</span
    ><span class="r">Total</span>
  </div>
  {#each [...g.list].sort(order) as m (m.name)}
    {@const t = all.find((x) => x.name === m.name)!}
    {@const usable = min ? m.total : t.good}
    <button type="button" class="row" class:on={selName === m.name} onclick={() => onselect(m.nodes[0])}>
      <span class="name"
        ><b>{m.name}</b>{#if multi}<span class="places">{[...t.places].join(" · ")}</span>{/if}</span
      >
      <span class="mixcell"><QualityMix list={t.nodes} bulk={m.bulk} {min} /></span>
      <span>{#if m.best > 0}<Quality q={m.best} />{/if}</span>
      <span class="amt r" class:good={usable > 0} class:none={!usable}
        >{t.best > 0 ? (usable > 0 ? fmtAmount(m.bulk, usable) : "—") : ""}</span
      >
      <span class="amt r">{fmtAmount(m.bulk, t.total)}</span>
    </button>
  {/each}
{/each}
</div>

<style>
  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    padding: 10px 0 2px;
  }
  .qlabel {
    margin-right: 2px;
    font-size: 0.72rem;
    color: var(--text-faint);
  }
  .grow {
    flex: 1;
  }
  .legend {
    margin: 6px 2px 0;
    font-size: 0.68rem;
    color: var(--text-faint);
  }
  .key {
    display: inline-flex;
    gap: 2px;
    margin: 0 2px;
    vertical-align: middle;
  }
  .key i {
    width: 9px;
    height: 9px;
    border-radius: 2px;
  }
  .row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 210px 60px 88px 88px;
    gap: 14px;
    align-items: center;
    width: 100%;
    padding: 6px 10px;
    border: 1px solid transparent;
    border-radius: 7px;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .row:not(.head):hover {
    background: var(--bg-raised);
  }
  .row.on {
    border-color: var(--accent);
    background: var(--accent-fill-faint);
  }
  .row.head {
    padding-top: 2px;
    padding-bottom: 2px;
    cursor: default;
    font-family: var(--font-mono);
    font-size: 0.56rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-faint);
  }
  .name {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }
  .name b {
    font-size: 0.84rem;
    font-weight: 600;
    color: var(--text-strong);
  }
  .places {
    font-size: 0.68rem;
    color: var(--text-dim);
  }
  .r {
    text-align: right;
  }
  .amt {
    font-family: var(--font-mono);
    font-size: 0.8rem;
    color: var(--text-strong);
  }
  .amt.good {
    color: color-mix(in srgb, var(--hue-quality) 85%, var(--text));
  }
  .amt.none {
    color: var(--text-faint);
  }
  .empty {
    padding: 14px 2px;
    font-size: 0.84rem;
    color: var(--text-dim);
  }
  /* Narrow pane: the strip drops to its own line under the name, the
     numbers stay in columns. Decided by the pane, not the window. */
  .rlist {
    container-type: inline-size;
  }
  @container (max-width: 620px) {
    .row {
      grid-template-columns: minmax(0, 1fr) 60px 80px 80px;
      gap: 4px 10px;
    }
    .mixcell {
      grid-column: 1 / -1;
      grid-row: 2;
    }
    .row.head .mixcell {
      display: none;
    }
  }
</style>
