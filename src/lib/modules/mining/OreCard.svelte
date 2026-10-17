<script lang="ts">
  // By ore: the rock types that hold the ore. With a loadout each row
  // carries the ship's class split and its good share (easy and
  // manageable rolls), best for the ship first; Richest sorts by the ore's share of
  // the rock when present.
  import { Segmented } from "@veelume/ui";
  import type { AreaView, ClassSharesView } from "$lib/bindings";
  import NowCard from "$lib/components/NowCard.svelte";
  import ClassBar from "./ClassBar.svelte";
  import { oreName, pct } from "./format";

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

  let sort = $state<"good" | "rich">("good");

  const good = (c: ClassSharesView | null) => (c ? c.easy + c.manageable : -1);
  const sources = $derived(
    areas
      .flatMap((a) =>
        a.rocks.flatMap((r) => {
          const o = r.ores.find((x) => x.element === ore);
          return o && r.rollable ? [{ a, r, o, c: classOf(a.id, r.rockType) }] : [];
        }),
      )
      .sort((x, y) =>
        sort === "good" && ship
          ? good(y.c) - good(x.c) || y.o.meanShare - x.o.meanShare
          : y.o.meanShare - x.o.meanShare || y.o.chance - x.o.chance,
      )
      .slice(0, 30),
  );
  const withClasses = $derived(ship !== null && sources.some((s) => s.c !== null));
</script>

<NowCard hue="var(--hue-sand)" chip="◆ ORE" version={name}>
  {#snippet top()}
    {#if ship}
      <Segmented
        class="sort"
        options={[
          { value: "good", label: `Best for your ${ship}` },
          { value: "rich", label: "Richest" },
        ]}
        value={sort}
        onchange={(v) => (sort = v as "good" | "rich")}
      />
    {/if}
  {/snippet}
  <div class="head label" class:cls={withClasses}>
    <span>Rock</span><span>{name}</span>{#if withClasses}<span>Your {ship}</span><span>good</span>{/if}
  </div>
  <div class="rocks">
    {#each sources as s (s.a.id + s.r.rockType)}
      <button
        class="rk"
        class:cls={withClasses}
        class:on={s.a.id === area && s.r.rockType === rockType}
        onclick={() => onpick(s.a.id, s.r.rockType)}
      >
        <span class="nm"><b>{oreName(s.r.name)}</b><span>{s.a.name}</span></span>
        <span class="n" title="Its share of the rock when present">~{pct(s.o.meanShare)}</span>
        {#if withClasses}
          {#if s.c}<ClassBar shares={s.c} />{:else}<span></span>{/if}
          <span class="n g">{s.c ? Math.round(good(s.c) * 100) : "—"}</span>
        {/if}
      </button>
    {/each}
  </div>
</NowCard>

<style>
  :global(.sort) {
    font-size: var(--text-small);
  }
  .head,
  .rk {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 54px;
    gap: 12px;
    align-items: center;
  }
  .head.cls,
  .rk.cls {
    grid-template-columns: minmax(0, 1fr) 54px minmax(70px, 1fr) 30px;
  }
  .head {
    margin: 0;
    font-size: 0.66rem;
    color: var(--text-faint);
  }
  .head span:nth-child(2),
  .head span:nth-child(4) {
    text-align: right;
  }
  .rocks {
    display: flex;
    flex-direction: column;
  }
  .rk {
    padding: 8px;
    margin-inline: -8px;
    border: 0;
    border-top: 1px solid color-mix(in srgb, var(--text) 7%, transparent);
    border-radius: 6px;
    background: none;
    font: inherit;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .rk:first-child {
    border-top-color: transparent;
  }
  .rk:hover {
    background: color-mix(in srgb, var(--text) 4%, transparent);
  }
  .rk.on {
    background: var(--accent-fill);
  }
  .nm {
    display: flex;
    flex-direction: column;
    min-width: 0;
    line-height: 1.3;
  }
  .nm b {
    font-size: 0.92rem;
    font-weight: var(--weight-semibold);
  }
  .nm span {
    font-size: var(--text-small);
    color: var(--text-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .n {
    font-family: var(--font-mono);
    font-size: 0.8rem;
    text-align: right;
    color: var(--text-dim);
  }
  .n.g {
    color: var(--good);
    font-weight: var(--weight-semibold);
  }
</style>
