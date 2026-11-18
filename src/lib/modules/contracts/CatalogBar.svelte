<script lang="ts">
  // The catalog's header band (Surface.List's `header` escape hatch), the bar
  // from the Contracts mock instead of the kit's search + filter panel:
  //
  //  - one field: typing offers factions, places, categories and blueprints;
  //    a pick becomes a token in the field (UX board verdict: "one typed
  //    field whose picks become filters"), × or Backspace removes it;
  //  - filter chips with the count each would leave (the surface's counts:
  //    search plus every other facet);
  //  - the sort.
  //
  // Everything stays the surface's URL-backed browse state, so back and
  // forward walk the filters. A candidate for the kit once a second catalog
  // wants it (a pattern on 2+ pages goes to the kit).
  import { getSurfaceContext } from "@veelume/ui";
  import { Search, X } from "lucide-svelte";

  type Browse = {
    values: Record<string, string | string[]>;
    set(key: string, value: string): void;
    setMany(values: Record<string, string>): void;
  };

  let {
    browse,
    blueprints,
    total,
  }: {
    browse: Browse;
    /** Blueprints any mission grants: guid and name, for the field's offers. */
    blueprints: { guid: string; name: string }[];
    /** Missions in the whole catalog, for the count line. */
    total: number;
  } = $props();

  const s = getSurfaceContext();

  /** Facets whose picks live as tokens in the field; the rest are chips. */
  const TOKEN_FACETS: Record<string, string> = { faction: "faction", region: "place", category: "category" };
  const CHIP_FACETS = ["gives"];

  // Mirrors the URL's q (the effect below), so typing never waits on navigation.
  let draft = $state("");
  let open = $state(false);
  let input: HTMLInputElement | undefined = $state();

  const value = (id: string) => String(browse.values[id] ?? "");

  const tokens = $derived([
    ...(value("bp")
      ? [{ id: "bp", label: blueprints.find((b) => b.guid === value("bp"))?.name ?? "blueprint", kind: "grants" }]
      : []),
    ...Object.entries(TOKEN_FACETS)
      .filter(([id]) => value(id))
      .map(([id, kind]) => ({
        id,
        label: s.facets.find((f) => f.id === id)?.options.find((o) => o.value === value(id))?.label ?? value(id),
        kind,
      })),
  ]);

  interface Offer {
    key: string;
    label: string;
    kind: string;
    count: number | null;
    pick: () => void;
  }

  // What the typed word could mean, most missions first; a blueprint scopes
  // the catalog to the missions that grant it.
  const offers = $derived.by((): Offer[] => {
    const q = draft.trim().toLowerCase();
    if (q.length < 2) return [];
    const out: Offer[] = [];
    for (const [id, kind] of Object.entries(TOKEN_FACETS)) {
      const facet = s.facets.find((f) => f.id === id);
      for (const o of facet?.options ?? []) {
        if (!o.value || value(id) === o.value || !o.label.toLowerCase().includes(q)) continue;
        out.push({
          key: `${id}:${o.value}`,
          label: o.label,
          kind,
          count: s.counts[id]?.[o.value] ?? null,
          pick: () => browse.setMany({ [id]: o.value, q: "" }),
        });
      }
    }
    out.sort((a, b) => (b.count ?? 0) - (a.count ?? 0));
    const bps = blueprints
      .filter((b) => b.name.toLowerCase().includes(q) && b.guid !== value("bp"))
      .slice(0, 4)
      .map((b) => ({
        key: `bp:${b.guid}`,
        label: b.name,
        kind: "blueprint",
        count: null,
        pick: () => browse.setMany({ bp: b.guid, q: "" }),
      }));
    return [...out.slice(0, 6), ...bps];
  });

  function pick(o: Offer) {
    o.pick();
    draft = "";
    open = false;
    input?.focus();
  }

  function typed(text: string) {
    draft = text;
    open = true;
    browse.set("q", text);
  }

  function keydown(e: KeyboardEvent) {
    if (e.key === "Enter" && open && offers[0]) {
      e.preventDefault();
      pick(offers[0]);
    } else if (e.key === "Escape") {
      open = false;
    } else if (e.key === "Backspace" && draft === "" && tokens.length) {
      browse.set(tokens[tokens.length - 1].id, "");
    }
  }

  // The URL is the truth: back/forward or a token's × can change q.
  $effect(() => {
    const q = String(browse.values.q ?? "");
    if (q !== draft && document.activeElement !== input) draft = q;
  });

  const chips = $derived(
    CHIP_FACETS.flatMap((id) => {
      const facet = s.facets.find((f) => f.id === id);
      return (facet?.options ?? [])
        .filter((o) => o.value)
        .map((o) => ({
          key: `${id}:${o.value}`,
          label: o.label,
          on: value(id) === o.value,
          count: s.counts[id]?.[o.value] ?? 0,
          toggle: () => browse.set(id, value(id) === o.value ? "" : o.value),
        }));
    }),
  );

  const shown = $derived(s.visible.length);
</script>

<div class="bar">
  <div class="field-wrap">
    <label class="field">
      <Search size={15} aria-hidden="true" />
      {#each tokens as t (t.id)}
        <span class="tok">
          {t.kind === "grants" ? `Grants ${t.label}` : t.label}<small>{t.kind === "grants" ? "blueprint" : t.kind}</small>
          <button type="button" aria-label="Remove {t.label}" onclick={() => browse.set(t.id, "")}><X size={11} /></button>
        </span>
      {/each}
      <input
        bind:this={input}
        type="text"
        value={draft}
        placeholder={tokens.length ? `Search ${shown} missions` : "Search, or type a faction, place, category or blueprint"}
        aria-label="Search the catalog"
        aria-expanded={open && offers.length > 0}
        aria-controls="contracts-offers"
        role="combobox"
        aria-autocomplete="list"
        oninput={(e) => typed(e.currentTarget.value)}
        onkeydown={keydown}
        onfocus={() => (open = true)}
        onblur={() => setTimeout(() => (open = false), 120)}
      />
    </label>
    {#if open && offers.length > 0}
      <ul class="offers" id="contracts-offers" role="listbox">
        {#each offers as o (o.key)}
          <li role="option" aria-selected="false">
            <button type="button" onmousedown={(e) => e.preventDefault()} onclick={() => pick(o)}>
              <span class="lbl">{o.label}</span>
              <span class="kind">{o.kind}</span>
              {#if o.count != null}<span class="n">{o.count}</span>{/if}
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>

  <div class="row2">
    <div class="chips" role="group" aria-label="Filter">
      {#each chips as c (c.key)}
        <button type="button" class="chip" class:on={c.on} aria-pressed={c.on} onclick={c.toggle}>
          {c.label}<span class="n">{c.count}</span>
        </button>
      {/each}
    </div>
    <span class="count">{shown === total ? `${total} missions` : `${shown} of ${total}`}</span>
    <label class="sort">
      <span class="sr-only">Sort</span>
      <select class="select" value={s.activeSort} onchange={(e) => browse.set("sort", e.currentTarget.value)}>
        {#each s.sorts as o (o.value)}<option value={o.value}>{o.label}</option>{/each}
      </select>
    </label>
  </div>
</div>

<style>
  .bar {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px 12px 10px;
    border-bottom: 1px solid var(--border);
  }
  .field-wrap {
    position: relative;
  }
  .field {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
    min-height: 38px;
    padding: 4px 10px;
    box-sizing: border-box;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--bg);
    color: var(--text-faint);
  }
  .field:focus-within {
    border-color: var(--accent-border);
  }
  .field input {
    all: unset;
    flex: 1;
    min-width: 180px;
    font-size: 0.875rem;
    color: var(--text);
  }
  .field input::placeholder {
    color: var(--text-faint);
  }
  .tok {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 24px;
    padding: 0 4px 0 9px;
    border-radius: var(--radius-chip);
    background: var(--accent-fill-weak);
    border: 1px solid var(--accent-border);
    font-size: var(--text-small);
    color: var(--text-strong);
    white-space: nowrap;
  }
  .tok small {
    color: var(--text-faint);
    font-size: 0.7rem;
  }
  .tok button {
    all: unset;
    cursor: pointer;
    display: grid;
    place-items: center;
    width: 16px;
    height: 16px;
    border-radius: 3px;
    color: var(--text-dim);
  }
  .tok button:hover {
    color: var(--text-strong);
  }
  .offers {
    position: absolute;
    z-index: 20;
    top: calc(100% + 4px);
    left: 0;
    min-width: 320px;
    max-width: 520px;
    margin: 0;
    padding: 4px;
    list-style: none;
    background: var(--bg-raised);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: var(--shadow-panel);
  }
  .offers button {
    all: unset;
    box-sizing: border-box;
    cursor: pointer;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto 48px;
    gap: 12px;
    align-items: baseline;
    width: 100%;
    padding: 7px 10px;
    border-radius: 6px;
    font-size: 0.875rem;
    color: var(--text);
  }
  .offers button:hover,
  .offers li:first-child button {
    background: var(--accent-fill-faint);
  }
  .offers .lbl {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .offers .kind {
    font-size: 0.75rem;
    color: var(--text-dim);
  }
  .offers .n {
    text-align: right;
    font-size: 0.75rem;
    color: var(--text-faint);
    font-variant-numeric: tabular-nums;
  }
  .row2 {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px 12px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chip {
    all: unset;
    box-sizing: border-box;
    cursor: pointer;
    display: inline-flex;
    align-items: baseline;
    gap: 6px;
    font-size: var(--text-small);
    line-height: 1;
    padding: 6px 10px;
    border-radius: var(--radius-chip);
    border: 1px solid var(--border);
    color: var(--text-dim);
    white-space: nowrap;
  }
  .chip:hover {
    color: var(--text);
  }
  .chip.on {
    background: var(--accent-fill-weak);
    border-color: var(--accent-border);
    color: var(--text-strong);
    font-weight: 600;
  }
  .chip .n {
    font-size: 0.7rem;
    color: var(--text-faint);
    font-weight: 400;
    font-variant-numeric: tabular-nums;
  }
  .count {
    margin-left: auto;
    font-size: var(--text-small);
    color: var(--text-faint);
    font-variant-numeric: tabular-nums;
  }
  .sort .select {
    font-size: var(--text-small);
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
  }
</style>
