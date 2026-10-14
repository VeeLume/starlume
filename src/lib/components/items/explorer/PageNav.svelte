<script lang="ts">
  // The page header: title left, the three views centred (Items · Ships ·
  // Resources), freshness and the refresh right. Switching views swaps only
  // what the panes show; this header and the tree stay put, so the page
  // doesn't jump (the toggle-buttons version did).
  import { Button } from "@veelume/ui";
  import { fmtN, type View } from "./ui";

  let {
    view,
    counts,
    updated,
    refreshing,
    onview,
    onrefresh,
  }: {
    view: View;
    counts: Record<View, number> | null;
    updated: string | null;
    refreshing: boolean;
    onview: (v: View) => void;
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
  <nav aria-label="View">
    {#each VIEWS as [id, label] (id)}
      <button type="button" class="tab" class:on={view === id} aria-current={view === id ? "page" : undefined} onclick={() => onview(id)}
        >{label}{#if counts}<span class="n">{fmtN(counts[id])}</span>{/if}</button
      >
    {/each}
  </nav>
  <div class="meta">
    <span class="updated">{updated ? `Updated ${updated}` : "Not fetched yet"}</span>
    <Button variant="outline" onclick={onrefresh} disabled={refreshing}>{refreshing ? "Fetching…" : "Refresh from game"}</Button>
  </div>
</header>

<style>
  .pagehead {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    gap: 12px;
    min-height: 52px;
    border-bottom: 1px solid var(--border);
  }
  h1 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.05rem;
    font-weight: 700;
    color: var(--text-strong);
  }
  nav {
    display: flex;
    gap: 2px;
    align-self: stretch;
  }
  .tab {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 0 14px;
    border: 0;
    background: transparent;
    color: var(--text-dim);
    font: inherit;
    font-size: 0.9rem;
    cursor: pointer;
  }
  .tab:hover {
    color: var(--text);
  }
  .tab.on {
    color: var(--accent);
    font-weight: 600;
  }
  .tab.on::after {
    content: "";
    position: absolute;
    left: 10px;
    right: 10px;
    bottom: -1px;
    height: 2px;
    border-radius: 2px;
    background: var(--accent);
  }
  .n {
    font-family: var(--font-mono);
    font-size: 0.66rem;
    font-weight: 400;
    color: var(--text-faint);
  }
  .tab.on .n {
    color: var(--accent);
    opacity: 0.75;
  }
  .meta {
    justify-self: end;
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .updated {
    white-space: nowrap;
    font-size: 0.8rem;
    color: var(--text-dim);
  }
</style>
