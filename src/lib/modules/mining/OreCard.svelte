<script lang="ts">
  // By ore: the rocks that hold the ore, one row per distribution (design
  // pass 4) — tables that roll the same rocks (same rock type, same meshes:
  // same masses, ores and classes) are one row listing their places, Cellin ·
  // Yela. Each row carries the ore's share of the rock, with a loadout the
  // ship's class split and good share (easy and manageable rolls), and the
  // mass range on the shared scale. Sorted best for the ship (good share by
  // the ore's share, so a trace of it in another ore's rock sinks), richest,
  // or heaviest. "Tune a loadout" opens Loadouts measured on this ore's rocks
  // (design pass 5: question 4, the best loadout for an ore, without a form).
  import { Segmented } from "@veelume/ui";
  import type { AreaView, ClassSharesView } from "$lib/bindings";
  import NowCard from "$lib/components/NowCard.svelte";
  import { miningUi } from "$lib/state/mining.svelte";
  import { oreName, pct } from "./format";
  import MassAxis from "./MassAxis.svelte";
  import RockRow from "./RockRow.svelte";

  let {
    ore,
    name,
    areas,
    area,
    rockType,
    classOf,
    ship,
    onpick,
  }: {
    ore: string;
    name: string;
    areas: AreaView[];
    area: string;
    rockType: string;
    /** The ship's class shares for a rock type at an area, when known. */
    classOf: (area: string, rockType: string) => ClassSharesView | null;
    ship: string | null;
    onpick: (area: string, rockType: string) => void;
  } = $props();

  let sort = $state<"good" | "rich" | "heavy">("good");

  const good = (c: ClassSharesView | null) => (c ? c.easy + c.manageable : -1);
  // One row per distribution, its places in the order the areas come.
  const rows = $derived.by(() => {
    const by = new Map<string, { at: { a: AreaView; rockType: string }[]; r: AreaView["rocks"][number]; share: number }>();
    for (const a of areas)
      for (const r of a.rocks) {
        const o = r.ores.find((x) => x.element === ore);
        if (!o || !r.rollable) continue;
        const row = by.get(r.dist);
        if (row) row.at.push({ a, rockType: r.rockType });
        else by.set(r.dist, { at: [{ a, rockType: r.rockType }], r, share: o.meanShare });
      }
    return [...by.values()].map((x) => ({ ...x, c: classOf(x.at[0].a.id, x.at[0].rockType) }));
  });
  const sorted = $derived(
    [...rows].sort((x, y) =>
      sort === "good" && ship
        ? good(y.c) * y.share - good(x.c) * x.share || y.share - x.share
        : sort === "heavy"
          ? (y.r.mass?.max ?? 0) - (x.r.mass?.max ?? 0)
          : y.share - x.share,
    ),
  );
  const tables = $derived(new Set(rows.flatMap((x) => x.at.map((t) => t.a.id))).size);
  const withClasses = $derived(ship !== null && rows.some((s) => s.c !== null));
  const sorts = $derived([
    ...(ship ? [{ value: "good", label: `Best for your ${ship}` }] : []),
    { value: "rich", label: "Richest" },
    { value: "heavy", label: "Heaviest" },
  ]);
</script>

<NowCard hue="var(--hue-sand)" chip="◆ ORE" version={name}>
  {#snippet top()}
    <Segmented class="sort" options={sorts} value={ship || sort !== "good" ? sort : "rich"} onchange={(v) => (sort = v as typeof sort)} />
  {/snippet}
  <p class="sub">{rows.length} {rows.length === 1 ? "distribution" : "distributions"} in {tables} spawn {tables === 1 ? "table" : "tables"}</p>
  <div class="tune">
    <button
      type="button"
      class="btn"
      onclick={() => {
        miningUi.measure = "all";
        miningUi.measureOre = ore;
        miningUi.view = "lo";
      }}>Tune a loadout for {name} →</button
    >
    <small class="faint">opens Loadouts measured on {name}</small>
  </div>
  <div class="head label" class:cls={withClasses}>
    <span>Where</span><span title="Its share of the rock when present">share</span>{#if withClasses}<span>Your {ship}</span><span>good</span>{/if}
  </div>
  <MassAxis />
  <div class="rocks">
    {#each sorted as s (s.r.dist)}
      {@const on = s.at.some((t) => t.a.id === area && t.rockType === rockType)}
      <RockRow
        name={oreName(s.r.name)}
        where={[...new Set(s.at.map((t) => t.a.name))].join(" · ")}
        figure="~{pct(s.share)}"
        figureTitle="Its share of the rock when present"
        classes={s.c}
        {withClasses}
        mass={s.r.mass}
        {on}
        onclick={() => onpick(s.at[0].a.id, s.at[0].rockType)}
      />
    {/each}
  </div>
</NowCard>

<style>
  :global(.sort) {
    font-size: var(--text-small);
  }
  .tune {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px 10px;
  }
  .tune small {
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .sub {
    margin: -6px 0 0;
    font-size: var(--text-small);
    color: var(--text-dim);
  }
  .head {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 54px;
    gap: 12px;
    align-items: center;
    margin: 0;
    font-size: 0.66rem;
    color: var(--text-faint);
  }
  .head.cls {
    grid-template-columns: minmax(0, 1fr) 54px minmax(70px, 1fr) 30px;
  }
  .head span:nth-child(2),
  .head span:nth-child(4) {
    text-align: right;
  }
  .rocks {
    display: flex;
    flex-direction: column;
  }
</style>
