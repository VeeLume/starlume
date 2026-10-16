<script lang="ts">
  // By ore: the rock types that hold the ore, richest first (its share of
  // the rock when present), and in how many of them it shows up at all.
  import type { AreaView } from "$lib/bindings";
  import NowCard from "$lib/components/NowCard.svelte";
  import { pct } from "./format";

  let {
    ore,
    name,
    areas,
    area,
    rockType,
    onpick,
  }: {
    ore: string;
    name: string;
    areas: AreaView[];
    area: string;
    rockType: string;
    onpick: (area: string, rockType: string) => void;
  } = $props();

  const sources = $derived(
    areas
      .flatMap((a) =>
        a.rocks.flatMap((r) => {
          const o = r.ores.find((x) => x.element === ore);
          return o && r.rollable ? [{ a, r, o }] : [];
        }),
      )
      .sort((x, y) => y.o.meanShare - x.o.meanShare || y.o.chance - x.o.chance)
      .slice(0, 24),
  );
  const top = $derived(Math.max(0.0001, ...sources.map((s) => s.o.meanShare)));
</script>

<NowCard hue="var(--hue-sand)" chip="◆ ORE" version={name} date="{sources.length} rock types">
  <div class="head label"><span>Rock</span><span></span><span>Share</span><span>In</span></div>
  <div class="rocks">
    {#each sources as s (s.a.id + s.r.rockType)}
      <button class="rk" class:on={s.a.id === area && s.r.rockType === rockType} onclick={() => onpick(s.a.id, s.r.rockType)}>
        <span class="nm"><b>{s.r.name}</b><span>{s.a.name}</span></span>
        <span class="track"><i style:width="{(s.o.meanShare / top) * 100}%"></i></span>
        <span class="n" title="Its share of the rock when present">~{pct(s.o.meanShare)}</span>
        <span class="n dim" title="Rocks of this type holding it">{pct(s.o.chance)}</span>
      </button>
    {/each}
  </div>
</NowCard>

<style>
  .head {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(40px, 0.8fr) 44px 44px;
    gap: 12px;
    margin: 0;
    font-size: 0.66rem;
    color: var(--text-faint);
  }
  .head span:not(:first-child) {
    text-align: right;
  }
  .rocks {
    display: flex;
    flex-direction: column;
  }
  .rk {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(40px, 0.8fr) 44px 44px;
    gap: 12px;
    align-items: center;
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
  .track {
    display: block;
    height: 4px;
    border-radius: 2px;
    background: color-mix(in srgb, var(--text) 8%, transparent);
    overflow: hidden;
  }
  .track i {
    display: block;
    height: 100%;
    background: color-mix(in srgb, var(--hue-sand) 65%, transparent);
  }
  .n {
    font-family: var(--font-mono);
    font-size: 0.8rem;
    text-align: right;
  }
</style>
