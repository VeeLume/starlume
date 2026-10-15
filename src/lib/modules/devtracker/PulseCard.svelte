<script lang="ts">
  // The dev tracker on Home: ops posts of the last two days first, then the
  // newest unread, three of each at most. Before the opt-in it is the
  // explainer. Reads the store; the poll runs Rust-side.
  import { onDestroy, onMount } from "svelte";
  import type { DevPostView } from "$lib/bindings";
  import { devtrackerStore, openPost, watchDevtracker } from "$lib/state/devtracker.svelte";
  import GateNotice from "./GateNotice.svelte";
  import { postAge } from "./time";

  const OPS_WINDOW_MS = 48 * 3600 * 1000;

  const overview = $derived(devtrackerStore.overview);
  const ops = $derived(
    (overview?.posts ?? [])
      .filter((p) => p.shown && p.ops && p.time_ms > Date.now() - OPS_WINDOW_MS)
      .slice(0, 3),
  );
  const unread = $derived(
    (overview?.posts ?? [])
      .filter((p) => p.unread && !ops.some((o) => o.key === p.key))
      .slice(0, 3),
  );

  const headline = (p: DevPostView) => p.subject ?? p.body.replace(/\s+/g, " ");

  let stop: (() => void) | undefined;
  onMount(() => (stop = watchDevtracker()));
  onDestroy(() => stop?.());
</script>

{#if overview?.gate.state === "closed"}
  <GateNotice reason={overview.gate.reason} compact />
{:else if overview}
  {#if overview.unread}
    <p class="summary"><span class="metric-inline">{overview.unread}</span> unread</p>
  {/if}
  {#each [...ops, ...unread] as p (p.key)}
    <button class="line" onclick={() => void openPost(p.key)} title="Open on Spectrum">
      {#if p.ops}<span class="badge accent">ops</span>{/if}
      <span class="who">{p.author}</span>
      <span class="what">{headline(p)}</span>
      <span class="dim when">{postAge(p.time_ms)}</span>
    </button>
  {:else}
    <p class="dim">
      {overview.fetched_at
        ? "Nothing new. Older posts are on the tracker page."
        : "Waiting for the first check…"}
    </p>
  {/each}
{/if}

<style>
  .summary {
    margin: 0 0 4px;
  }
  .metric-inline {
    font-family: var(--font-mono);
    color: var(--text-strong);
  }
  .line {
    display: flex;
    align-items: baseline;
    gap: 8px;
    width: 100%;
    padding: 4px 0;
    text-align: left;
    min-width: 0;
    border-top: 1px solid var(--border-soft);
  }
  .line:hover .what {
    color: var(--accent);
  }
  .who {
    flex-shrink: 0;
    font-weight: var(--weight-semibold);
    font-size: var(--text-small);
  }
  .what {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .when {
    flex-shrink: 0;
    font-size: var(--text-small);
  }
</style>
