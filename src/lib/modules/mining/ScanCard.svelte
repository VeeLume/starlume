<script lang="ts">
  // The rock on the calculator, as the scanner shows it: the name large with
  // Starlume's difficulty beside it, the figures as tiles, the composition
  // with bars and quality. "Laser on rock" shows what the scanner reads with
  // the beam on (the loadout's modifiers applied), "Bare rock" the rock on
  // its own. The draft is the store's (frontend rule 1).
  import { Segmented } from "@veelume/ui";
  import type { MiningCatalogView, MiningReportView } from "$lib/bindings";
  import NowCard from "$lib/components/NowCard.svelte";
  import { miningStore, parseNumber } from "$lib/state/mining.svelte";
  import { TONE_HUE, difficulty, int, namesOf, oreName } from "./format";
  import RockArt from "./RockArt.svelte";

  let {
    catalog,
    report,
    onedit,
    onroll,
  }: {
    catalog: MiningCatalogView;
    report: MiningReportView | null;
    onedit: () => void;
    onroll: () => void;
  } = $props();

  let beam = $state<"on" | "off">("on");

  const names = $derived(namesOf(catalog));
  const rock = miningStore.rock;
  const rows = $derived(
    rock.rows.flatMap((r) => {
      const percent = parseNumber(r.percent);
      if (!r.element || percent === null || percent <= 0) return [];
      const q = parseNumber(r.quality);
      return [{ name: names.get(r.element) ?? "Unknown", percent, quality: q === null ? null : Math.round(q) }];
    }),
  );
  const inert = $derived(Math.max(0, 100 - rows.reduce((s, r) => s + r.percent, 0)));
  const mass = $derived(parseNumber(rock.mass, true));
  const source = $derived(miningStore.source);
  const title = $derived(
    source?.rockType ??
      (rows.length ? oreName([...rows].sort((a, b) => b.percent - a.percent)[0].name) : "No rock yet"),
  );

  const yours = $derived(report?.yours ?? null);
  const word = $derived(yours ? difficulty(yours.eval, yours.guide) : null);
  const on = $derived(beam === "on" && yours !== null);
  const resistance = $derived(
    report ? (on && yours ? `${yours.scannerResistancePct.toFixed(0)} %` : `${(report.rock.gadgetResistance * 100).toFixed(0)} %`) : "—",
  );
  const instability = $derived(
    report ? (on && yours ? yours.scannerInstability.toFixed(1) : report.rock.gadgetInstability.toFixed(1)) : "—",
  );
</script>

<NowCard hue="var(--hue-sand)" chip="◆ SCAN" version={title} size="wide">
  {#snippet art()}<RockArt />{/snippet}
  {#snippet top()}
    <Segmented
      class="beam"
      options={[
        { value: "on", label: "Laser on rock" },
        { value: "off", label: "Bare rock" },
      ]}
      value={beam}
      onchange={(v) => (beam = v as "on" | "off")}
    />
  {/snippet}
  {#if word}
    <div><span class="hue-chip" style:--hue={TONE_HUE[word.tone]}>{word.word}</span></div>
  {/if}
  {#if rows.length || mass !== null}
    <div class="stat-tiles">
      <div class="stat-tile"><b>{mass !== null ? `${int(mass)} t` : "—"}</b><span>mass</span></div>
      <div class="stat-tile"><b>{report ? report.rock.scu.toFixed(1) : "—"}</b><span>SCU</span></div>
      <div class="stat-tile"><b class:raised={on}>{resistance}</b><span>resistance</span></div>
      <div class="stat-tile"><b class:raised={on}>{instability}</b><span>instability</span></div>
    </div>
    <div class="comp">
      {#each rows as r, i (i)}
        <span class="p">{r.percent.toFixed(2)}</span>
        <span class="nm">{r.name}</span>
        <span class="track"><i style:width="{Math.min(100, r.percent)}%"></i></span>
        <span class="q">{r.quality === null ? "" : `Q ${r.quality}`}</span>
      {/each}
      {#if inert > 0.005}
        <span class="p">{inert.toFixed(2)}</span>
        <span class="nm faint">Inert</span>
        <span class="track"><i style:width="{inert}%"></i></span>
        <span class="q"></span>
      {/if}
    </div>
  {/if}
  {#snippet action()}
    <span class="from">
      {#if source}{source.pick} at {source.place}{:else if rows.length}typed in{/if}
    </span>
    <button class="btn" onclick={onedit}>{rows.length ? "Edit" : "Type in a scan"}</button>
    <button class="btn" onclick={onroll}>Roll a rock</button>
  {/snippet}
</NowCard>

<style>
  :global(.beam) {
    font-size: var(--text-small);
  }
  .raised {
    color: var(--warn);
  }
  .comp {
    display: grid;
    grid-template-columns: 52px minmax(0, 1fr) minmax(60px, 140px) 52px;
    gap: 6px 12px;
    align-items: center;
    font-size: 0.86rem;
  }
  .p {
    font-family: var(--font-mono);
    font-size: 0.82rem;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .nm {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .faint {
    color: var(--text-faint);
  }
  .track {
    display: block;
    height: 4px;
    border-radius: 2px;
    background: color-mix(in srgb, var(--text) 8%, transparent);
    overflow: hidden;
  }
  .track i {
    display: block;
    height: 100%;
    background: color-mix(in srgb, var(--hue-sand) 65%, transparent);
  }
  .q {
    font-family: var(--font-mono);
    font-size: 0.75rem;
    color: color-mix(in srgb, var(--hue-quality) 80%, var(--text));
    text-align: right;
  }
  .from {
    margin-right: auto;
    font-size: var(--text-small);
    color: var(--text-faint);
  }
</style>
