<script lang="ts">
  // The gadget on the rock. A gadget changes the rock, not the ship, so it
  // sits beside the scan: the gadget in use as the title with its cracks,
  // every other one as a row with its outcome for this loadout and its two
  // strongest effects, one tap to put it on. Folded: the best other one.
  import type { LoadoutView, MiningCatalogView, MiningReportView } from "$lib/bindings";
  import NowCard from "$lib/components/NowCard.svelte";
  import { miningUi, setLoadout } from "$lib/state/mining.svelte";
  import { effects } from "./format";
  import OutcomeBar from "./OutcomeBar.svelte";

  let { catalog, report, loadout }: { catalog: MiningCatalogView; report: MiningReportView; loadout: LoadoutView } =
    $props();

  const byId = $derived(new Map(catalog.gadgets.map((g) => [g.id, g])));
  const name = (id: string | null) => (id ? (byId.get(id)?.name ?? "Unknown gadget") : "No gadget");
  const crack = (c: number | undefined) => Math.round((c ?? 0) * 100);
  const yours = $derived(report.yours?.eval.difficulty ?? null);
  const others = $derived(report.gadgets.filter((g) => g.gadget !== loadout.gadget));
  const best = $derived(others[0] ?? null);
  const use = (id: string | null) => void setLoadout({ ...loadout, gadget: id });
</script>

<NowCard hue="var(--hue-apricot)" chip="◆ GADGET" version={name(loadout.gadget)}>
  {#snippet top()}
    <button class="fold" onclick={() => (miningUi.fold = !miningUi.fold)} aria-expanded={!miningUi.fold}
      >{miningUi.fold ? "Show ▾" : "Fold ▴"}</button
    >
  {/snippet}
  <div class="sub"><b class="gn">{yours ? crack(yours.controlled) : "—"}</b> crack</div>
  {#if miningUi.fold}
    {#if best}
      <div class="line">
        <span class="faint">Best on this rock</span>
        <button class="btn try" onclick={() => use(best.gadget)}
          >{name(best.gadget)} <b class="gn">{best.eval.difficulty ? crack(best.eval.difficulty.controlled) : "—"}</b></button
        >
      </div>
    {/if}
  {:else}
    <div class="rows">
      {#each others as g (g.gadget ?? "")}
        {@const gadget = g.gadget ? byId.get(g.gadget) : null}
        <button class="row" onclick={() => use(g.gadget)}>
          <b>{name(g.gadget)}</b>
          <OutcomeBar d={g.eval.difficulty} height={6} />
          <span class="c">{g.eval.difficulty ? crack(g.eval.difficulty.controlled) : "—"}</span>
          <span class="fx">
            {#if gadget}
              {#each effects(gadget.modifiers).slice(0, 2) as fx (fx.text)}
                <span class="hue-chip" style:--hue={fx.good ? "var(--good)" : "var(--warn)"}>{fx.text}</span>
              {/each}
            {/if}
          </span>
        </button>
      {/each}
    </div>
  {/if}
</NowCard>

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
  .sub {
    margin-top: -6px;
    color: var(--text-dim);
  }
  .gn {
    font-family: var(--font-mono);
    color: var(--good);
  }
  .line {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px 16px;
    font-size: var(--text-small);
  }
  .faint {
    color: var(--text-faint);
  }
  .try {
    margin-left: auto;
    padding: 3px 10px;
    display: inline-flex;
    gap: 6px;
    align-items: baseline;
  }
  .rows {
    display: flex;
    flex-direction: column;
  }
  .row {
    display: grid;
    grid-template-columns: 96px minmax(50px, 1fr) 28px minmax(0, 1.7fr);
    gap: 10px;
    align-items: center;
    padding: 6px 8px;
    margin-inline: -8px;
    border: 0;
    border-top: 1px solid color-mix(in srgb, var(--text) 8%, transparent);
    border-radius: 6px;
    background: none;
    font: inherit;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .row:first-child {
    border-top-color: transparent;
  }
  .row:hover {
    background: color-mix(in srgb, var(--text) 4%, transparent);
  }
  .row b {
    font-size: 0.86rem;
    font-weight: var(--weight-semibold);
  }
  .c {
    font-family: var(--font-mono);
    font-size: 0.8rem;
    color: var(--good);
    text-align: right;
  }
  .fx {
    display: flex;
    flex-wrap: wrap;
    gap: 3px;
  }
  .fx .hue-chip {
    font-size: 0.62rem;
    padding: 0 5px;
  }
  @media (max-width: 560px) {
    .row {
      grid-template-columns: 80px minmax(40px, 1fr) 26px;
    }
    .fx {
      display: none;
    }
  }
</style>
