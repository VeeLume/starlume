<script lang="ts">
  // The Ships list: every ship of the build, as spviewer and erkul open on
  // their whole list. One filter row, every control the same dropdown:
  // where you can get it, maker, landing pad, crew, and the order (all
  // URL-backed, frontend rule 7). CIG's careers and roles stay on the cards
  // and in the search, not as a filter: they are sometimes senseless.
  // Untouched, yours, the new ones and the recently opened lead, then every
  // hull; any search or filter shows only the matches. Your ships come from
  // the inventory My Items last read (no fetch here); "new" counts from the
  // first build Starlume saw; prices are UEX's and the pledge store's.
  import { Select, createBrowseState, type SelectOption } from "@veelume/ui";
  import { Search } from "lucide-svelte";
  import type { ShipRowView, ShipsHomeView } from "$lib/bindings";
  import { shipsMarket } from "$lib/state/ships.svelte";
  import ShipTile from "./ShipTile.svelte";
  import UexLink from "./UexLink.svelte";

  let {
    ships,
    home,
    onopen,
  }: {
    /** Every ship of the build, editions included. */
    ships: ShipRowView[];
    home: ShipsHomeView | null;
    onopen: (guid: string) => void;
  } = $props();

  const browse = createBrowseState({
    q: { kind: "text" },
    get: { kind: "one", default: "" },
    maker: { kind: "one", default: "" },
    pad: { kind: "one", default: "" },
    crew: { kind: "one", default: "" },
    sort: { kind: "one", default: "maker", narrows: false },
  });

  const byGuid = $derived(new Map(ships.map((s) => [s.guid, s])));
  const bases = $derived(ships.filter((s) => !s.base && !s.ground));
  const owned = $derived((home?.owned ?? []).map((o) => ({ ...o, row: byGuid.get(o.ship) })).filter((o) => o.row));
  const ownedSet = $derived(new Set(owned.map((o) => o.ship)));
  const arrivals = $derived((home?.arrivals ?? []).map((a) => ({ ...a, row: byGuid.get(a.ship) })).filter((a) => a.row));
  const recent = $derived((home?.recent ?? []).map((g) => byGuid.get(g)).filter((r): r is ShipRowView => !!r));

  /** Where you can get a ship. "store" is the pledge store and nowhere in
   *  game; "none" is neither shop nor rental in game (rewards, loot, store
   *  ships never sold in game). */
  const GET: [string, string, (s: ShipRowView) => boolean][] = [
    ["owned", "Owned", (s) => ownedSet.has(s.guid)],
    ["buy", "Buy in game", (s) => (shipsMarket.of(s.guid)?.buyShops ?? 0) > 0],
    ["rent", "Rent in game", (s) => (shipsMarket.of(s.guid)?.rentShops ?? 0) > 0],
    ["store", "Store only", (s) => inGame(s) === false && shipsMarket.of(s.guid)?.storeUsd != null],
    ["none", "Not sold in game", (s) => inGame(s) === false],
  ];
  const inGame = (s: ShipRowView) => {
    const m = shipsMarket.of(s.guid);
    return m ? m.buyShops + m.rentShops > 0 : null;
  };
  const PADS = ["XS", "S", "M", "L", "XL"];
  const CREW: [string, string, (n: number) => boolean][] = [
    ["1", "Solo", (n) => n <= 1],
    ["2-4", "2 to 4 crew", (n) => n >= 2 && n <= 4],
    ["5+", "5 crew or more", (n) => n >= 5],
  ];
  /** The order: by name or maker A to Z, by price cheapest first (no price
   *  last), by cargo and crew most first. */
  const SORTS: [string, string][] = [
    ["maker", "By maker"],
    ["name", "Name"],
    ["buy", "In-game price"],
    ["store", "Store price"],
    ["cargo", "Cargo"],
    ["crew", "Crew"],
  ];

  const makers = $derived([...new Set(bases.map((s) => s.manufacturer ?? "Other"))].sort((a, b) => a.localeCompare(b)));
  const opts = (all: string, rows: [string, string][]): SelectOption[] => [{ value: "", label: all }, ...rows.map(([value, label]) => ({ value, label }))];
  const getOptions = $derived(opts("Anywhere", GET.map(([v, l]) => [v, l])));
  const makerOptions = $derived(opts("Every maker", makers.map((m) => [m, m])));
  const padOptions = opts("Any pad", PADS.map((p) => [p, `${p} pad`]));
  const crewOptions = opts("Any crew", CREW.map(([v, l]) => [v, l]));
  const sortOptions = SORTS.map(([value, label]) => ({ value, label }));

  const q = $derived(browse.values.q.trim().toLowerCase());
  const get = $derived(GET.find(([v]) => v === browse.values.get));
  const maker = $derived(browse.values.maker);
  const pad = $derived(browse.values.pad);
  const crew = $derived(CREW.find(([v]) => v === browse.values.crew));
  const sort = $derived(browse.values.sort);
  const narrowed = $derived(!!q || !!get || !!maker || !!pad || !!crew);
  const list = $derived.by(() => {
    const rows = bases.filter(
      (s) =>
        (!get || get[2](s)) &&
        (!maker || (s.manufacturer ?? "Other") === maker) &&
        (!pad || shipsMarket.of(s.guid)?.pad === pad) &&
        (!crew || crew[2](s.seats.crew)) &&
        (!q || `${s.name} ${s.role ?? ""} ${s.career ?? ""} ${s.manufacturer ?? ""}`.toLowerCase().includes(q)),
    );
    const byName = (a: ShipRowView, b: ShipRowView) => a.name.localeCompare(b.name);
    /** Cheapest first; a ship without the price goes last. */
    const price = (f: (s: ShipRowView) => number | null | undefined) => (a: ShipRowView, b: ShipRowView) => {
      const x = f(a) ?? Infinity;
      const y = f(b) ?? Infinity;
      return x === y ? byName(a, b) : x - y;
    };
    if (sort === "cargo") rows.sort((a, b) => (b.cargo ?? 0) - (a.cargo ?? 0) || byName(a, b));
    else if (sort === "crew") rows.sort((a, b) => b.seats.crew - a.seats.crew || byName(a, b));
    else if (sort === "buy") rows.sort(price((s) => shipsMarket.of(s.guid)?.buy?.price));
    else if (sort === "store") rows.sort(price((s) => shipsMarket.of(s.guid)?.storeUsd));
    else if (sort === "maker") rows.sort((a, b) => (a.manufacturer ?? "").localeCompare(b.manufacturer ?? "") || byName(a, b));
    else rows.sort(byName);
    return rows;
  });
  /** By maker: one group per maker, in the list's order. */
  const groups = $derived.by(() => {
    if (sort !== "maker") return [{ maker: "", rows: list }];
    const out: { maker: string; rows: ShipRowView[] }[] = [];
    for (const s of list) {
      const m = s.manufacturer ?? "Other";
      const last = out[out.length - 1];
      if (last?.maker === m) last.rows.push(s);
      else out.push({ maker: m, rows: [s] });
    }
    return out;
  });

  const age = (t: number) => {
    const days = Math.floor((Date.now() / 1000 - t) / 86400);
    return days <= 0 ? "today" : days === 1 ? "yesterday" : `${days} days ago`;
  };
</script>

<div class="home">
  <div class="tools">
    <label class="find">
      <Search size={15} />
      <input
        value={browse.values.q}
        oninput={(e) => browse.set("q", e.currentTarget.value)}
        placeholder="Find a ship by name, role or maker"
        aria-label="Find a ship"
      />
    </label>
    <Select label="Where to get it" options={getOptions} value={browse.values.get} onchange={(v) => browse.set("get", v)} />
    <Select label="Maker" options={makerOptions} value={maker} onchange={(v) => browse.set("maker", v)} />
    <Select label="Landing pad" options={padOptions} value={pad} onchange={(v) => browse.set("pad", v)} />
    <Select label="Crew" options={crewOptions} value={browse.values.crew} onchange={(v) => browse.set("crew", v)} />
    <span class="grow"></span>
    <Select label="Order" options={sortOptions} value={sort} onchange={(v) => browse.set("sort", v)} />
    <small class="source">In-game prices from <UexLink />, reported by players</small>
  </div>

  {#if !narrowed}
    {#if owned.length}
      <section>
        <h2>Your ships{#if home?.ownedAt}<small>read {age(home.ownedAt)}</small>{/if}</h2>
        <div class="grid">
          {#each owned as o (o.ship)}
            <ShipTile ship={o.row!} note={o.rented ? "rented" : o.count > 1 ? `×${o.count}` : ""} tone={o.rented ? "rent" : ""} {onopen} />
          {/each}
        </div>
      </section>
    {/if}
    {#if arrivals.length}
      <section>
        <h2>New ships</h2>
        <div class="grid">
          {#each arrivals as a (a.ship)}<ShipTile ship={a.row!} note="new in {a.since}" tone="new" {onopen} />{/each}
        </div>
      </section>
    {/if}
    {#if recent.length}
      <section>
        <h2>Recently opened</h2>
        <div class="grid">
          {#each recent as r (r.guid)}<ShipTile ship={r} {onopen} />{/each}
        </div>
      </section>
    {/if}
  {/if}

  <section>
    <h2>
      {narrowed ? `${list.length} ${list.length === 1 ? "ship" : "ships"}` : "All ships"}
      {#if !narrowed && home && home.owned === null}<small>yours show above once My Items has read your hangar</small>{/if}
    </h2>
    {#each groups as g (g.maker)}
      {#if g.maker}<h3>{g.maker}<span>{g.rows.length}</span></h3>{/if}
      <div class="grid">
        {#each g.rows as s (s.guid)}<ShipTile ship={s} {onopen} />{/each}
      </div>
    {:else}
      <p class="empty">No ship matches.</p>
    {/each}
  </section>
</div>

<style>
  .home {
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  /* One row of the same controls; on a narrow window they wrap. */
  .tools {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
  }
  .grow {
    flex: 1;
  }
  .find {
    display: flex;
    flex: 0 1 360px;
    align-items: center;
    gap: 8px;
    height: 36px;
    box-sizing: border-box;
    padding: 0 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-raised);
    color: var(--text-faint);
  }
  .find:focus-within {
    border-color: var(--accent-border);
  }
  .find input {
    all: unset;
    flex: 1;
    color: var(--text);
    font-size: var(--text-control);
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  h2 {
    display: flex;
    align-items: baseline;
    gap: 10px;
    margin: 8px 0 0;
    font-family: var(--font-display);
    font-size: var(--text-h2);
    color: var(--text-strong);
  }
  h2 small {
    font-family: var(--font-sans);
    font-size: var(--text-small);
    font-weight: var(--weight-normal);
    color: var(--text-faint);
  }
  /* In the filter row, so it shows above the first price however long the
   * list is. */
  .source {
    margin-left: auto;
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  h3 {
    display: flex;
    align-items: baseline;
    gap: 8px;
    margin: 10px 0 0;
    font-family: var(--font-display);
    font-size: 0.8rem;
    font-weight: var(--weight-bold);
    letter-spacing: var(--label-tracking);
    text-transform: var(--label-transform);
    color: var(--text-dim);
  }
  h3 span {
    font: 0.72rem var(--font-mono);
    letter-spacing: 0;
    color: var(--text-faint);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 12px;
  }
  .empty {
    margin: 0;
    font-size: var(--text-small);
    color: var(--text-faint);
  }
</style>
