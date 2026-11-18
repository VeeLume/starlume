<script lang="ts">
  // Contracts, the Catalog sub page (pass 2 of the design, Firefly note
  // 'Contracts design direction'). The cooked mission catalog on the kit
  // surface: URL-backed browse, windowed list, one-open accordion. Every
  // column of a row is a value over its caption (model.ts); the faction
  // column draws the rank ladder with the mission's window filled.
  //
  // `?bp=<blueprint guid>` scopes the catalog to the missions that can grant
  // one blueprint (Crafting's recipe sheet and Plans' "Earn" step link here)
  // and shows its odds above the list.
  //
  // Your own rank (the standing read) and your own times arrive in later
  // slices; until then the ladder shows the window without a "you".
  import { onMount } from "svelte";
  import {
    Surface,
    Expand,
    createBrowseState,
    createExpansion,
    type FacetDef,
    type Status,
  } from "@veelume/ui";
  import {
    Boxes,
    CalendarClock,
    Crosshair,
    Flag,
    Fuel,
    Package,
    Radio,
    Recycle,
    ScrollText,
    Search,
    Target,
    Truck,
    Wrench,
    Zap,
  } from "lucide-svelte";
  import type { NavIcon } from "@veelume/ui";
  import type { MissionEncounterView, MissionEntryView, SpawnSlotView } from "$lib/bindings";
  import NoGameData from "$lib/components/catalog/NoGameData.svelte";
  import RichText from "$lib/components/catalog/RichText.svelte";
  import CatalogBar from "./CatalogBar.svelte";
  import { dataStore, ensureChannel } from "$lib/state/data.svelte";
  import { getMissions } from "$lib/state/catalog.svelte";
  import { blueprintsStore, loadOwnedBlueprints } from "$lib/state/blueprints.svelte";
  import {
    blueprintCell,
    blueprintChance,
    blueprintName,
    ladderOf,
    missionTitle,
    needsNote,
    oddsText,
    ownRequirement,
    paysCell,
    rangeText,
    runsFor,
    timeCell,
    whereCell,
    windowText,
    type Ladder,
  } from "./model";

  type MissionRow = MissionEntryView & { key: string; title: string; ladder: Ladder | null };

  let missions = $state<MissionEntryView[]>([]);
  let browseError = $state("");
  let ready = $state(false);
  let loading = $state(false);

  const browse = createBrowseState({
    q: { kind: "text" },
    faction: { kind: "one", default: "" },
    region: { kind: "one", default: "" },
    category: { kind: "one", default: "" },
    gives: { kind: "one", default: "" },
    bp: { kind: "one", default: "", narrows: false },
    sort: { kind: "one", default: "pays", narrows: false },
  });
  const expanded = createExpansion("one");

  // ── Category: an icon and a hue per kind of work ─────────────────────
  const CATEGORY: Record<string, { icon: NavIcon; hue: string }> = {
    Mercenary: { icon: Crosshair, hue: "var(--hue-rose)" },
    "Bounty Hunter": { icon: Target, hue: "var(--hue-apricot)" },
    Delivery: { icon: Package, hue: "var(--hue-sky)" },
    Hauling: { icon: Truck, hue: "var(--hue-teal)" },
    "Hauling - Stellar": { icon: Truck, hue: "var(--hue-teal)" },
    "Hauling - Interstellar": { icon: Truck, hue: "var(--hue-teal)" },
    "Hauling - Planetary": { icon: Truck, hue: "var(--hue-teal)" },
    Refueling: { icon: Fuel, hue: "var(--hue-sand)" },
    Collection: { icon: Boxes, hue: "var(--hue-quality)" },
    Investigation: { icon: Search, hue: "var(--hue-sky)" },
    Maintenance: { icon: Wrench, hue: "var(--hue-sand)" },
    Salvage: { icon: Recycle, hue: "var(--hue-quality)" },
    Racing: { icon: Flag, hue: "var(--hue-apricot)" },
    Priority: { icon: Zap, hue: "var(--hue-apricot)" },
    Appointment: { icon: CalendarClock, hue: "var(--hue-sky)" },
    "Service Beacons": { icon: Radio, hue: "var(--hue-rose)" },
  };
  const categoryOf = (m: MissionEntryView) =>
    CATEGORY[m.category?.name ?? ""] ?? { icon: ScrollText, hue: "var(--text-dim)" };

  // ── Facets derive from the data, most frequent first ─────────────────
  function byFrequency(pick: (m: MissionEntryView) => (string | null | undefined)[]) {
    const counts = new Map<string, number>();
    for (const m of missions) for (const v of new Set(pick(m))) if (v) counts.set(v, (counts.get(v) ?? 0) + 1);
    return [...counts.entries()].sort((x, y) => y[1] - x[1]).map(([v]) => v);
  }
  const regionKey = (system: string, name: string) => (name ? `${name}, ${system}` : system);

  const facets = $derived<FacetDef<MissionRow>[]>([
    {
      id: "faction",
      label: "Faction",
      mode: "one",
      options: [
        { value: "", label: "All factions" },
        ...byFrequency((m) => [m.faction?.name]).map((name) => ({
          value: name,
          label: name,
          test: (r: MissionRow) => r.faction?.name === name,
        })),
      ],
    },
    {
      id: "region",
      label: "Where",
      mode: "one",
      options: [
        { value: "", label: "Everywhere" },
        ...byFrequency((m) => m.locations.map((r) => regionKey(r.system, r.name))).map((key) => ({
          value: key,
          label: key,
          test: (r: MissionRow) => r.locations.some((l) => regionKey(l.system, l.name) === key),
        })),
      ],
    },
    {
      id: "category",
      label: "Category",
      mode: "one",
      options: [
        { value: "", label: "All categories" },
        ...byFrequency((m) => [m.category?.name]).map((name) => ({
          value: name,
          label: name,
          test: (r: MissionRow) => r.category?.name === name,
        })),
      ],
    },
    {
      id: "gives",
      label: "Gives",
      mode: "one",
      options: [
        { value: "", label: "Anything" },
        { value: "bp", label: "Gives a blueprint", test: (r) => r.blueprint_rewards.length > 0 },
        { value: "event", label: "Gives event points", test: (r) => r.event_rewards.length > 0 },
        { value: "once", label: "Once only", test: (r) => r.once_only },
      ],
    },
  ]);

  const pays = (m: MissionEntryView) => m.payout.fixed ?? m.payout.estimate ?? 0;
  const scope = $derived(browse.values.bp as string);

  const descriptor = $derived({
    // The blueprint scope narrows the sources, not a facet: 700 blueprints
    // are no filter list, and the scope arrives as a link.
    sources: () => (scope ? missions.filter((m) => blueprintChance(m, scope) > 0) : missions),
    derive: (ms: MissionEntryView[]): MissionRow[] =>
      ms.map((m) => {
        const req = ownRequirement(m);
        return { ...m, key: m.mission_id, title: missionTitle(m), ladder: req ? ladderOf(req) : null };
      }),
    searchIn: (r: MissionRow) => [
      r.title,
      r.debug_name,
      r.faction?.name ?? "",
      ...r.locations.map((l) => l.name),
      ...r.blueprint_rewards.flatMap((p) => p.blueprints.map((b) => b.name ?? "")),
    ],
    facets,
    sorts: [
      { value: "pays", label: "Most aUEC", compare: (a: MissionRow, b: MissionRow) => pays(b) - pays(a) },
      {
        value: "rank",
        label: "Lowest rank",
        compare: (a: MissionRow, b: MissionRow) => (a.ladder?.lo ?? 0) - (b.ladder?.lo ?? 0) || pays(b) - pays(a),
      },
      {
        value: "time",
        label: "Shortest",
        compare: (a: MissionRow, b: MissionRow) =>
          (a.payout.time_to_complete || 1e9) - (b.payout.time_to_complete || 1e9),
      },
      { value: "name", label: "Name", compare: (a: MissionRow, b: MissionRow) => a.title.localeCompare(b.title) },
    ],
  });

  const status = $derived<Status>(
    browseError ? "error" : loading && missions.length === 0 ? "loading" : "ready",
  );

  // ── The blueprint scope's figures ─────────────────────────────────────
  const scoped = $derived(scope ? missions.filter((m) => blueprintChance(m, scope) > 0) : []);
  const scopeName = $derived(scope ? (blueprintName(missions, scope) ?? "this blueprint") : "");
  const bestChance = $derived(scoped.reduce((p, m) => Math.max(p, blueprintChance(m, scope)), 0));
  const runs = $derived(runsFor(bestChance));
  /** Missions per rank of the scope's main faction, on its ladder. */
  const scopeLadder = $derived.by(() => {
    const reqs = scoped.map(ownRequirement).filter((r) => r != null);
    const first = reqs[0];
    const ladder = first ? ladderOf(first) : null;
    if (!first || !ladder) return null;
    const counts = ladder.ranks.map(() => 0);
    for (const r of reqs) {
      const lw = r.faction === first.faction ? ladderOf(r) : null;
      if (lw && lw.lo < counts.length) counts[lw.lo] += 1;
    }
    return { faction: first.faction ?? "", ranks: ladder.ranks, counts };
  });
  const scopeRegions = $derived([...new Set(scoped.flatMap((m) => m.locations.map((l) => l.name || l.system)))]);
  const scopePool = $derived(
    scoped[0]?.blueprint_rewards.find((p) => p.blueprints.some((b) => b.blueprint_record_guid === scope)),
  );

  /** Every blueprint a mission can grant, for the field's offers. */
  const blueprints = $derived.by(() => {
    const seen = new Map<string, string>();
    for (const m of missions)
      for (const p of m.blueprint_rewards)
        for (const b of p.blueprints) if (b.name && !seen.has(b.blueprint_record_guid)) seen.set(b.blueprint_record_guid, b.name);
    return [...seen].map(([guid, name]) => ({ guid, name })).sort((a, b) => a.name.localeCompare(b.name));
  });

  // ── The opened row ────────────────────────────────────────────────────
  const SHIPS_SHOWN = 10;
  let shipsAll = $state<Record<string, boolean>>({});
  let placesAll = $state(false);
  function toggleRow(key: string) {
    expanded.toggle(key);
    shipsAll = {};
    placesAll = false;
  }

  const count = (min: number, max: number, one: string, many: string) =>
    min === max ? `${max} ${max === 1 ? one : many}` : `${min} to ${max} ${many}`;

  function spawnText(s: SpawnSlotView): string {
    const what = s.kinds.map((k) => k.replace(/^PU_/, "").replaceAll("_", " ")).join(", ") || "NPC";
    const n = s.count_min === s.count_max ? `${s.count_max}` : `${s.count_min} to ${s.count_max}`;
    return `${n} × ${what}`;
  }
  const encounterHead = (e: MissionEncounterView) =>
    [e.difficulty, ...new Set(e.waves.flatMap((w) => w.ships.flatMap((s) => s.factions)))].filter(Boolean).join(", ");

  const fmt = (n: number) => n.toLocaleString("en-US");

  onMount(() => {
    void loadOwnedBlueprints();
    void (async () => {
      const channel = await ensureChannel();
      ready = true;
      if (!channel) return;
      loading = true;
      const r = await getMissions(channel);
      loading = false;
      if (typeof r === "string") browseError = r;
      else missions = r;
    })();
  });
</script>

{#if ready && !dataStore.channel}
  <NoGameData />
{:else}
  {#if browseError}<p class="error">{browseError}</p>{/if}

  {#if scope}
    <section class="scope" aria-label="Missions that grant {scopeName}">
      <div>
        <h2 class="scope-h">
          <span>{scopeName}</span>
        </h2>
        <div class="stat-tiles">
          <div class="stat-tile"><b>{oddsText(bestChance)}</b><span>per run at best</span></div>
          <div class="stat-tile"><b>{scoped.length}</b><span>{scoped.length === 1 ? "mission" : "missions"}</span></div>
          <div class="stat-tile"><b>{runs ? runs.average : "—"}</b><span>runs on average</span></div>
          <div class="stat-tile"><b>{runs ? runs.ninety : "—"}</b><span>runs for 90%</span></div>
        </div>
        {#if scopePool}
          <p class="scope-pool">
            Pool of {scopePool.blueprints.length}:
            {#each scopePool.blueprints as b, i (b.blueprint_record_guid)}<span class:want={b.blueprint_record_guid === scope}
                >{b.name ?? "?"}</span
              >{i < scopePool.blueprints.length - 1 ? ", " : ""}{/each}
          </p>
        {/if}
      </div>
      {#if scopeLadder}
        <div>
          <p class="cap">{scopeLadder.faction} rank they need{scopeRegions.length ? `, around ${scopeRegions.join(", ")}` : ""}</p>
          <div class="bigl">
            {#each scopeLadder.ranks as name, i (i)}
              <div class:in={scopeLadder.counts[i] > 0}>
                <i></i><span>{name}</span><small>{scopeLadder.counts[i]} {scopeLadder.counts[i] === 1 ? "mission" : "missions"}</small>
              </div>
            {/each}
          </div>
        </div>
      {/if}
    </section>
  {/if}

  <div class="flex h-full min-h-0 flex-col">
    <Surface.Root {descriptor} {browse} class="min-h-0 flex-1">
      <Surface.List {status}>
        {#snippet header()}
          <CatalogBar {browse} {blueprints} total={missions.length} />
        {/snippet}
        {#snippet row(r: MissionRow)}
          {@const cat = categoryOf(r)}
          {@const note = needsNote(r)}
          {@const where = whereCell(r)}
          {@const bp = blueprintCell(r, blueprintsStore.owns, blueprintsStore.fetchedAt != null)}
          {@const pay = paysCell(r)}
          {@const time = timeCell(r)}
          <Expand.Row title={r.title} open={expanded.has(r.key)} ontoggle={() => toggleRow(r.key)}>
            {#snippet gutter()}
              <span class="tile" style:--hue={cat.hue} title={r.category?.name ?? "Mission"}>
                <cat.icon size={16} aria-hidden="true" />
              </span>
            {/snippet}
            {#snippet meta()}
              <span class="line2">
                <span>{r.category?.name ?? "Mission"}</span>
                {#if note}<span class="need">{note}</span>{/if}
                {#if r.once_only}<span>once only</span>{/if}
              </span>
            {/snippet}
            {#snippet right()}
              <span class="cols">
                <span class="c2"><span>{where.value}</span><small>{where.sub}</small></span>
                <span class="c2">
                  <span>{r.faction?.name ?? "No faction"}</span>
                  {#if r.ladder}
                    <span class="lad" title={windowText(r.ladder)} aria-label="Rank: {windowText(r.ladder)}">
                      {#each r.ladder.ranks as _, i (i)}<i class:in={i >= r.ladder.lo && i <= r.ladder.hi}></i>{/each}
                    </span>
                  {:else}
                    <small>no rank needed</small>
                  {/if}
                </span>
                <span class="c2"><span class={bp.value === "none" ? "none" : "bpv"}>{bp.value}</span><small>{bp.sub}</small></span>
                <span class="c2 r"><span class="v">{pay.value}{#if pay.value !== "—" && pay.value !== "calculated"}<i>aUEC</i>{/if}</span><small>{pay.sub}</small></span>
                <span class="c2 r"><span class={time.sub === "game estimate" ? "est" : "none"}>{time.value}</span><small>{time.sub}</small></span>
              </span>
            {/snippet}

            {@render opened(r)}
          </Expand.Row>
        {/snippet}
      </Surface.List>
    </Surface.Root>
  </div>
{/if}

{#snippet opened(m: MissionRow)}
  {@const own = ownRequirement(m)}
  {@const others = m.rep_required.filter((r) => !r.exclude && r.faction !== own?.faction)}
  {@const closedTo = m.rep_required.filter((r) => r.exclude)}
  {@const places = m.locations.flatMap((l) => l.places.map((p) => p.name ?? p.record_name))}
  <div class="opened">
    <section class="span2 brief">
      <h4>Briefing</h4>
      {#if m.description}
        <p><RichText text={m.description} /></p>
      {:else}
        <p class="none">The game gives this mission no text.</p>
      {/if}
    </section>

    <section>
      <h4>Time and place</h4>
      <dl class="kv">
        <dt>Game estimate</dt><dd class="mono">{m.payout.time_to_complete > 0 ? `${Math.round(m.payout.time_to_complete)} min` : "—"}</dd>
        {#if m.cooldown_seconds}<dt>Cooldown</dt><dd class="mono">{Math.round(m.cooldown_seconds / 60)} min</dd>{/if}
        {#each m.locations as l, i (i)}
          <dt>{i === 0 ? "Offered in" : ""}</dt><dd>{l.name ? `${l.name}, ${l.system}` : l.system}</dd>
        {/each}
        <dt>Places</dt><dd class="mono">{places.length}</dd>
      </dl>
      {#if places.length > 0}
        <p class="places">
          {(placesAll ? places : places.slice(0, 6)).join(", ")}
          {#if !placesAll && places.length > 6}
            <button type="button" class="more" onclick={() => (placesAll = true)}>+{places.length - 6} more</button>
          {/if}
        </p>
      {/if}
    </section>

    <section>
      <h4>Encounters</h4>
      {#if m.encounters.length === 0}
        <p class="none">No encounter in the data.</p>
      {/if}
      {#each m.encounters as e, ei (ei)}
        {#if encounterHead(e)}<p class="enc-h">{encounterHead(e)}</p>{/if}
        {#each e.waves as w, wi (wi)}
          <div class="wave">
            {#each w.ships as s, si (si)}
              {@const key = `${ei}:${wi}:${si}`}
              <div class="wave-h">
                <b>{w.name || "Ships"}</b>
                <span>{count(s.count_min, s.count_max, "ship", "ships")}</span>
                <span class="from">{s.ships.length === 1 ? "one type" : `from ${s.ships.length} types`}</span>
              </div>
              <div class="ships">
                {#each shipsAll[key] ? s.ships : s.ships.slice(0, SHIPS_SHOWN) as name (name)}<span>{name}</span>{/each}
                {#if !shipsAll[key] && s.ships.length > SHIPS_SHOWN}
                  <button type="button" onclick={() => (shipsAll = { ...shipsAll, [key]: true })}>+{s.ships.length - SHIPS_SHOWN} more</button>
                {/if}
              </div>
            {/each}
            {#each w.spawns as s, si (si)}
              <div class="wave-h">
                <b>{w.name || (s.target ? "Target" : "On site")}</b>
                <span>{spawnText(s)}</span>
                {#if s.factions.length}<span class="from">{s.factions.join(", ")}</span>{/if}
              </div>
            {/each}
          </div>
        {/each}
      {/each}
    </section>

    <section>
      <h4>Unlock</h4>
      {#if m.ladder && own}
        <p class="u-need">{windowText(m.ladder)}<small>{own.faction ?? "Faction"} rank</small></p>
        <div class="uladder" aria-label="{own.faction} ranks: {windowText(m.ladder)}">
          {#each m.ladder.ranks as name, i (i)}
            <div class:in={i >= m.ladder.lo && i <= m.ladder.hi}>
              <i></i><span>{name}</span><small>{fmt(m.ladder.reps[i])}</small>
            </div>
          {/each}
        </div>
        <div class="ulegend"><span><i class="k1"></i>can take it</span></div>
      {:else}
        <p class="u-need">Any rank<small>no rank in the data</small></p>
      {/if}
      <dl class="kv" style="margin-top:14px">
        <dt>Missions first</dt>
        <dd>{m.chain_required.length ? m.chain_required.map((c) => c.title ?? "?").join(", ") : "none"}</dd>
        <dt>Other standing</dt>
        <dd>{others.length ? others.map((o) => `${o.faction ?? "?"} ${rangeText(o)}`).join(", ") : "none"}</dd>
        {#each closedTo as c, i (i)}
          <dt>{i === 0 ? "Closed at" : ""}</dt><dd>{c.faction ?? "?"} {rangeText(c)}</dd>
        {/each}
        <dt>Legal</dt><dd>{m.illegal ? "illegal" : "legal"}</dd>
        {#if m.once_only}<dt>Repeat</dt><dd>once only</dd>{/if}
        {#if m.shareable}<dt>Share</dt><dd>shareable</dd>{/if}
      </dl>
    </section>

    <section>
      <h4>Gives</h4>
      <dl class="kv">
        <dt>aUEC</dt><dd class="mono">{paysCell(m).value}</dd>
        {#each m.reputation as r, i (i)}
          <dt>{i === 0 ? "Rep" : ""}</dt><dd class="mono">{r.amount == null ? "calculated" : `+${fmt(r.amount)}`}</dd>
        {/each}
        {#each m.scrip as s, i (i)}<dt>{s.name ?? "Scrip"}</dt><dd class="mono">{s.amount}</dd>{/each}
        {#each m.item_rewards as it (it.entity_guid)}<dt>{it.name ?? "Item"}</dt><dd class="mono">{it.amount}</dd>{/each}
        {#each m.cargo as leg, i (i)}
          <dt>{i === 0 ? "Cargo" : ""}</dt>
          <dd>{leg.commodity ?? "?"} {leg.min_scu === leg.max_scu ? leg.max_scu : `${leg.min_scu} to ${leg.max_scu}`} SCU</dd>
        {/each}
      </dl>
      {#each m.blueprint_rewards as p (p.pool_name)}
        {@const total = p.blueprints.reduce((n, b) => n + b.weight, 0)}
        <ul class="pool" aria-label="Blueprint pool">
          {#each p.blueprints as b (b.blueprint_record_guid)}
            {@const owned = blueprintsStore.owns(b.blueprint_record_guid)}
            <li class:own={owned} class:want={b.blueprint_record_guid === scope}>
              <span class="nm">{b.name ?? "?"}</span>
              <span class="od">{owned ? "yours" : `${Math.round((p.chance * b.weight * 1000) / (total || 1)) / 10}%`}</span>
            </li>
          {/each}
        </ul>
      {/each}
    </section>
  </div>
{/snippet}

<style>
  /* The row's columns sit in the kit row's right slot; each is a value over
   * its caption, in the body font with tabular figures. */
  .tile {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: var(--radius);
    box-sizing: border-box;
    color: color-mix(in srgb, var(--hue) 85%, var(--text));
    background: color-mix(in srgb, var(--hue) 12%, var(--bg-raised));
    border: 1px solid color-mix(in srgb, var(--hue) 26%, var(--border));
  }
  .line2 {
    display: flex;
    gap: 10px;
    min-width: 0;
    max-width: 100%;
    overflow: hidden;
    font-size: 0.75rem;
    color: var(--text-dim);
    white-space: nowrap;
  }
  .line2 > span {
    flex: none;
  }
  .line2 .need {
    flex: 0 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--warn);
  }
  .cols {
    display: grid;
    grid-template-columns: 132px 150px 112px 116px 96px;
    column-gap: 18px;
    align-items: center;
  }
  .c2 {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
    font-size: 0.875rem;
    line-height: 1.25;
    color: var(--text);
    font-variant-numeric: tabular-nums;
  }
  .c2 > * {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .c2.r {
    align-items: flex-end;
    text-align: right;
  }
  .c2 small {
    font-size: 0.75rem;
    color: var(--text-dim);
  }
  .c2 .v {
    color: var(--text-strong);
    font-weight: 500;
  }
  .c2 .v i {
    font-style: normal;
    font-weight: 400;
    font-size: 0.75rem;
    color: var(--text-dim);
    margin-left: 3px;
  }
  .c2 .bpv {
    color: color-mix(in srgb, var(--hue-rose) 75%, var(--text-strong));
    font-weight: 500;
  }
  .c2 .none {
    color: var(--text-faint);
  }
  .c2 .est {
    color: var(--text);
  }
  .lad {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: minmax(0, 1fr);
    gap: 2px;
    width: 112px;
  }
  .lad i,
  .uladder i,
  .bigl i {
    display: block;
    height: 8px;
    border-radius: 2px;
    background: var(--bg-sunk);
    border: 1px solid var(--border);
    box-sizing: border-box;
  }
  .lad i.in,
  .uladder .in i,
  .bigl .in i {
    background: color-mix(in srgb, var(--good) 40%, transparent);
    border-color: color-mix(in srgb, var(--good) 50%, transparent);
  }
  @media (max-width: 1180px) {
    .cols {
      grid-template-columns: 150px 112px 116px 96px;
    }
    .cols > .c2:first-child {
      display: none;
    }
  }

  /* The blueprint scope above the list */
  .scope {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: 16px 40px;
    align-items: end;
    margin-bottom: 14px;
  }
  .scope-h {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 0 10px;
    font-size: 1.05rem;
    font-weight: 600;
    color: var(--text-strong);
  }
  .scope-pool {
    margin: 10px 0 0;
    font-size: 0.8rem;
    color: var(--text-dim);
  }
  .scope-pool .want {
    color: color-mix(in srgb, var(--hue-rose) 80%, var(--text-strong));
    font-weight: 600;
  }
  .cap {
    margin: 0 0 8px;
    font-size: 0.75rem;
    color: var(--text-dim);
  }
  .bigl,
  .uladder {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: minmax(0, 1fr);
    gap: 3px;
  }
  .bigl div,
  .uladder div {
    display: flex;
    flex-direction: column;
    gap: 5px;
    min-width: 0;
  }
  .bigl i,
  .uladder i {
    height: 12px;
  }
  .bigl span,
  .uladder span {
    font-size: 0.68rem;
    line-height: 1.2;
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .bigl small,
  .uladder small {
    font-size: 0.68rem;
    color: var(--text-faint);
    font-variant-numeric: tabular-nums;
  }

  /* The opened row */
  .opened {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 22px 36px;
    padding: 6px 12px 10px 0;
    cursor: auto;
  }
  .opened section {
    min-width: 0;
  }
  .opened .span2 {
    grid-column: span 2;
  }
  .opened h4 {
    margin: 0 0 10px;
    font-family: var(--font-display);
    font-size: 0.68rem;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-faint);
    font-weight: 700;
  }
  .brief p {
    margin: 0;
    max-width: 72ch;
    font-size: 0.875rem;
    line-height: 1.55;
    color: var(--text);
  }
  .none {
    color: var(--text-faint);
    font-size: 0.8rem;
  }
  .kv {
    margin: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 6px 16px;
    font-size: var(--text-small);
  }
  .kv dt {
    color: var(--text-faint);
  }
  .kv dd {
    margin: 0;
    text-align: right;
    color: var(--text);
  }
  .kv dd.mono {
    font-variant-numeric: tabular-nums;
  }
  .places {
    margin: 10px 0 0;
    font-size: 0.75rem;
    line-height: 1.5;
    color: var(--text-dim);
  }
  .more,
  .ships button {
    all: unset;
    cursor: pointer;
    font-size: 0.75rem;
    color: var(--accent);
  }
  .enc-h {
    margin: 0 0 8px;
    font-size: 0.75rem;
    color: var(--text-dim);
  }
  .wave {
    padding: 8px 0;
    border-top: 1px solid color-mix(in srgb, var(--border) 65%, transparent);
  }
  .wave:first-of-type {
    border-top: 0;
    padding-top: 0;
  }
  .wave-h {
    display: flex;
    align-items: baseline;
    gap: 12px;
    margin-bottom: 6px;
    font-size: 0.875rem;
  }
  .wave-h b {
    font-weight: 600;
    color: var(--text-strong);
  }
  .wave-h .from {
    margin-left: auto;
    font-size: 0.75rem;
    color: var(--text-dim);
  }
  .ships {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-bottom: 6px;
  }
  .ships span,
  .ships button {
    box-sizing: border-box;
    font-size: 0.75rem;
    line-height: 1;
    padding: 4px 7px;
    border-radius: var(--radius-chip);
    border: 1px solid var(--border);
    color: var(--text-dim);
    white-space: nowrap;
  }
  .ships button {
    color: var(--accent);
  }
  .u-need {
    margin: 0 0 12px;
    font-size: 1rem;
    font-weight: 600;
    color: var(--text-strong);
  }
  .u-need small {
    display: block;
    margin-top: 2px;
    font-size: 0.75rem;
    font-weight: 400;
    color: var(--text-dim);
  }
  .ulegend {
    display: flex;
    gap: 16px;
    margin-top: 10px;
    font-size: 0.72rem;
    color: var(--text-dim);
  }
  .ulegend i {
    display: inline-block;
    width: 12px;
    height: 8px;
    margin-right: 6px;
    border-radius: 2px;
    box-sizing: border-box;
  }
  .ulegend .k1 {
    background: color-mix(in srgb, var(--good) 40%, transparent);
    border: 1px solid color-mix(in srgb, var(--good) 50%, transparent);
  }
  .pool {
    list-style: none;
    margin: 12px 0 0;
    padding: 10px 0 0;
    border-top: 1px solid color-mix(in srgb, var(--border) 65%, transparent);
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 5px 16px;
    font-size: var(--text-small);
  }
  .pool li {
    display: contents;
  }
  .pool .nm {
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .pool .od {
    text-align: right;
    font-variant-numeric: tabular-nums;
    color: var(--text-faint);
  }
  .pool .own .nm,
  .pool .own .od {
    color: var(--text-faint);
  }
  .pool .want .nm {
    color: var(--text-strong);
    font-weight: 600;
  }
  .pool .want .od {
    color: color-mix(in srgb, var(--hue-rose) 80%, var(--text));
  }
  @media (max-width: 1000px) {
    .opened {
      grid-template-columns: minmax(0, 1fr);
    }
    .opened .span2 {
      grid-column: auto;
    }
  }
</style>
