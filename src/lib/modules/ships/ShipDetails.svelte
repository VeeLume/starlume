<script lang="ts">
  // The ship's own stats and graphs, over its expanded holo: the columns
  // keep to the sides, the ship stays in the middle, and the empty parts
  // let the pointer through to it. The row's menus sit above, so switching
  // hull or loadout keeps this open.
  import { cubicOut } from "svelte/easing";
  import type { ShipPageView, ShipRowView } from "$lib/bindings";
  import { shipsPrices } from "$lib/state/ships.svelte";
  import AccelRose from "./AccelRose.svelte";
  import QtChart from "./QtChart.svelte";
  import ShipFigures from "./ShipFigures.svelte";
  import ShipPrices from "./ShipPrices.svelte";
  import { crewParts, crewTitle } from "./seats";

  let {
    page,
    row,
    name,
    from,
    onclose,
  }: {
    page: ShipPageView;
    row: ShipRowView | null;
    name: string;
    /** The holo card's place: what this grows out of and shrinks back into. */
    from: HTMLElement | undefined;
    onclose: () => void;
  } = $props();

  const s = $derived(page.summary);

  /** Reveals the sheet from the card's rectangle; nothing is resized, so
   *  the charts stand still while it opens. */
  function grow(node: HTMLElement, card: HTMLElement | undefined) {
    if (!card || matchMedia("(prefers-reduced-motion: reduce)").matches) return { duration: 0 };
    const a = card.getBoundingClientRect();
    const b = node.getBoundingClientRect();
    const edges = [a.top - b.top, b.right - a.right, b.bottom - a.bottom, a.left - b.left];
    return {
      duration: 200,
      easing: cubicOut,
      css: (t: number) => `clip-path: inset(${edges.map((e) => `${(e * (1 - t)).toFixed(1)}px`).join(" ")} round var(--radius-lg))`,
    };
  }

  function key(e: KeyboardEvent) {
    if (e.key === "Escape" && !e.defaultPrevented) onclose();
  }
</script>

<svelte:window onkeydown={key} />

<div class="sheet" role="region" aria-label="Ship stats" transition:grow={from}>
  <div class="panel graphs">
    {#if s.flight}
      <section aria-label="Flight">
        <h3 class="label">Flight</h3>
        <AccelRose normal={s.flight.acceleration} boosted={s.flight.accelerationBoosted} />
      </section>
    {/if}
    {#if s.quantum}
      <section aria-label="Quantum travel">
        <h3 class="label">Quantum travel</h3>
        <QtChart drive={s.quantum} />
      </section>
    {/if}
    <section aria-label="Where to get it">
      <h3 class="label">Where to get it</h3>
      <ShipPrices prices={shipsPrices.of(page.ship)} />
    </section>
  </div>
  <div class="panel id">
    <span class="meta">
      {#if row?.manufacturerCode}<b class="code">{row.manufacturerCode}</b>{/if}
      {#each [row?.career, row?.role].filter(Boolean) as m (m)}<span>{m}</span>{/each}
      <span class="crew" title={crewTitle(s.crew)}><b>{s.crew.crew} crew</b>{#each crewParts(s.crew) as c (c)}<i>{c}</i>{/each}</span>
    </span>
    <h1>{name}</h1>
  </div>
  <div class="panel figures">
    <ShipFigures {page} />
  </div>
</div>

<style>
  .sheet {
    position: absolute;
    inset: 0 0 auto;
    z-index: 10;
    min-height: 100%;
    box-sizing: border-box;
    display: grid;
    grid-template-columns: minmax(0, 440px) minmax(0, 1fr) minmax(0, 440px);
    grid-template-areas: "graphs id figures";
    gap: 16px;
    align-items: start;
    padding: 16px;
    pointer-events: none;
  }
  .panel {
    display: flex;
    flex-direction: column;
    gap: 20px;
    min-width: 0;
    padding: 14px 16px;
    border-radius: var(--radius-lg);
    background: color-mix(in srgb, var(--bg-sunk) 80%, transparent);
    pointer-events: auto;
  }
  .graphs {
    grid-area: graphs;
  }
  .figures {
    grid-area: figures;
  }
  .id {
    grid-area: id;
    justify-self: center;
    align-items: center;
    gap: 4px;
    text-align: center;
  }
  /* Too narrow for a free middle: the ship on top (ShipHolo), then the name
   * over the two columns. */
  @container (width < 1400px) {
    .sheet {
      padding-top: calc(55vh + 16px);
      grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
      grid-template-areas:
        "id id"
        "graphs figures";
    }
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
  }
  h3 {
    margin: 0;
  }
  h1 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.6rem;
    color: var(--text-strong);
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 6px 12px;
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .crew {
    display: inline-flex;
    flex-wrap: wrap;
    gap: 4px;
    align-items: center;
  }
  .crew b {
    font-weight: 600;
    color: var(--text-dim);
    margin-right: 2px;
  }
  .crew i {
    font-style: normal;
    font-size: 0.7rem;
    padding: 0 6px;
    border-radius: var(--radius-chip);
    border: 1px solid var(--border);
    color: var(--text-dim);
  }
  .code {
    font: 700 0.72rem var(--font-mono);
    color: var(--hue-sand);
  }
</style>
