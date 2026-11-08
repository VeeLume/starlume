<script lang="ts">
  // The ship as a hologram of what is fitted: drag orbits, the wheel zooms,
  // a plain click (or the label) opens the ship's stats and closes them
  // again. It stays this one view when the sheet opens over the build, so
  // nothing reloads. The store picture holds the place until the model is
  // drawn, and stays when there is none. The canvas exists only while this
  // does; memory rule "Cooked data is evictable" covers the Rust side.
  import { Maximize2, Minimize2 } from "lucide-svelte";
  import type { ShipScene as DrawnScene, ShipView } from "@veelume/sc-render";
  import { ShipViewport } from "@veelume/sc-render/svelte";
  import { tauriSource } from "@veelume/sc-render/tauri";
  import type { ShipScene } from "$lib/bindings";

  let {
    picture,
    scene,
    open = false,
    glow = null,
    lit = null,
    el = $bindable(),
    onclick,
  }: {
    picture: string | undefined;
    /** Null until assembled, and for a ship without a model. */
    scene: ShipScene | null;
    open?: boolean;
    /** The hue of the system under the pointer. */
    glow?: string | null;
    /** The ports under the pointer, lit in `glow`. */
    lit?: string[] | null;
    el?: HTMLElement;
    onclick: () => void;
  } = $props();

  let view = $state<ShipView | null>(null);
  let drawn = $state(false);
  let busy = $state(false);
  let shows = 0;

  $effect(() => {
    if (!view || !scene) return;
    const seq = ++shows;
    busy = true;
    view
      .show(scene as DrawnScene)
      .then(() => {
        if (seq === shows) drawn = true;
      })
      .catch((e) => console.warn("the ship holo failed:", e))
      .finally(() => {
        if (seq === shows) busy = false;
      });
  });

  /** three.js takes a colour value, not `var(--hue-sky)`. */
  const resolved = (hue: string, at: HTMLElement) => {
    const name = /var\((--[^)]+)\)/.exec(hue)?.[1];
    return name ? getComputedStyle(at).getPropertyValue(name).trim() : hue;
  };

  $effect(() => {
    if (!view || !drawn || !el) return;
    view.highlight(lit && glow ? lit : null, glow ? resolved(glow, el) : undefined);
  });

  // A drag orbits; only a click that stayed put opens the stats.
  let downAt: [number, number] | null = null;
  const STILL = 4;
  function pressed(e: PointerEvent) {
    downAt = [e.clientX, e.clientY];
  }
  function released(e: MouseEvent) {
    if (downAt && Math.hypot(e.clientX - downAt[0], e.clientY - downAt[1]) <= STILL) onclick();
  }
</script>

<!-- The label below is the keyboard's way to the same action. -->
<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div
  class="holo framed"
  class:lit={glow}
  style:--glow={glow ?? "var(--hue-sky)"}
  bind:this={el}
  onpointerdown={pressed}
  onclick={released}
>
  {#if picture && !drawn}<img class="shot" src={picture} alt="" />{/if}
  <!-- The view's own dark floor: on a transparent canvas the see-through
       hull layers up into a solid blob. -->
  <div class="model" class:drawn>
    <ShipViewport source={tauriSource()} bind:view options={{ mode: "holo", background: true, grid: true }} {busy} />
  </div>
  <button
    type="button"
    class="more"
    aria-expanded={open}
    onclick={(e) => {
      e.stopPropagation();
      onclick();
    }}
  >
    {#if open}
      <Minimize2 size={13} /> Back to the build
    {:else}
      <Maximize2 size={13} /> Ship stats
    {/if}
  </button>
</div>

<style>
  /* At a fixed height, so the power screen starts in view under it. */
  .holo {
    position: relative;
    z-index: 11; /* over the ship sheet, whose middle column leaves it room */
    height: var(--holo-height);
    border: 1px solid transparent;
    border-radius: var(--radius-lg);
    cursor: pointer;
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
  .model {
    position: absolute;
    inset: 0;
    overflow: hidden;
    border-radius: inherit;
    opacity: 0;
    transition: opacity 240ms var(--easing);
  }
  .model.drawn {
    opacity: 1;
  }
  .more {
    position: absolute;
    right: 10px;
    bottom: 10px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 3px 8px;
    border: 0;
    border-radius: var(--radius-chip);
    background: color-mix(in srgb, var(--bg-sunk) 70%, transparent);
    font-size: var(--text-small);
    color: var(--text-faint);
    cursor: pointer;
    transition: color 160ms var(--easing);
  }
  .more:hover,
  .more:focus-visible {
    color: var(--text-strong);
  }
  .more:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
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
  /* The glow lies over the model, around the edge. */
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
