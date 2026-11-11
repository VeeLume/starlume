<script lang="ts">
  // One item in the catalog as one dense row (UX board sketch 4: "a bit more
  // density without too much compression"): its colourways folded in, what
  // it takes and whether you hold it, where you stand with it.
  import { Check, Flag, Hammer, Heart } from "lucide-svelte";
  import { craftingStore } from "$lib/state/crafting.svelte";
  import { classText, fmtTime, readiness, type Family, type Readiness } from "./model";
  import { lookOf } from "./shelf";

  let { family, selected, onclick }: { family: Family; selected: boolean; onclick: () => void } = $props();

  const look = $derived(lookOf(family.shelf.main));
  const Icon = $derived(look.icon);
  const rep = $derived(family.variants[0]);
  const n = $derived(family.variants.length);
  const facts = $derived(
    [family.maker, family.shelf.sized ? `S${rep.size}` : "", classText(rep)].filter(Boolean).join(" · "),
  );

  const DOT: Record<Readiness, string> = { good: "var(--good)", short: "var(--warn)", none: "var(--text-faint)" };
  const WORD: Record<Readiness, string> = { good: "you have enough", short: "you have too little", none: "you have none" };
  const stateOf = (m: (typeof rep.materials)[number]) => readiness(craftingStore.stock.get(m.name), m, m.minQuality);
</script>

<button type="button" class="row" class:on={selected} style="--hue: {look.hue}" {onclick}>
  <span class="tile"><Icon size={15} /></span>
  <span class="name">{family.title}{#if n > 1}<span class="n" title="{n} colourways">×{n}</span>{/if}</span>
  <span class="facts" title={rep.manufacturer ?? ""}>{facts}</span>
  <span class="mats">
    {#each rep.materials as m, i (i)}
      {#if craftingStore.stockRead}
        {@const s = stateOf(m)}
        <span class="mat" title="{m.name}: {WORD[s]}"><i style="background: {DOT[s]}"></i>{m.name}</span>
      {:else}
        <span class="mat">{m.name}</span>
      {/if}
    {/each}
  </span>
  <span class="owned">
    {#if family.ownedCount > 0}
      <Check size={12} />{n > 1 ? `${family.ownedCount}/${n}` : "owned"}
    {/if}
  </span>
  <span class="marks">
    {#if family.wantBlueprint}<span title="You want this blueprint"><Flag size={12} /></span>{/if}
    {#if family.wantItem}<span title="You want this item"><Heart size={12} /></span>{/if}
    {#if family.craftable}<span class="craft" title="You can craft this now"><Hammer size={12} /></span>{/if}
  </span>
  <span class="time">{fmtTime(rep.craftSeconds)}</span>
</button>

<style>
  .row {
    display: grid;
    /* Narrow (the sheet open): maker and materials give way, the name
       keeps its room. */
    grid-template-columns: 26px minmax(130px, 1.4fr) minmax(0, 0.9fr) minmax(0, 1.5fr) 56px 46px 44px;
    gap: 12px;
    align-items: center;
    width: 100%;
    min-height: 36px;
    padding: 4px 10px;
    border: 0;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 55%, transparent);
    border-radius: 0;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: var(--text-control);
    text-align: left;
    cursor: pointer;
  }
  .row:hover {
    background: color-mix(in srgb, var(--text) 4%, transparent);
  }
  .row.on {
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .tile {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: var(--radius);
    color: color-mix(in srgb, var(--hue) 80%, var(--text-strong));
    background: color-mix(in srgb, var(--hue) 16%, var(--bg-sunk));
  }
  .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: var(--weight-semibold);
    color: var(--text-strong);
  }
  .n {
    margin-left: 6px;
    font-family: var(--font-mono);
    font-size: var(--text-small);
    font-weight: 400;
    color: var(--text-faint);
  }
  .facts,
  .mats {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--text-small);
    color: var(--text-dim);
  }
  .mat + .mat {
    margin-left: 10px;
  }
  .mat i {
    display: inline-block;
    width: 6px;
    height: 6px;
    margin-right: 5px;
    border-radius: 50%;
    vertical-align: 1px;
  }
  .owned {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-family: var(--font-mono);
    font-size: var(--text-small);
    color: var(--good);
  }
  .marks {
    display: inline-flex;
    gap: 6px;
    color: var(--text-dim);
  }
  .craft {
    color: var(--hue-teal);
  }
  .time {
    font-family: var(--font-mono);
    font-size: var(--text-small);
    color: var(--text-faint);
    text-align: right;
  }
</style>
