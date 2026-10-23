<script lang="ts">
  // A ship as a card: the art is its store picture when one is on disk,
  // else a gradient in its career's hue with the career's glyph, then
  // the maker, the name, the role and a few figures. `note` marks the card
  // (owned ×2, rented, new in 4.10.2).
  import type { ShipRowView } from "$lib/bindings";
  import { shipsImages } from "$lib/state/ships.svelte";
  import { careerLook } from "./careers";
  import { crewTitle } from "./seats";

  let {
    ship,
    note = "",
    tone = "",
    compact = false,
    onopen,
  }: {
    ship: ShipRowView;
    note?: string;
    tone?: "" | "new" | "rent";
    compact?: boolean;
    onopen: (guid: string) => void;
  } = $props();

  const look = $derived(careerLook(ship.career));
  const Icon = $derived(look.icon);
  const image = $derived(shipsImages.byGuid.get(ship.guid) ?? null);
  const short = $derived.by(() => {
    const m = ship.manufacturer?.split(" ")[0];
    return m && ship.name.startsWith(`${m} `) ? ship.name.slice(m.length + 1) : ship.name;
  });
</script>

<button type="button" class="tile" class:compact style:--hue={look.hue} onclick={() => onopen(ship.guid)}>
  <span class="art" aria-hidden="true">
    {#if image}
      <img src={image} alt="" loading="lazy" />
    {:else}
      <span class="glyph"><Icon size={compact ? 22 : 34} /></span>
    {/if}
    {#if note}<span class="note" class:new={tone === "new"} class:rent={tone === "rent"}>{note}</span>{/if}
  </span>
  <span class="body">
    <span class="maker">{ship.manufacturerCode ?? ship.manufacturer ?? ""}</span>
    <b>{short}</b>
    {#if !compact}
      <span class="role">{ship.role ?? ship.career ?? ""}</span>
      <span class="figs">
        <span title={crewTitle(ship.seats)}><i>crew</i>{ship.seats.crew}</span>
        {#if ship.cargo}<span><i>cargo</i>{ship.cargo.toLocaleString("en-US")} SCU</span>{/if}
      </span>
    {/if}
  </span>
</button>

<style>
  .tile {
    all: unset;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    min-width: 0;
    border: 1px solid color-mix(in srgb, var(--hue) 20%, var(--border));
    border-radius: var(--radius-lg);
    overflow: hidden;
    background: color-mix(in srgb, var(--bg-raised) 60%, transparent);
    cursor: pointer;
    transition: border-color var(--transition-fast);
  }
  .tile:hover {
    border-color: color-mix(in srgb, var(--hue) 55%, var(--border));
  }
  .art {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    /* The store pictures' own proportions (351 × 210): nothing cropped. */
    aspect-ratio: 5 / 3;
    background:
      radial-gradient(120% 140% at 80% 0%, color-mix(in srgb, var(--hue) 34%, transparent), transparent 60%),
      linear-gradient(160deg, color-mix(in srgb, var(--hue) 16%, var(--bg-sunk)), var(--bg-sunk) 75%);
  }
  .compact .art {
    aspect-ratio: auto;
    height: 58px;
  }
  img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .glyph {
    color: color-mix(in srgb, var(--hue) 70%, var(--text-strong));
    opacity: 0.75;
  }
  .note {
    position: absolute;
    top: 8px;
    right: 8px;
    font: 600 0.62rem var(--font-mono);
    letter-spacing: 0.05em;
    text-transform: uppercase;
    padding: 1px 7px;
    border-radius: var(--radius-pill);
    color: var(--text-strong);
    background: color-mix(in srgb, var(--bg-sunk) 70%, transparent);
    border: 1px solid color-mix(in srgb, var(--hue) 40%, transparent);
  }
  .note.new {
    color: var(--on-accent);
    background: var(--accent);
    border-color: transparent;
  }
  .note.rent {
    color: var(--warn);
    border-color: color-mix(in srgb, var(--warn) 45%, transparent);
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 9px 12px 11px;
    min-width: 0;
  }
  .maker {
    font: 700 0.66rem var(--font-mono);
    letter-spacing: 0.06em;
    color: var(--hue-sand);
  }
  b {
    font-family: var(--font-display);
    font-size: 1.05rem;
    line-height: 1.2;
    color: var(--text-strong);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .compact b {
    font-size: 0.92rem;
  }
  .role {
    font-size: var(--text-small);
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .figs {
    display: flex;
    gap: 12px;
    margin-top: 4px;
    font: 0.74rem var(--font-mono);
    color: var(--text-strong);
  }
  .figs i {
    margin-right: 4px;
    font-style: normal;
    font-family: var(--font-sans);
    font-size: 0.66rem;
    color: var(--text-faint);
  }
</style>
