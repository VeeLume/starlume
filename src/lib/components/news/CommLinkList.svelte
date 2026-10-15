<script lang="ts">
  // The newest comm-links in three tiers: the newest featured with its
  // wide image and summary, the next few as image cards, the rest as
  // compact rows with a square thumbnail. The images are local files the
  // Rust refresh downloaded (svc-commlinks); the webview fetches nothing
  // from RSI. Everything opens on RSI in the browser. Times are when
  // Starlume first saw the item (estimated from its age on the first fill).
  import { onDestroy, onMount } from "svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { Play } from "lucide-svelte";
  import type { CommLinkView } from "$lib/bindings";
  import { commLinksStore, openCommLink, watchCommLinks } from "$lib/state/commlinks.svelte";
  import { activityFor, activityLabel } from "$lib/state/activities.svelte";
  import { postAge } from "$lib/modules/devtracker/time";

  /** Image cards after the featured one — svc-commlinks' `CARDS`. */
  const CARDS = 4;

  const overview = $derived(commLinksStore.overview);
  const reading = $derived(activityFor("comm-links", ""));
  const items = $derived(overview?.items ?? []);
  const featured = $derived(items[0] ?? null);
  const cards = $derived(items.slice(1, 1 + CARDS));
  const rows = $derived(items.slice(1 + CARDS));

  // Comm-link channels in the category hues (app.css): the two that carry
  // most posts; the rest stay neutral.
  const CHANNEL_HUES: Record<string, string> = {
    transmission: "var(--hue-sand)",
    engineering: "var(--hue-teal)",
  };
  const channelName = (slug: string | null) =>
    slug ? slug.replace(/-/g, " ").replace(/^./, (c) => c.toUpperCase()) : null;
  const src = (c: CommLinkView) => (c.image ? convertFileSrc(c.image) : null);
  const when = (c: CommLinkView) => (c.published_ms ? postAge(c.published_ms) : "");
  const open = (c: CommLinkView) => void openCommLink(c.url);

  // Subscribe itself (ref-counted), like every store surface.
  let stop: (() => void) | undefined;
  onMount(() => (stop = watchCommLinks()));
  onDestroy(() => stop?.());
</script>

{#snippet chips(c: CommLinkView)}
  {#if c.kind === "video"}<span class="hue-chip" style:--hue="var(--hue-video)">Video</span>{/if}
  {#if channelName(c.channel)}
    <span class="hue-chip" style:--hue={CHANNEL_HUES[c.channel ?? ""]}>{channelName(c.channel)}</span>
  {/if}
{/snippet}

{#if commLinksStore.error}<p class="error">{commLinksStore.error}</p>{/if}
{#if overview?.last_error}<p class="error">{overview.last_error}</p>{/if}

{#if featured}
  <div class="commlinks">
    <button type="button" class="featured" title="Read on RSI" onclick={() => open(featured)}>
      <div class="media">
        {#if src(featured)}<img src={src(featured)} alt="" />{/if}
        {#if featured.kind === "video"}<span class="play"><Play size={20} /></span>{/if}
      </div>
      <div class="text">
        <div class="meta">{@render chips(featured)}<span>{when(featured)}</span></div>
        <h3>{featured.title}</h3>
        {#if featured.summary}<p>{featured.summary}</p>{/if}
      </div>
    </button>

    {#if cards.length}
      <div class="cards">
        {#each cards as c (c.id)}
          <button type="button" class="card" title="Read on RSI" onclick={() => open(c)}>
            <div class="media">
              {#if src(c)}<img src={src(c)} alt="" />{/if}
              {#if c.kind === "video"}<span class="play small"><Play size={14} /></span>{/if}
            </div>
            <h4>{c.title}</h4>
            <div class="meta">{@render chips(c)}<span>{when(c)}</span></div>
          </button>
        {/each}
      </div>
    {/if}

    {#if rows.length}
      <div class="rows">
        {#each rows as c (c.id)}
          <button type="button" class="row" title="Read on RSI" onclick={() => open(c)}>
            <span class="thumb">{#if src(c)}<img src={src(c)} alt="" />{/if}</span>
            <span class="row-text">
              <span class="row-title">{c.title}</span>
              <span class="row-sub">{c.summary || channelName(c.channel) || ""}</span>
            </span>
            <span class="when">{when(c)}</span>
          </button>
        {/each}
      </div>
    {/if}
  </div>
{:else if reading}
  <p class="dim">⟳ {activityLabel(reading)}</p>
{:else if overview && !overview.online}
  <p class="dim">
    Online features are off, so Starlume doesn't read RSI's comm-links
    (<a href="/settings/online">Settings → Online</a>).
  </p>
{:else if overview}
  <p class="dim">Not read yet. The first check runs shortly after start.</p>
{/if}

<style>
  .commlinks {
    container-type: inline-size;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  /* The tiers are buttons: reset to a plain block. */
  button {
    all: unset;
    box-sizing: border-box;
    cursor: pointer;
    min-width: 0;
  }
  button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
    border-radius: var(--radius-lg);
  }
  .media {
    position: relative;
    overflow: hidden;
    background: var(--bg-sunk);
  }
  .media img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
    transition: transform 300ms var(--easing);
  }
  button:hover .media img {
    transform: scale(1.03);
  }
  .play {
    position: absolute;
    left: 50%;
    top: 50%;
    translate: -50% -50%;
    display: grid;
    place-items: center;
    width: 48px;
    height: 48px;
    padding-left: 3px;
    border-radius: 50%;
    color: var(--text-strong);
    background: color-mix(in srgb, var(--bg-sunk) 55%, transparent);
    border: 2px solid color-mix(in srgb, var(--text-strong) 80%, transparent);
    backdrop-filter: blur(4px);
  }
  .play.small {
    width: 32px;
    height: 32px;
    padding-left: 2px;
  }
  .meta {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px 8px;
    font-size: var(--text-small);
    color: var(--text-faint);
  }

  /* Featured */
  .featured {
    display: grid;
    grid-template-columns: minmax(0, 1.25fr) minmax(0, 1fr);
    border-radius: var(--radius-lg);
    overflow: hidden;
    background: var(--bg-raised);
    border: 1px solid var(--border);
  }
  .featured:hover {
    border-color: var(--accent-border);
  }
  .featured .media {
    aspect-ratio: 16 / 9;
  }
  .text {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 14px 16px;
    min-width: 0;
  }
  h3 {
    margin: 0;
    font-family: var(--font-body);
    font-size: 1.2rem;
    line-height: 1.25;
    color: var(--text-strong);
    text-wrap: balance;
  }
  .text p {
    margin: 0;
    color: var(--text-dim);
    display: -webkit-box;
    -webkit-line-clamp: 5;
    line-clamp: 5;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  /* Cards */
  .cards {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 12px;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 7px;
  }
  .card .media {
    aspect-ratio: 5 / 3;
    border-radius: var(--radius);
  }
  h4 {
    margin: 0;
    font-size: 0.9rem;
    font-weight: var(--weight-semibold);
    line-height: 1.35;
    color: var(--text-strong);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .card:hover h4 {
    color: var(--accent);
  }

  /* Rows */
  .rows {
    display: flex;
    flex-direction: column;
    border-top: 1px solid var(--border);
  }
  .row {
    display: grid;
    grid-template-columns: 40px minmax(0, 1fr) auto;
    align-items: center;
    gap: 12px;
    padding: 7px 4px;
    border-bottom: 1px solid var(--border);
  }
  .row:hover {
    background: var(--accent-fill-faint);
  }
  .thumb {
    width: 40px;
    height: 40px;
    border-radius: var(--radius);
    overflow: hidden;
    background: var(--bg-sunk);
  }
  .thumb img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .row-text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .row-title,
  .row-sub {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .row-title {
    font-weight: var(--weight-medium);
  }
  .row-sub {
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .when {
    font-size: var(--text-small);
    color: var(--text-faint);
    white-space: nowrap;
  }

  @container (width < 640px) {
    .featured {
      grid-template-columns: minmax(0, 1fr);
    }
    .cards {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
</style>
