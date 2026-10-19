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
  import Crew from "$lib/modules/mining/Crew.svelte";
  import Loadouts from "$lib/modules/mining/Loadouts.svelte";
  import RockFinder from "$lib/modules/mining/RockFinder.svelte";
  import { dataStore, ensureChannel } from "$lib/state/data.svelte";
  import {
    listenForMiningChanges,
    loadClassTable,
    loadMassBand,
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
    ["crew", "Crew"],
  ];

  let ready = $state(false);
  const channel = $derived(dataStore.channel);
  const catalog = $derived(miningStore.catalog);
  const loadout = $derived<LoadoutView>(miningStore.state?.current ?? { lasers: [], gadget: null, lead: null });

  $effect(() => {
    const c = channel;
    if (c)
      untrack(() => {
        void loadMiningCatalog(c);
        void loadMiningAreas(c);
      });
  });

  // Re-solve when the rock, the tried mass, the loadout or the build changes.
  $effect(() => {
    const c = channel;
    const key = JSON.stringify([miningStore.rock, miningStore.massOverride, loadout, catalog !== null]);
    if (!c || !catalog || !key) return;
    const t = setTimeout(() => void solveRock(c), SOLVE_DEBOUNCE_MS);
    return () => clearTimeout(t);
  });

  // The mass band follows the scanned composition and the loadout (not the
  // tried mass: the band is the axis the slider moves along).
  $effect(() => {
    const c = channel;
    const key = JSON.stringify([miningStore.rock, miningStore.source?.heaviest, loadout, catalog !== null]);
    if (!c || !catalog || !key) return;
    const t = setTimeout(() => void loadMassBand(c), SOLVE_DEBOUNCE_MS);
    return () => clearTimeout(t);
  });

  // The finder's lists show how the loadout fares on every rock type.
  $effect(() => {
    const c = channel;
    const key = JSON.stringify(loadout);
    if (!c || !catalog || miningUi.view !== "find" || !key) return;
    untrack(() => void loadClassTable(c));
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
  <div class="tabs" role="tablist" aria-label="View">
    {#each VIEWS as [id, label] (id)}
      <button
        role="tab"
        class="tab"
        class:active={miningUi.view === id}
        aria-selected={miningUi.view === id}
        onclick={() => (miningUi.view = id)}>{label}</button
      >
    {/each}
    <button role="tab" class="tab later" disabled aria-selected="false">Overlay <span class="soon">later</span></button>
  </div>

  {#if ready && !channel}
    <NoGameData />
  {:else if miningStore.catalogError}
    <p class="error">{miningStore.catalogError}</p>
  {:else if catalog && miningStore.state && channel}
    {#if miningUi.view === "calc"}
      <Calculator {catalog} {channel} {loadout} />
    {:else if miningUi.view === "find"}
      <RockFinder {catalog} {channel} {loadout} />
    {:else if miningUi.view === "crew"}
      <Crew {catalog} {channel} {loadout} />
    {:else}
      <Loadouts {catalog} {channel} {loadout} />
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
  /* The page's gap spaces the tabs; app.css draws them. */
  .tabs {
    margin-bottom: 0;
  }
  .tab.later {
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
