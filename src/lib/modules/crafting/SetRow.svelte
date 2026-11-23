<script lang="ts">
  // An armour set as one row (UX board sketch 7): its pieces as words in
  // fixed places (a piece the set doesn't have is faint), how many
  // blueprints its colourways make and how many you own. The colourways
  // themselves open in the sheet. Its columns end like an item's row: what
  // you own, whether it is in Plans, the time (Valerie, 2026-11-23).
  import { Bookmark, Check, Shield, Target } from "lucide-svelte";
  import { plansStore } from "$lib/state/plans.svelte";
  import { PIECES, PIECE_SHORT, PIECE_TINY, type CraftSet } from "./browse";
  import { fmtTime } from "./model";

  let { set, selected, onclick }: { set: CraftSet; selected: boolean; onclick: () => void } = $props();

  const marks = $derived(set.families.flatMap((f) => f.variants.map((v) => plansStore.markOf(v.guid))));
  const time = $derived(
    set.minTime == null
      ? "—"
      : set.minTime === set.maxTime
        ? fmtTime(set.minTime)
        : `${fmtTime(set.minTime)}–${fmtTime(set.maxTime)}`,
  );
</script>

<button type="button" class="row" class:on={selected} {onclick}>
  <span class="tile"><Shield size={15} /></span>
  <span class="name"
    >{set.name}<span class="n" title="{set.blueprints} blueprints across its colourways">×{set.blueprints}</span></span
  >
  <span class="pieces">
    {#each PIECES as p (p)}
      {@const fams = set.byPiece.get(p)}
      {@const own = (fams ?? []).some((f) => f.ownedCount > 0)}
      <span class="piece" class:no={!fams} class:own title={PIECE_SHORT[p]}>{PIECE_TINY[p]}</span>
    {/each}
  </span>
  <span class="mats">
    {#each set.materials as m (m)}<span class="mat">{m}</span>{/each}
  </span>
  <span class="owned">
    {#if set.owned}<Check size={12} />{set.owned}/{set.blueprints}{/if}
  </span>
  <span class="marks">
    {#if marks.includes("planned")}<span class="planned" title="In your plans"><Target size={12} /></span>
    {:else if marks.includes("bookmarked")}<span class="marked" title="Bookmarked in Plans"><Bookmark size={12} /></span>{/if}
  </span>
  <span class="time">{time}</span>
</button>

<style>
  .row {
    display: grid;
    /* Narrow (the sheet open): the materials give way; the name and the
       pieces keep their room. */
    /* The last three columns are an item row's (FamilyRow), so owned,
       marks and time line up down the whole list. */
    grid-template-columns: 26px minmax(110px, 1fr) minmax(196px, 260px) minmax(0, 1.2fr) 56px 46px 72px;
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
    color: color-mix(in srgb, var(--hue-sky) 80%, var(--text-strong));
    background: color-mix(in srgb, var(--hue-sky) 16%, var(--bg-sunk));
  }
  .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: var(--weight-semibold);
    color: var(--text-strong);
  }
  .pieces {
    display: grid;
    grid-template-columns: repeat(6, minmax(0, 1fr));
    overflow: hidden;
    font-size: var(--text-small);
    color: var(--text-dim);
    white-space: nowrap;
  }
  .piece.no {
    color: color-mix(in srgb, var(--text-faint) 45%, transparent);
  }
  .piece.own {
    color: var(--good);
  }
  .n {
    margin-left: 6px;
    font-family: var(--font-mono);
    font-size: var(--text-small);
    font-weight: 400;
    color: var(--text-faint);
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
  }
  .planned {
    color: var(--accent);
  }
  .marked {
    color: var(--hue-sand);
  }
  .time {
    font-family: var(--font-mono);
    font-size: var(--text-small);
    color: var(--text-faint);
    text-align: right;
    white-space: nowrap;
  }
  .mat + .mat {
    margin-left: 10px;
  }
  .mats {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--text-small);
    color: var(--text-dim);
  }
</style>
