<script lang="ts">
  // The frame of a card in the News page's top row (LIVE, and module
  // cards like the dev tracker's PTU): channel + version on the left, date on
  // the right, the body, and one action at the bottom right — so the cards
  // read alike side by side.
  import type { Snippet } from "svelte";
  import { channelHue } from "$lib/channels";

  let {
    channel,
    version,
    date = "",
    muted = false,
    children,
    action,
  }: {
    channel: string;
    version: string;
    date?: string;
    /** Nothing current (e.g. no PTU cycle open). */
    muted?: boolean;
    children?: Snippet;
    action?: Snippet;
  } = $props();
</script>

<article class="card now" class:muted>
  <header>
    <span class="channel" style:--hue={channelHue(channel)}>{channel}</span>
    <span class="version">{version}</span>
    <span class="grow"></span>
    <span class="dim date">{date}</span>
  </header>
  {#if children}<div class="body">{@render children()}</div>{/if}
  {#if action}<footer>{@render action()}</footer>{/if}
</article>

<style>
  .now {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
  }
  .now.muted {
    opacity: 0.7;
  }
  header {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }
  .channel {
    --hue: var(--text-dim);
    font-family: var(--font-display);
    font-weight: var(--weight-bold);
    color: color-mix(in srgb, var(--hue) 75%, var(--text));
  }
  .version {
    font-family: var(--font-mono);
    font-size: 1.25rem;
    color: var(--text-strong);
  }
  .grow {
    flex: 1;
  }
  .date {
    font-size: var(--text-small);
    white-space: nowrap;
  }
  .body {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  footer {
    display: flex;
    justify-content: flex-end;
  }
</style>
