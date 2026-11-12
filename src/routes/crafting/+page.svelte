<script lang="ts">
  // Crafting: the blueprint catalog, scoped by one field (decision 'My Items
  // opens on the few you care about, and one field is its only scope line',
  // 2026-11-11; UX board sketches 3, 4, 7, 8). The field holds a type, a kind
  // and a weight or class as words, then free text. Armour lists one row per
  // set (its colourways open in the sheet); everything else is one row per
  // item, its colourways folded in (ship parts too: a table of their own was
  // too much of a style break, Valerie 2026-11-12). The recipe opens on demand beside
  // the list and closes with ✕ or Esc. What you own, want and hold are
  // overlaid on the cached catalog.
  import { onDestroy, onMount, type Component } from "svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { Button, Placeholder, Surface, createBrowseState, type GroupDef, type GroupHeader, type Status } from "@veelume/ui";
  import type { BlueprintRowView, ManufacturerRowView } from "$lib/bindings";
  import NoGameData from "$lib/components/catalog/NoGameData.svelte";
  import WordField, { type FieldGroup, type FieldRow, type FieldWord } from "$lib/components/WordField.svelte";
  import FamilyRow from "$lib/modules/crafting/FamilyRow.svelte";
  import RecipeSheet from "$lib/modules/crafting/RecipeSheet.svelte";
  import SetRow from "$lib/modules/crafting/SetRow.svelte";
  import SetSheet from "$lib/modules/crafting/SetSheet.svelte";
  import { foldSets, type CraftSet } from "$lib/modules/crafting/browse";
  import { CLASSES, WEIGHTS, canCraft, foldFamilies, shelfOf, shelfRank, type Family } from "$lib/modules/crafting/model";
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
    sel: { kind: "one", default: "", narrows: false },
  });
  const v = $derived(browse.values);

  let ready = $state(false);
  let loading = $state(false);
  let error = $state("");
  let blueprints = $state<BlueprintRowView[]>([]);
  let manufacturers = $state<ManufacturerRowView[]>([]);
  // The recipe opens on a pick, closes with ✕ or Esc (frontend rule 7:
  // page-local, the pick itself is in the URL).
  let sheetOpen = $state(!!browse.values.sel);
  let sheetWidth = $state<number | null>(440);

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
      const owned = variants.filter((x) => craftingStore.owns(x.guid)).length;
      return {
        key: rep.family,
        title: rep.familyName,
        shelf: shelfOf(rep),
        maker: rep.manufacturer ? (codeByName.get(rep.manufacturer) ?? rep.manufacturer) : "",
        variants,
        ownedCount: owned,
        wantBlueprint: variants.some((x) => craftingStore.wantsBlueprint(x.guid)),
        wantItem: variants.some((x) => craftingStore.wantsItem(x.guid)),
        craftable: owned > 0 && canCraft(rep.materials, craftingStore.stock),
      };
    }),
  );

  // ── What shows ────────────────────────────────────────────────────────────
  const VIEWS: [string, string, (f: Family) => boolean][] = [
    ["", "All", () => true],
    ["owned", "Owned", (f) => f.ownedCount > 0],
    ["missing", "Missing", (f) => f.ownedCount === 0],
    ["wantbp", "Want blueprint", (f) => f.wantBlueprint],
    ["wantitem", "Want item", (f) => f.wantItem],
    ["craftable", "Craftable", (f) => f.craftable],
  ];
  const viewTest = $derived(VIEWS.find(([id]) => id === v.show)?.[2] ?? VIEWS[0][2]);

  /** The field's words, each optional. */
  const inWords = (f: Family) =>
    (!v.type || f.shelf.top === v.type) &&
    (!v.kind || f.shelf.kind === v.kind) &&
    (!v.weight || f.shelf.weight === v.weight) &&
    (!v.class || (f.variants[0].class ?? "") === v.class);
  const q = $derived(v.q.trim().toLowerCase());
  const textOf = (f: Family) =>
    [f.title, f.shelf.main, f.shelf.sub, f.variants[0].manufacturer ?? "", ...f.variants.map((x) => x.name), ...f.variants[0].materials.map((m) => m.name)]
      .join("\n")
      .toLowerCase();

  // The list's entries: armour folds into sets, the rest one row per item.
  type Entry =
    | { key: string; title: string; type: "set"; set: CraftSet; main: string; sub: string; mainOrder: number; subOrder: number }
    | { key: string; title: string; type: "family"; family: Family; main: string; sub: string; mainOrder: number; subOrder: number };
  const sets = $derived(foldSets(families.filter((f) => f.shelf.top === "Armour")));
  const entries = $derived<Entry[]>([
    ...sets.map((s) => ({
      key: s.key,
      title: s.name,
      type: "set" as const,
      set: s,
      main: "Armour",
      sub: s.weight === "Flight & other" ? "Flight and other" : `${s.weight} armour`,
      mainOrder: 20,
      subOrder: WEIGHTS.indexOf(s.weight),
    })),
    ...families
      .filter((f) => f.shelf.top !== "Armour")
      .map((f) => ({
        key: f.key,
        title: f.title,
        type: "family" as const,
        family: f,
        main: f.shelf.main,
        sub: f.shelf.sub,
        mainOrder: f.shelf.mainOrder,
        subOrder: f.shelf.subOrder,
      })),
  ]);
  /** A set shows when any of its pieces does. */
  const familiesOf = (e: Entry) => (e.type === "set" ? e.set.families : [e.family]);
  const entryPasses = (e: Entry) =>
    familiesOf(e).some((f) => viewTest(f) && inWords(f)) && (e.type === "set" ? !v.weight || e.set.weight === v.weight : true);

  const groupBy = $derived<GroupDef<Entry>[]>([
    { key: (e) => e.main, compare: (a, b) => order(a) - order(b) },
    {
      key: (e) => `${e.main}/${e.sub}`,
      label: (k) => k.slice(k.indexOf("/") + 1),
      compare: (a, b) => (subRank.get(a) ?? 0) - (subRank.get(b) ?? 0),
    },
  ]);
  const mainRank = $derived(new Map(entries.map((e) => [e.main, e.mainOrder])));
  const order = (main: string) => mainRank.get(main) ?? 0;
  const subRank = $derived(new Map(entries.map((e) => [`${e.main}/${e.sub}`, e.subOrder])));

  const descriptor = $derived({
    sources: () => entries,
    derive: (list: Entry[]) => list.filter(entryPasses),
    searchIn: (e: Entry) =>
      e.type === "set" ? [e.set.name, ...e.set.families.flatMap((f) => [f.title, ...f.variants.map((x) => x.name)]), ...e.set.materials] : [textOf(e.family)],
    facets: [],
    sorts: [
      {
        value: "shelf",
        label: "Category",
        compare: (a: Entry, b: Entry) =>
          (a.type === "family" && b.type === "family" ? shelfRank(a.family.variants[0]) - shelfRank(b.family.variants[0]) : 0) ||
          a.title.localeCompare(b.title),
      },
    ],
    groupBy,
  });

  const status = $derived<Status>(error ? "error" : loading ? "loading" : "ready");
  const selSet = $derived(sets.find((s) => s.key === v.sel) ?? null);
  const selFamily = $derived(selSet ? null : (families.find((f) => f.key === v.sel) ?? null));

  function pick(key: string) {
    browse.set("sel", key);
    sheetOpen = true;
  }
  function onkeydown(e: KeyboardEvent) {
    if (e.key !== "Escape" || !sheetOpen) return;
    if ((e.target as HTMLElement | null)?.closest("input, textarea, select, [role=menu]")) return;
    sheetOpen = false;
  }

  // ── The field ─────────────────────────────────────────────────────────────
  /** Families the tab leaves, with every word but `except` applied: what a
   *  word's options count. */
  const countWith = (test: (f: Family) => boolean) => families.filter((f) => viewTest(f) && test(f)).length;
  const types = $derived(
    [...new Map(families.map((f) => [f.shelf.top, f.shelf.mainOrder])).entries()].sort((a, b) => a[1] - b[1]).map(([t]) => t),
  );
  const kindsOf = (type: string) => [
    ...new Set(
      families
        .filter((f) => f.shelf.top === type && f.shelf.kind)
        .sort((a, b) => a.shelf.mainOrder * 1000 + a.shelf.subOrder - (b.shelf.mainOrder * 1000 + b.shelf.subOrder))
        .map((f) => f.shelf.kind),
    ),
  ];
  const facetOf = $derived(v.type === "Armour" ? "weight" : v.type === "Ship parts" ? "class" : "");
  const facetValues = (facet: string) => (facet === "weight" ? WEIGHTS : CLASSES);
  const facetTest = (facet: string, val: string) => (f: Family) =>
    facet === "weight" ? f.shelf.weight === val : (f.variants[0].class ?? "") === val;

  const setWords = (patch: Record<string, string>) => browse.setMany({ q: "", sel: "", ...patch });
  const typeRow = (t: string): FieldRow => ({
    key: t,
    label: t,
    n: countWith((f) => f.shelf.top === t),
    icon: lookOf(t).icon,
    cur: t === v.type,
    pick: () => setWords({ type: t, kind: "", weight: "", class: "" }),
  });
  const kindRow = (type: string, k: string): FieldRow => ({
    key: `${type}/${k}`,
    label: k,
    note: v.type ? "" : type,
    n: countWith((f) => f.shelf.top === type && f.shelf.kind === k),
    icon: lookOf(k).icon,
    cur: k === v.kind,
    pick: () => setWords({ type, kind: k, weight: "", class: "" }),
  });
  const facetRow = (facet: string, val: string): FieldRow => {
    const n = countWith((f) => (!v.type || f.shelf.top === v.type) && (!v.kind || f.shelf.kind === v.kind) && facetTest(facet, val)(f));
    return { key: `${facet}/${val}`, label: val, n, off: !n, cur: v[facet as "weight" | "class"] === val, pick: () => setWords({ [facet]: val }) };
  };

  const words = $derived.by<FieldWord[]>(() => {
    const out: FieldWord[] = [
      {
        key: "type",
        label: v.type || "All blueprints",
        menu: [{ rows: [{ key: "", label: "All blueprints", n: countWith(() => true), cur: !v.type, pick: () => setWords({ type: "", kind: "", weight: "", class: "" }) }, ...types.map(typeRow)] }],
        remove: v.type ? () => setWords({ type: "", kind: "", weight: "", class: "" }) : undefined,
      },
    ];
    if (v.type && v.kind) {
      out.push({
        key: "kind",
        label: v.kind,
        menu: [{ rows: kindsOf(v.type).map((k) => kindRow(v.type, k)) }],
        remove: () => setWords({ kind: "", class: "" }),
      });
    }
    for (const facet of ["weight", "class"] as const) {
      if (!v[facet]) continue;
      out.push({ key: facet, label: v[facet], menu: [{ rows: facetValues(facet).map((x) => facetRow(facet, x)) }], remove: () => setWords({ [facet]: "" }) });
    }
    return out;
  });

  const suggestions = $derived.by<FieldGroup[]>(() => {
    if (!q) {
      if (!v.type) return [{ title: "Kinds", rows: types.map(typeRow) }];
      const g: FieldGroup[] = [];
      if (!v.kind) g.push({ title: `${v.type} kinds`, rows: kindsOf(v.type).map((k) => kindRow(v.type, k)) });
      if (facetOf && !v[facetOf as "weight" | "class"]) {
        g.push({ title: facetOf === "weight" ? "Weights" : "Classes", rows: facetValues(facetOf).map((x) => facetRow(facetOf, x)).filter((r) => !r.off) });
      }
      return g;
    }
    const kinds: FieldRow[] = [
      ...types.filter((t) => t.toLowerCase().includes(q)).map(typeRow),
      ...types.flatMap((t) => kindsOf(t).filter((k) => k.toLowerCase().includes(q) && k !== t).map((k) => kindRow(t, k))),
      ...(facetOf ? facetValues(facetOf).filter((x) => x.toLowerCase().includes(q)).map((x) => facetRow(facetOf, x)).filter((r) => !r.off) : []),
    ];
    const hits = entries
      .filter((e) => entryPasses(e) && e.title.toLowerCase().includes(q))
      .slice(0, 8)
      .map((e) => ({
        key: e.key,
        label: e.title,
        note: e.type === "set" ? e.sub : e.family.shelf.main,
        n: e.type === "set" ? e.set.blueprints : e.family.variants.length,
        icon: lookOf(e.main).icon,
        pick: () => (browse.setMany({ sel: e.key, q: "" }), (sheetOpen = true)),
      }));
    return [
      { title: "Kinds", rows: kinds.slice(0, 5) },
      { title: "Blueprints", rows: hits },
    ];
  });

  /** Drop the last word: a weight or class, the kind, then the type. */
  function onback() {
    if (v.class) setWords({ class: "" });
    else if (v.weight) setWords({ weight: "" });
    else if (v.kind) setWords({ kind: "" });
    else if (v.type) setWords({ type: "" });
  }

  // ── Header ────────────────────────────────────────────────────────────────
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
  const ownedIn = (list: Entry[]) => list.filter((e) => familiesOf(e).some((f) => f.ownedCount > 0)).length;
  const iconOf = (x: unknown) => x as Component<{ size?: number }>;
</script>

<svelte:window {onkeydown} />

{#snippet noHeader()}{/snippet}

{#snippet section(h: GroupHeader<Entry>)}
  {#if h.level === 0}
    {@const look = lookOf(h.label)}
    {@const Icon = iconOf(look.icon)}
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
            class:active={v.show === id}
            aria-selected={v.show === id}
            onclick={() => browse.set("show", id)}>{label}<span class="n">{fmtN(families.filter((f) => test(f) && inWords(f)).length)}</span></button
          >
        {/each}
      </div>
      <span class="grow"></span>
      {#if syncable}
        <span class="read" title={readAgo ? `What you own was read from the game ${readAgo}` : ""}
          >{readAgo ? `Read ${readAgo.split(",")[0]}` : "Not read yet"}</span
        >
        <Button variant="outline" disabled={blueprintsStore.refreshing} onclick={() => void refreshOwnedBlueprints()}>
          {blueprintsStore.refreshing ? "Reading…" : "Sync"}
        </Button>
      {:else}
        <a class="read" href="/settings/online">Read what you own from the game</a>
      {/if}
    </header>

    <div class="fieldrow">
      <WordField {words} q={v.q} placeholder="A blueprint, a kind, a material…" {suggestions} onq={(text) => browse.set("q", text)} {onback} />
    </div>

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
            <Surface.Root {descriptor} {browse} selected={v.sel || null} class="min-h-0 flex-1">
              <Surface.List {status} header={noHeader} group={section} onselect={(e: Entry) => pick(e.key)}>
                {#snippet row(e: Entry, isSelected: boolean)}
                  {#if e.type === "set"}
                    <SetRow set={e.set} selected={isSelected} onclick={() => pick(e.key)} />
                  {:else}
                    <FamilyRow family={e.family} selected={isSelected} onclick={() => pick(e.key)} />
                  {/if}
                {/snippet}
              </Surface.List>
            </Surface.Root>
        {/snippet}
        {#snippet pane()}
          {#if selSet && channel}
            <SetSheet set={selSet} {channel} />
          {:else if selFamily && channel}
            <RecipeSheet family={selFamily} {channel} />
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
  .fieldrow {
    display: flex;
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
