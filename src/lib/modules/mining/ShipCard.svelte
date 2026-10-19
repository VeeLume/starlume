<script lang="ts">
  // Your ship on the rock, and the place to change it. The tiles are what
  // it puts on the rock (power and its change to resistance, instability
  // and window; the gadget belongs to the rock). The heads are edited in
  // place: a tile opens the part picker over the scan (the calculator
  // places it). A change is a try-out: the saved loadout it came from
  // changes only on Save (Revert drops it, Save as new keeps both). Crew
  // mates on the rock are rows with ✕; Bring along offers your crew's saved
  // loadouts, each with what it adds, and only when there are some.
  import { X } from "lucide-svelte";
  import type { EvalView, LoadoutView, MiningCatalogView, MiningReportView } from "$lib/bindings";
  import { Select } from "@veelume/ui";
  import Monogram from "$lib/components/Monogram.svelte";
  import NowCard from "$lib/components/NowCard.svelte";
  import {
    addSupport,
    changesFrom,
    compareOnRock,
    miningStore,
    onCalculator,
    removeSupport,
    revertTryOut,
    saveTryOut,
    saveTryOutAs,
    setOwnLasers,
    useLoadout,
  } from "$lib/state/mining.svelte";
  import HeadsEditor from "./HeadsEditor.svelte";
  import { laserShort, lasersLine, namesOf, ownLasers, signedPct } from "./format";
  import TryOut from "./TryOut.svelte";

  let {
    catalog,
    channel,
    loadout,
    report,
    open,
    onopen,
  }: {
    catalog: MiningCatalogView;
    channel: string;
    loadout: LoadoutView;
    report: MiningReportView | null;
    /** The open part picker's key. */
    open: string;
    onopen: (key: string) => void;
  } = $props();

  const names = $derived(namesOf(catalog));
  const own = $derived(ownLasers(loadout));
  const saved = $derived(miningStore.state?.saved ?? []);
  const mine = $derived(saved.filter((s) => s.owner === null));
  const based = $derived(saved.find((s) => s.id === miningStore.state?.basedOn && s.owner === null) ?? null);
  const changes = $derived(based ? changesFrom(based.loadout.lasers, own) : 0);
  const trying = $derived(based ? changes > 0 : own.length > 0);

  // The saved loadout on this rock, for the try-out's "saved" bar.
  let was = $state<EvalView | null | undefined>(undefined);
  $effect(() => {
    const lasers = based?.loadout.lasers;
    const key = JSON.stringify([report?.rock.mass, loadout.gadget]);
    if (!trying || !lasers || !key) return;
    was = undefined;
    let live = true;
    void compareOnRock(channel, [onCalculator(lasers)]).then((e) => live && (was = e?.[0] ?? null));
    return () => (live = false);
  });

  // What the lasers (no gadget) put on the rock.
  const onRock = $derived(report?.yours?.effect ?? null);
  const tiles = $derived([
    { v: onRock ? Math.round(onRock.power).toLocaleString("en-US") : "—", k: "power", good: false },
    { v: onRock ? signedPct(onRock.resistance) : "—", k: "resistance", good: (onRock?.resistance ?? 0) < -0.005 },
    { v: onRock ? signedPct(onRock.instability) : "—", k: "instability", good: (onRock?.instability ?? 0) < -0.005 },
    { v: onRock ? signedPct(onRock.window) : "—", k: "window", good: (onRock?.window ?? 0) > 0.005 },
  ]);

  // Crew mates on the rock, one row per crew loadout; the rest on the bench.
  const aboard = $derived([...new Set(loadout.lasers.flatMap((l) => (l.crew ? [l.crew] : [])))]);
  const crewRows = $derived(
    aboard.map((id) => {
      const s = saved.find((x) => x.id === id);
      const lasers = loadout.lasers.filter((l) => l.crew === id);
      return { id, who: s?.owner ?? lasers[0]?.who ?? "Crew", ship: s?.name ?? "", gear: lasersLine({ lasers, gadget: null, lead: null }, names) };
    }),
  );
  const crack = (e: EvalView | null | undefined) => (e?.difficulty ? Math.round(e.difficulty.controlled * 100) : 0);
  const bench = $derived(
    saved
      .filter((s) => s.owner !== null && !aboard.includes(s.id))
      .map((s) => {
        const row = report?.crew.find((c) => c.id === s.id);
        return { s, gain: row ? crack(row.eval) - crack(report?.yours?.eval) : null };
      })
      .sort((a, b) => (b.gain ?? -1) - (a.gain ?? -1)),
  );

  const first = $derived(own.length ? (catalog.lasers.find((l) => l.id === own[0].laser) ?? null) : null);
  const range = $derived(
    first && first.optimalRangeM !== null && first.maxRangeM !== null
      ? `${first.optimalRangeM.toFixed(0)}–${first.maxRangeM.toFixed(0)} m`
      : "",
  );
  const title = $derived(based?.name ?? (first ? laserShort(first.name) : "No ship yet"));
  const sub = $derived(own.length > 1 ? `${own.length} heads${range ? `, ${range}` : ""}` : first ? `${laserShort(first.name)}${range ? `, ${range}` : ""}` : "");
</script>

<NowCard hue="var(--hue-sky)" chip={aboard.length ? "● ON THE ROCK" : "● YOUR SHIP"} version={title} name={trying && based ? "" : sub}>
  {#snippet top()}
    {#if mine.length}
      <Select
        label="Your loadout"
        options={mine.map((s) => ({ value: s.id, label: s.name }))}
        value={based?.id ?? ""}
        placeholder="Not saved"
        onchange={(id) => void useLoadout(id)}
      />
    {/if}
  {/snippet}
  {#if trying && based}<span class="mod">modified</span>{/if}
  {#if trying}
    <TryOut
      {was}
      now={report?.yours?.eval}
      {changes}
      saved={based !== null}
      saveLabel={based ? `Save to ${based.name}` : "Save"}
      name={based ? `${based.name} 2` : title}
      onrevert={() => void revertTryOut()}
      onsave={() => void saveTryOut()}
      onsaveas={saveTryOutAs}
    />
  {/if}
  {#if own.length}
    <div class="stat-tiles">
      {#each tiles as t (t.k)}<div class="stat-tile"><b class:good={t.good}>{t.v}</b><span>{t.k}</span></div>{/each}
    </div>
  {/if}
  <HeadsEditor {catalog} lasers={own} base={based?.loadout.lasers ?? null} {open} {onopen} onchange={(next) => void setOwnLasers(next)} />
  {#each crewRows as c (c.id)}
    <div class="crew">
      <Monogram name={c.who} size={22} />
      <b>{c.who}</b>
      <span class="faint">{c.ship ? `${c.ship}, ` : ""}{c.gear}</span>
      <button type="button" class="x" onclick={() => void removeSupport(c.id, c.who)} title="Take {c.who} off the rock" aria-label="Remove {c.who}"
        ><X size={13} /></button
      >
    </div>
  {/each}
  {#if bench.length && own.length}
    <div class="bring">
      <span class="faint">Bring along</span>
      {#each bench.slice(0, 4) as b (b.s.id)}
        <button type="button" class="bn" onclick={() => void addSupport(b.s.id)} title="{b.s.owner}'s {b.s.name}">
          <Monogram name={b.s.owner ?? ""} size={22} />{b.s.owner}{#if b.gain !== null}<b class:good={b.gain > 0}
              >{b.gain > 0 ? "+" : b.gain < 0 ? "−" : "±"}{Math.abs(b.gain)}</b
            >{/if}
        </button>
      {/each}
    </div>
  {/if}
</NowCard>

<style>
  .mod {
    margin-top: -6px;
    font-family: var(--font-mono);
    font-size: 0.78rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--warn);
  }
  .good {
    color: var(--good);
  }
  .crew {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 8px;
    border-radius: 8px;
    font-size: 0.82rem;
    background: color-mix(in srgb, var(--bg) 55%, transparent);
    border: 1px solid color-mix(in srgb, var(--text) 8%, transparent);
  }
  .crew .faint {
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
    padding: 3px 10px 3px 4px;
    border-radius: var(--radius-pill);
    font: inherit;
    font-size: 0.8rem;
    color: var(--text);
    background: transparent;
    border: 1px dashed var(--border);
    cursor: pointer;
  }
  .bn:hover {
    border-color: var(--accent);
  }
  .bn b {
    font-family: var(--font-mono);
    color: var(--text-dim);
  }
  .bn b.good {
    color: var(--good);
  }
</style>
