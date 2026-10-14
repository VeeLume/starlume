<script lang="ts">
  // My Items — the explorer over the player's items, ships and resources.
  // The data is the cached inventory (svc-dossier) joined with the game data
  // (mod-items); "Refresh from game" is the one network call, gated.
  import { onDestroy, onMount } from "svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import NoGameData from "$lib/components/catalog/NoGameData.svelte";
  import { createItemsBrowse } from "$lib/components/items/explorer/browse";
  import Explorer from "$lib/components/items/explorer/Explorer.svelte";
  import PageNav from "$lib/components/items/explorer/PageNav.svelte";
  import type { View } from "$lib/components/items/explorer/ui";
  import { units } from "$lib/components/items/model/tree";
  import { dataStore, ensureChannel } from "$lib/state/data.svelte";
  import { itemsStore, listenForItemsChanges, loadFinder, loadItems, refreshItems } from "$lib/state/items.svelte";

  const browse = createItemsBrowse();

  let channel = $state<string | null>(null);
  let ready = $state(false);

  const tree = $derived(itemsStore.tree);
  const finder = $derived(itemsStore.finder);
  const fetched = $derived(itemsStore.summary?.fetched_at ?? null);
  const updated = $derived(fetched ? new Date(fetched * 1000).toLocaleString() : null);
  const view = $derived<View>(browse.values.view === "ships" || browse.values.view === "res" ? browse.values.view : "items");
  const counts = $derived(
    tree && finder
      ? {
          items: units(tree.root, (x) => x.item.ownership !== "bricked" && !x.item.resource),
          ships: finder.ships.filter((s) => s.ownership !== "bricked").length,
          res: new Set(finder.items.flatMap((i) => (i.resource ? [i.resource.name] : []))).size,
        }
      : null,
  );

  function setView(next: View) {
    if (next !== view) browse.setMany({ view: next, sel: "" });
  }

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
  <PageNav {view} {counts} {updated} refreshing={itemsStore.refreshing} onview={setView} onrefresh={refreshItems} />

  {#if itemsStore.error}<p class="text-sm text-destructive">{itemsStore.error}</p>{/if}
  {#if itemsStore.finderError}<p class="text-sm text-destructive">{itemsStore.finderError}</p>{/if}
  {#if itemsStore.summary && itemsStore.summary.failed.length > 0}
    <p class="text-sm muted">
      {itemsStore.summary.failed.length} storage location(s) couldn't be read this time; their items are missing below.
    </p>
  {/if}

  {#if ready && !dataStore.channel}
    <NoGameData />
  {:else if !fetched}
    <p class="muted">
      Press “Refresh from game” to read your items and ships. It needs “Items &amp; ships” on in
      <a href="/settings/online">Settings → Online</a>.
    </p>
  {:else if tree}
    <Explorer {tree} {browse} />
  {:else if itemsStore.loading}
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
</style>
