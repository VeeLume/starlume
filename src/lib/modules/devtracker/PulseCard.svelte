<script lang="ts">
  // The dev tracker on Home: ops posts of the last two days first, then the
  // newest unread, three of each at most, in the feed's shapes (monogram,
  // lobby or kind chip). The unread count is the card's heading badge
  // (descriptor). Before the opt-in it is the explainer. Reads the store;
  // the poll runs Rust-side.
  import { onDestroy, onMount } from "svelte";
  import type { DevPostView } from "$lib/bindings";
  import { devtrackerStore, openPost, watchDevtracker } from "$lib/state/devtracker.svelte";
  import GateNotice from "./GateNotice.svelte";
  import Monogram from "./Monogram.svelte";
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
  {#each [...ops, ...unread] as p (p.key)}
    <button class="line" onclick={() => void openPost(p.key)} title="Open on Spectrum">
      <Monogram name={p.author} size={26} />
      <span class="txt">
        <span class="who">
          {p.author}
          {#if p.kind === "chat"}
            {#if p.place}<span class="hue-chip" style:--hue="var(--hue-chat)"># {p.place}</span>{/if}
          {:else}
            <span class="hue-chip" style:--hue={p.kind === "thread" ? "var(--hue-thread)" : "var(--hue-reply)"}>
              {p.kind === "thread" ? "Thread" : "Reply"}
            </span>
          {/if}
          {#if p.ops}<span class="badge accent">ops</span>{/if}
        </span>
        <span class="what">{headline(p)}</span>
      </span>
      <span class="when">{postAge(p.time_ms)}</span>
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
  .line {
    display: grid;
    grid-template-columns: 26px minmax(0, 1fr) auto;
    align-items: start;
    gap: 10px;
    width: 100%;
    padding: 7px 0;
    text-align: left;
    border-top: 1px solid var(--border-soft);
  }
  .line:first-child {
    border-top: 0;
    padding-top: 0;
  }
  .line:hover .what {
    color: var(--accent);
  }
  .txt {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .who {
    display: flex;
    align-items: center;
    gap: 6px;
    font-weight: var(--weight-semibold);
    font-size: var(--text-small);
    color: var(--text-strong);
  }
  .what {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-dim);
  }
  .when {
    font-size: var(--text-small);
    color: var(--text-faint);
    white-space: nowrap;
  }
</style>
