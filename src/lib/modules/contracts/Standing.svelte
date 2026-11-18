<script lang="ts">
  // Contracts, the Standing sub page (pass 2 of the design): one row per
  // faction that gives missions, on the catalog's row grammar. Your rank and
  // score come from the standing read (svc-dossier, gated); a faction without
  // a row stays "not read", never Neutral. Opened, a faction shows its named
  // ladder, a figure strip of where you are with it (rep to the next rank,
  // the most rep a run open to you gives, what the next rank opens), and
  // three charts: missions per rank (blueprint share), median pay per rank,
  // and the kinds of work it gives.
  import { onMount } from "svelte";
  import { Button, Expand, createExpansion } from "@veelume/ui";
  import type { MissionEntryView } from "$lib/bindings";
  import NoGameData from "$lib/components/catalog/NoGameData.svelte";
  import { dataStore, ensureChannel } from "$lib/state/data.svelte";
  import { getMissions } from "$lib/state/catalog.svelte";
  import { blueprintsStore, loadOwnedBlueprints } from "$lib/state/blueprints.svelte";
  import { loadStanding, refreshStanding, standingStore } from "$lib/state/standing.svelte";
  import { factionSummaries, type FactionSummary } from "./model";
  import RankLadder from "./RankLadder.svelte";
  import PlanButtons from "./PlanButtons.svelte";
  import { addRankPlan, plansStore } from "$lib/state/plans.svelte";

  let missions = $state<MissionEntryView[]>([]);
  let ready = $state(false);
  let error = $state("");
  const expanded = createExpansion("one");
  /** Width of the chart row: charts draw at their real size, text never scales. */
  let chartsWidth = $state(0);
  const colW = $derived(Math.max(240, (chartsWidth - 2 * 32) / 3));

  const factions = $derived(factionSummaries(missions, standingStore.rows, standingStore.completed));
  const fmt = (n: number) => n.toLocaleString("en-US");
  const ago = (secs: number) => {
    const m = Math.round((Date.now() / 1000 - secs) / 60);
    return m < 1 ? "just now" : m < 60 ? `${m} min ago` : m < 48 * 60 ? `${Math.round(m / 60)} h ago` : `${Math.round(m / 1440)} days ago`;
  };

  /** The rank picked on each faction's ladder, to plan; the next one by default. */
  let picks = $state<Record<string, number>>({});
  const pickOf = (f: FactionSummary) => picks[f.name] ?? (f.ladder && f.you >= 0 && f.you + 1 < f.ladder.ranks.length ? f.you + 1 : -1);
  const toNext = (f: FactionSummary) => (f.next != null && f.score != null ? Math.max(0, f.next - f.score) : null);
  const nextName = (f: FactionSummary) => (f.ladder && f.you >= 0 ? (f.ladder.ranks[f.you + 1] ?? null) : null);
  const newBlueprints = (f: FactionSummary) => f.openBlueprints.filter((g) => !blueprintsStore.owns(g)).length;

  // Chart geometry in pixels (the viewBox is the drawn size): H tall, the
  // baseline at BASE, the rank names under it; one column per rank, so a
  // ladder of any length fits.
  const H = 150;
  const BASE = 118;
  const TOP = 22;
  const step = (f: FactionSummary) => colW / Math.max(1, f.perRank.length);
  /** A rank's name cut to its column (11px text, about 6px a character). */
  const cut = (name: string, w: number) => {
    const max = Math.max(3, Math.floor((w - 6) / 6));
    return name.length <= max ? name : `${name.slice(0, max - 1)}…`;
  };
  function bars(f: FactionSummary) {
    const max = Math.max(1, ...f.perRank.map((p) => p.missions));
    const c = step(f);
    const bw = Math.min(40, c * 0.6);
    return f.perRank.map((p, i) => ({
      cx: c * i + c / 2,
      bw,
      h: (p.missions / max) * (BASE - TOP),
      bh: (p.blueprints / max) * (BASE - TOP),
      n: p.missions,
      you: i === f.you,
    }));
  }
  function line(f: FactionSummary) {
    const vals = f.perRank.map((p) => p.median);
    const max = Math.max(1, ...vals.filter((v): v is number => v != null));
    const c = step(f);
    return vals.map((v, i) => (v == null ? null : { x: c * i + c / 2, y: BASE - (v / max) * (BASE - TOP), v, you: i === f.you }));
  }
  const short = (v: number) => (v >= 1_000_000 ? `${(v / 1_000_000).toFixed(1)}M` : `${Math.round(v / 1000)}k`);

  onMount(() => {
    void loadStanding();
    void loadOwnedBlueprints();
    void (async () => {
      const channel = await ensureChannel();
      ready = true;
      if (!channel) return;
      const r = await getMissions(channel);
      if (typeof r === "string") error = r;
      else missions = r;
    })();
  });
</script>

{#if ready && !dataStore.channel}
  <NoGameData />
{:else}
  <div class="head">
    {#if standingStore.known && standingStore.fetchedAt != null}
      <span class="note">Read from the game {ago(standingStore.fetchedAt)}</span>
    {:else}
      <span class="note">Your rank with each faction is not read yet</span>
    {/if}
    <span class="grow"></span>
    <Button variant={standingStore.known ? "outline" : "primary"} onclick={() => void refreshStanding()} disabled={standingStore.refreshing}>
      {standingStore.refreshing ? "Reading…" : standingStore.known ? "Read again" : "Read your standing"}
    </Button>
  </div>
  {#if standingStore.error}<p class="error">{standingStore.error}</p>{/if}
  {#if error}<p class="error">{error}</p>{/if}

  <div class="list" role="list" aria-label="Factions">
    {#each factions as f (f.name)}
      {@const gap = toNext(f)}
      <div role="listitem">
        <Expand.Row title={f.name} open={expanded.has(f.name)} ontoggle={() => expanded.toggle(f.name)}>
          {#snippet right()}
            <span class="cols">
              <span class="c2">
                <span class:none={f.you < 0}>{f.you >= 0 && f.ladder ? f.ladder.ranks[f.you] : "not read"}</span>
                <small>{f.score != null ? `${fmt(f.score)} rep` : "no row"}</small>
              </span>
              <span class="c2 lad">
                {#if f.ladder}<RankLadder ladder={f.ladder} you={f.you} score={f.score} window={false} label="{f.name} ranks" />{/if}
              </span>
              <span class="c2 r">
                <span>{gap != null ? fmt(gap) : "—"}</span>
                <small>{gap == null && f.you >= 0 ? "top rank" : "rep to next rank"}</small>
              </span>
              <span class="c2 r"><span>{f.open != null ? fmt(f.open) : "—"}</span><small>open to you</small></span>
              <span class="c2 r"><span>{fmt(f.missions)}</span><small>missions</small></span>
              <span class="c2 r"><span>{fmt(f.withBlueprint)}</span><small>give a blueprint</small></span>
            </span>
          {/snippet}

          {#if f.ladder}
            {@const nx = nextName(f)}
            {@const pick = pickOf(f)}
            {@const runs = gap != null && f.bestRep ? Math.ceil(gap / f.bestRep) : null}
            {@const most = Math.max(1, ...f.kinds.map((k) => k.missions))}
            <div class="opened">
              <RankLadder
                ladder={f.ladder}
                you={f.you}
                score={f.score}
                window={false}
                named
                label="{f.name} ranks"
                picked={pick}
                onpick={f.geid && f.scope ? (i) => (picks[f.name] = i) : undefined}
              />
              {#if f.geid && f.scope && pick >= 0}
                {@const rank = { faction: f.name, geid: f.geid, scope: f.scope, standing: f.ladder.ids[pick], rank: f.ladder.ranks[pick] }}
                <div class="plan-rank">
                  <PlanButtons
                    planned={plansStore.rankOf(rank.geid, rank.scope, rank.standing)}
                    add={(parked) => addRankPlan(rank, parked)}
                    what={rank.rank}
                  />
                </div>
              {/if}

              {#if f.you >= 0}
                <div class="stat-tiles">
                  <div class="stat-tile"><b>{gap != null ? fmt(gap) : "—"}</b><span>{nx ? `rep to ${nx}` : "top rank"}</span></div>
                  <div class="stat-tile">
                    <b>{f.bestRep != null ? `+${fmt(f.bestRep)}` : "—"}</b><span>most rep per run open to you</span>
                  </div>
                  {#if nx}
                    <div class="stat-tile"><b>{runs ?? "—"}</b><span>runs of it to {nx}</span></div>
                    <div class="stat-tile"><b>{fmt(f.perRank[f.you + 1].missions)}</b><span>missions {nx} opens</span></div>
                  {/if}
                  <div class="stat-tile"><b>{fmt(f.openBlueprints.length)}</b><span>blueprints open to you</span></div>
                  {#if blueprintsStore.fetchedAt != null}
                    <div class="stat-tile"><b>{fmt(newBlueprints(f))}</b><span>of them new to you</span></div>
                  {/if}
                  <div class="stat-tile"><b>{fmt(f.done)}</b><span>contracts done</span></div>
                </div>
              {/if}

              <div class="charts" bind:clientWidth={chartsWidth}>
                <div>
                  <p class="cap">Missions by the rank they need <span class="key"><i></i>give a blueprint</span></p>
                  <svg class="chart" width={colW} height={H} viewBox="0 0 {colW} {H}" role="img" aria-label="{f.name}: missions by the rank they need">
                    {#each bars(f) as b, i (i)}
                      <rect class="bar" class:you={b.you} x={b.cx - b.bw / 2} y={BASE - b.h} width={b.bw} height={b.h} rx="2"></rect>
                      <rect class="bar bp" x={b.cx - b.bw / 2} y={BASE - b.bh} width={b.bw} height={b.bh} rx="2"></rect>
                      <text class="v" class:you={b.you} x={b.cx} y={BASE - b.h - 6} text-anchor="middle">{b.n}</text>
                      <text class:you={b.you} x={b.cx} y={BASE + 18} text-anchor="middle"
                        >{cut(f.ladder.ranks[i], step(f))}<title>{f.ladder.ranks[i]}</title></text
                      >
                    {/each}
                    <line class="base" x1="0" y1={BASE} x2={colW} y2={BASE}></line>
                  </svg>
                </div>
                <div>
                  <p class="cap">Median aUEC by the rank they need</p>
                  <svg class="chart" width={colW} height={H} viewBox="0 0 {colW} {H}" role="img" aria-label="{f.name}: median pay by the rank they need">
                    <polyline
                      class="line"
                      points={line(f)
                        .filter((p) => p != null)
                        .map((p) => `${p!.x},${p!.y}`)
                        .join(" ")}
                    ></polyline>
                    {#each line(f) as p, i (i)}
                      {#if p}
                        <circle class="dot" class:you={p.you} cx={p.x} cy={p.y} r="4"></circle>
                        <text class="v" class:you={p.you} x={p.x} y={p.y - 9} text-anchor="middle">{short(p.v)}</text>
                      {/if}
                      <text class:you={i === f.you} x={step(f) * i + step(f) / 2} y={BASE + 18} text-anchor="middle"
                        >{cut(f.ladder.ranks[i], step(f))}<title>{f.ladder.ranks[i]}</title></text
                      >
                    {/each}
                    <line class="base" x1="0" y1={BASE} x2={colW} y2={BASE}></line>
                  </svg>
                </div>
                <div>
                  <p class="cap">Kinds of work {#if f.open != null}<span class="key open"><i></i>open to you</span>{/if}</p>
                  <ul class="kinds">
                    {#each f.kinds as k (k.name)}
                      <li>
                        <span class="kn" title={k.name}>{k.name}</span>
                        <span class="kb">
                          <i style:width="{(k.missions / most) * 100}%"></i>
                          {#if f.open != null}<i class="o" style:width="{(k.open / most) * 100}%"></i>{/if}
                        </span>
                        <span class="kc">{f.open != null ? `${k.open} / ${k.missions}` : k.missions}</span>
                      </li>
                    {/each}
                  </ul>
                </div>
              </div>
              <p><a class="lk" href="/contracts?faction={encodeURIComponent(f.name)}&sort=rank">{f.name} missions in the catalog</a></p>
            </div>
          {/if}
        </Expand.Row>
      </div>
    {/each}
  </div>
{/if}

<style>
  .head {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .grow {
    flex: 1;
  }
  .note {
    font-size: var(--text-small);
    color: var(--text-dim);
  }
  .error {
    color: var(--bad);
    font-size: var(--text-small);
    margin: 0;
  }
  .list {
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    overflow: auto;
    min-height: 0;
  }
  .cols {
    display: grid;
    grid-template-columns: 140px 120px 112px 88px 80px 104px;
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
  .c2 .none {
    color: var(--text-faint);
  }
  .opened {
    display: flex;
    flex-direction: column;
    gap: 18px;
    padding: 6px 12px 10px 0;
  }
  .opened > p {
    margin: 0;
  }
  .plan-rank {
    margin-top: -6px;
  }
  .charts {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    column-gap: 32px;
  }
  .cap {
    display: flex;
    align-items: center;
    gap: 12px;
    margin: 0 0 6px;
    font-size: 0.75rem;
    color: var(--text-dim);
  }
  .key {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .key i {
    display: inline-block;
    width: 10px;
    height: 10px;
    border-radius: 2px;
    background: color-mix(in srgb, var(--hue-rose) 45%, transparent);
  }
  /* Drawn at the column's pixel size (viewBox = width x height): a scaled
     viewBox grew the text with the column. */
  .chart {
    display: block;
  }
  .key.open i {
    background: color-mix(in srgb, var(--accent) 55%, transparent);
  }
  .kinds {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .kinds li {
    display: grid;
    grid-template-columns: 112px minmax(0, 1fr) 64px;
    column-gap: 10px;
    align-items: center;
    font-size: 0.75rem;
  }
  .kn {
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .kb {
    position: relative;
    height: 10px;
  }
  .kb i {
    position: absolute;
    inset: 0 auto 0 0;
    border-radius: 2px;
    background: color-mix(in srgb, var(--text-dim) 28%, transparent);
  }
  .kb i.o {
    background: color-mix(in srgb, var(--accent) 55%, transparent);
  }
  .kc {
    text-align: right;
    color: var(--text-dim);
    font-variant-numeric: tabular-nums;
  }
  .chart text {
    font-size: 11px;
    fill: var(--text-faint);
  }
  .chart text.v {
    fill: var(--text-dim);
  }
  .chart text.you {
    fill: var(--text-strong);
    font-weight: 700;
  }
  .chart .bar {
    fill: color-mix(in srgb, var(--text-dim) 28%, transparent);
  }
  .chart .bar.you {
    fill: color-mix(in srgb, var(--accent) 45%, transparent);
  }
  .chart .bar.bp {
    fill: color-mix(in srgb, var(--hue-rose) 45%, transparent);
  }
  .chart .base {
    stroke: var(--border);
    stroke-width: 1;
  }
  .chart .line {
    fill: none;
    stroke: color-mix(in srgb, var(--hue-sand) 80%, transparent);
    stroke-width: 2;
  }
  .chart .dot {
    fill: var(--bg);
    stroke: var(--hue-sand);
    stroke-width: 2;
  }
  .chart .dot.you {
    fill: var(--hue-sand);
  }
  .lk {
    font-size: var(--text-small);
    color: var(--accent);
    text-decoration: none;
  }
  .lk:hover {
    text-decoration: underline;
  }
</style>
