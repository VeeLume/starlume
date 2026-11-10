<script lang="ts">
  // One item in the catalog, its colourways folded in: a tinted tile, the
  // name, what it takes and whether you hold it, where you stand with it.
  import { craftingStore } from "$lib/state/crafting.svelte";
  import { classText, fmtTime, readiness, type Family, type Readiness } from "./model";
  import { lookOf } from "./shelf";

  let { family, selected, onclick }: { family: Family; selected: boolean; onclick: () => void } = $props();

  const look = $derived(lookOf(family.shelf.main));
  const Icon = $derived(look.icon);
  const rep = $derived(family.variants[0]);
  const sized = $derived(family.shelf.sub.startsWith("Size"));
  const n = $derived(family.variants.length);

  const DOT: Record<Readiness, string> = { good: "var(--good)", short: "var(--warn)", none: "var(--text-faint)" };
  const WORD: Record<Readiness, string> = { good: "you have enough", short: "you have too little", none: "you have none" };
  const stateOf = (m: (typeof rep.materials)[number]) => readiness(craftingStore.stock.get(m.name), m, m.minQuality);
</script>

<button type="button" class="card" class:on={selected} style="--hue: {look.hue}" {onclick}>
  <span class="tile"><Icon size={20} /></span>
  <span class="body">
    <span class="line">
      <span class="name">{family.title}</span>
      {#if n > 1}<span class="variants">{n} variants</span>{/if}
      <span class="facts">
        {#if family.maker}<span class="maker" title={rep.manufacturer}>{family.maker}</span>{/if}
        {#if sized}<span>S{rep.size}</span>{/if}
        {#if classText(rep)}<span>{classText(rep)}</span>{/if}
        <span class="time">{fmtTime(rep.craftSeconds)}</span>
      </span>
    </span>
    <span class="line">
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
      <span class="marks">
        {#if family.ownedCount > 0}
          <span class="hue-chip" style="--hue: var(--good)">{n > 1 ? `${family.ownedCount} of ${n} owned` : "owned"}</span>
        {/if}
        {#if family.wantBlueprint}<span class="hue-chip" style="--hue: var(--hue-sky)">wants blueprint</span>{/if}
        {#if family.wantItem}<span class="hue-chip" style="--hue: var(--hue-rose)">wants item</span>{/if}
        {#if family.craftable}<span class="hue-chip" style="--hue: var(--hue-teal)">craftable</span>{/if}
      </span>
    </span>
  </span>
</button>

<style>
  .card {
    all: unset;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 12px;
    width: calc(100% - 12px);
    margin: 0 6px 6px;
    padding: 9px 12px 9px 9px;
    border: 1px solid color-mix(in srgb, var(--hue) 16%, var(--border));
    border-radius: var(--radius-lg);
    background: linear-gradient(100deg, color-mix(in srgb, var(--hue) 9%, var(--bg-raised)), var(--bg-raised) 42%);
    cursor: pointer;
    transition: border-color var(--transition-fast);
  }
  .card:hover {
    border-color: color-mix(in srgb, var(--hue) 50%, var(--border));
  }
  .card:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  /* Selection is violet: the one colour that means "you did this". */
  .card.on {
    border-color: color-mix(in srgb, var(--accent) 70%, var(--border));
    background: linear-gradient(100deg, color-mix(in srgb, var(--accent) 16%, var(--bg-raised)), color-mix(in srgb, var(--accent) 6%, var(--bg-raised)) 60%);
  }
  .tile {
    flex: 0 0 auto;
    display: grid;
    place-items: center;
    width: 46px;
    height: 46px;
    border-radius: var(--radius);
    border: 1px solid color-mix(in srgb, var(--hue) 28%, transparent);
    color: color-mix(in srgb, var(--hue) 75%, var(--text-strong));
    background:
      radial-gradient(120% 120% at 75% 10%, color-mix(in srgb, var(--hue) 30%, transparent), transparent 65%),
      var(--bg-sunk);
  }
  .body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .line {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
    min-height: 18px;
  }
  .name {
    font-family: var(--font-display);
    font-size: 1.04rem;
    font-weight: var(--weight-bold);
    line-height: 1.15;
    color: var(--text-strong);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .variants {
    flex: 0 0 auto;
    font-family: var(--font-mono);
    font-size: var(--text-badge);
    color: var(--text-faint);
  }
  .mats {
    flex: 1;
    display: flex;
    gap: 10px;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    /* Long recipes fade out under the chips instead of cutting a word. */
    mask-image: linear-gradient(to right, black calc(100% - 18px), transparent);
    font-size: var(--text-small);
    color: var(--text-dim);
  }
  .mat {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .mat i {
    width: 6px;
    height: 6px;
    border-radius: 50%;
  }
  .facts {
    flex: 0 0 auto;
    margin-left: auto;
    display: flex;
    align-items: baseline;
    gap: 9px;
    font-size: var(--text-small);
    color: var(--text-dim);
    white-space: nowrap;
  }
  .maker {
    max-width: 90px;
    overflow: hidden;
    text-overflow: ellipsis;
    font-family: var(--font-mono);
    font-weight: var(--weight-medium);
    color: var(--hue-sand);
  }
  .time {
    font-family: var(--font-mono);
    color: var(--text-faint);
  }
  .marks {
    flex: 0 0 auto;
    display: flex;
    gap: 4px;
  }
</style>
