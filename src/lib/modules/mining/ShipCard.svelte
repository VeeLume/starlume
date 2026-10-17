<script lang="ts">
  // Your ship on the rock, in News's card. The tiles are what it puts on
  // the rock: power, and its change to resistance, instability and window
  // (the gadget belongs to the rock, so it is not in them). Solo: the laser
  // large, each module with its effects. With crew mates brought along (or
  // a MOLE's heads): the combined tiles and one chip per operator; what
  // each brings is in the crew plan. The picker is your own loadout only;
  // crew mates come from Bring along (their saved crew loadouts) and leave
  // with their ✕.
  import { X } from "lucide-svelte";
  import type { LoadoutView, MiningCatalogView, MiningReportView } from "$lib/bindings";
  import Monogram from "$lib/components/Monogram.svelte";
  import NowCard from "$lib/components/NowCard.svelte";
  import { distinctHues } from "$lib/monogram";
  import { addSupport, miningStore, removeSupport, setLoadout } from "$lib/state/mining.svelte";
  import {
    effects,
    laserShort,
    lasersLine,
    namesOf,
    operatorNames,
    operatorPlace,
    ownLasers,
    sameLoadout,
    signedPct,
  } from "./format";

  let {
    catalog,
    loadout,
    report,
    onedit,
  }: {
    catalog: MiningCatalogView;
    loadout: LoadoutView;
    report: MiningReportView | null;
    onedit: () => void;
  } = $props();

  let bringing = $state(false);

  const names = $derived(namesOf(catalog));
  const laserById = $derived(new Map(catalog.lasers.map((l) => [l.id, l])));
  const moduleById = $derived(new Map(catalog.modules.map((m) => [m.id, m])));

  const own = $derived(ownLasers(loadout));
  const solo = $derived(loadout.lasers.length <= 1);
  const first = $derived(own.length ? (laserById.get(own[0].laser) ?? null) : null);
  const saved = $derived(miningStore.state?.saved ?? []);
  const mine = $derived(saved.filter((s) => s.owner === null));
  const current = $derived(mine.find((s) => sameLoadout(s.loadout, { ...loadout, lasers: own })) ?? null);
  const aboard = $derived(new Set(loadout.lasers.flatMap((l) => (l.crew ? [l.crew] : []))));
  const bench = $derived(saved.filter((s) => s.owner !== null && !aboard.has(s.id)));

  const effect = $derived(report?.yours?.effect ?? null);
  const tiles = $derived([
    { v: effect ? Math.round(effect.power).toLocaleString("en-US") : "—", k: "power", good: false },
    { v: effect ? signedPct(effect.resistance) : "—", k: "resistance", good: (effect?.resistance ?? 0) < -0.005 },
    { v: effect ? signedPct(effect.instability) : "—", k: "instability", good: (effect?.instability ?? 0) < -0.005 },
    { v: effect ? signedPct(effect.window) : "—", k: "window", good: (effect?.window ?? 0) > 0.005 },
  ]);

  const opNames = $derived(operatorNames(loadout.lasers));
  const hues = $derived(distinctHues(opNames));
  const places = $derived(
    loadout.lasers.map((_, i) => operatorPlace(loadout.lasers, i, (id) => saved.find((s) => s.id === id)?.name)),
  );
  const leadIndex = $derived(report?.yours?.operators.findIndex((o) => o.lead) ?? 0);
  const crewShips = $derived(
    [...new Set(loadout.lasers.flatMap((l) => (l.crew ? [l.crew] : [])))].map(
      (id) => saved.find((s) => s.id === id)?.name ?? "ship",
    ),
  );
  const shipName = $derived(current?.name ?? (own.length ? lasersLine({ lasers: own, gadget: null, lead: null }, names) : "No laser"));
  const ships = $derived(new Set(loadout.lasers.map((l) => l.owner)).size);
  const range = $derived(
    first && first.optimalRangeM !== null && first.maxRangeM !== null
      ? `, ${first.optimalRangeM.toFixed(0)}–${first.maxRangeM.toFixed(0)} m`
      : "",
  );

  /** Switch your own loadout, keeping the crew mates on the rock. */
  function pick(id: string) {
    const s = mine.find((x) => x.id === id);
    if (!s) return;
    const crewLasers = loadout.lasers.filter((l) => l.crew !== null);
    const next = Math.max(-1, ...s.loadout.lasers.map((l) => l.owner)) + 1;
    const base = Math.min(...crewLasers.map((l) => l.owner), Infinity);
    void setLoadout({
      ...s.loadout,
      gadget: loadout.gadget,
      lead: null,
      lasers: [...s.loadout.lasers, ...crewLasers.map((l) => ({ ...l, owner: next + (l.owner - base) }))],
    });
  }
</script>

{#snippet picker()}
  {#if mine.length}
    <select
      class="select pick"
      value={current?.id ?? ""}
      onchange={(e) => pick(e.currentTarget.value)}
      aria-label="Your loadout"
    >
      {#if !current}<option value="" disabled>Unsaved</option>{/if}
      {#each mine as s (s.id)}<option value={s.id}>{s.name}</option>{/each}
    </select>
  {/if}
{/snippet}

{#snippet bringAlong()}
  {#if bench.length}
    <div class="bring">
      <span class="faint">Bring along</span>
      {#each bench.slice(0, 4) as b (b.id)}
        <button class="bn" onclick={() => void addSupport(b.id)}>
          <Monogram name={b.owner ?? ""} size={22} />
          <span><b>{b.owner}</b><span class="faint">{b.name}, {lasersLine(b.loadout, names)}</span></span>
        </button>
      {/each}
    </div>
  {:else if bringing}
    <p class="faint none">No crew loadouts yet: add them under Loadouts → Crew.</p>
  {/if}
{/snippet}

{#if solo}
  <NowCard
    hue="var(--hue-sky)"
    chip="● YOUR SHIP"
    version={first ? laserShort(first.name) : "No laser"}
    name={first ? `Mining Laser S${first.size}${range}` : ""}
  >
    {#snippet top()}{@render picker()}{/snippet}
    <div class="stat-tiles">
      {#each tiles as t (t.k)}<div class="stat-tile"><b class:good={t.good}>{t.v}</b><span>{t.k}</span></div>{/each}
    </div>
    {#if own[0]}
      <div class="mods">
        {#each own[0].modules as id, s (s)}
          {@const m = moduleById.get(id)}
          <div class="m">
            <b>{m?.name ?? "Unknown module"}</b>
            {#if m}
              {#each effects(m.modifiers, m.powerMultiplier).slice(0, 3) as fx (fx.text)}
                <span class="hue-chip" style:--hue={fx.good ? "var(--good)" : "var(--warn)"}>{fx.text}</span>
              {/each}
            {/if}
          </div>
        {/each}
      </div>
    {/if}
    {#if bringing}{@render bringAlong()}{/if}
    {#snippet action()}
      <button class="btn crew" onclick={() => (bringing = !bringing)}>+ Crew</button>
      <button class="btn" onclick={onedit}>Edit ship</button>
    {/snippet}
  </NowCard>
{:else}
  <NowCard
    hue="var(--hue-sky)"
    chip={crewShips.length ? "● ON THE ROCK" : "● YOUR SHIP"}
    version={[shipName, ...crewShips].join(" + ")}
    name={crewShips.length ? `${ships} ships, ${loadout.lasers.length} operators` : `${loadout.lasers.length} heads${range}`}
  >
    {#snippet top()}{@render picker()}{/snippet}
    <div class="stat-tiles">
      {#each tiles as t (t.k)}<div class="stat-tile"><b class:good={t.good}>{t.v}</b><span>{t.k}</span></div>{/each}
    </div>
    <div class="ops">
      {#each loadout.lasers as l, i (i)}
        <div class="op">
          <Monogram name={opNames[i]} hue={hues[i]} size={22} />
          <b>{opNames[i]}</b>
          <span class="faint">{places[i] || laserShort(laserById.get(l.laser)?.name ?? "")}</span>
          {#if i === leadIndex}<span class="role">lead</span>{/if}
          {#if l.crew}
            <button
              class="x"
              onclick={() => void removeSupport(l.crew, l.who)}
              title="Take {opNames[i]} off the rock"
              aria-label="Remove {opNames[i]}"><X size={13} /></button
            >
          {/if}
        </div>
      {/each}
    </div>
    {@render bringAlong()}
    {#snippet action()}
      <button class="btn" onclick={onedit}>Edit your ship</button>
    {/snippet}
  </NowCard>
{/if}

<style>
  .pick {
    font-size: var(--text-small);
    padding-block: 2px;
  }
  .good {
    color: var(--good);
  }
  .mods {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .m {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
    font-size: 0.86rem;
  }
  .m b {
    font-weight: var(--weight-semibold);
    margin-right: 4px;
    min-width: 76px;
  }
  .crew {
    margin-right: auto;
  }
  .ops {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .op {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 8px;
    border-radius: 8px;
    font-size: 0.86rem;
    background: color-mix(in srgb, var(--bg) 55%, transparent);
    border: 1px solid color-mix(in srgb, var(--text) 8%, transparent);
  }
  .op .faint {
    flex: 1;
    min-width: 0;
    font-size: 0.74rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .faint {
    color: var(--text-faint);
  }
  .role {
    font-family: var(--font-mono);
    font-size: 0.58rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    padding: 0 5px;
    border-radius: var(--radius-pill);
    color: color-mix(in srgb, var(--good) 85%, var(--text));
    border: 1px solid color-mix(in srgb, var(--good) 40%, transparent);
  }
  .x {
    display: inline-flex;
    padding: 2px;
    border: 1px solid transparent;
    border-radius: 4px;
    background: none;
    color: var(--text-faint);
    cursor: pointer;
  }
  .x:hover {
    color: var(--bad);
    border-color: var(--danger-border);
  }
  .bring {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
    font-size: var(--text-small);
  }
  .bn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 8px 4px 5px;
    border-radius: var(--radius-pill);
    font: inherit;
    text-align: left;
    color: var(--text);
    background: color-mix(in srgb, var(--bg) 60%, transparent);
    border: 1px dashed var(--border);
    cursor: pointer;
  }
  .bn:hover {
    border-color: var(--accent);
  }
  .bn > span {
    display: flex;
    flex-direction: column;
    line-height: 1.2;
  }
  .bn b {
    font-size: 0.8rem;
  }
  .bn .faint {
    font-size: 0.64rem;
  }
  .none {
    margin: 0;
    font-size: var(--text-small);
  }
</style>
