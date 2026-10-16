<script lang="ts">
  // Mining, in views: the Calculator (one scanned rock against your ship:
  // whether it cracks, how to fly it, what would help), the Rock finder
  // (where rocks spawn and how the ship fares there, from the game's spawn
  // rules) and your and your crew's Loadouts. The overlay is planned. Reads
  // the cooked game data only: no network.
  import { onDestroy, onMount, untrack } from "svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import type { LoadoutView } from "$lib/bindings";
  import NoGameData from "$lib/components/catalog/NoGameData.svelte";
  import Calculator from "$lib/modules/mining/Calculator.svelte";
  import Loadouts from "$lib/modules/mining/Loadouts.svelte";
  import RockFinder from "$lib/modules/mining/RockFinder.svelte";
  import { dataStore, ensureChannel } from "$lib/state/data.svelte";
  import {
    listenForMiningChanges,
    loadMiningAreas,
    loadMiningCatalog,
    loadMiningState,
    miningStore,
    miningUi,
    solveRock,
    type MiningView,
  } from "$lib/state/mining.svelte";

  /** Typing settles before a solve: it runs a few thousand simulated attempts. */
  const SOLVE_DEBOUNCE_MS = 300;

  const VIEWS: [MiningView, string][] = [
    ["calc", "Calculator"],
    ["find", "Rock finder"],
    ["lo", "Loadouts"],
  ];

  let ready = $state(false);
  const channel = $derived(dataStore.channel);
  const catalog = $derived(miningStore.catalog);
  const loadout = $derived<LoadoutView>(miningStore.state?.current ?? { lasers: [], gadget: null });

  $effect(() => {
    const c = channel;
    if (c)
      untrack(() => {
        void loadMiningCatalog(c);
        void loadMiningAreas(c);
      });
  });

  // Re-solve when the rock, the loadout or the build changes.
  $effect(() => {
    const c = channel;
    const key = JSON.stringify([miningStore.rock, loadout, catalog !== null]);
    if (!c || !catalog || !key) return;
    const t = setTimeout(() => void solveRock(c), SOLVE_DEBOUNCE_MS);
    return () => clearTimeout(t);
  });

  let unlisten: UnlistenFn | undefined;
  onMount(() => {
    void (async () => {
      unlisten = await listenForMiningChanges(() => dataStore.channel);
      await ensureChannel();
      ready = true;
      await loadMiningState();
    })();
  });
  onDestroy(() => unlisten?.());
</script>

<div class="page">
  <header class="head">
    <div class="tabs" role="tablist" aria-label="View">
      {#each VIEWS as [id, label] (id)}
        <button role="tab" class:on={miningUi.view === id} aria-selected={miningUi.view === id} onclick={() => (miningUi.view = id)}
          >{label}</button
        >
      {/each}
      <button role="tab" class="later" disabled aria-selected="false">Overlay <span class="soon">later</span></button>
    </div>
  </header>

  {#if ready && !channel}
    <NoGameData />
  {:else if miningStore.catalogError}
    <p class="error">{miningStore.catalogError}</p>
  {:else if catalog && miningStore.state && channel}
    {#if miningUi.view === "calc"}
      <Calculator {catalog} {loadout} />
    {:else if miningUi.view === "find"}
      <RockFinder {catalog} {channel} {loadout} />
    {:else}
      <Loadouts {catalog} {loadout} />
    {/if}
  {:else if ready}
    <NoGameData waiting />
  {/if}
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: 16px;
    height: 100%;
    min-height: 0;
    overflow: auto;
  }
  .head {
    display: flex;
    align-items: flex-end;
    border-bottom: 1px solid var(--border);
  }
  .tabs {
    display: flex;
    gap: 2px;
  }
  .tabs button {
    padding: 8px 12px 10px;
    border: 0;
    border-bottom: 2px solid transparent;
    background: transparent;
    font: inherit;
    font-size: 0.9rem;
    color: var(--text-dim);
    cursor: pointer;
  }
  .tabs button:hover:not(:disabled) {
    color: var(--text);
  }
  .tabs button.on {
    color: var(--text-strong);
    border-bottom-color: var(--accent);
    font-weight: var(--weight-semibold);
  }
  .tabs button.later {
    opacity: 0.55;
    cursor: default;
  }
  .soon {
    font-family: var(--font-mono);
    font-size: 0.62rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-faint);
  }
</style>
