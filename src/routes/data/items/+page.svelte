<script lang="ts">
  // Item catalog — paginated search over the loaded channel's inventory
  // items (Rust-side pagination; the one big corpus). Browse state lives in
  // catalog.svelte.ts so navigating away and back restores this page as
  // left, without refetching (docs/frontend.md).
  import { onMount, onDestroy } from "svelte";
  import type { ItemTypeFacetView } from "$lib/bindings";
  import { dataStore, ensureChannel, searchItems, itemDetail } from "$lib/state/data.svelte";
  import { getItemTypes, itemsBrowse, syncItemsChannel } from "$lib/state/catalog.svelte";

  const PAGE_SIZE = 50;

  const b = itemsBrowse;
  let types = $state<ItemTypeFacetView[]>([]);
  let browseError = $state("");
  let ready = $state(false);

  let searchTimer: ReturnType<typeof setTimeout> | undefined;

  const totalPages = $derived(b.results ? Math.max(1, Math.ceil(b.results.total / PAGE_SIZE)) : 1);

  async function runSearch() {
    if (!b.channel) return;
    browseError = "";
    const r = await searchItems(b.channel, b.query, b.itemType, b.page * PAGE_SIZE, PAGE_SIZE);
    if (typeof r === "string") browseError = r;
    else b.results = r;
  }

  function debouncedSearch() {
    b.page = 0;
    clearTimeout(searchTimer);
    searchTimer = setTimeout(() => void runSearch(), 200);
  }

  async function openDetail(guid: string) {
    if (!b.channel) return;
    const r = await itemDetail(b.channel, guid);
    if (typeof r === "string") browseError = r;
    else b.detail = r;
  }

  onMount(() => {
    void (async () => {
      const channel = await ensureChannel();
      ready = true;
      if (!channel) return;
      syncItemsChannel(channel);
      const t = await getItemTypes(channel);
      if (typeof t === "string") browseError = t;
      else types = t;
      // Cached results render instantly; fetch only when there are none.
      if (!b.results) await runSearch();
    })();
  });

  onDestroy(() => clearTimeout(searchTimer));
</script>

{#if ready && !dataStore.channel}
  <p class="dim">
    No game data loaded — go to <a href="/data">Overview</a> and press Load first.
  </p>
{:else}
  {#if browseError}
    <p class="error">{browseError}</p>
  {/if}

  <div class="toolbar">
    <input
      type="search"
      placeholder="Search items by name or GUID…"
      bind:value={b.query}
      oninput={debouncedSearch}
    />
    <select
      bind:value={b.itemType}
      onchange={() => {
        b.page = 0;
        void runSearch();
      }}
    >
      <option value={null}>All types</option>
      {#each types as t (t.item_type)}
        <option value={t.item_type}>{t.item_type} ({t.count})</option>
      {/each}
    </select>
    {#if b.results}
      <span class="dim count">{b.results.total} items</span>
    {/if}
  </div>

  <div class="split">
    <div class="list">
      {#if b.results && b.results.rows.length > 0}
        <table class="data clickable">
          <thead>
            <tr><th>Name</th><th>Type</th><th>SubType</th><th>Size</th><th>Grade</th></tr>
          </thead>
          <tbody>
            {#each b.results.rows as row (row.guid)}
              <tr
                class:selected={b.detail?.guid === row.guid}
                onclick={() => void openDetail(row.guid)}
              >
                <td>{row.name}</td>
                <td class="dim">{row.item_type}</td>
                <td class="dim">{row.item_sub_type}</td>
                <td>{row.size || "—"}</td>
                <td>{row.grade || "—"}</td>
              </tr>
            {/each}
          </tbody>
        </table>
        {#if totalPages > 1}
          <div class="pager">
            <button
              disabled={b.page === 0}
              onclick={() => {
                b.page -= 1;
                void runSearch();
              }}>‹</button
            >
            <span class="dim">{b.page + 1} / {totalPages}</span>
            <button
              disabled={b.page + 1 >= totalPages}
              onclick={() => {
                b.page += 1;
                void runSearch();
              }}>›</button
            >
          </div>
        {/if}
      {:else if b.results}
        <p class="dim">No items match.</p>
      {:else}
        <p class="dim">Searching…</p>
      {/if}
    </div>

    {#if b.detail}
      <aside class="detail-panel">
        <div class="detail-head">
          <h3>{b.detail.name}</h3>
          <button class="subtle" onclick={() => (b.detail = null)} aria-label="Close">✕</button>
        </div>
        <dl>
          {#if b.detail.short_name}<dt>Short name</dt><dd>{b.detail.short_name}</dd>{/if}
          <dt>Type</dt><dd>{b.detail.item_type} / {b.detail.item_sub_type}</dd>
          {#if b.detail.size}<dt>Size</dt><dd>{b.detail.size}</dd>{/if}
          {#if b.detail.grade}<dt>Grade</dt><dd>{b.detail.grade}</dd>{/if}
          {#if b.detail.description}<dt>Description</dt><dd class="desc">{b.detail.description}</dd>{/if}
          {#if b.detail.record_path}<dt>Record</dt><dd class="mono">{b.detail.record_path}</dd>{/if}
          <dt>GUID</dt><dd class="mono">{b.detail.guid}</dd>
        </dl>
      </aside>
    {/if}
  </div>
{/if}

<style>
  .count {
    font-size: 0.8rem;
  }
  .desc {
    grid-column: 1 / -1;
    white-space: pre-wrap;
    color: var(--text);
    font-size: 0.8rem;
    max-height: 200px;
    overflow: auto;
  }
</style>
