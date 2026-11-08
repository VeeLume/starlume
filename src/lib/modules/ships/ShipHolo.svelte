<script lang="ts">
  // The ship's picture as the one way between the build and the ship's own
  // stats: on Build it opens them, in the open sheet (same place, same size)
  // it closes them again. The label says which, in the same corner.
  import { Maximize2, Minimize2 } from "lucide-svelte";

  let {
    picture,
    open = false,
    glow = null,
    el = $bindable(),
    onclick,
  }: {
    picture: string | undefined;
    open?: boolean;
    /** The hue of the system under the pointer (Build only). */
    glow?: string | null;
    el?: HTMLButtonElement;
    onclick: () => void;
  } = $props();
</script>

<button
  type="button"
  class="holo"
  class:framed={!open}
  class:lit={glow}
  style:--glow={glow ?? "var(--hue-sky)"}
  aria-expanded={open}
  bind:this={el}
  {onclick}
>
  {#if picture}<img class="shot" src={picture} alt="" />{/if}
  <span class="more">
    {#if open}
      <Minimize2 size={13} /> Back to the build
    {:else}
      <Maximize2 size={13} /> Ship stats
    {/if}
  </span>
</button>

<style>
  /* At a fixed height, so the power screen starts in view under it. */
  .holo {
    position: relative;
    display: block;
    width: 100%;
    height: clamp(220px, 32vh, 380px);
    padding: 0;
    border: 1px solid transparent;
    border-radius: var(--radius-lg);
    background: transparent;
    cursor: pointer;
  }
  .holo:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .shot {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
    mask-image: radial-gradient(closest-side, black 70%, transparent);
    opacity: 0.92;
    pointer-events: none;
  }
  .more {
    position: absolute;
    right: 10px;
    bottom: 10px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 3px 8px;
    border-radius: var(--radius-chip);
    background: color-mix(in srgb, var(--bg-sunk) 70%, transparent);
    font-size: var(--text-small);
    color: var(--text-faint);
    transition: color 160ms var(--easing);
  }
  .holo:hover .more,
  .holo:focus-visible .more {
    color: var(--text-strong);
  }
  /* The window on Build: the system under the pointer tints its glow
   * (colour only, nothing moves). */
  .framed {
    border-color: color-mix(in srgb, var(--glow) 30%, var(--border));
    background: radial-gradient(
      120% 90% at 50% 45%,
      color-mix(in srgb, var(--glow) 10%, var(--bg-sunk)),
      var(--bg-sunk) 70%
    );
    transition:
      border-color 160ms var(--easing),
      background 160ms var(--easing);
  }
  .framed:hover,
  .framed.lit {
    border-color: color-mix(in srgb, var(--glow) 55%, var(--border));
  }
  /* A store picture is a full scene that hides the background, so the
   * glow lies over it, around the edge. */
  .framed::after {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: inherit;
    pointer-events: none;
    box-shadow: inset 0 0 0 transparent;
    transition: box-shadow 160ms var(--easing);
  }
  .framed.lit::after {
    box-shadow: inset 0 0 56px color-mix(in srgb, var(--glow) 45%, transparent);
  }
</style>
