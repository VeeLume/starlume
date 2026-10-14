<script lang="ts">
  // The Ships view: every owned vehicle — spawned or not — with where it is,
  // and, expanded, its parts as fitted now beside its saved and stock
  // loadouts.
  import {
    Surface,
    Expand,
    createExpansion,
    type FacetDef,
    type Status,
    type SurfaceBrowse,
  } from "@veelume/ui";
  import type { LoadoutSlotView, ShipView } from "$lib/bindings";
  import { OWNERSHIP_LABEL, SOURCE_LABEL, placeLabel, portLabel } from "./labels";

  let {
    ships,
    browse,
    status,
  }: { ships: ShipView[]; browse: SurfaceBrowse; status: Status } = $props();

  type Row = ShipView & { title: string; where: string; state: string };

  const expanded = createExpansion("many");

  /** Where a ship is, in words. */
  function where(s: ShipView): string {
    if (s.place) {
      const inside = s.holders.length > 0 ? ` · in ${s.holders.map((h) => h.name).join(" › ")}` : "";
      return `${placeLabel(s.place)}${inside}`;
    }
    // Spawned but not in any stash the walk reaches (inside another ship's
    // interior, or out in the world) vs never spawned.
    return s.geid ? "Spawned — location not readable" : "Not spawned";
  }

  function state(s: ShipView): string {
    if (s.ownership === "bricked") return "bricked";
    if (s.place) return "stored";
    return s.geid ? "spawned" : "unspawned";
  }

  const rows = $derived<Row[]>(
    ships.map((s) => ({ ...s, title: s.kind.name, where: where(s), state: state(s) })),
  );

  const facets = $derived<FacetDef<Row>[]>([
    {
      id: "ship",
      label: "Status",
      mode: "one",
      options: [
        { value: "active", label: "Usable", test: (r: Row) => r.state !== "bricked" },
        { value: "", label: "Everything" },
        { value: "stored", label: "In a stash", test: (r: Row) => r.state === "stored" },
        { value: "unspawned", label: "Not spawned", test: (r: Row) => r.state === "unspawned" },
        { value: "bricked", label: "Bricked", test: (r: Row) => r.state === "bricked" },
      ],
    },
  ]);

  const descriptor = $derived({
    sources: () => rows,
    derive: (rs: Row[]) => rs,
    searchIn: (r: Row) => [r.title, r.where],
    facets,
    sorts: [
      {
        value: "name",
        label: "Name",
        compare: (a: Row, b: Row) => a.title.localeCompare(b.title),
      },
    ],
  });

  /** A loadout's entries as "port: item" lines. */
  function slotLines(slots: LoadoutSlotView[]): { port: string; item: string }[] {
    return slots.map((s) => ({
      port: s.path.map(portLabel).join(" › "),
      item: s.item?.name ?? "Empty",
    }));
  }

  function sameLoadout(a: LoadoutSlotView[], b: LoadoutSlotView[]): boolean {
    if (a.length !== b.length) return false;
    return a.every(
      (s, i) => s.path.join("/") === b[i].path.join("/") && s.item?.guid === b[i].item?.guid,
    );
  }
</script>

<Surface.Root {descriptor} {browse} class="min-h-0 flex-1">
  <Surface.List {status}>
    {#snippet row(r: Row)}
      <Expand.Row
        title={r.title}
        subtitle={r.where}
        open={expanded.has(r.key)}
        ontoggle={() => expanded.toggle(r.key)}
      >
        {#snippet right()}
          <span class="badge">{SOURCE_LABEL[r.source] ?? r.source}</span>
          {#if r.ownership === "rented" || r.ownership === "bricked"}
            <span class="badge">
              {OWNERSHIP_LABEL[r.ownership]}{#if r.rental_expiry}
                · until {new Date(r.rental_expiry * 1000).toLocaleDateString()}{/if}
            </span>
          {/if}
        {/snippet}

        <div class="ship-detail">
          <section>
            <h4>Fitted now</h4>
            {#if r.fitted.length === 0}
              <p class="dim">
                {r.place ? "Nothing swappable fitted." : "Not readable until it is in a stash."}
              </p>
            {:else}
              <ul class="parts">
                {#each r.fitted as p (p.geid)}
                  <li style="padding-left: {p.depth * 1.25}rem">
                    {p.kind.name}
                    {#if p.port}<span class="dim"> · {portLabel(p.port)}</span>{/if}
                  </li>
                {/each}
              </ul>
            {/if}
          </section>

          {#if r.saved || r.stock}
            <section>
              <h4>Saved loadout</h4>
              {#if !r.saved}
                <p class="dim">None saved.</p>
              {:else if r.stock && sameLoadout(r.saved, r.stock)}
                <p class="dim">Same as stock.</p>
              {:else}
                <ul class="parts">
                  {#each slotLines(r.saved) as l, i (i)}
                    <li>{l.item}<span class="dim"> · {l.port}</span></li>
                  {/each}
                </ul>
              {/if}
            </section>
            <section>
              <h4>Stock loadout</h4>
              {#if r.stock}
                <ul class="parts">
                  {#each slotLines(r.stock) as l, i (i)}
                    <li>{l.item}<span class="dim"> · {l.port}</span></li>
                  {/each}
                </ul>
              {:else}
                <p class="dim">Not known.</p>
              {/if}
            </section>
          {:else}
            <section>
              <h4>Loadouts</h4>
              <p class="dim">Not available for this ship.</p>
            </section>
          {/if}
        </div>
      </Expand.Row>
    {/snippet}
  </Surface.List>
</Surface.Root>

<style>
  .ship-detail {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(16rem, 1fr));
    gap: var(--space-4);
    margin-top: var(--space-2);
  }
  h4 {
    margin: 0 0 var(--space-1);
    font-size: var(--text-sm);
    color: var(--text-dim);
  }
  .parts {
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: var(--text-sm);
  }
</style>
