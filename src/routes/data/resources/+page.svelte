<script lang="ts">
  // Resource catalog on the kit surface — every ResourceType with refining
  // edge + density. Small corpus; cached whole in catalog.svelte.ts. Rows
  // expand in place (`many` — a shallow peek) to the full description.
  import { onMount } from "svelte";
  import {
    Surface,
    Expand,
    createBrowseState,
    createExpansion,
    type Status,
  } from "@veelume/ui";
  import type { ResourceRowView } from "$lib/bindings";
  import { dataStore, ensureChannel } from "$lib/state/data.svelte";
  import { getResources } from "$lib/state/catalog.svelte";

  type ResourceRow = ResourceRowView & { key: string; name: string; title: string };

  let resources = $state<ResourceRowView[]>([]);
  let browseError = $state("");
  let ready = $state(false);
  let loading = $state(false);

  const browse = createBrowseState({
    q: { kind: "text" },
    sort: { kind: "one", default: "name", narrows: false },
  });
  const expanded = createExpansion("many");

  const descriptor = {
    sources: () => resources,
    derive: (rs: ResourceRowView[]): ResourceRow[] =>
      rs.map((r) => ({ ...r, key: r.guid, title: r.name })),
    searchIn: (r: ResourceRow) => [r.title, r.refined_into],
    sorts: [
      {
        value: "name",
        label: "Name",
        compare: (a: ResourceRow, b: ResourceRow) => a.title.localeCompare(b.title),
      },
      {
        value: "density",
        label: "Density",
        compare: (a: ResourceRow, b: ResourceRow) =>
          (b.density_kg_per_m3 ?? 0) - (a.density_kg_per_m3 ?? 0),
      },
    ],
  };

  const status = $derived<Status>(
    browseError ? "error" : loading && resources.length === 0 ? "loading" : "ready",
  );

  onMount(() => {
    void (async () => {
      const channel = await ensureChannel();
      ready = true;
      if (!channel) return;
      loading = true;
      const r = await getResources(channel);
      loading = false;
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

  <div class="flex h-full min-h-0 flex-col">
    <Surface.Root {descriptor} {browse} class="min-h-0 flex-1">
      <Surface.List {status}>
        {#snippet row(r: ResourceRow)}
          <Expand.Row
            title={r.title}
            subtitle={r.refined_into ? `refines into ${r.refined_into}` : undefined}
            open={expanded.has(r.key)}
            ontoggle={r.description ? () => expanded.toggle(r.key) : undefined}
          >
            {#snippet right()}
              {#if r.density_kg_per_m3 != null}
                <span class="metric">{r.density_kg_per_m3.toFixed(0)} kg/m³</span>
              {/if}
            {/snippet}
            {#if r.description}
              <p class="desc">{r.description}</p>
            {/if}
          </Expand.Row>
        {/snippet}
      </Surface.List>
    </Surface.Root>
  </div>
{/if}

<style>
  .desc {
    margin: 0;
    max-width: 70ch;
    color: var(--text-dim);
  }
</style>
