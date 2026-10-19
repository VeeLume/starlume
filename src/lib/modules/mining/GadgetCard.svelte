<script lang="ts">
  // The gadget on the rock. It changes the rock, not the ship, so it sits
  // under the scan: the gadget in use as the title with its own effects in
  // words, then every gadget as a row (the one in use highlighted, best
  // first) with its crack for this loadout and its figures in the picker's
  // aligned columns; one tap puts it on. When no gadget makes the rock
  // crack, one line says why and the crack column drops out.
  import type { LoadoutView, MiningCatalogView, MiningReportView } from "$lib/bindings";
  import NowCard from "$lib/components/NowCard.svelte";
  import { setLoadout } from "$lib/state/mining.svelte";
  import CrackCell from "./CrackCell.svelte";
  import FxLines from "./FxLines.svelte";
  import { FX_COLUMNS, fxOf, helps, int, signed } from "./format";

  let { catalog, report, loadout }: { catalog: MiningCatalogView; report: MiningReportView; loadout: LoadoutView } =
    $props();

  const COLS = FX_COLUMNS.filter((c) => c.key !== "power" && c.key !== "overcharge");
  const byId = $derived(new Map(catalog.gadgets.map((g) => [g.id, g])));
  const name = (id: string | null) => (id ? (byId.get(id)?.name ?? "Unknown gadget") : "No gadget");
  const crack = (c: number | undefined) => Math.round((c ?? 0) * 100);
  const current = $derived(byId.get(loadout.gadget ?? "") ?? null);
  const yours = $derived(report.yours?.eval ?? null);
  const nothing = $derived(report.gadgets.length > 0 && report.gadgets.every((g) => !g.eval.difficulty));
  // How far short the best gadget leaves the loadout.
  const short = $derived(nothing ? Math.min(...report.gadgets.map((g) => g.eval.need - g.eval.power)) : 0);
  const use = (id: string | null) => void setLoadout({ ...loadout, gadget: id });
</script>

<NowCard
  hue="var(--hue-apricot)"
  chip="◆ GADGET"
  version={name(loadout.gadget)}
  name={!nothing && yours?.difficulty ? `${crack(yours.difficulty.controlled)} crack` : ""}
>
  <div class="fx">
    {#if current}<FxLines fx={fxOf(current.modifiers)} />{:else}<span class="faint">nothing on the rock</span>{/if}
  </div>
  {#if nothing}
    <p class="why">
      <b>No gadget cracks it at {int(report.rock.mass)} t.</b>
      {short > 0 ? `The loadout is ${int(short)} power short even with the best one; gadgets only ease the rock.` : ""}
      A lighter rock (What if it weighs) or more lasers.
    </p>
  {/if}
  <div class="rows">
    <div class="row h" class:nogo={nothing}>
      <span>Gadget</span>{#if !nothing}<span>crack</span>{/if}{#each COLS as c (c.key)}<span>{c.label}</span>{/each}
    </div>
    {#each report.gadgets as g (g.gadget ?? "")}
      {@const fx = g.gadget ? fxOf(byId.get(g.gadget)?.modifiers ?? fxNone) : null}
      <button type="button" class="row" class:cur={g.gadget === loadout.gadget} class:nogo={nothing} onclick={() => use(g.gadget)}>
        <b>{name(g.gadget)}</b>
        {#if !nothing}<CrackCell value={g.eval.difficulty ? crack(g.eval.difficulty.controlled) : null} />{/if}
        {#each COLS as c (c.key)}
          {@const v = fx?.[c.key] ?? 0}
          <span class="v" class:good={v !== 0 && helps(c.key, v)} class:bad={v !== 0 && !helps(c.key, v)} class:z={v === 0}
            >{v === 0 ? "·" : signed(v)}</span
          >
        {/each}
      </button>
    {/each}
  </div>
</NowCard>

<script lang="ts" module>
  const fxNone = {
    resistance: null,
    instability: null,
    windowSize: null,
    windowRate: null,
    shatterDamage: null,
    clusterFactor: null,
    catastrophicRate: null,
  };
</script>

<style>
  .fx {
    margin-top: -6px;
    font-size: 0.86rem;
  }
  .faint {
    color: var(--text-faint);
  }
  .why {
    margin: 0;
    padding: 7px 10px;
    border-radius: 8px;
    font-size: var(--text-small);
    color: var(--text-dim);
    background: color-mix(in srgb, var(--text) 4%, transparent);
  }
  .why b {
    color: var(--text);
  }
  .rows {
    display: flex;
    flex-direction: column;
  }
  .row {
    display: grid;
    grid-template-columns: minmax(88px, 1.1fr) 112px repeat(4, minmax(44px, 0.55fr));
    gap: 0 8px;
    align-items: center;
    padding: 4px 8px;
    margin-inline: -8px;
    border: 0;
    border-radius: 6px;
    background: none;
    font: inherit;
    font-size: 0.84rem;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .row.nogo {
    grid-template-columns: minmax(88px, 1.1fr) repeat(4, minmax(44px, 0.55fr));
  }
  .row b {
    font-weight: var(--weight-semibold);
  }
  .row:hover {
    background: color-mix(in srgb, var(--text) 5%, transparent);
  }
  .row.cur {
    background: var(--accent-fill-weak);
    box-shadow: inset 0 0 0 1px var(--accent-border);
  }
  .row.h {
    cursor: default;
    padding-block: 0 4px;
    font-size: 0.64rem;
    letter-spacing: 0.06em;
    text-transform: var(--label-transform);
    color: var(--text-faint);
  }
  .row.h:hover {
    background: none;
  }
  .row.h span:not(:first-child) {
    text-align: right;
  }
  .v {
    font-family: var(--font-mono);
    font-size: 0.78rem;
    text-align: right;
  }
  .good {
    color: var(--good);
  }
  .bad {
    color: var(--bad);
  }
  .z {
    color: var(--text-faint);
    opacity: 0.55;
  }
  @media (max-width: 560px) {
    .row,
    .row.nogo {
      grid-template-columns: minmax(80px, 1fr) 100px;
    }
    .row .v,
    .row.h span:nth-child(n + 3) {
      display: none;
    }
  }
</style>
