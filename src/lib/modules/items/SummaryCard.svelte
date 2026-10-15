<script lang="ts">
  // My Items on Home: what the cached inventory holds and how old it is.
  // The cached read is local and ungated; refreshing stays on the page
  // (the one gated network call).
  import { onMount } from "svelte";
  import { itemsStore, loadItems } from "$lib/state/items.svelte";

  const summary = $derived(itemsStore.summary);
  const updated = $derived(
    summary?.fetched_at
      ? new Date(summary.fetched_at * 1000).toLocaleString([], { dateStyle: "short", timeStyle: "short" })
      : null,
  );

  onMount(() => void loadItems());
</script>

{#if summary?.fetched_at}
  <div class="stat-tiles">
    <div class="stat-tile"><b>{summary.items.toLocaleString()}</b><span>items</span></div>
    <div class="stat-tile"><b>{summary.stashes.toLocaleString()}</b><span>storage spots</span></div>
    <div class="stat-tile"><b>{summary.entitlements.toLocaleString()}</b><span>entitlements</span></div>
    {#if summary.loadouts != null}
      <div class="stat-tile"><b>{summary.loadouts.toLocaleString()}</b><span>loadouts</span></div>
    {/if}
  </div>
  <p class="dim small">Read from the game {updated}.</p>
{:else if summary}
  <p class="dim">
    Not read yet. Turn on “Items &amp; ships” in <a href="/settings/online">Settings → Online</a>,
    then refresh on <a href="/items">My Items</a>.
  </p>
{/if}

<style>
  p {
    margin: 0 0 4px;
  }
  .small {
    margin: 8px 0 0;
    font-size: var(--text-small);
  }
</style>
