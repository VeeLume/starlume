<script lang="ts">
  // Manufacturer catalog on the kit surface — code + resolved name. Flat
  // rows (nothing to expand); search covers both.
  import { onMount } from "svelte";
  import { Surface, Expand, createBrowseState, type Status } from "@veelume/ui";
  import type { ManufacturerRowView } from "$lib/bindings";
  import { dataStore, ensureChannel } from "$lib/state/data.svelte";
  import { getManufacturers } from "$lib/state/catalog.svelte";

  type ManufacturerRow = ManufacturerRowView & { key: string; title: string };

  let manufacturers = $state<ManufacturerRowView[]>([]);
  let browseError = $state("");
  let ready = $state(false);
  let loading = $state(false);

  const browse = createBrowseState({
    q: { kind: "text" },
    sort: { kind: "one", default: "name", narrows: false },
  });

  const descriptor = {
    sources: () => manufacturers,
    derive: (ms: ManufacturerRowView[]): ManufacturerRow[] =>
      ms.map((m) => ({ ...m, key: m.guid, title: m.name ?? m.code })),
    searchIn: (m: ManufacturerRow) => [m.title, m.code],
    sorts: [
      {
        value: "name",
        label: "Name",
        compare: (a: ManufacturerRow, b: ManufacturerRow) => a.title.localeCompare(b.title),
      },
      {
        value: "code",
        label: "Code",
        compare: (a: ManufacturerRow, b: ManufacturerRow) => a.code.localeCompare(b.code),
      },
    ],
  };

  const status = $derived<Status>(
    browseError ? "error" : loading && manufacturers.length === 0 ? "loading" : "ready",
  );

  onMount(() => {
    void (async () => {
      const channel = await ensureChannel();
      ready = true;
      if (!channel) return;
      loading = true;
      const m = await getManufacturers(channel);
      loading = false;
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

  <div class="flex h-full min-h-0 flex-col">
    <Surface.Root {descriptor} {browse} class="min-h-0 flex-1">
      <Surface.List {status}>
        {#snippet row(m: ManufacturerRow)}
          <Expand.Row title={m.title}>
            {#snippet right()}
              <span class="mono dim">{m.code}</span>
            {/snippet}
          </Expand.Row>
        {/snippet}
      </Surface.List>
    </Surface.Root>
  </div>
{/if}
