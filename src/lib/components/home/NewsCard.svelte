<script lang="ts">
  // News on Home: the newest comm-link with its image, then the next three
  // headlines. Opens like the News page (videos on YouTube). Reads the
  // comm-links store; the refresh runs Rust-side.
  import { onDestroy, onMount } from "svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import type { CommLinkView } from "$lib/bindings";
  import { commLinksStore, openCommLink, watchCommLinks } from "$lib/state/commlinks.svelte";
  import { postAge } from "$lib/modules/devtracker/time";

  const items = $derived(commLinksStore.overview?.items ?? []);
  const first = $derived(items[0] ?? null);
  const open = (c: CommLinkView) => void openCommLink(c.video_url ?? c.url);
  const when = (c: CommLinkView) => (c.published_ms ? postAge(c.published_ms) : "");

  let stop: (() => void) | undefined;
  onMount(() => (stop = watchCommLinks()));
  onDestroy(() => stop?.());
</script>

{#if first}
  <button type="button" class="lead" onclick={() => open(first)}>
    <span class="media">{#if first.image}<img src={convertFileSrc(first.image)} alt="" />{/if}</span>
    <span class="lead-text">
      <span class="meta">
        {#if first.kind === "video"}<span class="hue-chip" style:--hue="var(--hue-video)">Video</span>{/if}
        <span>{when(first)}</span>
      </span>
      <span class="lead-title">{first.title}</span>
    </span>
  </button>
  <div class="rows">
    {#each items.slice(1, 4) as c (c.id)}
      <button type="button" class="row" onclick={() => open(c)}>
        <span class="row-title">{c.title}</span>
        <span class="when">{when(c)}</span>
      </button>
    {/each}
  </div>
{:else if commLinksStore.overview}
  <p class="dim">
    {commLinksStore.overview.online
      ? "Not read yet. The first check runs shortly after start."
      : "Online features are off, so Starlume doesn't read RSI's comm-links."}
  </p>
{/if}

<style>
  button {
    all: unset;
    box-sizing: border-box;
    cursor: pointer;
    min-width: 0;
  }
  button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
    border-radius: var(--radius);
  }
  .lead {
    display: grid;
    grid-template-columns: 132px minmax(0, 1fr);
    gap: 12px;
  }
  .media {
    display: block;
    aspect-ratio: 5 / 3;
    border-radius: var(--radius);
    overflow: hidden;
    background: var(--bg-sunk);
  }
  .media img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .lead-text {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .meta {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .lead-title {
    font-weight: var(--weight-semibold);
    line-height: 1.3;
    color: var(--text-strong);
  }
  .lead:hover .lead-title,
  .row:hover .row-title {
    color: var(--accent);
  }
  .rows {
    display: flex;
    flex-direction: column;
  }
  .row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 10px;
    padding: 6px 0;
    border-top: 1px solid var(--border-soft);
  }
  .row-title {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .when {
    font-size: var(--text-small);
    color: var(--text-faint);
    white-space: nowrap;
  }
  p {
    margin: 0;
  }
</style>
