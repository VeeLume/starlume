<script lang="ts">
  // Contracts, the Standing sub page (pass 2 of the design): one row per
  // faction that gives missions, on the catalog's row grammar. Your rank and
  // score come from the standing read (svc-dossier, gated); a faction without
  // a row stays "not read", never Neutral. Opened, a faction shows its named
  // ladder, how many missions each rank opens (and how many give a
  // blueprint) and the median pay per rank.
  import { onMount } from "svelte";
  import { Button, Expand, createExpansion } from "@veelume/ui";
  import type { MissionEntryView } from "$lib/bindings";
  import NoGameData from "$lib/components/catalog/NoGameData.svelte";
  import { dataStore, ensureChannel } from "$lib/state/data.svelte";
  import { getMissions } from "$lib/state/catalog.svelte";
  import { loadStanding, refreshStanding, standingStore } from "$lib/state/standing.svelte";
  import { factionSummaries, type FactionSummary } from "./model";
  import RankLadder from "./RankLadder.svelte";

  let missions = $state<MissionEntryView[]>([]);
  let ready = $state(false);
  let error = $state("");
  const expanded = createExpansion("one");

  const factions = $derived(factionSummaries(missions, standingStore.rows));
  const fmt = (n: number) => n.toLocaleString("en-US");
  const ago = (secs: number) => {
    const m = Math.round((Date.now() / 1000 - secs) / 60);
    return m < 1 ? "just now" : m < 60 ? `${m} min ago` : m < 48 * 60 ? `${Math.round(m / 60)} h ago` : `${Math.round(m / 1440)} days ago`;
  };

  // Chart geometry: seven columns of 60px in a 420-wide box.
  const COL = 60;
  function bars(f: FactionSummary) {
    const max = Math.max(1, ...f.perRank.map((p) => p.missions));
    return f.perRank.map((p, i) => ({
      x: 8 + i * COL,
      h: (p.missions / max) * 104,
      bh: (p.blueprints / max) * 104,
      n: p.missions,
      you: i === f.you,
    }));
  }
  function line(f: FactionSummary) {
    const vals = f.perRank.map((p) => p.median);
    const max = Math.max(1, ...vals.filter((v): v is number => v != null));
    return vals.map((v, i) => (v == null ? null : { x: 30 + i * COL, y: 118 - (v / max) * 96, v, you: i === f.you }));
  }
  const short = (v: number) => (v >= 1_000_000 ? `${(v / 1_000_000).toFixed(1)}M` : `${Math.round(v / 1000)}k`);

  onMount(() => {
    void loadStanding();
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
                <span>{f.next != null ? fmt(f.next) : "—"}</span>
                <small>{f.next != null ? "next rank at" : f.you >= 0 ? "top rank" : "next rank"}</small>
              </span>
              <span class="c2 r"><span>{f.open != null ? fmt(f.open) : "—"}</span><small>open to you</small></span>
              <span class="c2 r"><span>{fmt(f.missions)}</span><small>missions</small></span>
              <span class="c2 r"><span>{fmt(f.withBlueprint)}</span><small>give a blueprint</small></span>
            </span>
          {/snippet}

          {#if f.ladder}
            <div class="opened">
              <div class="full">
                <RankLadder ladder={f.ladder} you={f.you} score={f.score} window={false} named label="{f.name} ranks" />
              </div>
              <div>
                <p class="cap">Missions by the rank they need <span class="key"><i></i>give a blueprint</span></p>
                <svg class="chart" viewBox="0 0 430 150" role="img" aria-label="{f.name}: missions by the rank they need">
                  {#each bars(f) as b, i (i)}
                    <rect class="bar" class:you={b.you} x={b.x} y={120 - b.h} width="44" height={b.h} rx="2"></rect>
                    <rect class="bar bp" class:you={b.you} x={b.x} y={120 - b.bh} width="44" height={b.bh} rx="2"></rect>
                    <text class="v" class:you={b.you} x={b.x + 22} y={114 - b.h} text-anchor="middle">{b.n}</text>
                    <text x={b.x + 22} y="138" text-anchor="middle">{f.ladder.ranks[i].split(" ")[0]}</text>
                  {/each}
                  <line class="base" x1="0" y1="120" x2="430" y2="120"></line>
                </svg>
              </div>
              <div>
                <p class="cap">Median aUEC by the rank they need</p>
                <svg class="chart" viewBox="0 0 430 150" role="img" aria-label="{f.name}: median pay by the rank they need">
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
                    <text x={30 + i * COL} y="138" text-anchor="middle">{f.ladder.ranks[i].split(" ")[0]}</text>
                  {/each}
                  <line class="base" x1="0" y1="120" x2="430" y2="120"></line>
                </svg>
              </div>
              <p class="full"><a class="lk" href="/contracts?faction={encodeURIComponent(f.name)}&sort=rank">{f.name} missions in the catalog</a></p>
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
    grid-template-columns: 140px 120px 96px 88px 80px 104px;
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
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: 18px 40px;
    padding: 6px 12px 10px 0;
  }
  .full {
    grid-column: 1 / -1;
    margin: 0;
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
  /* Drawn at its own size (430 x 150): stretched to the column, the text
     scaled with it and the charts outgrew the row. */
  .chart {
    display: block;
    width: 100%;
    max-width: 430px;
    height: auto;
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
