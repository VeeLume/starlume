<script lang="ts">
  // The holo opened: the ship's own stats and graphs, grown out of the
  // picture over the build. The columns are Build's, so the picture stays
  // exactly where it was and the sheet opens around it. Its menus sit above
  // it, so switching hull or loadout keeps it open.
  import { onMount } from "svelte";
  import { cubicOut } from "svelte/easing";
  import type { ShipPageView, ShipRowView } from "$lib/bindings";
  import { shipsPrices } from "$lib/state/ships.svelte";
  import AccelRose from "./AccelRose.svelte";
  import QtChart from "./QtChart.svelte";
  import ShipFigures from "./ShipFigures.svelte";
  import ShipHolo from "./ShipHolo.svelte";
  import ShipPrices from "./ShipPrices.svelte";
  import { crewParts, crewTitle } from "./seats";

  let {
    page,
    row,
    name,
    picture,
    from,
    onclose,
  }: {
    page: ShipPageView;
    row: ShipRowView | null;
    name: string;
    picture: string | undefined;
    /** The Build holo: what the sheet grows out of and shrinks back into. */
    from: HTMLElement | undefined;
    onclose: () => void;
  } = $props();

  const s = $derived(page.summary);
  let holo = $state<HTMLButtonElement>();

  onMount(() => holo?.focus({ preventScroll: true }));

  /** Reveals the sheet from the holo's rectangle; nothing is resized, so the
   *  charts and the picture stand still while it opens. */
  function grow(node: HTMLElement, holoEl: HTMLElement | undefined) {
    if (!holoEl || matchMedia("(prefers-reduced-motion: reduce)").matches) return { duration: 0 };
    const a = holoEl.getBoundingClientRect();
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
  <div class="graphs">
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
  </div>
  <div class="core">
    <ShipHolo {picture} open bind:el={holo} onclick={onclose} />
    <div class="id">
      <span class="meta">
        {#if row?.manufacturerCode}<b class="code">{row.manufacturerCode}</b>{/if}
        {#each [row?.career, row?.role].filter(Boolean) as m (m)}<span>{m}</span>{/each}
        <span class="crew" title={crewTitle(s.crew)}><b>{s.crew.crew} crew</b>{#each crewParts(s.crew) as c (c)}<i>{c}</i>{/each}</span>
      </span>
      <h1>{name}</h1>
    </div>
  </div>
  <section class="prices" aria-label="Where to get it">
    <h3 class="label">Where to get it</h3>
    <ShipPrices prices={shipsPrices.of(page.ship)} />
  </section>
  <div class="figures">
    <ShipFigures {page} />
  </div>
</div>

<style>
  /* Over the build rather than instead of it: the cards keep their folds
   * for when the sheet closes, and its frame hugs what is in it. Under the
   * row's menus. */
  .sheet {
    position: absolute;
    inset: 0 0 auto;
    z-index: 10;
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(400px, 520px) minmax(0, 1fr);
    grid-template-rows: auto 1fr;
    grid-template-areas:
      "graphs core figures"
      "graphs prices figures";
    gap: 16px;
    align-items: start;
    border: 1px solid color-mix(in srgb, var(--hue-sky) 30%, var(--border));
    border-radius: var(--radius-lg);
    background: radial-gradient(
      100% 70% at 50% 20%,
      color-mix(in srgb, var(--hue-sky) 10%, var(--bg-sunk)),
      var(--bg-sunk) 70%
    );
  }
  .graphs,
  .core,
  .figures {
    display: flex;
    flex-direction: column;
    gap: 20px;
    min-width: 0;
  }
  .graphs {
    grid-area: graphs;
    padding: 16px 0 16px 18px;
  }
  .core {
    grid-area: core;
  }
  .prices {
    grid-area: prices;
    padding-bottom: 16px;
  }
  .figures {
    grid-area: figures;
    padding: 16px 18px 16px 0;
  }
  /* Build's narrow layout: the picture beside a 380 px column, the graphs
   * before the shops. */
  @container (width < 1400px) {
    .sheet {
      grid-template-columns: minmax(0, 1fr) 380px;
      grid-template-rows: auto auto 1fr;
      grid-template-areas:
        "core figures"
        "graphs figures"
        "prices figures";
    }
    .graphs {
      padding-block: 0;
    }
    .id,
    .prices {
      padding-left: 18px;
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
  .id {
    display: flex;
    flex-direction: column;
    gap: 4px;
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
