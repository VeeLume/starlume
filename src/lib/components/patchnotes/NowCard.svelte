<script lang="ts">
  // The frame of a card in the News page's top row (LIVE, and module
  // cards like the dev tracker's PTU): tinted in the channel's hue, the
  // channel and date on top, the version large with the patch name beside
  // it, the body, and one action at the bottom right — so the cards read
  // alike side by side. An image (a local file) sits on the left when given.
  import type { Snippet } from "svelte";
  import { channelHue } from "$lib/channels";

  let {
    channel,
    version,
    name = "",
    date = "",
    image = null,
    muted = false,
    children,
    action,
  }: {
    channel: string;
    version: string;
    /** The patch name ("RSI Discovery Month"), beside the version. */
    name?: string;
    date?: string;
    /** An asset URL (`convertFileSrc`) — never a remote one. */
    image?: string | null;
    /** Nothing current (e.g. no PTU cycle open). */
    muted?: boolean;
    children?: Snippet;
    action?: Snippet;
  } = $props();
</script>

<article class="now" class:muted class:with-image={!!image} style:--hue={channelHue(channel)}>
  {#if image}<div class="image"><img src={image} alt="" /></div>{/if}
  <div class="main">
    <header>
      <span class="hue-chip channel">{channel}</span>
      <span class="grow"></span>
      <span class="date">{date}</span>
    </header>
    <div class="title">
      <span class="version">{version}</span>
      {#if name}<span class="name">{name}</span>{/if}
    </div>
    {#if children}<div class="body">{@render children()}</div>{/if}
    {#if action}<footer>{@render action()}</footer>{/if}
  </div>
</article>

<style>
  .now {
    --hue: var(--text-dim);
    display: grid;
    min-width: 0;
    border-radius: var(--radius-lg);
    overflow: hidden;
    background: linear-gradient(
      135deg,
      color-mix(in srgb, var(--hue) 13%, var(--bg-raised)),
      var(--bg-raised) 60%
    );
    border: 1px solid color-mix(in srgb, var(--hue) 28%, var(--border));
  }
  .now.with-image {
    grid-template-columns: minmax(140px, 36%) minmax(0, 1fr);
  }
  .now.muted {
    opacity: 0.7;
  }
  .image {
    position: relative;
    min-height: 100%;
    background: var(--bg-sunk);
  }
  .image img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .main {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px 16px;
    min-width: 0;
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .grow {
    flex: 1;
  }
  .date {
    font-size: var(--text-small);
    color: var(--text-dim);
    white-space: nowrap;
  }
  .title {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 6px 14px;
  }
  .version {
    font-family: var(--font-display);
    font-weight: var(--weight-bold);
    font-size: 2.2rem;
    line-height: 1;
    color: color-mix(in srgb, var(--hue) 55%, var(--text-strong));
  }
  .name {
    font-weight: var(--weight-semibold);
    font-size: 1.05rem;
    color: var(--text-strong);
  }
  .body {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  footer {
    display: flex;
    justify-content: flex-end;
  }
</style>
