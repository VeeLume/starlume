<script lang="ts">
  // One rock type at a place: its share and typical ores, how the
  // calculator's ship fares on a few thousand rolls of it (the classes in
  // power order, each a pick of its own), and the picks — most common, by
  // mass, by ore. The roll ends the card; "Use in calculator" takes it over.
  import { Segmented } from "@veelume/ui";
  import type { AreaRockView, GeneratedView, MiningCatalogView } from "$lib/bindings";
  import NowCard from "$lib/components/NowCard.svelte";
  import { CLASSES, int, namesOf, oreName, pct, type Pick } from "./format";

  let {
    catalog,
    rock,
    rolled,
    stale,
    error,
    pick,
    ore,
    ship,
    onpick,
    onore,
    onuse,
  }: {
    catalog: MiningCatalogView;
    rock: AreaRockView;
    rolled: GeneratedView | null;
    /** The roll on show is for an earlier pick. */
    stale: boolean;
    error: string;
    pick: Pick;
    ore: string;
    /** The calculator's loadout's name, for the classes; null without a laser. */
    ship: string | null;
    onpick: (pick: Pick) => void;
    onore: (element: string) => void;
    onuse: () => void;
  } = $props();

  const names = $derived(namesOf(catalog));
  const name = (id: string) => names.get(id) ?? "Unknown";
  const ores = $derived([...rock.ores].sort((a, b) => b.meanShare * b.chance - a.meanShare * a.chance));
  const classes = $derived(rolled?.classes ?? null);
</script>

<NowCard hue="var(--hue-sky)" chip="● ROCK TYPE" version={rock.name}>
  <div class="stat-tiles">
    <div class="stat-tile"><b>{pct(rock.share)}</b><span>of rocks here</span></div>
    {#each ores.slice(0, 2) as o (o.element)}
      <div class="stat-tile" title="In {pct(o.chance)} of these rocks">
        <b>~{pct(o.meanShare)}</b><span>{oreName(name(o.element))}</span>
      </div>
    {/each}
  </div>

  {#if ship && classes && rolled}
    <p class="label">Your {ship} on {int(rolled.rolled)} of them</p>
    <span class="bar">
      {#each CLASSES as c (c.id)}
        {#if classes[c.id] > 0}<i style:width="{classes[c.id] * 100}%" style:--hue={c.hue}></i>{/if}
      {/each}
    </span>
    <div class="keys">
      {#each CLASSES as c (c.id)}
        <button
          class:on={pick === `class:${c.id}`}
          disabled={classes[c.id] <= 0}
          style:--hue={c.hue}
          title="Roll a {c.label} one"
          onclick={() => onpick(`class:${c.id}`)}><i></i><b>{Math.round(classes[c.id] * 100)}</b>{c.label}</button
        >
      {/each}
    </div>
  {/if}

  <div class="picks">
    <button class="common" class:on={pick === "common"} onclick={() => onpick("common")}>Most common</button>
    <div class="grp">
      <span class="k">Mass</span>
      <Segmented
        options={[
          { value: "small", label: "Lightest" },
          { value: "average", label: "Median" },
          { value: "large", label: "Heaviest" },
        ]}
        value={pick}
        onchange={(v) => onpick(v as Pick)}
      />
    </div>
    {#if ores.length}
      <div class="grp">
        {#if ores.length > 1}
          <select class="select k-ore" value={ore} onchange={(e) => onore(e.currentTarget.value)} aria-label="Ore">
            {#each ores as o (o.element)}<option value={o.element}>{oreName(name(o.element))}</option>{/each}
          </select>
        {:else}
          <span class="k">{oreName(name(ores[0].element))}</span>
        {/if}
        <Segmented
          options={[
            { value: "ore", label: "Richest" },
            { value: "quality", label: "Best quality" },
          ]}
          value={pick}
          onchange={(v) => onpick(v as Pick)}
        />
      </div>
    {/if}
  </div>

  {#if error}
    <p class="error">{error}</p>
  {:else if rolled}
    <div class="roll" class:stale>
      <div class="stat-tiles">
        <div class="stat-tile"><b>{int(rolled.mass)} t</b><span>mass</span></div>
        <div class="stat-tile"><b>{rolled.scu.toFixed(1)}</b><span>SCU</span></div>
      </div>
      <div class="comp">
        {#each rolled.entries as e, i (i)}
          <span class="p">{e.percent.toFixed(2)}</span>
          <span class="nm">{name(e.element)}</span>
          <span class="track"><i style:width="{Math.min(100, e.percent)}%"></i></span>
          <span class="q">{e.quality === null ? "" : `Q ${e.quality}`}</span>
        {/each}
      </div>
    </div>
  {/if}

  {#snippet action()}
    {#if rolled && !stale}
      <span class="from">
        {rolled.pickShare !== null ? `its ore mix in ${pct(rolled.pickShare)} of rolls` : `one of ${int(rolled.rolled)} rolls`}
      </span>
    {/if}
    <button class="btn primary" disabled={!rolled || stale} onclick={onuse}>Use in calculator</button>
  {/snippet}
</NowCard>

<style>
  .label {
    margin: 0;
  }
  .bar {
    display: flex;
    height: 10px;
    gap: 2px;
    border-radius: 5px;
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    background: color-mix(in srgb, var(--hue) 70%, transparent);
  }
  .keys {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 6px;
    margin-inline: -6px;
  }
  .keys button {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 2px 6px;
    border: 1px solid transparent;
    border-radius: var(--radius);
    background: none;
    font: inherit;
    font-size: var(--text-small);
    color: var(--text-dim);
    cursor: pointer;
  }
  .keys button:hover:not(:disabled) {
    color: var(--text);
    background: color-mix(in srgb, var(--text) 5%, transparent);
  }
  .keys button.on {
    border-color: var(--accent);
    color: var(--text);
  }
  .keys button:disabled {
    cursor: default;
    opacity: 0.6;
  }
  .keys i {
    width: 8px;
    height: 8px;
    border-radius: 2px;
    background: color-mix(in srgb, var(--hue) 70%, transparent);
  }
  .keys b {
    font-family: var(--font-mono);
    color: var(--text);
    font-weight: var(--weight-semibold);
  }
  .picks {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding-top: 10px;
    border-top: 1px solid color-mix(in srgb, var(--text) 8%, transparent);
  }
  .common {
    padding: 8px 12px;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: color-mix(in srgb, var(--bg) 70%, transparent);
    font: inherit;
    font-size: 0.9rem;
    font-weight: var(--weight-semibold);
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .common.on {
    border-color: var(--accent);
    background: var(--accent-fill);
  }
  .grp {
    display: grid;
    grid-template-columns: 96px minmax(0, 1fr);
    gap: 10px;
    align-items: center;
    justify-items: start;
  }
  .k {
    font-size: 0.72rem;
    color: var(--text-faint);
    text-transform: var(--label-transform);
    letter-spacing: 0.08em;
  }
  .k-ore {
    width: 100%;
    font-size: var(--text-small);
  }
  .roll {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding-top: 10px;
    border-top: 1px solid color-mix(in srgb, var(--text) 8%, transparent);
  }
  .stale {
    opacity: 0.6;
  }
  .comp {
    display: grid;
    grid-template-columns: 52px minmax(0, 1fr) minmax(50px, 120px) 52px;
    gap: 6px 12px;
    align-items: center;
    font-size: 0.86rem;
  }
  .p {
    font-family: var(--font-mono);
    font-size: 0.82rem;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .nm {
    min-width: 0;
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
  .q {
    font-family: var(--font-mono);
    font-size: 0.75rem;
    color: color-mix(in srgb, var(--hue-quality) 80%, var(--text));
    text-align: right;
  }
  .from {
    margin-right: auto;
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .error {
    margin: 0;
  }
</style>
