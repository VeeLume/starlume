<script lang="ts">
  // Rocks that really spawn, rolled from the game's own spawn rules (the
  // rock generator in mod-mining). By place: an area and one of its rock
  // types, then which rock — most common (the most frequent ore mix),
  // average (median mass), smallest, largest, most of an ore,
  // best quality of an ore, or a typical one of a class for the loadout. By
  // ore: the rock types where that ore takes the largest share when present.
  // The rolled rock goes into the scan editor below and is solved there.
  import { Segmented } from "@veelume/ui";
  import type { MiningCatalogView, PickView, RockClassView } from "$lib/bindings";
  import { generateRock, miningStore } from "$lib/state/mining.svelte";

  let { catalog, channel }: { catalog: MiningCatalogView; channel: string } = $props();

  type Mode = "place" | "ore";
  let mode = $state<Mode>("place");
  let areaId = $state("");
  let rockId = $state("");
  let oreId = $state("");
  let qualityOre = $state("");

  const areas = $derived(miningStore.areas?.areas ?? []);
  const area = $derived(areas.find((a) => a.id === areaId) ?? null);
  const rock = $derived(area?.rocks.find((r) => r.rockType === rockId) ?? null);
  const elementName = $derived(new Map(catalog.elements.map((e) => [e.id, e.name])));
  const name = (id: string) => elementName.get(id) ?? "Unknown";
  const pct = (v: number) => `${Math.round(v * 100)} %`;

  // Keep the picks valid as areas and rock types change.
  $effect(() => {
    if (!area && areas.length) areaId = areas[0].id;
  });
  $effect(() => {
    if (area && !rock) rockId = area.rocks.find((r) => r.rollable)?.rockType ?? area.rocks[0]?.rockType ?? "";
  });
  $effect(() => {
    if (rock && !rock.ores.some((o) => o.element === qualityOre)) qualityOre = rock.ores[0]?.element ?? "";
  });

  // By ore: rock types holding it, largest share when present first.
  const oreSources = $derived(
    oreId
      ? areas
          .flatMap((a) =>
            a.rocks.flatMap((r) => {
              const o = r.ores.find((x) => x.element === oreId);
              return o && r.rollable ? [{ area: a, rock: r, odds: o }] : [];
            }),
          )
          .sort((x, y) => y.odds.meanShare - x.odds.meanShare || y.odds.chance - x.odds.chance)
          .slice(0, 8)
      : [],
  );
  const ores = $derived(
    [...new Set(areas.flatMap((a) => a.rocks.flatMap((r) => r.ores.map((o) => o.element))))]
      .map((id) => ({ id, name: name(id) }))
      .sort((a, b) => a.name.localeCompare(b.name)),
  );

  function roll(pick: PickView, label: string, a = area, r = rock) {
    if (!a || !r) return;
    void generateRock(channel, { area: a.id, rockType: r.rockType, pick, place: null }, `${label} · ${r.name} · ${a.name}`);
  }

  function pickSource(i: number, pick: "mostOre" | "bestQuality") {
    const s = oreSources[i];
    if (!s) return;
    areaId = s.area.id;
    rockId = s.rock.rockType;
    roll(
      pick === "mostOre" ? { kind: "mostOre", element: oreId } : { kind: "bestQuality", element: oreId },
      pick === "mostOre" ? `Most ${name(oreId)}` : `Best ${name(oreId)}`,
      s.area,
      s.rock,
    );
  }

  const classes: { id: RockClassView; label: string }[] = [
    { id: "safe", label: "Safe" },
    { id: "okay", label: "Okay" },
    { id: "dangerous", label: "Dangerous" },
    { id: "tooStrong", label: "Too strong" },
    { id: "impossible", label: "Impossible" },
  ];
  const hasLaser = $derived((miningStore.state?.current.lasers.length ?? 0) > 0);
  const shares = $derived(miningStore.generated?.rock.classes ?? null);
  const shareOf = (c: RockClassView) =>
    shares === null
      ? null
      : { safe: shares.safe, okay: shares.okay, dangerous: shares.dangerous, tooStrong: shares.tooStrong, impossible: shares.impossible }[c];

  const placesNote = $derived(
    miningStore.areas?.places === "ready"
      ? null
      : miningStore.areas?.places === "building"
        ? "Mapping where rocks spawn in the background (once per game build) — areas show game-file names until then."
        : "Where rocks spawn gets mapped in the background once per game build, while Star Citizen isn't running.",
  );
</script>

<div class="gen">
  <Segmented
    options={[
      { value: "place", label: "By place" },
      { value: "ore", label: "By ore" },
    ]}
    value={mode}
    onchange={(v) => (mode = v as Mode)}
  />

  {#if miningStore.areasError}
    <p class="error">{miningStore.areasError}</p>
  {:else if !miningStore.areas}
    <p class="dim small">Loading spawn tables…</p>
  {:else if mode === "place"}
    <select class="select wide" bind:value={areaId} aria-label="Area">
      {#each areas as a (a.id)}
        <option value={a.id}>{a.name}{a.fromRecord ? " *" : ""}</option>
      {/each}
    </select>
    {#if area}
      <select class="select wide" bind:value={rockId} aria-label="Rock type">
        {#each area.rocks as r (r.rockType)}
          <option value={r.rockType} disabled={!r.rollable}>{r.name} — {pct(r.share)} of rocks{r.rollable ? "" : " (no mesh)"}</option>
        {/each}
      </select>
    {/if}
    {#if rock}
      <p class="dim small ores">
        {#each rock.ores as o, i (o.element)}{i ? " · " : ""}{name(o.element)} {pct(o.chance)} of rocks, ~{pct(o.meanShare)}{/each}
      </p>
      <div class="row">
        <button class="btn" onclick={() => roll({ kind: "mostCommon" }, "Most common")} title="The most frequent ore mix, and its most typical rock"
          >Most common</button
        >
        <button class="btn" onclick={() => roll({ kind: "average" }, "Average")}>Average</button>
        <button class="btn" onclick={() => roll({ kind: "smallest" }, "Smallest")}>Smallest</button>
        <button class="btn" onclick={() => roll({ kind: "largest" }, "Largest")}>Largest</button>
      </div>
      <div class="row">
        <select class="select" bind:value={qualityOre} aria-label="Ore">
          {#each rock.ores as o (o.element)}<option value={o.element}>{name(o.element)}</option>{/each}
        </select>
        <button class="btn" onclick={() => roll({ kind: "mostOre", element: qualityOre }, `Most ${name(qualityOre)}`)}>Most ore</button>
        <button class="btn" onclick={() => roll({ kind: "bestQuality", element: qualityOre }, `Best ${name(qualityOre)}`)}
          >Best quality</button
        >
      </div>
      <div class="row" title={hasLaser ? "" : "Add a laser to sort rocks by how they fly for it"}>
        {#each classes as c (c.id)}
          {@const s = shareOf(c.id)}
          <button class="btn subtle cls" disabled={!hasLaser} onclick={() => roll({ kind: "class", class: c.id }, c.label)}>
            {c.label}{s !== null ? ` ${pct(s)}` : ""}
          </button>
        {/each}
      </div>
    {/if}
  {:else}
    <select class="select wide" bind:value={oreId} aria-label="Ore">
      <option value="">Pick an ore…</option>
      {#each ores as o (o.id)}<option value={o.id}>{o.name}</option>{/each}
    </select>
    {#if oreId}
      <table class="data">
        <thead><tr><th>Rock</th><th class="num">Share</th><th class="num">In</th><th></th></tr></thead>
        <tbody>
          {#each oreSources as s, i (s.area.id + s.rock.rockType)}
            <tr>
              <td>{s.rock.name}<span class="dim small"> · {s.area.name}</span></td>
              <td class="num" title="Its share of the rock when present">~{pct(s.odds.meanShare)}</td>
              <td class="num dim" title="Share of these rocks holding it">{pct(s.odds.chance)}</td>
              <td class="acts">
                <button class="btn subtle" onclick={() => pickSource(i, "mostOre")}>Most</button>
                <button class="btn subtle" onclick={() => pickSource(i, "bestQuality")}>Best q</button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  {/if}

  {#if miningStore.generateError}
    <p class="error">{miningStore.generateError}</p>
  {:else if miningStore.generated}
    {@const g = miningStore.generated}
    <p class="dim small">
      {g.label}: {Math.round(g.rock.mass).toLocaleString()} t, {g.rock.scu.toFixed(1)} SCU — one of {g.rock.rolled.toLocaleString()} rolls{g
        .rock.pickShare !== null
        ? `, its ore mix in ${pct(g.rock.pickShare)} of them`
        : ""}{g.rock.place ? `, quality as at ${g.rock.place.name}` : ""}.
    </p>
  {/if}
  {#if placesNote}<p class="dim small">{placesNote}{areas.some((a) => a.fromRecord) ? " (* game-file name)" : ""}</p>{/if}
</div>

<style>
  .gen {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .wide {
    max-width: none;
    width: 100%;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }
  .row .select {
    flex: 1 1 120px;
  }
  .cls {
    padding: 3px 8px;
  }
  .small {
    font-size: var(--text-small);
    margin: 0;
  }
  .ores {
    line-height: 1.4;
  }
  .num {
    text-align: right;
    white-space: nowrap;
  }
  .acts {
    white-space: nowrap;
    text-align: right;
  }
  .acts .btn {
    padding: 2px 6px;
  }
</style>
