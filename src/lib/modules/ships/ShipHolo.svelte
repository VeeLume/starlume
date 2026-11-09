<script lang="ts">
  // The ship as a hologram of what is fitted. As a card it turns about its
  // vertical axis, and pointing at a part turns that part to the viewer; a
  // plain click (or the label) expands it into the page's background, where
  // the camera is free. It is this one view both ways, so nothing reloads.
  // The store picture holds the place until the model is drawn, and stays
  // when there is none. The canvas exists only while this does; memory rule
  // "Cooked data is evictable" covers the Rust side.
  import { Maximize2, Minimize2 } from "lucide-svelte";
  import type { ShipScene as DrawnScene, ShipView } from "@veelume/sc-render";
  import { ShipViewport } from "@veelume/sc-render/svelte";
  import { tauriSource } from "@veelume/sc-render/tauri";
  import type { ShipScene } from "$lib/bindings";
  import { Turntable } from "./holoCamera";

  let {
    picture,
    live,
    scene,
    open = false,
    slot,
    hue = null,
    lit = null,
    focus = null,
    onclick,
  }: {
    picture: string | undefined;
    /** The 3D view (the "Ship holo" setting); off, just the picture. */
    live: boolean;
    /** Null until assembled, and for a ship without a model. */
    scene: ShipScene | null;
    /** Expanded behind the ship sheet. */
    open?: boolean;
    /** The card's place, which the expansion grows out of. */
    slot?: HTMLElement;
    /** The colour of the system under the pointer. */
    hue?: string | null;
    /** The ports under the pointer, lit in `hue`. */
    lit?: string[] | null;
    /** The ports of the one line under the pointer, to turn to. */
    focus?: string[] | null;
    onclick: () => void;
  } = $props();

  let view = $state<ShipView | null>(null);
  let box = $state<HTMLElement>();
  let drawn = $state(false);
  let busy = $state(false);
  let shows = 0;
  const table = $derived(view ? new Turntable(view) : null);

  // Off and on again is a new canvas: the picture holds the place again.
  $effect(() => {
    if (!live) drawn = false;
  });

  $effect(() => {
    if (!view || !scene) return;
    const seq = ++shows;
    const turntable = table;
    busy = true;
    view
      .show(scene as DrawnScene)
      .then(() => {
        if (seq !== shows) return;
        drawn = true;
        // `show` framed it anew.
        if (open) {
          turntable?.free();
          turntable?.level();
        } else turntable?.lock();
      })
      .catch((e) => console.warn("the ship holo failed:", e))
      .finally(() => {
        if (seq === shows) busy = false;
      });
  });

  // Back to a card is back to its one view.
  $effect(() => {
    if (!view || !table || !drawn) return;
    if (open) table.free();
    else {
      view.frame();
      table.lock();
    }
  });

  // The expansion is revealed out of the card; the canvas is at its new
  // size at once (resizing it every frame would stutter on a big ship).
  $effect(() => {
    if (!open || !slot || !box || matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const a = slot.getBoundingClientRect();
    const b = box.getBoundingClientRect();
    const from = `inset(${a.top - b.top}px ${b.right - a.right}px ${b.bottom - a.bottom}px ${a.left - b.left}px round var(--radius-lg))`;
    box.animate([{ clipPath: from }, { clipPath: "inset(0 round var(--radius-lg))" }], { duration: 200, easing: "cubic-bezier(0.33, 1, 0.68, 1)" });
  });

  /** three.js takes a colour value, not `var(--hue-sky)`. */
  const resolved = (colour: string, at: HTMLElement) => {
    const name = /var\((--[^)]+)\)/.exec(colour)?.[1];
    return name ? getComputedStyle(at).getPropertyValue(name).trim() : colour;
  };

  $effect(() => {
    if (!view || !drawn || !box) return;
    view.highlight(lit && hue ? lit : null, hue ? resolved(hue, box) : undefined);
  });

  // A short wait before turning back, so sweeping across the lines doesn't
  // swing the ship home in the gaps between them.
  const SETTLE_AFTER = 180;
  $effect(() => {
    const turntable = table;
    if (!turntable || !drawn || open) return;
    if (focus) {
      turntable.turnTo(focus);
      return;
    }
    const wait = setTimeout(() => turntable.settle(), SETTLE_AFTER);
    return () => clearTimeout(wait);
  });

  $effect(() => {
    const turntable = table;
    return () => turntable?.stop();
  });

  // A drag turns it; only a click that stayed put expands or collapses.
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
<div class="holo" class:open bind:this={box} onpointerdown={pressed} onclick={released}>
  {#if picture && !drawn}<img class="shot" src={picture} alt="" />{/if}
  <!-- The view's own dark floor: on a transparent canvas the see-through
       hull layers up into a solid blob. -->
  {#if live}
    <div class="model" class:drawn>
      <ShipViewport source={tauriSource()} bind:view options={{ mode: "holo", background: true, grid: true }} {busy} />
    </div>
  {/if}
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
  .holo {
    position: relative;
    height: 100%;
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--bg-sunk);
    cursor: pointer;
  }
  /* Behind the ship sheet, over the whole of the page below the tabs. */
  .holo.open {
    position: absolute;
    inset: 0;
    z-index: 9;
    height: auto;
  }
  /* Too narrow for the stats to sit beside the ship: it goes above them. */
  @container (width < 1400px) {
    .holo.open {
      bottom: auto;
      height: 55vh;
    }
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
  /* The middle of the page: the side columns cover its corners. */
  .open .more {
    right: 50%;
    transform: translateX(50%);
  }
  .more:hover,
  .more:focus-visible {
    color: var(--text-strong);
  }
  .more:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
</style>
