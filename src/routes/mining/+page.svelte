<script lang="ts">
  // Mining — the calculator. Type in the rock the scanner shows and the
  // loadout you fly; the solve (mod-mining) says whether it cracks, how hard
  // it is for our player model, and which gadget, modules or support would
  // close the gap. Reads the cooked game data only: no network.
  import { onDestroy, onMount, untrack } from "svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { Trash2 } from "lucide-svelte";
  import type { LoadoutView } from "$lib/bindings";
  import NoGameData from "$lib/components/catalog/NoGameData.svelte";
  import LoadoutEditor from "$lib/modules/mining/LoadoutEditor.svelte";
  import Report from "$lib/modules/mining/Report.svelte";
  import RockEditor from "$lib/modules/mining/RockEditor.svelte";
  import RockGenerator from "$lib/modules/mining/RockGenerator.svelte";
  import { dataStore, ensureChannel } from "$lib/state/data.svelte";
  import {
    deleteLoadout,
    listenForMiningChanges,
    loadMiningAreas,
    loadMiningCatalog,
    loadMiningState,
    miningStore,
    saveLoadout,
    setLoadout,
    solveRock,
  } from "$lib/state/mining.svelte";

  /** Typing settles before a solve: it runs a few thousand simulated attempts. */
  const SOLVE_DEBOUNCE_MS = 300;

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

  let saveName = $state("");
  let saveError = $state("");
  const saved = $derived(miningStore.state?.saved ?? []);
  const sameLoadout = (a: LoadoutView, b: LoadoutView) => JSON.stringify(a) === JSON.stringify(b);
  const current = $derived(saved.find((s) => sameLoadout(s.loadout, loadout)) ?? null);

  async function save() {
    saveError = (await saveLoadout(saveName, loadout)) ?? "";
    if (!saveError) saveName = "";
  }

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
  {#if ready && !channel}
    <NoGameData />
  {:else if miningStore.catalogError}
    <p class="error">{miningStore.catalogError}</p>
  {:else if catalog && miningStore.state}
    <div class="cols">
      <div class="inputs">
        <section class="card">
          <div class="title">
            <h2 class="label">Loadout</h2>
            {#if saved.length}
              <select
                class="select"
                value={current?.id ?? ""}
                onchange={(e) => {
                  const s = saved.find((x) => x.id === e.currentTarget.value);
                  if (s) void setLoadout(s.loadout);
                }}
                aria-label="Saved loadouts"
              >
                <option value="" disabled>Saved loadouts…</option>
                {#each saved as s (s.id)}<option value={s.id}>{s.name}</option>{/each}
              </select>
              {#if current}
                <button
                  class="btn subtle icon"
                  onclick={() => void deleteLoadout(current.id)}
                  title="Delete “{current.name}”"
                  aria-label="Delete saved loadout"><Trash2 size={14} /></button
                >
              {/if}
            {/if}
          </div>
          <LoadoutEditor {catalog} {loadout} onchange={(next) => void setLoadout(next)} />
          {#if loadout.lasers.length && !current}
            <form
              class="save"
              onsubmit={(e) => {
                e.preventDefault();
                void save();
              }}
            >
              <input class="input" placeholder="Name this loadout" bind:value={saveName} aria-label="Loadout name" />
              <button class="btn" type="submit" disabled={!saveName.trim()}>Save</button>
            </form>
            {#if saveError}<p class="error">{saveError}</p>{/if}
          {/if}
        </section>

        <section class="card">
          <div class="title"><h2 class="label">Rock generator</h2></div>
          {#if channel}<RockGenerator {catalog} {channel} />{/if}
        </section>

        <section class="card">
          <div class="title"><h2 class="label">Scanned rock</h2></div>
          <RockEditor {catalog} />
        </section>
      </div>

      <div class="results">
        {#if miningStore.solveError}
          <p class="error">{miningStore.solveError}</p>
        {:else if miningStore.report}
          <div class:stale={miningStore.solving}>
            <Report report={miningStore.report} {catalog} />
          </div>
        {:else if miningStore.solving}
          <p class="dim">Solving…</p>
        {:else}
          <p class="dim">
            Type in the rock as the scanner shows it — mass and each composition line — and pick your lasers. The
            verdict, the charge bar and what would do better appear here.
          </p>
        {/if}
      </div>
    </div>
  {:else if ready}
    <p class="dim">Loading…</p>
  {/if}
</div>

<style>
  .page {
    height: 100%;
    min-height: 0;
    overflow: auto;
  }
  .cols {
    display: grid;
    grid-template-columns: minmax(300px, 380px) minmax(0, 1fr);
    gap: var(--space-3);
    align-items: start;
  }
  @media (max-width: 860px) {
    .cols {
      grid-template-columns: minmax(0, 1fr);
    }
  }
  .inputs,
  .results,
  .results > div {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
  .title {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin-bottom: var(--space-2);
  }
  .title h2 {
    margin: 0 auto 0 0;
  }
  .icon {
    padding: 4px 6px;
    display: inline-flex;
  }
  .save {
    display: flex;
    gap: 6px;
    margin-top: var(--space-2);
  }
  .save .input {
    flex: 1;
    min-width: 0;
  }
  .stale {
    opacity: 0.6;
    transition: opacity var(--transition-fast);
  }
</style>
