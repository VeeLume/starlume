<script lang="ts">
  // The Items view: every owned item with where it is — the place, and what
  // holds it (a ship, a turret, a backpack). Search + place / type / status
  // facets over the client pipeline; rows expand in place.
  import {
    Surface,
    Expand,
    createExpansion,
    type FacetDef,
    type Fact,
    type Status,
    type SurfaceBrowse,
  } from "@veelume/ui";
  import type { OwnedItemView } from "$lib/bindings";
  import {
    OWNERSHIP_LABEL,
    TYPE_GROUPS,
    placeLabel,
    typeGroup,
    whereLabel,
  } from "./labels";

  let {
    items,
    browse,
    status,
  }: { items: OwnedItemView[]; browse: SurfaceBrowse; status: Status } = $props();

  type Row = OwnedItemView & { key: string; title: string; group: string; where: string };

  const expanded = createExpansion("many");

  const rows = $derived<Row[]>(
    items.map((i) => ({
      ...i,
      key: i.geid,
      title: i.kind.name,
      group: typeGroup(i.kind.item_type),
      where: whereLabel(i.place, i.holders),
    })),
  );

  function countBy(key: (r: Row) => string): Map<string, number> {
    const counts = new Map<string, number>();
    for (const r of rows) counts.set(key(r), (counts.get(key(r)) ?? 0) + 1);
    return counts;
  }

  const facets = $derived.by<FacetDef<Row>[]>(() => {
    const places = [...countBy((r) => r.place.name).entries()].sort((a, b) =>
      a[0].localeCompare(b[0]),
    );
    const groups = countBy((r) => r.group);
    return [
      {
        id: "place",
        label: "Place",
        mode: "one",
        options: [
          { value: "", label: "All places" },
          ...places.map(([p, n]) => ({
            value: p,
            label: `${p} (${n})`,
            test: (r: Row) => r.place.name === p,
          })),
        ],
      },
      {
        id: "type",
        label: "Type",
        mode: "one",
        options: [
          { value: "", label: "All types" },
          ...TYPE_GROUPS.filter((g) => groups.has(g)).map((g) => ({
            value: g,
            label: `${g} (${groups.get(g)})`,
            test: (r: Row) => r.group === g,
          })),
        ],
      },
      {
        id: "status",
        label: "Status",
        mode: "one",
        options: [
          { value: "usable", label: "Usable", test: (r: Row) => r.ownership !== "bricked" },
          { value: "", label: "Everything" },
          ...["owned", "rented", "unowned", "bricked"].map((o) => ({
            value: o,
            label: OWNERSHIP_LABEL[o],
            test: (r: Row) => r.ownership === o,
          })),
        ],
      },
    ];
  });

  const descriptor = $derived({
    sources: () => rows,
    derive: (rs: Row[]) => rs,
    searchIn: (r: Row) => [r.title, r.where, r.kind.item_type, r.resource?.name ?? ""],
    facets,
    sorts: [
      {
        value: "name",
        label: "Name",
        compare: (a: Row, b: Row) => a.title.localeCompare(b.title),
      },
      {
        value: "place",
        label: "Place",
        compare: (a: Row, b: Row) => a.where.localeCompare(b.where) || a.title.localeCompare(b.title),
      },
    ],
  });

  function facts(r: Row): Fact[] {
    const out: Fact[] = [
      { label: "Type", value: r.kind.item_type.replace(/_/g, " ") },
      { label: "Where", value: placeLabel(r.place) },
    ];
    if (r.holders.length > 0)
      out.push({
        label: r.fitted ? "Fitted to" : "Stowed in",
        value: r.holders.map((h) => h.name).join(" › "),
      });
    if (r.kind.size > 0) out.push({ label: "Size", value: `S${r.kind.size}` });
    if (r.kind.grade > 0) out.push({ label: "Grade", value: String(r.kind.grade) });
    out.push({ label: "Status", value: OWNERSHIP_LABEL[r.ownership] ?? r.ownership });
    if (r.stack > 1) out.push({ label: "Stack", value: `×${r.stack}` });
    if (r.ammo != null) out.push({ label: "Rounds", value: String(r.ammo) });
    if (r.resource)
      out.push({
        label: "Contents",
        value: `${r.resource.name} · ${r.resource.scu.toLocaleString("en-US", { maximumFractionDigits: 2 })} SCU · quality ${r.resource.quality}`,
      });
    if (r.crafted) out.push({ label: "Crafted", value: "yes" });
    return out;
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
          {#if r.stack > 1}<span class="badge">×{r.stack}</span>{/if}
          {#if r.kind.size > 0}<span class="badge">S{r.kind.size}</span>{/if}
          {#if r.ownership === "rented" || r.ownership === "bricked"}
            <span class="badge">{OWNERSHIP_LABEL[r.ownership]}</span>
          {/if}
        {/snippet}
        <Expand.Facts facts={facts(r)} />
      </Expand.Row>
    {/snippet}
  </Surface.List>
</Surface.Root>
