<script lang="ts">
  // A ship as a card (erkul's and ccugame's catalogue cards as references):
  // the store picture with the name on it (else a gradient in the career's
  // hue with its glyph), then the landing pad, the maker, CIG's career and
  // role, crew and cargo, and what it costs: in game the cheapest shop
  // (UEX), else the cheapest rental, and the pledge store's price beside
  // it. `note` marks the card (owned ×2, rented, new in 4.10.2).
  import type { ShipRowView } from "$lib/bindings";
  import { shipsImages, shipsMarket } from "$lib/state/ships.svelte";
  import { careerLook } from "./careers";
  import { crewTitle } from "./seats";

  let {
    ship,
    note = "",
    tone = "",
    onopen,
  }: {
    ship: ShipRowView;
    note?: string;
    tone?: "" | "new" | "rent";
    onopen: (guid: string) => void;
  } = $props();

  const look = $derived(careerLook(ship.career));
  const Icon = $derived(look.icon);
  const image = $derived(shipsImages.byGuid.get(ship.guid) ?? null);
  const market = $derived(shipsMarket.of(ship.guid));
  const short = $derived.by(() => {
    const m = ship.manufacturer?.split(" ")[0];
    return m && ship.name.startsWith(`${m} `) ? ship.name.slice(m.length + 1) : ship.name;
  });
  const kind = $derived([ship.career, ship.role].filter(Boolean).join(" · "));

  const aUEC = (v: number) => Math.round(v).toLocaleString("en-US");
  /** "New Deal Lorville" at "Lorville · Hurston · Stanton": the terminal
   *  already says the city, so only what it doesn't. */
  const from = (o: { terminal: string; place: string }) => {
    const site = o.place.split(" · ")[0] ?? "";
    return site && !o.terminal.includes(site) ? `${o.terminal} · ${site}` : o.terminal;
  };
</script>

<button type="button" class="tile" style:--hue={look.hue} onclick={() => onopen(ship.guid)}>
  <span class="art" aria-hidden="true">
    {#if image}
      <img src={image} alt="" loading="lazy" />
    {:else}
      <span class="glyph"><Icon size={34} /></span>
    {/if}
    {#if note}<span class="note" class:new={tone === "new"} class:rent={tone === "rent"}>{note}</span>{/if}
    <span class="title"><b>{short}</b></span>
  </span>
  <span class="body">
    <span class="who">
      {#if market?.pad}<span class="pad" title="Lands on a {market.pad} pad">{market.pad}</span>{/if}
      <span class="maker">{ship.manufacturer ?? ship.manufacturerCode ?? ""}</span>
    </span>
    {#if kind}<span class="kind">{kind}</span>{/if}
    <span class="figs">
      <span title={crewTitle(ship.seats)}><i>crew</i>{ship.seats.crew}</span>
      {#if ship.cargo}<span><i>cargo</i>{ship.cargo.toLocaleString("en-US")} SCU</span>{/if}
    </span>
    <span class="cost">
      <span class="game">
        {#if market?.buy}
          <span class="price"><b>{aUEC(market.buy.price)}</b><small>aUEC</small></span>
          <span class="from">from {from(market.buy)}</span>
        {:else if market?.rent}
          <span class="price"><b>{aUEC(market.rent.price)}</b><small>aUEC / day</small></span>
          <span class="from">rent only, from {from(market.rent)}</span>
        {:else}
          <span class="none">Not sold in game</span>
        {/if}
      </span>
      <span class="side">
        {#if market?.buyShops}
          <span class="avail" title="Shops UEX lists selling it">{market.buyShops} {market.buyShops === 1 ? "shop" : "shops"}</span>
        {:else if market?.rentShops}
          <span class="avail rent" title="Shops UEX lists renting it">{market.rentShops} to rent</span>
        {/if}
        {#if market?.storeUsd}<span class="usd" title="Standalone price on the pledge store">${market.storeUsd.toLocaleString("en-US")}</span>{/if}
      </span>
    </span>
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
  /* The name on the picture, over a shade that keeps it readable on any. */
  .title {
    position: absolute;
    inset: auto 0 0 0;
    padding: 28px 12px 8px;
    background: linear-gradient(to top, color-mix(in srgb, var(--bg-sunk) 88%, transparent), transparent);
  }
  .title b {
    display: block;
    font-family: var(--font-display);
    font-size: 1.12rem;
    line-height: 1.2;
    color: var(--text-strong);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    text-shadow: 0 1px 3px color-mix(in srgb, var(--bg-sunk) 80%, transparent);
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
    gap: 3px;
    padding: 9px 12px 11px;
    min-width: 0;
  }
  .who {
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
  }
  .pad {
    flex: none;
    min-width: 22px;
    padding: 0 4px;
    border-radius: 4px;
    font: 700 0.66rem/1.5 var(--font-mono);
    text-align: center;
    color: var(--hue-sand);
    border: 1px solid color-mix(in srgb, var(--hue-sand) 45%, transparent);
  }
  .maker {
    font-size: var(--text-small);
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .kind {
    font-size: 0.74rem;
    color: var(--text-faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .figs {
    display: flex;
    gap: 12px;
    font: 0.74rem var(--font-mono);
    color: var(--text-dim);
  }
  .figs i {
    margin-right: 4px;
    font-style: normal;
    font-family: var(--font-sans);
    font-size: 0.66rem;
    color: var(--text-faint);
  }
  /* What it costs: in game on the left, the one bright figure; how many
   * shops and the store's dollars on the right, quieter. */
  .cost {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 10px;
    margin-top: 6px;
    padding-top: 8px;
    border-top: 1px solid color-mix(in srgb, var(--text) 7%, transparent);
  }
  .game {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .price b {
    font: 600 1.05rem var(--font-mono);
    color: var(--text-strong);
  }
  .price small {
    margin-left: 4px;
    font-size: 0.66rem;
    color: var(--text-faint);
  }
  .from {
    font-size: 0.7rem;
    color: var(--text-faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .none {
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .side {
    flex: none;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 2px;
    font-size: 0.7rem;
    color: var(--text-dim);
  }
  .avail::before {
    content: "●";
    margin-right: 5px;
    color: var(--good);
  }
  .avail.rent::before {
    color: var(--warn);
  }
  .usd {
    font: 0.74rem var(--font-mono);
    color: var(--text-dim);
  }
</style>
