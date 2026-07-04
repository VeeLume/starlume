<script lang="ts">
  // Game Data overview — per-install cache/load state. Statuses are hydrated
  // at app start (root layout); mounting here just refreshes them in the
  // background. Loading a channel is what makes the catalog tabs browsable;
  // the catalogs themselves never trigger a parse.
  import { onMount } from "svelte";
  import { dataStore, loadStatus, startLoad, wipe, setChannel } from "$lib/state/data.svelte";

  const tierNote = (tier: string) =>
    tier === "processed"
      ? "cached — loads in ~1s"
      : tier === "extract"
        ? "snapshot — ~20s re-parse"
        : "full parse — 30s+ on first load";

  async function load(channel: string) {
    await startLoad(channel);
    const status = dataStore.statuses.find((s) => s.channel === channel);
    if (status?.loaded) setChannel(channel);
  }

  onMount(() => void loadStatus());
</script>

{#if dataStore.error}
  <p class="error">{dataStore.error}</p>
{/if}

{#if !dataStore.statusLoaded}
  <p class="dim">Scanning installs…</p>
{:else if dataStore.statuses.length === 0}
  <p class="dim">No Star Citizen installation found on this machine.</p>
{:else}
  <div class="cards">
    {#each dataStore.statuses as status (status.channel)}
      <div class="card install">
        <div class="card-head">
          <span class="channel">
            {status.channel}
            {#if status.is_default}<span
                class="badge accent"
                title="Newest PU build — warmed at startup, preselected by the catalogs"
                >default</span
              >{/if}
          </span>
          <span class="tier" class:warm={status.predicted_tier === "processed"}>
            {tierNote(status.predicted_tier)}
          </span>
        </div>
        <div class="card-meta">
          <span>{status.version}</span>
          <span class="dim">build {status.build_id}</span>
          {#if status.loaded}
            <span class="loaded-note">
              ✓ loaded — {status.item_count} items, {status.resource_count} resources,
              {status.mission_count} missions
            </span>
          {/if}
        </div>
        <div class="card-actions">
          {#if dataStore.loading[status.channel]}
            <span class="progress">{dataStore.loading[status.channel]}</span>
          {:else}
            <button class="primary" onclick={() => void load(status.channel)}>
              {status.loaded ? "Reload" : "Load"}
            </button>
            {#if status.loaded || status.predicted_tier === "processed"}
              <button
                onclick={() => setChannel(status.channel)}
                disabled={dataStore.channel === status.channel}
              >
                {dataStore.channel === status.channel ? "Browsing" : "Browse"}
              </button>
            {/if}
            {#if status.predicted_tier !== "live"}
              <button class="subtle" onclick={() => void wipe(status.channel)}>Wipe cache</button>
            {/if}
          {/if}
        </div>
      </div>
    {/each}
  </div>

  {#if dataStore.statuses.some((s) => s.loaded || s.predicted_tier === "processed")}
    <p class="hint dim">
      Browse the catalogs: <a href="/data/items">Items</a> ·
      <a href="/data/resources">Resources</a> · <a href="/data/missions">Missions</a> ·
      <a href="/data/manufacturers">Manufacturers</a>
    </p>
  {/if}
{/if}

<style>
  .cards {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    margin-bottom: 16px;
  }
  .card.install {
    min-width: 300px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .card-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
  }
  .channel {
    font-weight: 600;
    font-size: 1rem;
  }
  .channel .badge {
    margin-left: 6px;
    vertical-align: 2px;
  }
  .tier {
    font-size: 0.72rem;
    color: var(--text-dim);
  }
  .tier.warm {
    color: var(--accent);
  }
  .card-meta {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 0.8rem;
  }
  .loaded-note {
    color: var(--accent);
    font-size: 0.78rem;
  }
  .card-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 1.9rem;
  }
  .progress {
    font-size: 0.82rem;
    color: var(--accent);
  }
  .hint {
    font-size: 0.85rem;
  }
</style>
