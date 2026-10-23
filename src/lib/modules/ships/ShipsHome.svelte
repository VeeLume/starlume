<script lang="ts">
  // The Ships page with no ship open: every ship of the build, as spviewer
  // and erkul open on their whole list. Search, career and sort sit on top
  // (URL-backed, frontend rule 7). Untouched, yours, the new ones and the
  // recently opened lead, then every hull by maker; searching or filtering
  // shows only the matches. Your ships come from the inventory My Items
  // last read (no fetch here); "new" counts from the first build Starlume
  // saw.
  import { Segmented, createBrowseState } from "@veelume/ui";
  import { Search } from "lucide-svelte";
  import type { ShipRowView, ShipsHomeView } from "$lib/bindings";
  import { careerLook } from "./careers";
  import ShipTile from "./ShipTile.svelte";

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
    career: { kind: "one", default: "" },
    sort: { kind: "one", default: "maker", narrows: false },
  });

  const byGuid = $derived(new Map(ships.map((s) => [s.guid, s])));
  const bases = $derived(ships.filter((s) => !s.base && !s.ground));
  const owned = $derived((home?.owned ?? []).map((o) => ({ ...o, row: byGuid.get(o.ship) })).filter((o) => o.row));
  const arrivals = $derived((home?.arrivals ?? []).map((a) => ({ ...a, row: byGuid.get(a.ship) })).filter((a) => a.row));
  const recent = $derived((home?.recent ?? []).map((g) => byGuid.get(g)).filter((r): r is ShipRowView => !!r));

  const careers = $derived.by(() => {
    const by = new Map<string, number>();
    for (const s of bases) by.set(s.career ?? "Other", (by.get(s.career ?? "Other") ?? 0) + 1);
    return [...by.entries()].sort((a, b) => b[1] - a[1]);
  });

  const q = $derived(browse.values.q.trim().toLowerCase());
  const career = $derived(browse.values.career);
  const sort = $derived(browse.values.sort);
  const narrowed = $derived(!!q || !!career);
  const list = $derived.by(() => {
    const rows = bases.filter(
      (s) =>
        (!career || (s.career ?? "Other") === career) &&
        (!q || `${s.name} ${s.role ?? ""} ${s.career ?? ""} ${s.manufacturer ?? ""}`.toLowerCase().includes(q)),
    );
    const byName = (a: ShipRowView, b: ShipRowView) => a.name.localeCompare(b.name);
    if (sort === "cargo") rows.sort((a, b) => (b.cargo ?? 0) - (a.cargo ?? 0) || byName(a, b));
    else if (sort === "crew") rows.sort((a, b) => b.seats.crew - a.seats.crew || byName(a, b));
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
    <Segmented
      options={[
        { value: "maker", label: "By maker" },
        { value: "name", label: "Name" },
        { value: "cargo", label: "Cargo" },
        { value: "crew", label: "Crew" },
      ]}
      value={sort}
      onchange={(v) => browse.set("sort", v)}
    />
  </div>
  <div class="careers" role="group" aria-label="Career">
    <button type="button" class="career" class:on={!career} onclick={() => browse.set("career", "")}>
      <span>All</span><b>{bases.length}</b>
    </button>
    {#each careers as [c, n] (c)}
      {@const look = careerLook(c)}
      {@const Icon = look.icon}
      <button type="button" class="career" class:on={career === c} style:--hue={look.hue} onclick={() => browse.set("career", career === c ? "" : c)}>
        <Icon size={15} /><span>{c}</span><b>{n}</b>
      </button>
    {/each}
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
  .tools {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .find {
    display: flex;
    flex: 1;
    align-items: center;
    gap: 8px;
    max-width: 560px;
    padding: 7px 12px;
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
  .careers {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .career {
    all: unset;
    --hue: var(--accent);
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 5px 12px;
    border-radius: var(--radius-pill);
    border: 1px solid color-mix(in srgb, var(--hue) 30%, var(--border));
    background: color-mix(in srgb, var(--hue) 8%, transparent);
    color: color-mix(in srgb, var(--hue) 70%, var(--text));
    cursor: pointer;
  }
  .career span {
    color: var(--text);
  }
  .career b {
    font: 600 0.74rem var(--font-mono);
    color: var(--text-faint);
  }
  .career:hover,
  .career.on {
    border-color: color-mix(in srgb, var(--hue) 65%, var(--border));
    background: color-mix(in srgb, var(--hue) 18%, transparent);
  }
</style>
