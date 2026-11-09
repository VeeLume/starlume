<script lang="ts">
  // The ship as a hologram of what is fitted. As a card it turns about its
  // vertical axis, and pointing at a part turns that part to the viewer; a
  // plain click (or the label) expands it into the page's background, where
  // the camera is free. It is this one view both ways, so nothing reloads.
  // The store picture holds the place until the model is drawn, and stays
  // when there is none. The canvas exists only while this does; memory rule
  // "Cooked data is evictable" covers the Rust side.
  import { Box, Image as ImageIcon, Maximize2, Minimize2 } from "lucide-svelte";
  import { cubicInOut } from "svelte/easing";
  import type { ShipScene as DrawnScene, ShipView } from "@veelume/sc-render";
  import { ShipViewport } from "@veelume/sc-render/svelte";
  import { tauriSource } from "@veelume/sc-render/tauri";
  import type { ShipScene } from "$lib/bindings";
  import { shipsUi } from "$lib/state/ships.svelte";
  import { Turntable } from "./holoCamera";

  let {
    picture,
    name,
    detail = "",
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
    /** The nameplate on the card. */
    name: string;
    detail?: string;
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
  let sweeping = $state(false);
  let shows = 0;
  /** The picture covers the holo, which stays drawn underneath. */
  const showPicture = $derived(!!picture && (!drawn || shipsUi.picture));
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
        if (!drawn && picture && !shipsUi.picture && !calm()) sweeping = true;
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

  const calm = () => matchMedia("(prefers-reduced-motion: reduce)").matches;

  // Keep in step with `.scan`'s sweep below.
  const UNVEIL_MS = 520;

  /** The picture is wiped away from the top, uncovering the drawn holo. */
  function unveil(_: Element) {
    if (calm()) return { duration: 160, css: (t: number) => `opacity: ${t}` };
    return {
      duration: UNVEIL_MS,
      easing: cubicInOut,
      css: (t: number) => `clip-path: inset(${((1 - t) * 100).toFixed(2)}% 0 0 0)`,
    };
  }

  /** The picture coming back: the same edge, drawing it down over the holo. */
  function veil(_: Element) {
    if (calm()) return { duration: 160, css: (t: number) => `opacity: ${t}` };
    return {
      duration: UNVEIL_MS,
      easing: cubicInOut,
      css: (t: number) => `clip-path: inset(0 0 ${((1 - t) * 100).toFixed(2)}% 0)`,
    };
  }

  function choose(asPicture: boolean, e: MouseEvent) {
    e.stopPropagation();
    if (asPicture === shipsUi.picture) return;
    if (!calm()) sweeping = true;
    shipsUi.picture = asPicture;
  }

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
<div class="holo" class:open class:pictured={showPicture} class:projected={live && drawn && !showPicture} bind:this={box} onpointerdown={pressed} onclick={released}>
  {#if showPicture}<img class="shot" src={picture} alt="" in:veil out:unveil />{/if}
  {#if sweeping}<span class="scan" onanimationend={() => (sweeping = false)}></span>{/if}
  <!-- The view's own dark floor: on a transparent canvas the see-through
       hull layers up into a solid blob. -->
  {#if live}
    <div class="model" class:drawn>
      <ShipViewport source={tauriSource()} bind:view options={{ mode: "holo", background: true, grid: true }} {busy} />
    </div>
  {/if}
  {#if !open}
    <div class="frame"></div>
    <div class="plate">
      <span class="name">{name}</span>
      {#if detail}<span class="detail">{detail}</span>{/if}
    </div>
  {/if}
  <div class="actions">
    {#if live && drawn && picture}
      <div class="view" role="group" aria-label="Show">
        <button type="button" title="Picture" aria-label="Picture" aria-pressed={shipsUi.picture} onclick={(e) => choose(true, e)}>
          <ImageIcon size={13} />
        </button>
        <button type="button" title="Holo" aria-label="Holo" aria-pressed={!shipsUi.picture} onclick={(e) => choose(false, e)}>
          <Box size={13} />
        </button>
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
    transition: border-color 400ms var(--easing);
  }
  .holo.projected {
    border-color: color-mix(in srgb, var(--hue-sky) 32%, var(--border));
  }
  /* The card's own light over either view: the nameplate's ground below, and
   * the view's edges sunk into the card. */
  .holo:not(.open)::after {
    content: "";
    position: absolute;
    inset: 0;
    z-index: 2;
    pointer-events: none;
    background: linear-gradient(to top, color-mix(in srgb, var(--bg-sunk) 90%, transparent), transparent 42%);
    box-shadow: inset 0 0 48px color-mix(in srgb, var(--bg-sunk) 70%, transparent);
  }
  /* Corner marks: the same frame around the picture and the projection. */
  .frame {
    --mark: color-mix(in srgb, var(--text-strong) 38%, transparent);
    --long: 14px;
    position: absolute;
    inset: 10px;
    z-index: 3;
    pointer-events: none;
    background:
      linear-gradient(var(--mark), var(--mark)) top left / var(--long) 1px no-repeat,
      linear-gradient(var(--mark), var(--mark)) top left / 1px var(--long) no-repeat,
      linear-gradient(var(--mark), var(--mark)) top right / var(--long) 1px no-repeat,
      linear-gradient(var(--mark), var(--mark)) top right / 1px var(--long) no-repeat;
  }
  .projected .frame {
    --mark: color-mix(in srgb, var(--hue-sky) 70%, transparent);
  }
  .plate {
    position: absolute;
    left: 16px;
    bottom: 11px;
    z-index: 3;
    display: flex;
    flex-direction: column;
    gap: 1px;
    max-width: calc(100% - 140px);
    pointer-events: none;
  }
  .name {
    overflow: hidden;
    font-family: var(--font-display);
    font-size: 1.15rem;
    color: var(--text-strong);
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .detail {
    font-size: var(--text-small);
    color: var(--text-dim);
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
  /* Over the model, which is drawn underneath before the picture leaves. */
  .shot {
    position: absolute;
    inset: 0;
    z-index: 1;
    width: 100%;
    height: 100%;
    object-fit: cover;
    pointer-events: none;
  }
  /* Expanded, the picture is the page's background as the holo is, sinking
   * away behind the stats columns and the foot of the page. */
  .holo.open.pictured::after {
    content: "";
    position: absolute;
    inset: 0;
    z-index: 2;
    pointer-events: none;
    background:
      linear-gradient(
        to right,
        var(--bg-sunk) 8%,
        transparent 34%,
        transparent 66%,
        var(--bg-sunk) 92%
      ),
      linear-gradient(to top, var(--bg-sunk), transparent 40%);
  }
  /* The edge of the wipe: as long as the picture's `unveil`. */
  .scan {
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
    z-index: 2;
    height: 2px;
    background: var(--hue-sky);
    box-shadow: 0 0 14px 2px color-mix(in srgb, var(--hue-sky) 65%, transparent);
    pointer-events: none;
    animation: sweep 520ms cubic-bezier(0.65, 0, 0.35, 1) both;
  }
  @keyframes sweep {
    from {
      top: 0;
      opacity: 0;
    }
    12% {
      opacity: 1;
    }
    88% {
      opacity: 1;
    }
    to {
      top: 100%;
      opacity: 0;
    }
  }
  .model {
    position: absolute;
    inset: 0;
    opacity: 0;
    transition: opacity 120ms var(--easing);
  }
  .model.drawn {
    opacity: 1;
  }
  .more {
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
  .open .actions {
    right: 50%;
    transform: translateX(50%);
  }
  .actions {
    position: absolute;
    right: 10px;
    bottom: 10px;
    z-index: 3;
    display: flex;
    gap: 6px;
  }
  .view {
    display: inline-flex;
    padding: 2px;
    border-radius: var(--radius-chip);
    background: color-mix(in srgb, var(--bg-sunk) 70%, transparent);
  }
  .view button {
    display: inline-flex;
    padding: 2px 6px;
    border: 0;
    border-radius: var(--radius-chip);
    background: transparent;
    color: var(--text-faint);
    cursor: pointer;
    transition:
      color 160ms var(--easing),
      background 160ms var(--easing);
  }
  .view button:hover {
    color: var(--text-strong);
  }
  .view button[aria-pressed="true"] {
    background: var(--accent-fill);
    color: var(--text-strong);
  }
  .view button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
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
