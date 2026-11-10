<script lang="ts">
  // Crafting: the blueprint catalog beside the recipe of the one you picked.
  // The catalog is reference data, cached per channel; what you own, want and
  // hold are overlaid in `derive`, so the filters can read them.
  import { onDestroy, onMount } from "svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { Check, Flag, Heart } from "lucide-svelte";
  import {
    Button,
    Expand,
    Placeholder,
    Segmented,
    Surface,
    createBrowseState,
    type FacetDef,
    type GroupDef,
    type Status,
  } from "@veelume/ui";
  import type { BlueprintRowView, ManufacturerRowView } from "$lib/bindings";
  import NoGameData from "$lib/components/catalog/NoGameData.svelte";
  import RecipeSheet from "$lib/modules/crafting/RecipeSheet.svelte";
  import { canCraft, classText, fmtTime, shelfOf, type Shelf } from "$lib/modules/crafting/model";
  import { blueprintsStore, loadOwnedBlueprints, refreshOwnedBlueprints } from "$lib/state/blueprints.svelte";
  import { getBlueprints, getManufacturers } from "$lib/state/catalog.svelte";
  import { craftingStore, listenForStockChanges, loadMarks, loadStock, toggleMark } from "$lib/state/crafting.svelte";
  import { dataStore, ensureChannel } from "$lib/state/data.svelte";
  import { settingsStore } from "$lib/state/settings.svelte";

  type Row = BlueprintRowView & {
    key: string;
    title: string;
    shelf: Shelf;
    maker: string;
    owned: boolean;
    inGame: boolean;
    wantBlueprint: boolean;
    wantItem: boolean;
    craftable: boolean;
  };

  const browse = createBrowseState({
    q: { kind: "text" },
    show: { kind: "one", default: "", narrows: false },
    shelf: { kind: "one", default: "" },
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
  let sheetWidth = $state<number | null>(null);

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

  const rows = $derived<Row[]>(
    blueprints.map((b) => ({
      ...b,
      key: b.guid,
      title: b.name,
      shelf: shelfOf(b),
      maker: b.manufacturer ? (codeByName.get(b.manufacturer) ?? b.manufacturer) : "",
      owned: craftingStore.owns(b.guid),
      inGame: craftingStore.ownedInGame(b.guid),
      wantBlueprint: craftingStore.wantsBlueprint(b.guid),
      wantItem: craftingStore.wantsItem(b.guid),
      craftable: craftingStore.owns(b.guid) && canCraft(b.materials, craftingStore.stock),
    })),
  );

  const SHOW: Record<string, (r: Row) => boolean> = {
    "": () => true,
    owned: (r) => r.owned,
    missing: (r) => !r.owned,
    wanted: (r) => r.wantBlueprint || r.wantItem,
    craftable: (r) => r.craftable,
  };
  const showOptions = $derived(
    (
      [
        ["", "All"],
        ["owned", "Owned"],
        ["missing", "Missing"],
        ["wanted", "Wanted"],
        ["craftable", "Craftable"],
      ] as const
    ).map(([value, label]) => ({
      value,
      // A no-break space: the count stays on the label's line.
      label: `${label} ${rows.filter(SHOW[value]).length.toLocaleString("en-US")}`,
    })),
  );

  /** Distinct values, in `compare` order. */
  function options(pick: (r: Row) => string, compare?: (a: string, b: string) => number) {
    return [...new Set(rows.map(pick).filter(Boolean))].sort(compare);
  }

  const shelfOrder = $derived(new Map(rows.map((r) => [r.shelf.main, r.shelf.mainOrder])));
  const byShelf = (a: string, b: string) => (shelfOrder.get(a) ?? 0) - (shelfOrder.get(b) ?? 0);

  const facets = $derived<FacetDef<Row>[]>([
    {
      id: "shelf",
      label: "Category",
      mode: "one",
      options: [
        { value: "", label: "All categories" },
        ...options((r) => r.shelf.main, byShelf).map((v) => ({
          value: v,
          label: v,
          test: (r: Row) => r.shelf.main === v,
        })),
      ],
    },
    {
      id: "maker",
      label: "Manufacturer",
      mode: "one",
      options: [
        { value: "", label: "All manufacturers" },
        ...options((r) => r.manufacturer ?? "").map((v) => ({
          value: v,
          label: v,
          test: (r: Row) => r.manufacturer === v,
        })),
      ],
    },
  ]);

  const owned = (list: Row[]) => list.filter((r) => r.owned).length;
  const shelved = $derived(browse.values.sort === "shelf");
  const groupBy = $derived<GroupDef<Row>[] | undefined>(
    shelved
      ? [
          {
            key: (r) => r.shelf.main,
            label: (k, list) => `${k} · ${owned(list)} owned of`,
            compare: byShelf,
          },
          {
            key: (r) => `${r.shelf.main}/${r.shelf.sub}`,
            label: (k, list) => `${k.slice(k.indexOf("/") + 1) || "All"} · ${owned(list)} owned of`,
            compare: (a, b) => a.localeCompare(b, "en", { numeric: true }),
          },
        ]
      : undefined,
  );

  const descriptor = $derived({
    sources: () => rows,
    derive: (list: Row[]) => list.filter(SHOW[browse.values.show] ?? SHOW[""]),
    searchIn: (r: Row) => [r.title, r.shelf.main, r.shelf.sub, r.manufacturer, ...r.materials.map((m) => m.name)],
    facets,
    sorts: [
      { value: "shelf", label: "Category", compare: (a: Row, b: Row) => a.title.localeCompare(b.title) },
      { value: "name", label: "Name", compare: (a: Row, b: Row) => a.title.localeCompare(b.title) },
      {
        value: "time",
        label: "Craft time",
        compare: (a: Row, b: Row) => (a.craftSeconds ?? 0) - (b.craftSeconds ?? 0),
      },
    ],
    groupBy,
  });

  const status = $derived<Status>(error ? "error" : loading ? "loading" : "ready");
  const selected = $derived(rows.find((r) => r.key === browse.values.sel) ?? null);

  const settings = $derived(settingsStore.current);
  const syncable = $derived(
    !!settings?.online_enabled && !!settings?.grpc_enabled && (settings?.grpc_features.includes("blueprints") ?? false),
  );
  const readAgo = $derived(
    blueprintsStore.fetchedAt
      ? new Date(blueprintsStore.fetchedAt * 1000).toLocaleString([], { dateStyle: "short", timeStyle: "short" })
      : null,
  );
</script>

{#if ready && !channel}
  <NoGameData />
{:else}
  <div class="page">
    {#if error}<p class="error">{error}</p>{/if}
    {#if craftingStore.error}<p class="error">{craftingStore.error}</p>{/if}
    {#if blueprintsStore.error}<p class="error">{blueprintsStore.error}</p>{/if}

    <Surface.Supporting
      title="Recipe"
      open={sheetOpen}
      onopen={(next) => (sheetOpen = next)}
      width={sheetWidth}
      onresize={(w) => (sheetWidth = w)}
      class="min-h-0 flex-1"
    >
      {#snippet main()}
        <Surface.Root {descriptor} {browse} selected={browse.values.sel || null} class="min-h-0 flex-1">
          <Surface.Toolbar>
            {#snippet leading()}
              <Segmented options={showOptions} value={browse.values.show} onchange={(v) => browse.set("show", v)} />
            {/snippet}
            {#snippet actions()}
              {#if syncable}
                <Button
                  variant="outline"
                  title={readAgo ? `Owned blueprints last read ${readAgo}` : "Read your owned blueprints from the game"}
                  disabled={blueprintsStore.refreshing}
                  onclick={() => void refreshOwnedBlueprints()}
                >
                  {blueprintsStore.refreshing ? "Reading…" : "Sync"}
                </Button>
              {:else}
                <a class="read" href="/settings/online" title="Read your owned blueprints from the game">Turn on sync</a>
              {/if}
            {/snippet}
          </Surface.Toolbar>

          <Surface.List {status} onselect={(r: Row) => browse.set("sel", r.key)}>
            {#snippet row(r: Row, isSelected: boolean)}
              <Expand.Row title={r.title} selected={isSelected} onselect={() => browse.set("sel", r.key)}>
                {#snippet gutter()}
                  <button
                    type="button"
                    class="tog"
                    class:on={r.owned}
                    disabled={r.inGame}
                    aria-pressed={r.owned}
                    aria-label={r.inGame ? "Owned in the game" : r.owned ? "Mark not owned" : "Mark owned"}
                    title={r.inGame ? "Owned in the game" : r.owned ? "Mark not owned" : "Mark owned"}
                    onclick={() => void toggleMark("owned", r.guid)}><Check size={13} /></button
                  >
                {/snippet}
                {#snippet meta()}
                  <span class="mats">{r.materials.map((m) => m.name).join(", ")}</span>
                {/snippet}
                {#snippet right()}
                  <span class="facts">
                    <span class="mfr" title={r.manufacturer}>{r.maker}</span>
                    <span>{r.shelf.sub.startsWith("Size") ? `S${r.size}` : ""}</span>
                    <span class="cls">{classText(r)}</span>
                    <span class="time">{fmtTime(r.craftSeconds)}</span>
                  </span>
                {/snippet}
                {#snippet actions()}
                  <button
                    type="button"
                    class="tog want"
                    class:on={r.wantBlueprint}
                    aria-pressed={r.wantBlueprint}
                    aria-label="Want this blueprint"
                    title="Want this blueprint"
                    onclick={() => void toggleMark("wantBlueprint", r.guid)}><Flag size={13} /></button
                  >
                  <button
                    type="button"
                    class="tog want"
                    class:on={r.wantItem}
                    aria-pressed={r.wantItem}
                    aria-label="Want the item"
                    title="Want the item"
                    onclick={() => void toggleMark("wantItem", r.guid)}><Heart size={13} /></button
                  >
                {/snippet}
              </Expand.Row>
            {/snippet}
          </Surface.List>
        </Surface.Root>
      {/snippet}
      {#snippet pane()}
        {#if selected && channel}
          <RecipeSheet row={selected} {channel} />
        {:else}
          <Placeholder>Pick a blueprint to see its recipe.</Placeholder>
        {/if}
      {/snippet}
    </Surface.Supporting>
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
  .read {
    font-size: var(--text-small);
    color: var(--text-dim);
    white-space: nowrap;
  }
  .tog {
    all: unset;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    box-sizing: border-box;
    border: 1px solid var(--border);
    border-radius: var(--radius-chip);
    color: var(--text-faint);
    cursor: pointer;
  }
  .tog:hover:not(:disabled) {
    border-color: var(--accent);
    color: var(--text);
  }
  .tog:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .tog.on {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--on-accent);
  }
  .tog:disabled {
    cursor: default;
  }
  .tog.want {
    border-color: transparent;
  }
  .tog.want.on {
    background: var(--accent-fill);
    border-color: var(--accent-border);
    color: var(--accent);
  }
  .mats {
    font-size: var(--text-small);
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .facts {
    display: grid;
    grid-template-columns: 56px 24px 92px 44px;
    gap: 8px;
    align-items: baseline;
  }
  .mfr {
    font-family: var(--font-mono);
    font-weight: var(--weight-medium);
    color: var(--hue-sand);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .cls {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .time {
    font-family: var(--font-mono);
    text-align: right;
  }
</style>
