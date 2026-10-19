<script lang="ts">
  // The rock finder: where rocks spawn and how the calculator's ship fares
  // there, from the game's own spawn rules (mod-mining's generator). By ore
  // first (the way to find a spot for the ship): the ores with how good
  // the ship does on it, the rock types holding one. By place: the tree, the
  // place's rock types. Every row carries the ship's class split (the class
  // table); the rock type card rolls a rock. The selection is the store's
  // session state, so a visit to the calculator keeps it.
  import { Segmented } from "@veelume/ui";
  import type { ClassSharesView, LoadoutView, MiningCatalogView, PlaceNodeView } from "$lib/bindings";
  import { miningStore, miningUi, rollRock, useRolled } from "$lib/state/mining.svelte";
  import { namesOf, oreName, ownLasers, pickRequest, type Pick } from "./format";
  import OreCard from "./OreCard.svelte";
  import PlaceCard from "./PlaceCard.svelte";
  import PlaceTree from "./PlaceTree.svelte";
  import RockTypeCard from "./RockTypeCard.svelte";

  let { catalog, channel, loadout }: { catalog: MiningCatalogView; channel: string; loadout: LoadoutView } = $props();

  /** A pick waits this long so a quick change of mind rolls once. */
  const ROLL_DEBOUNCE_MS = 150;

  const names = $derived(namesOf(catalog));
  const name = (id: string) => names.get(id) ?? "Unknown";
  const areas = $derived(miningStore.areas?.areas ?? []);
  const areasById = $derived(new Map(areas.map((a) => [a.id, a])));
  const systems = $derived(miningStore.places ?? []);

  // Every node with its parent, by id.
  const nodes = $derived.by(() => {
    const out = new Map<string, { node: PlaceNodeView; parent: PlaceNodeView | null }>();
    const walk = (list: PlaceNodeView[], parent: PlaceNodeView | null) => {
      for (const n of list) {
        out.set(n.id, { node: n, parent });
        walk(n.children, n);
      }
    };
    for (const s of systems) walk(s.nodes, null);
    return out;
  });
  const firstWithRocks = $derived([...nodes.values()].find((x) => x.node.areas.length)?.node ?? null);
  const place = $derived(nodes.get(miningUi.place) ?? null);

  const ores = $derived(
    [...new Set(areas.flatMap((a) => a.rocks.filter((r) => r.rollable).flatMap((r) => r.ores.map((o) => o.element))))]
      .map((id) => ({ id, name: oreName(name(id)) }))
      .sort((a, b) => a.name.localeCompare(b.name)),
  );
  let oreQuery = $state("");
  const shownOres = $derived(ores.filter((o) => o.name.toLowerCase().includes(oreQuery.trim().toLowerCase())));

  // The ship's class shares per rock type and area (the class table).
  const classes = $derived(
    new Map((miningStore.classTable ?? []).map((r) => [`${r.area}|${r.rockType}`, r.classes] as const)),
  );
  const classOf = (areaId: string, rockType: string): ClassSharesView | null =>
    classes.get(`${areaId}|${rockType}`) ?? null;
  // Per ore, the ship's Everyday on it (Loadouts' figure): of the ore you'd
  // find, how much sits in rocks it cracks easy or manageable — each rock
  // by its share of its table and how much of the ore it holds, so a trace
  // in another ore's rock barely counts.
  const oreGood = $derived.by(() => {
    const out = new Map<string, number>();
    if (!classes.size) return out;
    const sums = new Map<string, [number, number]>();
    for (const a of areas)
      for (const r of a.rocks) {
        const c = classOf(a.id, r.rockType);
        if (!c || !r.rollable) continue;
        for (const o of r.ores) {
          const w = r.share * o.chance * o.meanShare;
          const s = sums.get(o.element) ?? [0, 0];
          sums.set(o.element, [s[0] + w * (c.easy + c.manageable), s[1] + w]);
        }
      }
    for (const [ore, [g, w]] of sums) if (w > 0) out.set(ore, g / w);
    return out;
  });

  const area = $derived(areasById.get(miningUi.area) ?? null);
  const rock = $derived(area?.rocks.find((r) => r.rockType === miningUi.rockType) ?? null);
  const pick = $derived(miningUi.pick as Pick);

  /** The most common rollable rock type of an area. */
  const commonRock = (id: string) =>
    [...(areasById.get(id)?.rocks ?? [])].filter((r) => r.rollable).sort((a, b) => b.share - a.share)[0]?.rockType ?? "";

  // Keep the selection valid as the data and the picks change.
  $effect(() => {
    if (miningUi.mode !== "place") return;
    if (!place && firstWithRocks) miningUi.place = firstWithRocks.id;
    const node = place?.node;
    if (node && node.areas.length && !node.areas.includes(miningUi.area)) {
      miningUi.area = node.areas[0];
      miningUi.rockType = commonRock(node.areas[0]);
    }
  });
  $effect(() => {
    if (area && !rock) miningUi.rockType = commonRock(area.id);
  });
  $effect(() => {
    if (rock && !rock.ores.some((o) => o.element === miningUi.ore)) {
      miningUi.ore = [...rock.ores].sort((a, b) => b.meanShare * b.chance - a.meanShare * a.chance)[0]?.element ?? "";
    }
  });

  const ship = $derived.by(() => {
    const own = ownLasers(loadout);
    if (!own.length) return null;
    const state = miningStore.state;
    const name = state?.saved.find((s) => s.id === state.basedOn)?.name ?? "ship";
    // The classes count the crew aboard too, so the name says so.
    const crew = [
      ...new Set(loadout.lasers.flatMap((l) => (l.crew ? [state?.saved.find((s) => s.id === l.crew)?.owner ?? l.who ?? "crew"] : []))),
    ];
    return crew.length ? `${name} + ${crew.join(", ")}` : name;
  });

  // The place quality rolls at: Roll at's pick, else the picked place when
  // the area has it, else the area's first place.
  const qualityAt = $derived.by(() => {
    if (!area) return null;
    if (area.places.some((p) => p.id === miningUi.at)) return miningUi.at;
    if (miningUi.mode === "place" && place && area.places.some((p) => p.id === place.node.id)) return place.node.id;
    return area.places[0]?.id ?? null;
  });

  // The ship's score at a place, for the tree: each of its tables' good
  // share (easy and manageable rolls) by rock share, the tables averaged.
  function placeScore(n: PlaceNodeView): number | null {
    if (!ship || !classes.size) return null;
    let sum = 0;
    let k = 0;
    for (const id of n.areas) {
      const a = areasById.get(id);
      if (!a) continue;
      let w = 0;
      let g = 0;
      for (const r of a.rocks) {
        const c = classOf(a.id, r.rockType);
        if (!c) continue;
        w += r.share;
        g += r.share * (c.easy + c.manageable);
      }
      if (w > 0) {
        sum += g / w;
        k++;
      }
    }
    return k ? Math.round((sum / k) * 100) : null;
  }
  const request = $derived(
    area && rock?.rollable
      ? {
          area: area.id,
          rockType: rock.rockType,
          pick: pickRequest(pick, miningUi.ore, oreName(name(miningUi.ore))).view,
          place: qualityAt,
        }
      : null,
  );
  // The classes depend on the loadout, so it is part of what a roll is for.
  const key = $derived(request ? JSON.stringify([request, loadout.lasers, loadout.gadget]) : "");
  const stale = $derived(miningStore.generating || miningStore.rolledKey !== key);

  $effect(() => {
    const r = request;
    const k = key;
    if (!r || k === miningStore.rolledKey) return;
    const t = setTimeout(() => void rollRock(channel, r, k), ROLL_DEBOUNCE_MS);
    return () => clearTimeout(t);
  });

  function selectPlace(n: PlaceNodeView) {
    miningUi.place = n.id;
  }
  function selectRock(areaId: string, rockType: string) {
    miningUi.area = areaId;
    miningUi.rockType = rockType;
  }
  function selectOre(id: string) {
    miningUi.ore = id;
    // The richest rock type for it.
    const best = areas
      .flatMap((a) => a.rocks.flatMap((r) => (r.rollable ? (r.ores.find((o) => o.element === id) ? [{ a, r }] : []) : [])))
      .sort(
        (x, y) =>
          (y.r.ores.find((o) => o.element === id)?.meanShare ?? 0) - (x.r.ores.find((o) => o.element === id)?.meanShare ?? 0),
      )[0];
    if (best) selectRock(best.a.id, best.r.rockType);
  }
  $effect(() => {
    if (miningUi.mode === "ore" && !miningUi.ore && ores[0]) selectOre(ores[0].id);
  });

  function setMode(mode: "place" | "ore") {
    miningUi.mode = mode;
    if (mode === "ore" && !miningUi.ore && ores[0]) selectOre(ores[0].id);
  }

  function use() {
    if (!rock || !area) return;
    useRolled({
      pick: pickRequest(pick, miningUi.ore, oreName(name(miningUi.ore))).label,
      rockType: rock.name,
      place: miningStore.rolled?.place?.name ?? (miningUi.mode === "place" && place ? place.node.name : area.name),
    });
  }
</script>

{#if miningStore.areasError}
  <p class="error">{miningStore.areasError}</p>
{:else if !miningStore.areas || !miningStore.places}
  <p class="dim">Loading spawn tables…</p>
{:else}
  <div class="find" class:classes-stale={miningStore.classTableStale}>
    <nav class="side">
      <div class="sticky">
        <Segmented
          class="mode"
          options={[
            { value: "ore", label: "By ore" },
            { value: "place", label: "By place" },
          ]}
          value={miningUi.mode}
          onchange={(v) => setMode(v as "place" | "ore")}
        />
      </div>
      {#if miningUi.mode === "place"}
        <PlaceTree {systems} selected={miningUi.place} score={placeScore} onselect={selectPlace} />
      {:else}
        <input
          class="input search"
          type="search"
          placeholder="Find an ore"
          aria-label="Find an ore"
          bind:value={oreQuery}
        />
        <div class="ores">
          {#if oreGood.size}<span class="oh"><span>Ore</span><span title="Of the ore you'd find, how much sits in rocks your {ship ?? 'ship'} cracks easy or manageable">everyday</span></span>{/if}
          {#each shownOres as o (o.id)}
            {@const g = oreGood.get(o.id)}
            <button class:on={o.id === miningUi.ore} onclick={() => selectOre(o.id)}>
              <span class="on-nm">{o.name}</span>
              {#if oreGood.size}
                <span class="g"><i style:width="{(g ?? 0) * 100}%"></i></span>
                <b>{g === undefined ? "—" : Math.round(g * 100)}</b>
              {/if}
            </button>
          {/each}
        </div>
      {/if}
      {#if !miningStore.areas.places}
        <p class="dim note">Places couldn't be read from the game files: areas show their file names.</p>
      {/if}
    </nav>

    {#if miningUi.mode === "place"}
      {#if place}
        <PlaceCard
          node={place.node}
          parent={place.parent}
          areas={areasById}
          area={miningUi.area}
          rockType={miningUi.rockType}
          {classOf}
          {ship}
          onpick={selectRock}
        />
      {:else}
        <p class="dim">No place with ship rocks.</p>
      {/if}
    {:else if miningUi.ore}
      <OreCard
        ore={miningUi.ore}
        name={oreName(name(miningUi.ore))}
        {areas}
        area={miningUi.area}
        rockType={miningUi.rockType}
        {classOf}
        {ship}
        onpick={selectRock}
      />
    {/if}

    {#if rock}
      <RockTypeCard
        {catalog}
        {rock}
        where={area?.name ?? ""}
        places={area?.places ?? []}
        at={qualityAt ?? ""}
        rolled={miningStore.rolled}
        {stale}
        error={miningStore.generateError}
        {pick}
        ore={miningUi.ore}
        {ship}
        onpick={(p) => (miningUi.pick = p)}
        onore={(id) => (miningUi.ore = id)}
        onat={(id) => (miningUi.at = id)}
        onuse={use}
      />
    {/if}
  </div>
{/if}

<style>
  /* the class table is still being worked out for the current loadout */
  .classes-stale :global(.cbar) {
    opacity: 0.35;
    transition: opacity 0.15s;
  }
  .find {
    container-type: inline-size;
    display: grid;
    grid-template-columns: 220px minmax(0, 1fr) minmax(0, 1.1fr);
    gap: 16px;
    align-items: start;
  }
  .side {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-height: calc(100vh - 160px);
    overflow: auto;
    padding-right: 4px;
    scrollbar-width: thin;
    scrollbar-color: var(--border) transparent;
  }
  .sticky {
    position: sticky;
    top: 0;
    z-index: 2;
    padding-bottom: 4px;
    background: var(--bg);
  }
  .search {
    position: sticky;
    top: 40px;
    z-index: 2;
  }
  .oh {
    display: flex;
    justify-content: space-between;
    padding: 4px 8px;
    font-size: 0.66rem;
    color: var(--text-faint);
    text-transform: var(--label-transform);
    letter-spacing: 0.08em;
  }
  .ores button {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto auto;
    gap: 8px;
    align-items: center;
  }
  .ores .g {
    width: 40px;
    height: 4px;
    border-radius: 2px;
    background: color-mix(in srgb, var(--text) 8%, transparent);
    overflow: hidden;
  }
  .ores .g i {
    display: block;
    height: 100%;
    background: color-mix(in srgb, var(--good) 70%, transparent);
  }
  .ores b {
    width: 24px;
    font-family: var(--font-mono);
    font-size: 0.72rem;
    font-weight: var(--weight-medium);
    color: var(--text-dim);
    text-align: right;
  }
  .on-nm {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .side :global(.mode) {
    display: grid;
    grid-template-columns: 1fr 1fr;
  }
  .ores {
    display: flex;
    flex-direction: column;
  }
  .ores button {
    padding: 5px 8px;
    border: 0;
    border-radius: 6px;
    background: none;
    font: inherit;
    font-size: 0.875rem;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .ores button:hover {
    background: color-mix(in srgb, var(--text) 4%, transparent);
  }
  .ores button.on {
    background: var(--accent-fill);
    color: var(--text-strong);
  }
  .note {
    font-size: var(--text-small);
  }
  @media (max-width: 1100px) {
    .find {
      grid-template-columns: 200px minmax(0, 1fr);
    }
    .find > :global(:last-child) {
      grid-column: 1 / -1;
    }
  }
  @media (max-width: 760px) {
    .find {
      grid-template-columns: minmax(0, 1fr);
    }
    .side {
      max-height: 240px;
    }
  }
</style>
