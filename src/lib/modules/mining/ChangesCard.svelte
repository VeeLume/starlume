<script lang="ts">
  // Crew and loadout changes: what would do better than the loadout as it
  // is, besides the gadget. The best one is the title (in the crew mate's
  // hue for a crew ship), the next ones are rows, everything the solve
  // tried sits behind "All alternatives". Folded: the title and one line.
  import { Dialog } from "@veelume/ui";
  import type { LoadoutView, MiningCatalogView, MiningReportView } from "$lib/bindings";
  import Monogram from "$lib/components/Monogram.svelte";
  import NowCard from "$lib/components/NowCard.svelte";
  import { nameHue } from "$lib/monogram";
  import { miningUi } from "$lib/state/mining.svelte";
  import { alternatives, byOutcome, improves } from "./alternatives";
  import HelpRow from "./HelpRow.svelte";

  let { report, loadout, catalog }: { report: MiningReportView; loadout: LoadoutView; catalog: MiningCatalogView } =
    $props();

  let all = $state(false);

  const alts = $derived(alternatives(report, loadout, catalog).filter((a) => a.group !== "Gadgets"));
  const yours = $derived(report.yours?.eval ?? null);
  const better = $derived(alts.filter((a) => improves(a.eval, yours)).sort(byOutcome));
  const head = $derived(better[0] ?? null);
  const rest = $derived(better.slice(1, 4));
  const groups = $derived(
    (["Modules", "Crew", "Support"] as const)
      .map((g) => ({ g, rows: alts.filter((a) => a.group === g).sort(byOutcome) }))
      .filter((x) => x.rows.length),
  );
  const crack = (c: number | undefined) => Math.round((c ?? 0) * 100);
</script>

<NowCard
  hue={head?.who ? nameHue(head.who) : "var(--hue-rose)"}
  chip="● CREW AND CHANGES"
  version={head ? "" : "Nothing does better"}
>
  {#snippet top()}
    <button class="fold" onclick={() => (miningUi.fold = !miningUi.fold)} aria-expanded={!miningUi.fold}
      >{miningUi.fold ? "Show ▾" : "Fold ▴"}</button
    >
  {/snippet}
  {#if head}
    <div class="title">
      {#if head.who}<Monogram name={head.who} size={32} />{:else}<span class="glyph" aria-hidden="true">▤</span>{/if}
      <span class="big">{head.name}</span>
      <span class="cr"><b class="gn">{crack(head.eval.difficulty?.controlled)}</b> crack</span>
      <button class="btn add" onclick={() => void head.apply()}>{head.verb}</button>
    </div>
    <span class="kind">{head.kind}</span>
    {#if miningUi.fold}
      {#if rest.length}
        <div class="line">
          {#each rest as a (a.key)}
            <span class="mini"
              >{#if a.who}<Monogram name={a.who} size={22} />{:else}<span class="glyph sm" aria-hidden="true">▤</span
                >{/if}<span>{a.who ?? a.name.split(", ")[0]}</span><b class="gn">{crack(a.eval.difficulty?.controlled)}</b></span
            >
          {/each}
        </div>
      {/if}
    {:else}
      <div class="rows">
        {#each rest as a (a.key)}<HelpRow alt={a} />{/each}
      </div>
    {/if}
  {/if}
  {#snippet action()}
    {#if alts.length && !miningUi.fold}
      <button class="link" onclick={() => (all = true)}>All alternatives ›</button>
    {/if}
  {/snippet}
</NowCard>

<Dialog open={all} onclose={() => (all = false)} title="All alternatives" class="max-w-2xl!">
  <div class="all">
    {#each groups as x (x.g)}
      <p class="label">{x.g}</p>
      <div class="rows">
        {#each x.rows as a (a.key)}<HelpRow alt={a} onapply={() => (all = false)} />{/each}
      </div>
    {/each}
  </div>
</Dialog>

<style>
  .fold {
    padding: 2px 8px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: none;
    font: inherit;
    font-size: var(--text-small);
    color: var(--text-dim);
    cursor: pointer;
  }
  .fold:hover {
    color: var(--text);
    border-color: var(--accent-border);
  }
  .title {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .big {
    font-family: var(--font-display);
    font-weight: var(--weight-bold);
    font-size: 1.9rem;
    line-height: 1;
    color: color-mix(in srgb, var(--hue) 55%, var(--text-strong));
  }
  .cr {
    color: var(--text-dim);
  }
  .gn {
    font-family: var(--font-mono);
    color: var(--good);
  }
  .add {
    margin-left: auto;
    padding: 3px 12px;
  }
  .kind {
    margin-top: -6px;
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .glyph {
    width: 32px;
    height: 32px;
    border-radius: 8px;
    display: grid;
    place-items: center;
    font-size: 0.8rem;
    color: color-mix(in srgb, var(--hue-sand) 85%, var(--text));
    background: color-mix(in srgb, var(--hue-sand) 14%, var(--bg));
    border: 1px solid color-mix(in srgb, var(--hue-sand) 30%, transparent);
  }
  .glyph.sm {
    width: 22px;
    height: 22px;
    border-radius: 6px;
    font-size: 0.66rem;
  }
  .line {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px 16px;
  }
  .mini {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 0.86rem;
  }
  .rows {
    display: flex;
    flex-direction: column;
  }
  .link {
    padding: 0;
    border: 0;
    background: none;
    font: inherit;
    font-size: var(--text-small);
    color: var(--accent);
    cursor: pointer;
  }
  .link:hover {
    text-decoration: underline;
  }
  .all {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 16px 16px;
    overflow: auto;
  }
  .all .label:not(:first-child) {
    margin-top: 8px;
  }
</style>
