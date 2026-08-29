<script lang="ts">
  // Item catalog on the kit surface — the one big corpus, now shipped whole
  // (`data_items_all`) and run through the client pipeline like every other
  // catalog: search + type facet + sorts, viewport windowing, rows expanding
  // in place. Expanding a row lazily fetches its detail (description, record
  // path, and the weapons-index combat stats when the item is a weapon).
  import { onMount } from "svelte";
  import {
    Surface,
    Expand,
    createBrowseState,
    createExpansion,
    type FacetDef,
    type Fact,
    type Status,
  } from "@veelume/ui";
  import type { ItemDetailView, ItemRowView } from "$lib/bindings";
  import { dataStore, ensureChannel, itemDetail } from "$lib/state/data.svelte";
  import { getItemsAll } from "$lib/state/catalog.svelte";

  type ItemRow = ItemRowView & { key: string; title: string };

  let items = $state<ItemRowView[]>([]);
  let browseError = $state("");
  let ready = $state(false);
  let loading = $state(false);
  let channel = $state<string | null>(null);

  const browse = createBrowseState({
    q: { kind: "text" },
    type: { kind: "one", default: "" },
    sort: { kind: "one", default: "name", narrows: false },
  });
  const expanded = createExpansion("many");

  // Lazy per-item detail, fetched on first expand and kept for the session.
  let details = $state<Record<string, ItemDetailView>>({});
  function toggleRow(guid: string) {
    expanded.toggle(guid);
    if (expanded.has(guid) && !details[guid] && channel) {
      void itemDetail(channel, guid).then((r) => {
        if (typeof r === "string") browseError = r;
        else details[guid] = r;
      });
    }
  }

  const typeFacet = $derived.by(() => {
    const counts = new Map<string, number>();
    for (const i of items) counts.set(i.item_type, (counts.get(i.item_type) ?? 0) + 1);
    return [...counts.entries()].sort((x, y) => y[1] - x[1]);
  });

  const facets = $derived<FacetDef<ItemRow>[]>([
    {
      id: "type",
      label: "Type",
      mode: "one",
      options: [
        { value: "", label: "All types" },
        ...typeFacet.map(([t, n]) => ({
          value: t,
          label: `${t} (${n})`,
          test: (r: ItemRow) => r.item_type === t,
        })),
      ],
    },
  ]);

  const descriptor = $derived({
    sources: () => items,
    derive: (rs: ItemRowView[]): ItemRow[] =>
      rs.map((r) => ({ ...r, key: r.guid, title: r.name })),
    searchIn: (r: ItemRow) => [r.title, r.guid],
    facets,
    sorts: [
      {
        value: "name",
        label: "Name",
        compare: (a: ItemRow, b: ItemRow) => a.title.localeCompare(b.title),
      },
      {
        value: "size",
        label: "Size",
        compare: (a: ItemRow, b: ItemRow) => b.size - a.size,
      },
      {
        value: "grade",
        label: "Grade",
        compare: (a: ItemRow, b: ItemRow) => b.grade - a.grade,
      },
    ],
  });

  const status = $derived<Status>(
    browseError ? "error" : loading && items.length === 0 ? "loading" : "ready",
  );

  const fmt = (n: number, digits = 0) =>
    n.toLocaleString("en-US", { maximumFractionDigits: digits });

  function weaponFacts(d: ItemDetailView): Fact[] {
    const facts: Fact[] = [];
    const w = d.ship_weapon;
    if (w) {
      if (w.damage && w.damage.total > 0)
        facts.push({ label: "Alpha", value: fmt(w.damage.total, 1) });
      if (w.penetration_m != null)
        facts.push({ label: "Penetration", value: `${fmt(w.penetration_m, 2)} m` });
      if (w.ammo_speed != null)
        facts.push({ label: "Projectile speed", value: `${fmt(w.ammo_speed)} m/s` });
      if (w.ammo_speed != null && w.ammo_lifetime != null)
        facts.push({ label: "Range", value: `≈${fmt(w.ammo_speed * w.ammo_lifetime)} m` });
      if (w.total_ammo != null) facts.push({ label: "Ammo", value: fmt(w.total_ammo) });
      if (w.capacitor != null) facts.push({ label: "Capacitor", value: fmt(w.capacitor) });
    }
    const m = d.missile;
    if (m) {
      if (m.damage && m.damage.total > 0)
        facts.push({ label: "Damage", value: fmt(m.damage.total, 1) });
      if (m.speed != null) facts.push({ label: "Speed", value: `${fmt(m.speed)} m/s` });
      if (m.arm_time > 0) facts.push({ label: "Arm time", value: `${fmt(m.arm_time, 2)} s` });
      if (m.tracking) {
        facts.push({ label: "Tracking", value: m.tracking.signal });
        facts.push({ label: "Lock time", value: `${fmt(m.tracking.lock_time, 1)} s` });
        facts.push({ label: "Lock angle", value: `${fmt(m.tracking.lock_angle_deg)}°` });
        facts.push({
          label: "Lock range",
          value: `${fmt(m.tracking.lock_range_min_m)}–${fmt(m.tracking.lock_range_max_m)} m`,
        });
      }
    }
    return facts;
  }

  function damageBreakdown(d: ItemDetailView): string | null {
    const dmg = d.ship_weapon?.damage ?? d.missile?.damage;
    if (!dmg) return null;
    const parts: string[] = [];
    const push = (v: number, label: string) => {
      if (v > 0) parts.push(`${fmt(v, 1)} ${label}`);
    };
    push(dmg.physical, "phys");
    push(dmg.energy, "energy");
    push(dmg.distortion, "dist");
    push(dmg.thermal, "therm");
    push(dmg.biochemical, "bio");
    push(dmg.stun, "stun");
    return parts.length > 1 ? parts.join(" · ") : null;
  }

  onMount(() => {
    void (async () => {
      channel = await ensureChannel();
      ready = true;
      if (!channel) return;
      loading = true;
      const r = await getItemsAll(channel);
      loading = false;
      if (typeof r === "string") browseError = r;
      else items = r;
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
        {#snippet row(r: ItemRow)}
          {@const d = details[r.guid]}
          <Expand.Row
            title={r.title}
            subtitle="{r.item_type}{r.item_sub_type && r.item_sub_type !== 'UNDEFINED'
              ? ` / ${r.item_sub_type}`
              : ''}"
            open={expanded.has(r.key)}
            ontoggle={() => toggleRow(r.guid)}
          >
            {#snippet right()}
              {#if r.size > 0}<span class="badge">S{r.size}</span>{/if}
              {#if r.grade > 0}<span class="badge">G{r.grade}</span>{/if}
            {/snippet}

            {#if d}
              {@const combat = weaponFacts(d)}
              {@const breakdown = damageBreakdown(d)}
              {#if combat.length > 0}
                <Expand.Facts facts={combat} />
                {#if breakdown}
                  <p class="dim breakdown">{breakdown}</p>
                {/if}
              {/if}
              {#if d.description}
                <p class="desc">{d.description}</p>
              {/if}
              <p class="mono dim gid">
                {d.guid}{#if d.record_path}
                  · {d.record_path}{/if}
              </p>
            {:else}
              <p class="dim">Loading…</p>
            {/if}
          </Expand.Row>
        {/snippet}
      </Surface.List>
    </Surface.Root>
  </div>
{/if}

<style>
  .desc {
    margin: 6px 0 0;
    max-width: 80ch;
    white-space: pre-wrap;
    color: var(--text-dim);
  }
  .breakdown {
    margin: 2px 0 0;
    font-size: 0.78rem;
  }
  .gid {
    margin: 6px 0 0;
    font-size: 0.72rem;
    word-break: break-all;
  }
</style>
