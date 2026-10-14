<script lang="ts">
  // My Items — the finder over the player's items and ships. The data is
  // the cached inventory (svc-dossier) joined with the game data
  // (mod-items); "Refresh from game" is the one network call, gated.
  import { onDestroy, onMount } from "svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { Button, Segmented, createBrowseState, type Status } from "@veelume/ui";
  import NoGameData from "$lib/components/catalog/NoGameData.svelte";
  import ItemsList from "$lib/components/items/ItemsList.svelte";
  import ShipsList from "$lib/components/items/ShipsList.svelte";
  import { dataStore, ensureChannel } from "$lib/state/data.svelte";
  import {
    itemsStore,
    listenForItemsChanges,
    loadFinder,
    loadItems,
    refreshItems,
  } from "$lib/state/items.svelte";

  const browse = createBrowseState({
    q: { kind: "text" },
    view: { kind: "one", default: "items", narrows: false },
    place: { kind: "one", default: "" },
    type: { kind: "one", default: "" },
    status: { kind: "one", default: "usable" },
    ship: { kind: "one", default: "active" },
    sort: { kind: "one", default: "name", narrows: false },
  });

  let channel = $state<string | null>(null);
  let ready = $state(false);

  const finder = $derived(itemsStore.finder);
  const fetched = $derived(itemsStore.summary?.fetched_at ?? null);
  const fetchedLabel = $derived(fetched ? new Date(fetched * 1000).toLocaleString() : null);
  const status = $derived<Status>(
    itemsStore.finderError ? "error" : !finder && itemsStore.loading ? "loading" : "ready",
  );

  let unlisten: UnlistenFn | undefined;
  onMount(() => {
    void (async () => {
      unlisten = await listenForItemsChanges(() => channel);
      channel = await ensureChannel();
      ready = true;
      await loadItems();
      if (channel && itemsStore.summary?.fetched_at) await loadFinder(channel);
    })();
  });
  onDestroy(() => unlisten?.());
</script>

<div class="page">
  <div class="head">
    <Segmented
      options={[
        { value: "items", label: `Items${finder ? ` (${finder.items.length})` : ""}` },
        { value: "ships", label: `Ships${finder ? ` (${finder.ships.length})` : ""}` },
      ]}
      value={browse.values.view}
      onchange={(v) => browse.set("view", v)}
    />
    <span class="muted text-sm updated">
      {fetchedLabel ? `Updated ${fetchedLabel}` : "Not fetched yet"}
    </span>
    <Button variant="outline" onclick={refreshItems} disabled={itemsStore.refreshing}>
      {itemsStore.refreshing ? "Fetching…" : "Refresh from game"}
    </Button>
  </div>

  {#if itemsStore.error}
    <p class="text-sm text-destructive">{itemsStore.error}</p>
  {/if}
  {#if itemsStore.finderError}
    <p class="text-sm text-destructive">{itemsStore.finderError}</p>
  {/if}
  {#if itemsStore.summary && itemsStore.summary.failed.length > 0}
    <p class="text-sm muted">
      {itemsStore.summary.failed.length} storage location(s) couldn't be read this time; their items
      are missing below.
    </p>
  {/if}

  {#if ready && !dataStore.channel}
    <NoGameData />
  {:else if !fetched}
    <p class="muted">
      Press “Refresh from game” to read your items and ships. It needs “Items &amp; ships” on in
      <a href="/settings/online">Settings → Online</a>.
    </p>
  {:else if finder}
    {#if browse.values.view === "ships"}
      <ShipsList ships={finder.ships} {browse} {status} />
    {:else}
      <ItemsList items={finder.items} {browse} {status} />
    {/if}
  {:else if status === "loading"}
    <p class="muted">Loading…</p>
  {/if}
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    height: 100%;
    min-height: 0;
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    flex-wrap: wrap;
  }
  .updated {
    margin-left: auto;
  }
</style>
