<script lang="ts">
  // The tinted card of the News page's top row, shared with Mining: tinted
  // in a hue, a chip on top (the channel, or what the card is about), the
  // title large with a name beside it, the body, and one action at the
  // bottom right, so cards read alike side by side. An image (a local file)
  // sits on the left when given; `art` draws decoration behind the content.
  //
  // `size` sets its share of a flex row: `wide` for the lead card, `normal`,
  // `compact` for a quiet one (narrow, untinted, the title small).
  import type { Snippet } from "svelte";
  import { channelHue } from "$lib/channels";

  let {
    channel = undefined,
    hue = undefined,
    chip = undefined,
    version,
    name = "",
    date = "",
    image = null,
    size = "normal",
    muted = false,
    top,
    art,
    children,
    action,
  }: {
    /** A release channel: its hue and its name as the chip. */
    channel?: string;
    /** Any hue (a CSS colour); overrides the channel's. */
    hue?: string;
    /** The chip text; defaults to the channel. */
    chip?: string;
    /** The large title: a version, a rock, a ship. */
    version: string;
    /** Beside the title ("RSI Discovery Month", "Mining Laser S1"). */
    name?: string;
    date?: string;
    /** An asset URL (`convertFileSrc`) — never a remote one. */
    image?: string | null;
    size?: "wide" | "normal" | "compact";
    /** Nothing current (e.g. no PTU cycle open). */
    muted?: boolean;
    /** Controls at the right of the chip row, in place of the date. */
    top?: Snippet;
    /** Decoration drawn behind the content (top right). */
    art?: Snippet;
    children?: Snippet;
    action?: Snippet;
  } = $props();

  const tint = $derived(hue ?? (channel ? channelHue(channel) : "var(--text-dim)"));
  const label = $derived(chip ?? channel ?? "");
</script>

<article
  class="now {size}"
  class:muted
  class:with-image={!!image && size !== "compact"}
  style:--hue={tint}
>
  {#if image && size !== "compact"}<div class="image"><img src={image} alt="" /></div>{/if}
  {#if art}<div class="art" aria-hidden="true">{@render art()}</div>{/if}
  <div class="main">
    <header>
      {#if label}<span class="hue-chip channel">{label}</span>{/if}
      <span class="grow"></span>
      {#if top}{@render top()}{:else}<span class="date">{date}</span>{/if}
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
    flex: 1 1 320px;
    position: relative;
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
  .now.wide {
    flex: 2 1 440px;
  }
  .now.compact {
    flex: 0 1 230px;
    background: var(--bg-raised);
    border-color: var(--border);
  }
  .compact .main {
    gap: 6px;
    padding: 12px 14px;
  }
  .compact .version {
    font-size: 1.3rem;
    color: var(--text-dim);
  }
  .compact .name {
    font-size: var(--text-base);
    color: var(--text-dim);
  }
  .now.with-image {
    grid-template-columns: minmax(140px, 30%) minmax(0, 1fr);
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
  .art {
    position: absolute;
    top: -14px;
    right: -18px;
    pointer-events: none;
  }
  .main {
    position: relative;
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
    align-items: center;
    gap: 8px;
  }
</style>
