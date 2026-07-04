<script lang="ts">
  // Resource catalog — every ResourceType with refining edge + density.
  // Small corpus; cached whole in catalog.svelte.ts, filtered client-side.
  import { onMount } from "svelte";
  import type { ResourceRowView } from "$lib/bindings";
  import { dataStore, ensureChannel } from "$lib/state/data.svelte";
  import { getResources, resourcesBrowse } from "$lib/state/catalog.svelte";

  const b = resourcesBrowse;
  let resources = $state<ResourceRowView[]>([]);
  let browseError = $state("");
  let ready = $state(false);

  const filtered = $derived(
    b.query.trim() === ""
      ? resources
      : resources.filter((r) => r.name.toLowerCase().includes(b.query.trim().toLowerCase())),
  );

  onMount(() => {
    void (async () => {
      const channel = await ensureChannel();
      ready = true;
      if (!channel) return;
      const r = await getResources(channel);
      if (typeof r === "string") browseError = r;
      else resources = r;
    })();
  });
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
    <input type="search" placeholder="Filter resources…" bind:value={b.query} />
    <span class="dim count">{filtered.length} / {resources.length} resources</span>
  </div>

  <table class="data">
    <thead>
      <tr><th>Name</th><th>Refines into</th><th>Density (kg/m³)</th><th>Description</th></tr>
    </thead>
    <tbody>
      {#each filtered as r (r.guid)}
        <tr>
          <td>{r.name}</td>
          <td class="dim">{r.refined_into ?? "—"}</td>
          <td>{r.density_kg_per_m3?.toFixed(0) ?? "—"}</td>
          <td class="dim desc-cell">{r.description ?? ""}</td>
        </tr>
      {/each}
    </tbody>
  </table>
{/if}

<style>
  .count {
    font-size: 0.8rem;
  }
  .desc-cell {
    max-width: 40ch;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
