<script lang="ts">
  // Crafting: the blueprint catalog beside the recipe of the one you picked.
  // The catalog is reference data, cached per channel. An item and its
  // colourways fold into one row; what you own, want and hold are overlaid
  // in `derive`, so the filters can read them.
  import { onDestroy, onMount } from "svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import {
    Button,
    Placeholder,
    Surface,
    createBrowseState,
    type FacetDef,
    type GroupDef,
    type GroupHeader,
    type Status,
  } from "@veelume/ui";
  import type { BlueprintRowView, ManufacturerRowView } from "$lib/bindings";
  import NoGameData from "$lib/components/catalog/NoGameData.svelte";
  import BlueprintCard from "$lib/modules/crafting/BlueprintCard.svelte";
  import RecipeSheet from "$lib/modules/crafting/RecipeSheet.svelte";
  import CategoryRail from "$lib/modules/crafting/CategoryRail.svelte";
  import {
    CLASSES,
    GRADES,
    WEIGHTS,
    canCraft,
    foldFamilies,
    gradeLetter,
    shelfOf,
    shelfRank,
    type Family,
  } from "$lib/modules/crafting/model";
  import { lookOf } from "$lib/modules/crafting/shelf";
  import { blueprintsStore, loadOwnedBlueprints, refreshOwnedBlueprints } from "$lib/state/blueprints.svelte";
  import { getBlueprints, getManufacturers } from "$lib/state/catalog.svelte";
  import { craftingStore, listenForStockChanges, loadMarks, loadStock } from "$lib/state/crafting.svelte";
  import { dataStore, ensureChannel } from "$lib/state/data.svelte";
  import { settingsStore } from "$lib/state/settings.svelte";

  const browse = createBrowseState({
    q: { kind: "text" },
    show: { kind: "one", default: "", narrows: false },
    type: { kind: "one", default: "", narrows: false },
    kind: { kind: "one", default: "", narrows: false },
    weight: { kind: "one", default: "", narrows: false },
    class: { kind: "one", default: "", narrows: false },
    grade: { kind: "one", default: "", narrows: false },
    maker: { kind: "one", default: "" },
    sort: { kind: "one", default: "shelf", narrows: false },
    sel: { kind: "one", default: "", narrows: false },
  });

  let ready = $state(false);
  let loading = $state(false);
  let error = $state("");
  let blueprints = $state<BlueprintRowView[]>([]);
  let manufacturers = $state<ManufacturerRowView[]>([]);
  let sheetOpen = $state(true);
  let sheetWidth = $state<number | null>(430);

  const channel = $derived(dataStore.channel);

  // A channel appears during a cook, and changes with the picker.
  $effect(() => {
    const c = channel;
    if (!c) return;
    void load(c);
  });

  async function load(c: string) {
    loading = blueprints.length === 0;
    const [rows, makers] = await Promise.all([getBlueprints(c), getManufacturers(c), loadStock(c)]);
    loading = false;
    if (typeof rows === "string") error = rows;
    else {
      error = "";
      blueprints = rows;
    }
    if (typeof makers !== "string") manufacturers = makers;
  }

  let unlisten: UnlistenFn | undefined;
  onMount(() => {
    void (async () => {
      unlisten = await listenForStockChanges(() => dataStore.channel);
      await Promise.all([ensureChannel(), loadOwnedBlueprints(), loadMarks()]);
      ready = true;
    })();
  });
  onDestroy(() => unlisten?.());

  const codeByName = $derived(new Map(manufacturers.flatMap((m) => (m.name ? [[m.name, m.code] as const] : []))));

  const families = $derived<Family[]>(
    foldFamilies(blueprints).map((variants) => {
      const rep = variants[0];
      const owned = variants.filter((v) => craftingStore.owns(v.guid)).length;
      return {
        key: rep.family,
        title: rep.familyName,
        shelf: shelfOf(rep),
        maker: rep.manufacturer ? (codeByName.get(rep.manufacturer) ?? rep.manufacturer) : "",
        variants,
        ownedCount: owned,
        wantBlueprint: variants.some((v) => craftingStore.wantsBlueprint(v.guid)),
        wantItem: variants.some((v) => craftingStore.wantsItem(v.guid)),
        craftable: owned > 0 && canCraft(rep.materials, craftingStore.stock),
      };
    }),
  );

  const VIEWS: [string, string, (f: Family) => boolean][] = [
    ["", "All", () => true],
    ["owned", "Owned", (f) => f.ownedCount > 0],
    ["missing", "Missing", (f) => f.ownedCount === 0],
    ["wantbp", "Want blueprint", (f) => f.wantBlueprint],
    ["wantitem", "Want item", (f) => f.wantItem],
    ["craftable", "Craftable", (f) => f.craftable],
  ];
  const viewTest = $derived(VIEWS.find(([id]) => id === browse.values.show)?.[2] ?? VIEWS[0][2]);

  // The category tree: type, then its subtype, weight, class and grade. Each
  // group counts what the others leave, so a number is what a click would show.
  type Field = "type" | "kind" | "weight" | "class" | "grade";
  const FIELD: Record<Field, (f: Family) => string> = {
    type: (f) => f.shelf.top,
    kind: (f) => f.shelf.kind,
    weight: (f) => f.shelf.weight,
    class: (f) => f.variants[0].class ?? "",
    grade: (f) => gradeLetter(f.variants[0]),
  };
  const FIELDS = Object.keys(FIELD) as Field[];
  const passes = (f: Family, except?: Field) =>
    FIELDS.every((k) => k === except || !browse.values[k] || FIELD[k](f) === browse.values[k]);
  /** A row's values in `order`, each with its count. */
  function chips(field: Field, order: string[]): [string, number][] {
    const counts = new Map<string, number>();
    for (const f of families) {
      const v = FIELD[field](f);
      // Types are the way out of a narrowing, so they ignore the rows below.
      if (v && viewTest(f) && (field === "type" || passes(f, field))) counts.set(v, (counts.get(v) ?? 0) + 1);
    }
    return [...counts].sort(([a], [b]) => order.indexOf(a) - order.indexOf(b));
  }
  const orderBy = (key: (f: Family) => number, pick: (f: Family) => string) => [
    ...new Set([...families].sort((a, b) => key(a) - key(b)).map(pick)),
  ];
  const typeChips = $derived(chips("type", orderBy((f) => f.shelf.mainOrder, (f) => f.shelf.top)));
  const kindChips = $derived(
    browse.values.type ? chips("kind", orderBy((f) => f.shelf.mainOrder * 1000 + f.shelf.subOrder, (f) => f.shelf.kind)) : [],
  );
  const weightChips = $derived(browse.values.type ? chips("weight", WEIGHTS) : []);
  const classChips = $derived(browse.values.type ? chips("class", CLASSES) : []);
  const gradeChips = $derived(browse.values.type ? chips("grade", GRADES) : []);
  /** Picking a type starts its filters fresh. */
  const pickType = (t: string) => browse.setMany({ type: t, kind: "", weight: "", class: "", grade: "" });
  const toggle = (field: Field, v: string) => browse.set(field, browse.values[field] === v ? "" : v);

  const shelfOrder = $derived(new Map(families.map((f) => [f.shelf.main, f.shelf.mainOrder])));
  const byShelf = (a: string, b: string) => (shelfOrder.get(a) ?? 0) - (shelfOrder.get(b) ?? 0);
  const distinct = (pick: (f: Family) => string) => [...new Set(families.map(pick).filter(Boolean))];

  const facets = $derived<FacetDef<Family>[]>([
    {
      id: "maker",
      label: "Manufacturer",
      mode: "one",
      options: [
        { value: "", label: "All manufacturers" },
        ...distinct((f) => f.variants[0].manufacturer ?? "")
          .sort()
          .map((v) => ({ value: v, label: v, test: (f: Family) => f.variants[0].manufacturer === v })),
      ],
    },
  ]);

  const subRank = $derived(new Map(families.map((f) => [`${f.shelf.main}/${f.shelf.sub}`, f.shelf.subOrder])));
  const groupBy = $derived<GroupDef<Family>[] | undefined>(
    browse.values.sort === "shelf"
      ? [
          { key: (f) => f.shelf.main, compare: byShelf },
          {
            key: (f) => `${f.shelf.main}/${f.shelf.sub}`,
            label: (k) => k.slice(k.indexOf("/") + 1),
            compare: (a, b) => (subRank.get(a) ?? 0) - (subRank.get(b) ?? 0),
          },
        ]
      : undefined,
  );

  const descriptor = $derived({
    sources: () => families,
    derive: (list: Family[]) => list.filter((f) => viewTest(f) && passes(f)),
    searchIn: (f: Family) => [
      f.title,
      f.shelf.main,
      f.shelf.sub,
      f.variants[0].manufacturer,
      ...f.variants.map((v) => v.name),
      ...f.variants[0].materials.map((m) => m.name),
    ],
    facets,
    sorts: [
      {
        value: "shelf",
        label: "Category",
        compare: (a: Family, b: Family) =>
          shelfRank(a.variants[0]) - shelfRank(b.variants[0]) || a.title.localeCompare(b.title),
      },
      { value: "name", label: "Name", compare: (a: Family, b: Family) => a.title.localeCompare(b.title) },
      {
        value: "time",
        label: "Craft time",
        compare: (a: Family, b: Family) => (a.variants[0].craftSeconds ?? 0) - (b.variants[0].craftSeconds ?? 0),
      },
    ],
    groupBy,
  });

  const status = $derived<Status>(error ? "error" : loading ? "loading" : "ready");
  const selected = $derived(families.find((f) => f.key === browse.values.sel) ?? null);

  const settings = $derived(settingsStore.current);
  const syncable = $derived(
    !!settings?.online_enabled && !!settings?.grpc_enabled && (settings?.grpc_features.includes("blueprints") ?? false),
  );
  const readAgo = $derived(
    blueprintsStore.fetchedAt
      ? new Date(blueprintsStore.fetchedAt * 1000).toLocaleString([], { dateStyle: "short", timeStyle: "short" })
      : null,
  );
  const fmtN = (n: number) => n.toLocaleString("en-US");
  const ownedIn = (list: Family[]) => list.filter((f) => f.ownedCount > 0).length;
</script>

{#snippet section(h: GroupHeader<Family>)}
  {#if h.level === 0}
    {@const look = lookOf(h.label)}
    {@const Icon = look.icon}
    <div class="shelf" style="--hue: {look.hue}">
      <Icon size={15} />
      <h2>{h.label}</h2>
      <span class="count">{ownedIn(h.rows)} of {fmtN(h.rows.length)} owned</span>
    </div>
  {:else if h.label}
    <div class="sub">{h.label}<span class="count">{fmtN(h.rows.length)}</span></div>
  {/if}
{/snippet}

{#if ready && !channel}
  <NoGameData />
{:else}
  <div class="page">
    <header class="pagehead">
      <h1>Crafting</h1>
      <div class="tabs views" role="tablist" aria-label="Show">
        {#each VIEWS as [id, label, test] (id)}
          <button
            type="button"
            role="tab"
            class="tab"
            class:active={browse.values.show === id}
            aria-selected={browse.values.show === id}
            onclick={() => browse.set("show", id)}>{label}<span class="n">{fmtN(families.filter((f) => test(f) && passes(f)).length)}</span></button
          >
        {/each}
      </div>
      <span class="grow"></span>
      {#if syncable}
        <span class="read">{readAgo ? `Owned read ${readAgo}` : "Owned not read yet"}</span>
        <Button variant="outline" disabled={blueprintsStore.refreshing} onclick={() => void refreshOwnedBlueprints()}>
          {blueprintsStore.refreshing ? "Reading…" : "Sync"}
        </Button>
      {:else}
        <a class="read" href="/settings/online">Read what you own from the game</a>
      {/if}
    </header>

    {#if error}<p class="error">{error}</p>{/if}
    {#if craftingStore.error}<p class="error">{craftingStore.error}</p>{/if}
    {#if blueprintsStore.error}<p class="error">{blueprintsStore.error}</p>{/if}

    <div class="work">
      <Surface.Supporting
        title="Recipe"
        open={sheetOpen}
        onopen={(next) => (sheetOpen = next)}
        width={sheetWidth}
        onresize={(w) => (sheetWidth = w)}
      >
        {#snippet main()}
          <div class="browse">
            <CategoryRail
              all={families.filter(viewTest).length}
              tops={typeChips}
              kinds={kindChips}
              weights={weightChips}
              classes={classChips}
              grades={gradeChips}
              values={{
                type: browse.values.type,
                kind: browse.values.kind,
                weight: browse.values.weight,
                class: browse.values.class,
                grade: browse.values.grade,
              }}
              ontype={pickType}
              ontoggle={toggle}
            />
            <div class="list">
              <Surface.Root {descriptor} {browse} selected={browse.values.sel || null} class="min-h-0 flex-1">
                <Surface.List {status} group={section} onselect={(f: Family) => browse.set("sel", f.key)}>
                  {#snippet row(f: Family, isSelected: boolean)}
                    <BlueprintCard family={f} selected={isSelected} onclick={() => browse.set("sel", f.key)} />
                  {/snippet}
                </Surface.List>
              </Surface.Root>
            </div>
          </div>
        {/snippet}
        {#snippet pane()}
          {#if selected && channel}
            <RecipeSheet family={selected} {channel} />
          {:else}
            <Placeholder>Pick a blueprint to see its recipe.</Placeholder>
          {/if}
        {/snippet}
      </Surface.Supporting>
    </div>
  </div>
{/if}

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    height: 100%;
    min-height: 0;
  }
  /* app.css bullets every `main ul`; a short list is the kit's bare <ul>. */
  .page :global(ul) {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .browse {
    display: flex;
    gap: 10px;
    height: 100%;
    min-height: 0;
    padding-right: 10px;
  }
  .list {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .work {
    flex: 1;
    min-height: 0;
  }
  .pagehead {
    display: flex;
    align-items: center;
    gap: 14px;
    flex-wrap: wrap;
  }
  h1 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.6rem;
    font-weight: var(--weight-bold);
    color: var(--text-strong);
  }
  .views {
    align-self: flex-end;
    margin-bottom: 0;
    border-bottom: 0;
  }
  .grow {
    flex: 1;
  }
  .read {
    white-space: nowrap;
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  a.read {
    color: var(--accent);
  }
  .shelf {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 12px 6px 8px;
    padding-bottom: 6px;
    border-bottom: 1px solid color-mix(in srgb, var(--hue) 30%, var(--border));
    color: color-mix(in srgb, var(--hue) 75%, var(--text-strong));
  }
  .shelf h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.12rem;
    font-weight: var(--weight-bold);
    color: var(--text-strong);
  }
  .count {
    margin-left: auto;
    font-family: var(--font-mono);
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .sub {
    display: flex;
    align-items: baseline;
    margin: 8px 6px 6px;
    font-family: var(--font-mono);
    font-size: var(--text-small);
    color: var(--text-dim);
    text-transform: var(--label-transform);
    letter-spacing: var(--label-tracking);
  }
</style>
