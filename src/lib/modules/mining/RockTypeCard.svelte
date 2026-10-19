<script lang="ts">
  // One rock type at a place: its share there, lightest, most common and
  // heaviest, its mass distribution with the calculator's ship's class at
  // each mass (drag to roll a rock of that mass), and what to roll as one
  // choice (style guide: one state, one control) — most common, a typical
  // one of a class, the richest or best quality of an ore. A row standing
  // for several places rolls quality at the one picked under Roll at
  // (quality depends on the place; masses and classes don't). The roll ends
  // the card; "Use in calculator" takes it over.
  import { Select } from "@veelume/ui";
  import type { AreaRockView, GeneratedView, MiningCatalogView, SpawnPlaceView } from "$lib/bindings";
  import NowCard from "$lib/components/NowCard.svelte";
  import { CLASSES, int, kt, namesOf, oreName, pct, type Pick } from "./format";
  import MassHist from "./MassHist.svelte";

  let {
    catalog,
    rock,
    where,
    places,
    at,
    rolled,
    stale,
    error,
    pick,
    ore,
    ship,
    onpick,
    onore,
    onat,
    onuse,
  }: {
    catalog: MiningCatalogView;
    rock: AreaRockView;
    /** Where the row is (the area's name). */
    where: string;
    /** The places it spawns at, for Roll at. */
    places: SpawnPlaceView[];
    /** The place quality rolls at. */
    at: string;
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
    onat: (place: string) => void;
    onuse: () => void;
  } = $props();

  const names = $derived(namesOf(catalog));
  const name = (id: string) => names.get(id) ?? "Unknown";
  const ores = $derived([...rock.ores].sort((a, b) => b.meanShare * b.chance - a.meanShare * a.chance));
  const classes = $derived(rolled?.classes ?? null);
  const handle = $derived(pick.startsWith("mass:") ? Number(pick.slice(5)) : (rolled?.mass ?? 0));
  const cap = (s: string) => s[0].toUpperCase() + s.slice(1);

  // What to roll, as one select: the pick and, for an ore pick, its ore.
  const rollValue = $derived(pick === "ore" || pick === "quality" ? `${pick}|${ore}` : pick);
  const rollOptions = $derived([
    { value: "common", label: "Most common" },
    ...(pick.startsWith("mass:") ? [{ value: pick, label: `At ${int(Number(pick.slice(5)))} t (the slider)` }] : []),
    ...(ship && classes
      ? CLASSES.filter((c) => classes[c.id] > 0).map((c) => ({
          value: `class:${c.id}`,
          label: cap(c.label),
          group: "A typical one that is",
        }))
      : []),
    ...ores.slice(0, 4).flatMap((o) => {
      const n = oreName(name(o.element));
      return [
        { value: `ore|${o.element}`, label: `Richest in ${n}`, group: "By ore" },
        { value: `quality|${o.element}`, label: `Best ${n} quality`, group: "By ore" },
      ];
    }),
  ]);
  function roll(v: string) {
    const [p, element] = v.split("|");
    if (element) onore(element);
    onpick(p as Pick);
  }
</script>

<NowCard hue="var(--hue-sky)" chip="● ROCK TYPE" version={oreName(rock.name)} date={where}>
  {#if places.length > 1}
    <div class="row">
      <span class="label">Roll at</span>
      <Select label="Roll at" options={places.map((p) => ({ value: p.id, label: p.name }))} value={at} onchange={onat} />
      <span class="faint small">quality is per place</span>
    </div>
  {/if}
  <div class="stat-tiles">
    <div class="stat-tile"><b>{pct(rock.share)}</b><span>of rocks there</span></div>
    {#if rock.mass}
      <div class="stat-tile"><b>{kt(rock.mass.min)} t</b><span>lightest</span></div>
      <div class="stat-tile"><b>{kt(rock.mass.mode)} t</b><span>most common</span></div>
      <div class="stat-tile"><b>{kt(rock.mass.max)} t</b><span>heaviest</span></div>
    {/if}
  </div>

  {#if rolled && rolled.masses.length}
    <p class="label">{ship && classes ? `Your ${ship} on ${int(rolled.rolled)} of them, by mass` : "By mass"}</p>
    <MassHist
      buckets={rolled.masses}
      value={handle}
      onmass={(m) => onpick(`mass:${Math.round(m)}`)}
      onsnap={(p) => onpick(p)}
    />
  {/if}
  {#if ship && classes && rolled}
    <div class="keys">
      {#each CLASSES as c (c.id)}
        <span style:--hue={c.hue}><i></i><b>{Math.round(classes[c.id] * 100)}</b>{c.label}</span>
      {/each}
    </div>
  {/if}

  <div class="row roll">
    <span class="label">Roll</span>
    <Select label="Roll" options={rollOptions} value={rollValue} onchange={roll} />
  </div>

  {#if error}
    <p class="error">{error}</p>
  {:else if rolled}
    <div class="rolled" class:stale>
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
        {#if rolled.place}· rolled at {rolled.place.name}{/if}
      </span>
    {/if}
    <button class="btn primary" disabled={!rolled || stale} onclick={onuse}>Use in calculator</button>
  {/snippet}
</NowCard>

<style>
  .label {
    margin: 0;
  }
  .row {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px 10px;
  }
  .small {
    font-size: var(--text-small);
  }
  .keys {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 14px;
  }
  .keys span {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: var(--text-small);
    color: var(--text-dim);
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
  .roll {
    padding-top: 10px;
    border-top: 1px solid color-mix(in srgb, var(--text) 8%, transparent);
  }
  .rolled {
    display: flex;
    flex-direction: column;
    gap: 10px;
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
