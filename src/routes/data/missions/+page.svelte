<script lang="ts">
  // Mission catalog on the kit surface: the pooled contract templates (one
  // row per mission a player perceives), derive → search → facet → sort
  // through the kit pipeline, URL-backed browse state, windowed list, and
  // the design-doc accordion (`one`-mode expansion — a deep read, so opening
  // a mission closes the previous one). The whole list is cached per channel
  // in catalog.svelte.ts; everything below runs client-side (the Hearth
  // model — data_missions ships the whole pooled list).
  import { onMount } from "svelte";
  import {
    Surface,
    Expand,
    createBrowseState,
    createExpansion,
    type FacetDef,
    type Fact,
    type Status,
  } from "@veelume/ui";
  import type { MissionEntryView, MissionPlaceView } from "$lib/bindings";
  import { dataStore, ensureChannel } from "$lib/state/data.svelte";
  import { getMissions } from "$lib/state/catalog.svelte";
  import { blueprintsStore, loadOwnedBlueprints } from "$lib/state/blueprints.svelte";
  import CatalogDescriptions from "$lib/components/catalog/CatalogDescriptions.svelte";

  type MissionRow = MissionEntryView & { key: string; title: string };

  let missions = $state<MissionEntryView[]>([]);
  let browseError = $state("");
  let ready = $state(false);
  let loading = $state(false);

  const displayName = (m: MissionEntryView) => m.title ?? m.debug_name;
  const payoutOf = (m: MissionEntryView) => m.payout.fixed ?? m.payout.estimate ?? 0;

  const browse = createBrowseState({
    q: { kind: "text" },
    category: { kind: "one", default: "" },
    faction: { kind: "one", default: "" },
    legality: { kind: "one", default: "" },
    bp: { kind: "one", default: "" },
    sort: { kind: "one", default: "name", narrows: false },
  });

  // Accordion state is page-local, never the URL (the kit rule); `one` mode
  // is the deep-read accordion the design doc pinned for missions.
  const expanded = createExpansion("one");

  /** Distinct values by frequency — facet options derive from the data. */
  function byFrequency(pick: (m: MissionEntryView) => string | undefined | null) {
    const counts = new Map<string, number>();
    for (const m of missions) {
      const v = pick(m);
      if (v) counts.set(v, (counts.get(v) ?? 0) + 1);
    }
    return [...counts.entries()].sort((x, y) => y[1] - x[1]).map(([v]) => v);
  }

  const facets = $derived<FacetDef<MissionRow>[]>([
    {
      id: "category",
      label: "Category",
      mode: "one",
      options: [
        { value: "", label: "All categories" },
        ...byFrequency((m) => m.category?.name).map((name) => ({
          value: name,
          label: name,
          test: (r: MissionRow) => r.category?.name === name,
        })),
      ],
    },
    {
      id: "faction",
      label: "Faction",
      mode: "one",
      options: [
        { value: "", label: "All factions" },
        ...byFrequency((m) => m.faction?.name).map((name) => ({
          value: name,
          label: name,
          test: (r: MissionRow) => r.faction?.name === name,
        })),
      ],
    },
    {
      id: "legality",
      label: "Legality",
      mode: "one",
      options: [
        { value: "", label: "Legal + illegal" },
        { value: "legal", label: "Legal only", test: (r) => !r.illegal },
        { value: "illegal", label: "Illegal only", test: (r) => r.illegal },
      ],
    },
    {
      id: "bp",
      label: "Rewards",
      mode: "one",
      options: [
        { value: "", label: "All rewards" },
        {
          value: "bp",
          label: "Blueprint rewards",
          test: (r) => r.blueprint_rewards.length > 0,
        },
      ],
    },
  ]);

  const descriptor = $derived({
    sources: () => missions,
    derive: (ms: MissionEntryView[]): MissionRow[] =>
      ms.map((m) => ({ ...m, key: m.mission_id, title: displayName(m) })),
    searchIn: (r: MissionRow) => [r.title, r.debug_name],
    facets,
    sorts: [
      {
        value: "name",
        label: "Name",
        compare: (a: MissionRow, b: MissionRow) => a.title.localeCompare(b.title),
      },
      {
        value: "payout",
        label: "Payout",
        compare: (a: MissionRow, b: MissionRow) => payoutOf(b) - payoutOf(a),
      },
    ],
  });

  const status = $derived<Status>(
    browseError ? "error" : loading && missions.length === 0 ? "loading" : "ready",
  );

  const fmtUec = (n: number) => n.toLocaleString("en-US");

  function payoutLabel(m: MissionEntryView): string {
    if (m.payout.fixed != null) return `${fmtUec(m.payout.fixed)} aUEC`;
    if (m.payout.estimate != null) return `~${fmtUec(m.payout.estimate)} aUEC`;
    return m.payout.calculated ? "calculated" : "—";
  }

  /** Short scalar facts for the top of a row expansion. */
  function missionFacts(m: MissionEntryView): Fact[] {
    const facts: Fact[] = [{ label: "Payout", value: payoutLabel(m) }];
    if (m.payout.buy_in > 0)
      facts.push({ label: "Buy-in", value: `${fmtUec(m.payout.buy_in)} aUEC` });
    if (m.payout.time_to_complete > 0)
      facts.push({ label: "Time", value: `${m.payout.time_to_complete} min` });
    if (m.cooldown_seconds)
      facts.push({ label: "Cooldown", value: `${Math.round(m.cooldown_seconds / 60)} min` });
    if (m.difficulty)
      facts.push({
        label: "Difficulty",
        value: `skill ${m.difficulty.mechanical_skill} · load ${m.difficulty.mental_load} · risk ${m.difficulty.risk_of_loss} · know ${m.difficulty.game_knowledge}`,
      });
    facts.push({
      label: "Offered",
      value: `${m.instance_count} spot${m.instance_count === 1 ? "" : "s"}`,
    });
    const flags = [
      m.illegal ? "illegal" : null,
      m.once_only ? "once only" : null,
      m.shareable ? "shareable" : null,
    ].filter(Boolean);
    if (flags.length) facts.push({ label: "Flags", value: flags.join(" · ") });
    return facts;
  }

  // Which "Available in" regions are expanded (indices into the open mission's
  // locations). All collapsed by default; reset when a different mission opens.
  let openRegions = $state<number[]>([]);
  const isRegionOpen = (ri: number) => openRegions.includes(ri);
  function toggleRegion(ri: number) {
    openRegions = isRegionOpen(ri) ? openRegions.filter((x) => x !== ri) : [...openRegions, ri];
  }
  function toggleRow(key: string) {
    expanded.toggle(key);
    openRegions = [];
  }

  // Group a region's places by DCB LocationKind into display buckets, ordered.
  // Unmapped/unrecognized kinds fall through to "Other".
  const PLACE_GROUPS: { label: string; kinds: string[] }[] = [
    { label: "Planets", kinds: ["Planet", "S42_Planet"] },
    { label: "Moons", kinds: ["Moon", "S42_Moon"] },
    { label: "Landing zones", kinds: ["LandingZone"] },
    { label: "Outposts", kinds: ["Outpost", "Outpost_InvalidQT"] },
    { label: "Structures", kinds: ["Manmade", "Manmade_VisibleOnInteraction", "ManmadeJumpPoint"] },
    { label: "Asteroids", kinds: ["Asteroid", "Asteroid_ValidQT"] },
    { label: "Jump points", kinds: ["JumpPoint"] },
    {
      label: "Points of interest",
      kinds: [
        "PointOfInterest",
        "Anomaly",
        "CardinalPoint",
        "NavPoint",
        "QuantumTracePoint",
        "YouAreHere",
      ],
    },
  ];
  const placeName = (p: MissionPlaceView) => p.name ?? p.record_name;
  const sortPlaces = (arr: MissionPlaceView[]) =>
    arr.toSorted((a, b) => placeName(a).localeCompare(placeName(b)));

  function groupPlaces(places: MissionPlaceView[]): { label: string; places: MissionPlaceView[] }[] {
    const byKind = new Map<string, MissionPlaceView[]>();
    for (const p of places) {
      const k = p.kind ?? "";
      (byKind.get(k) ?? byKind.set(k, []).get(k)!).push(p);
    }
    const used = new Set<string>();
    const groups: { label: string; places: MissionPlaceView[] }[] = [];
    for (const g of PLACE_GROUPS) {
      const items: MissionPlaceView[] = [];
      for (const k of g.kinds) {
        const arr = byKind.get(k);
        if (arr) {
          items.push(...arr);
          used.add(k);
        }
      }
      if (items.length) groups.push({ label: g.label, places: sortPlaces(items) });
    }
    const other: MissionPlaceView[] = [];
    for (const [k, arr] of byKind) if (!used.has(k)) other.push(...arr);
    if (other.length) groups.push({ label: "Other", places: sortPlaces(other) });
    return groups;
  }

  /** Does this mission have any post-description detail (the right column)? */
  const hasDetail = (m: MissionEntryView) =>
    m.scrip.length > 0 ||
    m.item_rewards.length > 0 ||
    m.reputation.length > 0 ||
    m.blueprint_rewards.length > 0 ||
    m.rep_required.length > 0 ||
    m.chain_required.length > 0 ||
    m.cargo.length > 0 ||
    m.locations.length > 0 ||
    m.encounters.length > 0;

  onMount(() => {
    // Owned-blueprint set for the pool decoration (cached read; empty until
    // fetched on the Me page — undecorated is the correct default).
    void loadOwnedBlueprints();
    void (async () => {
      const channel = await ensureChannel();
      ready = true;
      if (!channel) return;
      // Cache hit renders instantly (usually prefetched at startup).
      loading = true;
      const r = await getMissions(channel);
      loading = false;
      if (typeof r === "string") browseError = r;
      else missions = r;
    })();
  });
</script>

{#if ready && !dataStore.channel}
  <p class="dim">
    No game data loaded — go to <a href="/data">Overview</a> and press Load first.
  </p>
{:else}
  {#if browseError}
    <p class="error">{browseError}</p>
  {/if}

  <div class="flex h-full min-h-0 flex-col">
    <Surface.Root {descriptor} {browse} class="min-h-0 flex-1">
      <Surface.List {status}>
        {#snippet row(r: MissionRow)}
          <Expand.Row
            title={r.title}
            open={expanded.has(r.key)}
            ontoggle={() => toggleRow(r.key)}
          >
            {#snippet meta()}
              {#if r.category?.name}<span class="badge">{r.category.name}</span>{/if}
              {#if r.faction?.name}<span class="badge">{r.faction.name}</span>{/if}
              {#if r.illegal}<span class="badge danger" title="Illegal">⚠</span>{/if}
              {#if r.once_only}<span class="badge" title="Once only">1×</span>{/if}
              {#if r.shareable}<span class="badge" title="Shareable">⇄</span>{/if}
              {#if r.blueprint_rewards.length > 0}<span
                  class="badge accent"
                  title="Blueprint rewards">BP</span
                >{/if}
              {#if r.facts.crimestat !== "none"}<span
                  class="badge danger"
                  title="Killing friendly NPCs risks a crimestat{r.facts.crimestat === 'high'
                    ? ' — no HUD markers to tell friend from foe'
                    : ''}">CS risk</span
                >{/if}
            {/snippet}
            {#snippet right()}
              <span class="metric">{payoutLabel(r)}</span>
            {/snippet}

            <Expand.Facts facts={missionFacts(r)} />

            {#if r.description && hasDetail(r)}
              <Expand.Cols>
                {#snippet main()}
                  <CatalogDescriptions
                    flush
                    rich
                    blocks={[{ label: "Description", text: r.description ?? "" }]}
                  />
                {/snippet}
                {#snippet side()}
                  {@render detail(r)}
                {/snippet}
              </Expand.Cols>
            {:else if r.description}
              <CatalogDescriptions
                flush
                rich
                blocks={[{ label: "Description", text: r.description ?? "" }]}
              />
            {:else if hasDetail(r)}
              {@render detail(r)}
            {/if}

            <p class="mono dim gid">{r.mission_id} · {r.debug_name}</p>
          </Expand.Row>
        {/snippet}
      </Surface.List>
    </Surface.Root>
  </div>
{/if}

{#snippet detail(m: MissionRow)}
  <div class="catalog-detail">
    {#if m.scrip.length > 0 || m.item_rewards.length > 0 || m.reputation.length > 0}
      <div>
        <h5>Rewards</h5>
        <ul>
          {#each m.scrip as s, i (i)}
            <li>{s.amount}× {s.name ?? "scrip"}</li>
          {/each}
          {#each m.item_rewards as it (it.entity_guid)}
            <li>{it.amount}× {it.name ?? it.entity_guid}</li>
          {/each}
          {#each m.reputation as r, i (i)}
            <li class="dim">
              reputation {r.amount != null ? `+${r.amount}` : "(calculated)"}
            </li>
          {/each}
        </ul>
      </div>
    {/if}

    {#if m.blueprint_rewards.length > 0}
      <div>
        <h5>Blueprint pools</h5>
        {#each m.blueprint_rewards as pool (pool.pool_name)}
          {@const ownedInPool = pool.blueprints.filter((bp) =>
            blueprintsStore.owns(bp.blueprint_record_guid),
          ).length}
          <div class="pool">
            <span class="pool-head">
              {pool.pool_name || "Pool"}
              <span class="dim">({Math.round(pool.chance * 100)}% draw)</span>
              {#if ownedInPool > 0}
                <span class="owned-summary" title="Blueprints you already own">
                  {ownedInPool}/{pool.blueprints.length} owned
                </span>
              {/if}
            </span>
            <ul>
              {#each pool.blueprints as bp (bp.blueprint_record_guid)}
                {@const owned = blueprintsStore.owns(bp.blueprint_record_guid)}
                <li class:owned>
                  {#if owned}<span class="owned-mark" title="Owned">✓</span>{/if}
                  {bp.name ?? bp.blueprint_record_guid} <span class="dim">w{bp.weight}</span>
                </li>
              {/each}
            </ul>
          </div>
        {/each}
      </div>
    {/if}

    {#if m.rep_required.length > 0}
      <div>
        <h5>Reputation required</h5>
        <ul>
          {#each m.rep_required as r, i (i)}
            <li>
              {r.faction ?? "Unknown faction"}
              {#if r.min_rank}from {r.min_rank}{/if}
              {#if r.max_rank}up to {r.max_rank}{/if}
              {#if r.exclude}<span class="dim">(excluded)</span>{/if}
            </li>
          {/each}
        </ul>
      </div>
    {/if}

    {#if m.chain_required.length > 0}
      <div>
        <h5>Requires completing</h5>
        <ul>
          {#each m.chain_required as c (c.mission_id)}
            <li>
              {c.title ?? c.mission_id}{#if c.once_only}
                <span class="dim">(once only)</span>{/if}
            </li>
          {/each}
        </ul>
      </div>
    {/if}

    {#if m.cargo.length > 0}
      <div>
        <h5>Cargo manifest</h5>
        <ul>
          {#each m.cargo as leg, i (i)}
            <li>
              {leg.commodity ?? "Unknown commodity"}
              <span class="dim">
                {leg.min_scu === leg.max_scu
                  ? `${leg.max_scu} SCU`
                  : `${leg.min_scu}–${leg.max_scu} SCU`}
                {#if leg.max_box > 0}· max box {leg.max_box}{/if}
              </span>
            </li>
          {/each}
        </ul>
      </div>
    {/if}

    {#if m.locations.length > 0}
      <div class="locations">
        <h5>Available in</h5>
        {#each m.locations as region, ri (ri)}
          <div class="region">
            <button
              class="pill region-toggle"
              class:active={isRegionOpen(ri)}
              onclick={() => toggleRegion(ri)}
            >
              {region.system}{region.name ? ` — ${region.name}` : ""}
              <span class="count">{region.places.length}</span>
              <span class="region-caret">{isRegionOpen(ri) ? "▾" : "▸"}</span>
            </button>
            {#if isRegionOpen(ri)}
              <div class="place-groups">
                {#each groupPlaces(region.places) as g (g.label)}
                  <div class="place-group">
                    <span class="place-group-label">{g.label}</span>
                    <div class="place-list">
                      {#each g.places as p (p.record_name)}
                        <span class="place" title={p.record_name}>{placeName(p)}</span>
                      {/each}
                    </div>
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}

    {#if m.encounters.length > 0}
      <div>
        <h5>Encounters</h5>
        {#each m.encounters as enc, i (i)}
          <div class="pool">
            <span class="pool-head">
              {enc.label}
              {#if enc.difficulty}<span class="dim">({enc.difficulty})</span>{/if}
            </span>
            <ul>
              {#each enc.waves as wave, j (j)}
                <li>
                  {wave.name || `Wave ${j + 1}`}:
                  {wave.ships
                    .map(
                      (s) =>
                        `${s.count_min === s.count_max ? s.count_max : `${s.count_min}–${s.count_max}`}× ${
                          s.ships.slice(0, 3).join(" / ") || "ship"
                        }${s.ships.length > 3 ? " / …" : ""}`,
                    )
                    .join("; ")}
                  {#if wave.cargo.length > 0}
                    <span class="dim">cargo: {wave.cargo.join(", ")}</span>
                  {/if}
                </li>
              {/each}
            </ul>
          </div>
        {/each}
      </div>
    {/if}
  </div>
{/snippet}

<style>
  .pool {
    margin-bottom: 6px;
  }
  .pool-head {
    font-weight: var(--weight-medium);
  }
  .owned-summary {
    margin-left: 6px;
    font-size: 0.75rem;
    color: var(--accent);
  }
  li.owned {
    color: var(--accent);
  }
  .owned-mark {
    color: var(--accent);
    font-weight: 700;
    margin-right: 2px;
  }
  .gid {
    margin: 4px 0 2px;
    font-size: 0.72rem;
  }

  /* Available-in: a per-region expander (pill header → grouped place grid). */
  .locations {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .region {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
  }
  .region-toggle {
    max-width: 100%;
  }
  .region-caret {
    font-size: 0.7rem;
    color: var(--text-faint);
  }
  .region-toggle.active .region-caret {
    color: var(--accent);
  }
  .place-groups {
    align-self: stretch;
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin: 2px 0 2px 4px;
  }
  .place-group-label {
    display: block;
    font-family: var(--font-mono);
    font-size: 0.6rem;
    letter-spacing: var(--label-tracking);
    text-transform: uppercase;
    color: var(--text-faint);
    margin-bottom: 2px;
  }
  .place-list {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 1px 16px;
  }
  .place {
    position: relative;
    padding-left: 10px;
    font-size: 0.8rem;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .place::before {
    content: "·";
    position: absolute;
    left: 0;
    color: var(--text-faint);
  }
</style>
