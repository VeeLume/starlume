<script lang="ts">
  // Manufacturer catalog — code + resolved name. Cached in catalog.svelte.ts.
  import { onMount } from "svelte";
  import type { ManufacturerRowView } from "$lib/bindings";
  import { dataStore, ensureChannel } from "$lib/state/data.svelte";
  import { getManufacturers } from "$lib/state/catalog.svelte";

  let manufacturers = $state<ManufacturerRowView[]>([]);
  let browseError = $state("");
  let ready = $state(false);

  onMount(() => {
    void (async () => {
      const channel = await ensureChannel();
      ready = true;
      if (!channel) return;
      const m = await getManufacturers(channel);
      if (typeof m === "string") browseError = m;
      else manufacturers = m;
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

  <table class="data narrow">
    <thead>
      <tr><th>Code</th><th>Name</th></tr>
    </thead>
    <tbody>
      {#each manufacturers as m (m.guid)}
        <tr>
          <td class="mono">{m.code}</td>
          <td>{m.name ?? "—"}</td>
        </tr>
      {/each}
    </tbody>
  </table>
{/if}

<style>
  table.narrow {
    max-width: 560px;
  }
</style>
