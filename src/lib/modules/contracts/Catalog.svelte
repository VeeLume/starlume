<script lang="ts">
  // Contracts, the Catalog sub page (pass 2 of the design, Firefly note
  // 'Contracts design direction'). The cooked mission catalog on the kit
  // surface: URL-backed browse, windowed list, one-open accordion. Every
  // column of a row is a value over its caption (model.ts); the faction
  // column draws the rank ladder with the mission's window filled.
  //
  // `?bp=<blueprint guid>` scopes the catalog to the missions that can grant
  // one blueprint (Crafting's recipe sheet and Plans' "Earn" step link here)
  // and shows its odds above the list. `?unlock=<mission id>.<gate>` scopes
  // it to the missions that count toward one chain gate of a mission (Plans'
  // Unlock step links here) and shows your progress above the list.
  //
  // Your own rank (the standing read) and your own times arrive in later
  // slices; until then the ladder shows the window without a "you".
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import { replaceState } from "$app/navigation";
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
  import RankLadder from "./RankLadder.svelte";
  import PlanButtons from "./PlanButtons.svelte";
  import { addMissionPlan, plansStore } from "$lib/state/plans.svelte";
  import { dataStore, ensureChannel } from "$lib/state/data.svelte";
  import { getMissions } from "$lib/state/catalog.svelte";
  import { blueprintsStore, loadOwnedBlueprints } from "$lib/state/blueprints.svelte";
  import { loadStanding, standingStore } from "$lib/state/standing.svelte";
  import { loadRunMinutes, sessionStore } from "$lib/state/session.svelte";
  import {
    blueprintCell,
    blueprintChance,
    blueprintName,
    commonLadder,
    countsPoints,
    gateProgress,
    runsOf,
    hasNewBlueprint,
    ladderOf,
    missionTitle,
    needsNote,
    oddsText,
    opensAt,
    ownRequirement,
    paysCell,
    rangeText,
    rankIndex,
    rowFor,
    standingOf,
    STANDING_GROUPS,
    type Standing,
    poolDraw,
    runsAll,
    timeCell,
    whereCell,
    windowText,
    type Ladder,
    ownRep,
  } from "./model";

  type MissionRow = MissionEntryView & {
    key: string;
    title: string;
    ladder: Ladder | null;
    /** Your rank on that ladder, -1 when not known. */
    you: number;
    /** What the mission is to you; null until your standing is read. */
    standing: Standing | null;
  };

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
    system: { kind: "one", default: "" },
    legal: { kind: "one", default: "" },
    share: { kind: "one", default: "" },
    repeat: { kind: "one", default: "" },
    rank: { kind: "one", default: "" },
    status: { kind: "one", default: "" },
    bp: { kind: "one", default: "", narrows: false },
    unlock: { kind: "one", default: "", narrows: false },
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

  /**
   * The rank picker's ladder: the picked faction's when one is picked (ranks
   * are named per faction), else the ladder most missions use.
   */
  const ladders = $derived(
    missions.map((m) => {
      const r = ownRequirement(m);
      return { faction: m.faction?.name ?? "", ladder: r ? ladderOf(r) : null };
    }),
  );
  const rankNames = $derived.by(() => {
    const f = browse.values.faction as string;
    return commonLadder(ladders.filter((x) => !f || x.faction === f).map((x) => x.ladder));
  });
  const ownedKnown = $derived(blueprintsStore.fetchedAt != null);

  const groupOrder = (id: string) => STANDING_GROUPS.findIndex((g) => g.id === id);

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
    ...(standingStore.known
      ? [
          {
            id: "status",
            label: "For you",
            mode: "one" as const,
            options: [
              { value: "", label: "Every mission" },
              ...STANDING_GROUPS.map((g) => ({ value: g.id, label: g.label, test: (r: MissionRow) => r.standing === g.id })),
            ],
          },
        ]
      : []),
    {
      id: "gives",
      label: "Gives",
      mode: "one",
      options: [
        { value: "", label: "Anything" },
        { value: "bp", label: "Gives a blueprint", test: (r) => r.blueprint_rewards.length > 0 },
        // Only once the owned set was read: unread, every pool looks new.
        ...(ownedKnown
          ? [{ value: "new", label: "New blueprints for you", test: (r: MissionRow) => hasNewBlueprint(r, blueprintsStore.owns) }]
          : []),
        { value: "event", label: "Gives event points", test: (r) => r.event_rewards.length > 0 },
      ],
    },
    {
      id: "system",
      label: "System",
      mode: "one",
      options: [
        { value: "", label: "Every system" },
        ...byFrequency((m) => m.locations.map((l) => l.system)).map((sys) => ({
          value: sys,
          label: sys,
          test: (r: MissionRow) => r.locations.some((l) => l.system === sys),
        })),
      ],
    },
    {
      id: "legal",
      label: "Legality",
      mode: "one",
      options: [
        { value: "", label: "Legal and illegal" },
        { value: "legal", label: "Legal", test: (r) => !r.illegal },
        { value: "illegal", label: "Illegal", test: (r) => r.illegal },
      ],
    },
    {
      id: "share",
      label: "Sharing",
      mode: "one",
      options: [
        { value: "", label: "Any" },
        { value: "yes", label: "Shareable", test: (r) => r.shareable },
      ],
    },
    {
      id: "repeat",
      label: "Repeat",
      mode: "one",
      options: [
        { value: "", label: "Any" },
        { value: "once", label: "Once only", test: (r) => r.once_only },
      ],
    },
    {
      id: "rank",
      label: "Open at rank",
      mode: "one",
      options: [
        { value: "", label: "Any rank" },
        ...rankNames.map((name) => ({ value: name, label: name, test: (r: MissionRow) => opensAt(r.ladder, name) })),
      ],
    },
  ]);

  const pays = (m: MissionEntryView) => m.payout.fixed ?? m.payout.estimate ?? 0;
  /** An amount a minute by the row's time (yours, else the game's), for
   *  the opened row; a dash without the amount or a time. */
  const perMinute = (amount: number | null, m: MissionEntryView) => {
    const min = minutesOf(m);
    if (amount == null || !(amount > 0) || min >= 1e9) return "—";
    const v = amount / min;
    return v >= 100 ? fmt(Math.round(v)) : v.toFixed(1);
  };
  /** Own-faction rep a minute, by the row's time (yours, else the game's);
   *  -1 without rep or a time, so those sort last. */
  const repPerMinute = (m: MissionEntryView) => {
    const rep = ownRep(m);
    const min = minutesOf(m);
    return rep != null && rep > 0 && min < 1e9 ? rep / min : -1;
  };
  /** The time the row shows, for the Shortest sort: yours, else the game's. */
  const minutesOf = (m: MissionEntryView) => {
    const v = parseFloat(timeCell(m, sessionStore.runMinutes).value);
    return Number.isFinite(v) ? v : 1e9;
  };
  const scope = $derived(browse.values.bp as string);
  /** The chain-gate scope: the mission it opens, the gate, your progress and
   *  the contracts that count toward it (a catalog entry is in when one of
   *  its contracts is). */
  const gateScope = $derived.by(() => {
    const v = browse.values.unlock as string;
    if (!v) return null;
    const [id, n] = v.split(".");
    const target = missions.find((m) => m.mission_id === id);
    const gate = target?.chain_gates[Number(n ?? 0)];
    if (!target || !gate) return null;
    const contracts = new Set(gate.missions.flatMap((c) => c.contracts));
    return { target, gate, contracts, progress: gateProgress(gate, standingStore.completed, standingStore.tags) };
  });
  /** The gate missions behind one catalog entry: what a run of it adds. */
  const addsOf = (m: MissionEntryView) =>
    gateScope?.gate.missions.find((c) => c.contracts.some((x) => m.contracts.includes(x))) ?? null;

  const descriptor = $derived({
    // The blueprint scope narrows the sources, not a facet: 700 blueprints
    // are no filter list, and the scope arrives as a link.
    sources: () =>
      scope
        ? missions.filter((m) => blueprintChance(m, scope) > 0)
        : gateScope
          ? missions.filter((m) => m.contracts.some((c) => gateScope.contracts.has(c)))
          : missions,
    derive: (ms: MissionEntryView[]): MissionRow[] =>
      ms.map((m) => {
        const req = ownRequirement(m);
        const ladder = req ? ladderOf(req) : null;
        const row = req ? rowFor(req, standingStore.rows) : null;
        return {
          ...m,
          key: m.mission_id,
          title: missionTitle(m),
          ladder,
          you: ladder && row ? rankIndex(ladder, row) : -1,
          standing: standingStore.known ? standingOf(m, standingStore.rows, standingStore.completed, standingStore.tags) : null,
        };
      }),
    // Once your standing is read, the list falls into what each mission is
    // to you, in a fixed order; unread, it stays one list.
    groupBy: standingStore.known
      ? [
          {
            key: (r: MissionRow) => r.standing ?? "unknown",
            label: (k: string) => STANDING_GROUPS.find((g) => g.id === k)?.label ?? k,
            compare: (a: string, b: string) => groupOrder(a) - groupOrder(b),
          },
        ]
      : undefined,
    searchIn: (r: MissionRow) => [
      r.title,
      r.description ?? "",
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
          minutesOf(a) - minutesOf(b),
      },
      // Grinding rep: Plans' rep steps link here with Rep per minute.
      { value: "repmin", label: "Rep per minute", compare: (a: MissionRow, b: MissionRow) => repPerMinute(b) - repPerMinute(a) || pays(b) - pays(a) },
      { value: "rep", label: "Most rep", compare: (a: MissionRow, b: MissionRow) => (ownRep(b) ?? -1e9) - (ownRep(a) ?? -1e9) || pays(b) - pays(a) },
      { value: "name", label: "Name", compare: (a: MissionRow, b: MissionRow) => a.title.localeCompare(b.title) },
    ],
  });

  const status = $derived<Status>(
    browseError ? "error" : loading && missions.length === 0 ? "loading" : "ready",
  );

  // ── The blueprint scope's figures ─────────────────────────────────────
  const scoped = $derived(scope ? missions.filter((m) => blueprintChance(m, scope) > 0) : []);
  const scopeName = $derived(scope ? (blueprintName(missions, scope) ?? "this blueprint") : "");
  /** What you own counts once it was read: a run gives one you lack. */
  const ownsFn = $derived(ownedKnown ? (g: string) => blueprintsStore.owns(g) : () => false);
  const bestChance = $derived(scoped.reduce((p, m) => Math.max(p, blueprintChance(m, scope, ownsFn)), 0));
  /** Runs until a pool holding it has given all it holds for you: missions
   *  on one pool give the same, so the soonest. */
  const runs = $derived(
    scoped.reduce<number | null>((best, m) => {
      const n = runsAll(poolDraw(m, scope, ownsFn));
      return n != null && (best == null || n < best) ? n : best;
    }, null),
  );
  /** The quickest way to have it: those runs, each by the row's time. */
  const fastest = $derived(
    scoped.reduce<number | null>((best, m) => {
      const n = runsAll(poolDraw(m, scope, ownsFn));
      const min = minutesOf(m);
      if (n == null || min >= 1e9) return best;
      return best == null || n * min < best ? n * min : best;
    }, null),
  );
  const span = (min: number) => (min >= 60 ? `${Math.floor(min / 60)} h ${Math.round(min % 60)} min` : `${Math.round(min)} min`);
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
  // ── The chain-gate scope's figures ────────────────────────────────────
  const gateRows = $derived(gateScope ? missions.filter((m) => m.contracts.some((c) => gateScope.contracts.has(c))) : []);
  /** Fewest runs to open it, and the quickest way in time. */
  const gateRuns = $derived.by(() => {
    if (!gateScope) return { fewest: null as number | null, fastest: null as number | null };
    let fewest: number | null = null;
    let fastest: number | null = null;
    for (const m of gateRows) {
      const way = addsOf(m);
      if (!way) continue;
      const n = runsOf(gateScope.progress.left, way);
      if (fewest == null || n < fewest) fewest = n;
      const min = minutesOf(m);
      if (min < 1e9 && (fastest == null || n * min < fastest)) fastest = n * min;
    }
    return { fewest, fastest };
  });
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
  /** A gate's "N missions": the catalog scoped to what counts toward it. */
  function showGate(m: MissionEntryView, gate: number) {
    browse.setMany({
      q: "",
      faction: "",
      region: "",
      category: "",
      gives: "",
      system: "",
      legal: "",
      share: "",
      repeat: "",
      rank: "",
      status: "",
      bp: "",
      unlock: `${m.mission_id}.${gate}`,
    });
    expanded.clear();
  }

  /** A chain link: find the mission before this one and open it. The
   *  filters are cleared so nothing hides it; the sort stays. */
  function openMission(ref: { mission_id: string; title: string | null }) {
    browse.setMany({
      q: ref.title ?? "",
      faction: "",
      region: "",
      category: "",
      gives: "",
      system: "",
      legal: "",
      share: "",
      repeat: "",
      rank: "",
      status: "",
      bp: "",
      unlock: "",
    });
    expanded.open(ref.mission_id);
    shipsAll = {};
    placesAll = false;
  }

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
    void loadStanding();
    void loadRunMinutes();
    // A link from Plans names one mission (`?open=<id>&q=<title>`): open
    // it once, then drop the id, since expansion is page-local (rule 7).
    const linked = page.url.searchParams.get("open");
    if (linked) {
      openMission({ mission_id: linked, title: page.url.searchParams.get("q") });
      const url = new URL(page.url);
      url.searchParams.delete("open");
      replaceState(url, page.state);
    }
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

  {#if gateScope}
    {@const g = gateScope.gate}
    {@const points = countsPoints(g)}
    <section class="scope" aria-label="Missions that unlock {missionTitle(gateScope.target)}">
      <div>
        <h2 class="scope-h">
          <span>Unlocks {missionTitle(gateScope.target)}</span>
        </h2>
        <div class="stat-tiles">
          <div class="stat-tile">
            <b>{standingStore.known ? `${Math.min(gateScope.progress.have, g.need)} of ${g.need}` : g.need}</b>
            <span>{points ? "points" : "runs"}{standingStore.known ? " done" : " it asks for"}</span>
          </div>
          <div class="stat-tile"><b>{gateRows.length}</b><span>{gateRows.length === 1 ? "mission" : "missions"} count</span></div>
          <div class="stat-tile"><b>{gateRuns.fewest ?? "—"}</b><span>runs at the fewest</span></div>
          <div class="stat-tile"><b>{gateRuns.fastest != null ? span(gateRuns.fastest) : "—"}</b><span>at the quickest</span></div>
        </div>
      </div>
    </section>
  {:else if scope}
    <section class="scope" aria-label="Missions that grant {scopeName}">
      <div>
        <h2 class="scope-h">
          <span>{scopeName}</span>
        </h2>
        <div class="stat-tiles">
          <div class="stat-tile"><b>{oddsText(bestChance)}</b><span>per run at best</span></div>
          <div class="stat-tile"><b>{scoped.length}</b><span>{scoped.length === 1 ? "mission" : "missions"}</span></div>
          <div class="stat-tile"><b>{runs ?? "—"}</b><span>runs to have it</span></div>
          <div class="stat-tile"><b>{fastest != null ? span(fastest) : "—"}</b><span>at the quickest</span></div>
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
          <CatalogBar {browse} {blueprints} {rankNames} total={missions.length} />
        {/snippet}
        {#snippet row(r: MissionRow)}
          {@const cat = categoryOf(r)}
          {@const note = needsNote(r, standingStore.rows, standingStore.completed, standingStore.tags)}
          {@const where = whereCell(r)}
          {@const bp = blueprintCell(r, blueprintsStore.owns, blueprintsStore.fetchedAt != null)}
          {@const pay = paysCell(r)}
          {@const time = timeCell(r, sessionStore.runMinutes)}
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
                    <span title={windowText(r.ladder)}><RankLadder ladder={r.ladder} you={r.you} label="Rank: {windowText(r.ladder)}" /></span>
                  {:else}
                    <small>no rank needed</small>
                  {/if}
                </span>
                <span class="c2"><span class={bp.value === "none" ? "none" : "bpv"}>{bp.value}</span><small>{bp.sub}</small></span>
                <span class="c2 r"><span class="v">{pay.value}{#if pay.value !== "—" && pay.value !== "calculated"}<i>aUEC</i>{/if}</span><small>{pay.sub}</small></span>
                <span class="c2 r"><span class={time.sub === "your time" ? "v" : time.sub === "game estimate" ? "est" : "none"}>{time.value}</span><small>{time.sub}</small></span>
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
    <section class="brief">
      <div class="plan-row">
        <PlanButtons
          planned={plansStore.missionOf(m.mission_id)}
          add={(parked) => addMissionPlan({ mission: m.mission_id, title: missionTitle(m) }, parked)}
        />
      </div>
      <h4>Briefing</h4>
      {#if m.description}
        <p><RichText text={m.description} /></p>
      {:else}
        <p class="none">The game gives this mission no text.</p>
      {/if}
    </section>

    <!-- The facts beside the briefing, so a long text never pushes them
         down (Valerie, 2026-11-18; Hearth's shape). -->
    <div class="facts">
      <section class="wide">
        <h4>Unlock</h4>
        {#if m.ladder && own}
          <p class="u-need">{windowText(m.ladder)}<small>{own.faction ?? "Faction"} rank</small></p>
          <RankLadder ladder={m.ladder} you={m.you} named label="{own.faction} ranks: {windowText(m.ladder)}" />
          <div class="ulegend">
            <span><i class="k1"></i>can take it</span>
            {#if m.you >= 0}<span><i class="k2"></i>your rank</span>{/if}
          </div>
        {:else}
          <p class="u-need">Any rank<small>no rank in the data</small></p>
        {/if}
        <!-- Every gate must open; any of a gate's missions counts toward it
             (each grants the completion tag it counts). -->
        {#each m.chain_gates as g, gi (gi)}
          {@const p = gateProgress(g, standingStore.completed, standingStore.tags)}
          {@const points = countsPoints(g)}
          <p class="chain-h" class:open={p.left === 0}>
            {g.need === 1
              ? g.missions.length > 1
                ? "Any one of these first"
                : "Mission first"
              : `${g.need} ${points ? "points" : "runs"} of ${g.missions.length > 1 ? "any of these" : "this"}`}
            {#if g.missions.length > 1}
              <button type="button" class="link" title="Show the missions that count toward it" onclick={() => showGate(m, gi)}
                >{g.missions.length} missions</button
              >
            {/if}
            {#if g.need > 1 && standingStore.known}<b>{Math.min(p.have, g.need)} of {g.need}</b>{/if}
          </p>
          <ul class="chain">
            {#each g.missions as c (c.mission_id)}
              {@const done = c.contracts.some((x) => standingStore.completed.has(x))}
              <li class:done>
                <button type="button" class="link" title="Find {c.title ?? 'it'} in the catalog" onclick={() => openMission(c)}>↳ {c.title ?? "?"}</button>
                {#if points && p.left > 0}<span class="runs">{runsOf(p.left, c)} runs</span>{/if}
                {#if done}<small>done</small>{/if}
              </li>
            {/each}
          </ul>
        {/each}
        <dl class="kv" style="margin-top:14px">
          {#if !m.chain_gates.length}<dt>Missions first</dt><dd>none</dd>{/if}
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
        <h4>Time and place</h4>
        <dl class="kv">
          {#each [timeCell(m, sessionStore.runMinutes)].filter((t) => t.sub === "your time") as yours (yours.value)}
            <dt>Your time</dt><dd class="mono">{yours.value}</dd>
          {/each}
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
        <h4>Gives</h4>
        <dl class="kv">
          <dt>aUEC</dt><dd class="mono">{paysCell(m).value}</dd>
          <dt>aUEC / min</dt><dd class="mono">{perMinute(pays(m), m)}</dd>
          {#each m.reputation as r, i (i)}
            <dt>{i === 0 ? "Rep" : ""}</dt><dd class="mono">{r.amount == null ? "calculated" : `+${fmt(r.amount)}`}</dd>
          {/each}
          {#if m.reputation.length > 0}<dt>Rep / min</dt><dd class="mono">{perMinute(ownRep(m), m)}</dd>{/if}
          {#each m.scrip as s, i (i)}<dt>{s.name ?? "Scrip"}</dt><dd class="mono">{s.amount}</dd>{/each}
          {#each m.item_rewards as it (it.entity_guid)}<dt>{it.name ?? "Item"}</dt><dd class="mono">{it.amount}</dd>{/each}
          {#each m.cargo as leg, i (i)}
            <dt>{i === 0 ? "Cargo" : ""}</dt>
            <dd>{leg.commodity ?? "?"} {leg.min_scu === leg.max_scu ? leg.max_scu : `${leg.min_scu} to ${leg.max_scu}`} SCU</dd>
          {/each}
        </dl>
        {#each m.blueprint_rewards as p (p.pool_name)}
          {@const lack = p.blueprints.filter((b) => !blueprintsStore.owns(b.blueprint_record_guid)).length}
          <ul class="pool" aria-label="Blueprint pool">
            {#each p.blueprints as b (b.blueprint_record_guid)}
              {@const owned = blueprintsStore.owns(b.blueprint_record_guid)}
              <li class:own={owned} class:want={b.blueprint_record_guid === scope}>
                <span class="nm">{b.name ?? "?"}</span>
                <span class="od">{owned ? "yours" : `${Math.round((p.chance * 1000) / (lack || 1)) / 10}%`}</span>
              </li>
            {/each}
          </ul>
        {/each}
      </section>

      <section class="wide">
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
    </div>
  </div>
{/snippet}

<style>
  .plan-row {
    margin-bottom: 16px;
  }
  .plan-row:empty {
    display: none;
  }
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
  .bigl i {
    display: block;
    height: 8px;
    border-radius: 2px;
    background: var(--bg-sunk);
    border: 1px solid var(--border);
    box-sizing: border-box;
  }
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
  .bigl {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: minmax(0, 1fr);
    gap: 3px;
  }
  .bigl div {
    display: flex;
    flex-direction: column;
    gap: 5px;
    min-width: 0;
  }
  .bigl i {
    height: 12px;
  }
  .bigl span {
    font-size: 0.68rem;
    line-height: 1.2;
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .bigl small {
    font-size: 0.68rem;
    color: var(--text-faint);
    font-variant-numeric: tabular-nums;
  }

  /* The opened row */
  /* The briefing on the left, the facts beside it in two columns. */
  .opened {
    display: grid;
    grid-template-columns: minmax(0, 5fr) minmax(0, 7fr);
    gap: 22px 40px;
    padding: 6px 12px 10px 0;
    cursor: auto;
    align-items: start;
  }
  .facts {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 22px 36px;
    align-items: start;
  }
  .opened section {
    min-width: 0;
  }
  .facts .wide {
    grid-column: span 2;
  }
  .chain-h {
    display: flex;
    align-items: baseline;
    gap: 10px;
    margin: 14px 0 6px;
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .chain-h .link {
    all: unset;
    cursor: pointer;
    color: var(--accent);
  }
  .chain-h .link:hover {
    text-decoration: underline;
  }
  .chain-h .link:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .chain-h b {
    margin-left: auto;
    font-family: var(--font-mono);
    font-weight: 500;
    color: var(--text);
  }
  .chain-h.open b {
    color: var(--good);
  }
  .chain {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 4px;
    font-size: var(--text-small);
  }
  .chain li {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }
  .chain .link {
    all: unset;
    cursor: pointer;
    color: var(--accent);
  }
  .chain .link:hover {
    text-decoration: underline;
  }
  .chain .link:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .chain small {
    margin-left: auto;
    color: var(--good);
  }
  .chain .runs {
    margin-left: auto;
    font-family: var(--font-mono);
    color: var(--text-dim);
  }
  .chain .runs + small {
    margin-left: 0;
  }
  .chain .done .link {
    color: var(--text-dim);
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
  .ulegend .k2 {
    background: var(--bg-sunk);
    border: 1px solid var(--border);
    outline: 2px solid var(--accent);
    outline-offset: 1px;
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
  @media (max-width: 1100px) {
    .opened {
      grid-template-columns: minmax(0, 1fr);
    }
  }
  @media (max-width: 700px) {
    .facts {
      grid-template-columns: minmax(0, 1fr);
    }
    .facts .wide {
      grid-column: auto;
    }
  }
</style>
