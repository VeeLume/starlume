<script lang="ts">
  // The page header, one row (design pass): the title, the three views
  // (Items · Ships · Resources) as a segmented switch with their counts,
  // the search, then freshness and the refresh. Switching views swaps only
  // what the explorer shows; this header stays put.
  import { Button } from "@veelume/ui";
  import { fmtN, type View } from "./ui";

  let {
    view,
    counts,
    updated,
    refreshing,
    q,
    placeholder,
    onview,
    onq,
    onrefresh,
  }: {
    view: View;
    counts: Record<View, number> | null;
    updated: string | null;
    refreshing: boolean;
    q: string;
    placeholder: string;
    onview: (v: View) => void;
    onq: (q: string) => void;
    onrefresh: () => void;
  } = $props();

  const VIEWS: [View, string][] = [
    ["items", "Items"],
    ["ships", "Ships"],
    ["res", "Resources"],
  ];
</script>

<header class="pagehead">
  <h1>My Items</h1>
  <div class="views" role="tablist" aria-label="View">
    {#each VIEWS as [id, label] (id)}
      <button
        type="button"
        role="tab"
        class:on={view === id}
        aria-selected={view === id}
        onclick={() => onview(id)}
        >{label}{#if counts}<span class="n">{fmtN(counts[id])}</span>{/if}</button
      >
    {/each}
  </div>
  <input
    class="input search"
    type="search"
    value={q}
    {placeholder}
    aria-label={placeholder}
    oninput={(e) => onq(e.currentTarget.value)}
  />
  <span class="grow"></span>
  <span class="updated">{updated ? `Read from the game ${updated}` : "Not read yet"}</span>
  <Button variant="outline" onclick={onrefresh} disabled={refreshing}>{refreshing ? "Reading…" : "Refresh"}</Button>
</header>

<style>
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
    display: inline-flex;
    gap: 3px;
    padding: 3px;
    border-radius: var(--radius-lg);
    background: var(--bg-raised);
    border: 1px solid var(--border);
  }
  .views button {
    display: inline-flex;
    align-items: baseline;
    gap: 6px;
    padding: 5px 12px;
    border: 0;
    border-radius: var(--radius);
    background: transparent;
    color: var(--text-dim);
    font: inherit;
    font-size: var(--text-control);
    cursor: pointer;
  }
  .views button:hover {
    color: var(--text);
  }
  .views button.on {
    background: var(--bg);
    color: var(--text-strong);
    box-shadow: inset 0 0 0 1px var(--border);
  }
  .n {
    font-family: var(--font-mono);
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .search {
    flex: 1 1 240px;
    max-width: 420px;
    min-width: 0;
  }
  .grow {
    flex: 1;
  }
  .updated {
    white-space: nowrap;
    font-size: var(--text-small);
    color: var(--text-faint);
  }
</style>
