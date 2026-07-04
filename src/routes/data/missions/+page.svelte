<script lang="ts">
  // Mission catalog — the pooled contract templates (one row per mission a
  // player perceives). Rendered through the unified CatalogRow: each row
  // expands inline (accordion) to facts, description, and the reward/location
  // detail. The whole list is cached per channel in catalog.svelte.ts;
  // search/filter/sort/open-row persist across navigation and run client-side.
  import { onMount } from "svelte";
  import type { MissionEntryView, MissionPlaceView } from "$lib/bindings";
  import { dataStore, ensureChannel } from "$lib/state/data.svelte";
  import { getMissions, missionsBrowse, syncMissionsChannel } from "$lib/state/catalog.svelte";
  import CatalogRow from "$lib/components/catalog/CatalogRow.svelte";
  import CatalogFacts from "$lib/components/catalog/CatalogFacts.svelte";
  import CatalogDescriptions from "$lib/components/catalog/CatalogDescriptions.svelte";

  type Fact = { k: string; v: string | number; mono?: boolean };

  const STEP = 60; // rows revealed per scroll batch

  const b = missionsBrowse;
  let missions = $state<MissionEntryView[]>([]);
  let browseError = $state("");
  let ready = $state(false);
  let loading = $state(false);

  const displayName = (m: MissionEntryView) => m.title ?? m.debug_name;
  const payoutOf = (m: MissionEntryView) => m.payout.fixed ?? m.payout.estimate ?? 0;

  /** Distinct category names, by frequency. */
  const categories = $derived.by(() => {
    const counts = new Map<string, number>();
    for (const m of missions) {
      const c = m.category?.name;
      if (c) counts.set(c, (counts.get(c) ?? 0) + 1);
    }
    return [...counts.entries()].sort((x, y) => y[1] - x[1]);
  });

  /** Distinct faction names, by frequency. */
  const factions = $derived.by(() => {
    const counts = new Map<string, number>();
    for (const m of missions) {
      const f = m.faction?.name;
      if (f) counts.set(f, (counts.get(f) ?? 0) + 1);
    }
    return [...counts.entries()].sort((x, y) => y[1] - x[1]);
  });

  const filtered = $derived.by(() => {
    const needle = b.query.trim().toLowerCase();
    let rows = missions.filter((m) => {
      if (b.category && m.category?.name !== b.category) return false;
      if (b.faction && m.faction?.name !== b.faction) return false;
      if (b.legality === "legal" && m.illegal) return false;
      if (b.legality === "illegal" && !m.illegal) return false;
      if (b.bpOnly && m.blueprint_rewards.length === 0) return false;
      if (
        needle &&
        !displayName(m).toLowerCase().includes(needle) &&
        !m.debug_name.toLowerCase().includes(needle)
      )
        return false;
      return true;
    });
    if (b.sort === "payout") {
      rows = rows.toSorted((x, y) => payoutOf(y) - payoutOf(x));
    }
    return rows;
  });

  // Infinite scroll: reveal STEP rows at a time, growing as the sentinel nears
  // the viewport. Reset to the first batch whenever the filtered set changes
  // (new search / filter / sort / freshly loaded data).
  let visible = $state(STEP);
  const shown = $derived(filtered.slice(0, visible));
  $effect(() => {
    // Reset the reveal window whenever the inputs or dataset change.
    void [b.query, b.category, b.faction, b.legality, b.bpOnly, b.sort, missions];
    visible = STEP;
  });
  function loadMore() {
    if (visible < filtered.length) visible = Math.min(visible + STEP, filtered.length);
  }
  function sentinel(node: HTMLElement) {
    const io = new IntersectionObserver(
      (entries) => entries.some((e) => e.isIntersecting) && loadMore(),
      { rootMargin: "600px 0px" },
    );
    io.observe(node);
    return { destroy: () => io.disconnect() };
  }

  const fmtUec = (n: number) => n.toLocaleString("en-US");

  function payoutLabel(m: MissionEntryView): string {
    if (m.payout.fixed != null) return `${fmtUec(m.payout.fixed)} aUEC`;
    if (m.payout.estimate != null) return `~${fmtUec(m.payout.estimate)} aUEC`;
    return m.payout.calculated ? "calculated" : "—";
  }

  /** Short scalar facts for the top of a row expansion. */
  function missionFacts(m: MissionEntryView): Fact[] {
    const facts: Fact[] = [{ k: "Payout", v: payoutLabel(m) }];
    if (m.payout.buy_in > 0) facts.push({ k: "Buy-in", v: `${fmtUec(m.payout.buy_in)} aUEC` });
    if (m.payout.time_to_complete > 0)
      facts.push({ k: "Time", v: `${m.payout.time_to_complete} min` });
    if (m.cooldown_seconds)
      facts.push({ k: "Cooldown", v: `${Math.round(m.cooldown_seconds / 60)} min` });
    if (m.difficulty)
      facts.push({
        k: "Difficulty",
        v: `skill ${m.difficulty.mechanical_skill} · load ${m.difficulty.mental_load} · risk ${m.difficulty.risk_of_loss} · know ${m.difficulty.game_knowledge}`,
      });
    facts.push({ k: "Offered", v: `${m.instance_count} spot${m.instance_count === 1 ? "" : "s"}` });
    const flags = [
      m.illegal ? "illegal" : null,
      m.once_only ? "once only" : null,
      m.shareable ? "shareable" : null,
    ].filter(Boolean);
    if (flags.length) facts.push({ k: "Flags", v: flags.join(" · ") });
    return facts;
  }

  const isOpen = (m: MissionEntryView) => b.detail?.mission_id === m.mission_id;

  // Which "Available in" regions are expanded (indices into the open mission's
  // locations). All collapsed by default; reset when a different mission opens.
  // Ephemeral UI — one mission is expanded at a time.
  let openRegions = $state<number[]>([]);
  const isRegionOpen = (ri: number) => openRegions.includes(ri);
  function toggleRegion(ri: number) {
    openRegions = isRegionOpen(ri) ? openRegions.filter((x) => x !== ri) : [...openRegions, ri];
  }

  function toggle(m: MissionEntryView) {
    const opening = !isOpen(m);
    b.detail = opening ? m : null;
    if (opening) openRegions = [];
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
      kinds: ["PointOfInterest", "Anomaly", "CardinalPoint", "NavPoint", "QuantumTracePoint", "YouAreHere"],
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
    void (async () => {
      const channel = await ensureChannel();
      ready = true;
      if (!channel) return;
      syncMissionsChannel(channel);
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

  <div class="toolbar">
    <input
      type="search"
      placeholder="Search missions by title…"
      bind:value={b.query}
    />
    <select bind:value={b.category}>
      <option value={null}>All categories</option>
      {#each categories as [name, count] (name)}
        <option value={name}>{name} ({count})</option>
      {/each}
    </select>
    <select bind:value={b.faction}>
      <option value={null}>All factions</option>
      {#each factions as [name, count] (name)}
        <option value={name}>{name} ({count})</option>
      {/each}
    </select>
    <select bind:value={b.legality}>
      <option value="all">Legal + illegal</option>
      <option value="legal">Legal only</option>
      <option value="illegal">Illegal only</option>
    </select>
    <label class="check">
      <input type="checkbox" bind:checked={b.bpOnly} />
      Blueprint rewards
    </label>
    <select bind:value={b.sort}>
      <option value="name">Sort: name</option>
      <option value="payout">Sort: payout</option>
    </select>
  </div>

  {#if loading && missions.length === 0}
    <p class="dim">Loading mission catalog…</p>
  {:else if missions.length > 0}
    <p class="dim result-count">{filtered.length} / {missions.length} missions</p>

    <div class="catalog">
      {#each shown as m (m.mission_id)}
        <CatalogRow
          expandable
          title={displayName(m)}
          open={isOpen(m)}
          ontoggle={() => toggle(m)}
        >
          {#snippet meta()}
            {#if m.category?.name}<span class="badge">{m.category.name}</span>{/if}
            {#if m.faction?.name}<span class="badge">{m.faction.name}</span>{/if}
            {#if m.illegal}<span class="badge danger" title="Illegal">⚠</span>{/if}
            {#if m.once_only}<span class="badge" title="Once only">1×</span>{/if}
            {#if m.shareable}<span class="badge" title="Shareable">⇄</span>{/if}
            {#if m.blueprint_rewards.length > 0}<span class="badge accent" title="Blueprint rewards"
                >BP</span
              >{/if}
          {/snippet}
          {#snippet right()}
            <span class="metric">{payoutLabel(m)}</span>
          {/snippet}

          <CatalogFacts items={missionFacts(m)} />

          {#if m.description || hasDetail(m)}
            <div class="catalog-cols" class:single={!m.description || !hasDetail(m)}>
              {#if m.description}
                <CatalogDescriptions
                  flush
                  rich
                  blocks={[{ label: "Description", text: m.description }]}
                />
              {/if}

              {#if hasDetail(m)}
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
                  <div class="pool">
                    <span class="pool-head">
                      {pool.pool_name || "Pool"}
                      <span class="dim">({Math.round(pool.chance * 100)}% draw)</span>
                    </span>
                    <ul>
                      {#each pool.blueprints as bp (bp.blueprint_record_guid)}
                        <li>{bp.name ?? bp.blueprint_record_guid} <span class="dim">w{bp.weight}</span></li>
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
              {/if}
            </div>
          {/if}

          <p class="mono dim gid">{m.mission_id} · {m.debug_name}</p>
        </CatalogRow>
      {/each}
    </div>

    {#if visible < filtered.length}
      <!-- Reveals the next batch as it nears the viewport (infinite scroll). -->
      <div class="sentinel" use:sentinel aria-hidden="true"></div>
    {/if}
  {:else}
    <p class="dim">No missions in this build's catalog.</p>
  {/if}
{/if}

<style>
  .result-count {
    font-size: 0.8rem;
    margin: 0 0 8px;
  }
  .sentinel {
    height: 1px;
  }
  .pool {
    margin-bottom: 6px;
  }
  .pool-head {
    font-weight: var(--weight-medium);
  }
  .gid {
    margin: 4px 10px 6px 52px;
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
