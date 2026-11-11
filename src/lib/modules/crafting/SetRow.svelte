<script lang="ts">
  // An armour set as one row (UX board sketch 7): its pieces as words in
  // fixed places (a piece the set doesn't have is faint), how many
  // blueprints its colourways make and how many you own. The colourways
  // themselves open in the sheet.
  import { Shield } from "lucide-svelte";
  import { PIECES, PIECE_SHORT, PIECE_TINY, type CraftSet } from "./browse";
  import { fmtTime } from "./model";

  let { set, selected, onclick }: { set: CraftSet; selected: boolean; onclick: () => void } = $props();

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
  <span class="name">{set.name}</span>
  <span class="pieces">
    {#each PIECES as p (p)}
      {@const fams = set.byPiece.get(p)}
      {@const own = (fams ?? []).some((f) => f.ownedCount > 0)}
      <span class="piece" class:no={!fams} class:own title={PIECE_SHORT[p]}>{PIECE_TINY[p]}</span>
    {/each}
  </span>
  <span class="n" title="{set.blueprints} blueprints across its colourways">{set.blueprints}</span>
  <span class="owned">{set.owned ? `${set.owned} of ${set.blueprints}` : ""}</span>
  <span class="mats">{set.materials.join(" · ")}</span>
  <span class="time">{time}</span>
</button>

<style>
  .row {
    display: grid;
    /* Narrow (the sheet open): the materials give way; the name and the
       pieces keep their room. */
    grid-template-columns: 26px minmax(96px, 1fr) minmax(196px, 260px) 28px 62px minmax(0, 0.9fr) 64px;
    gap: 10px;
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
  .n,
  .owned,
  .time {
    font-family: var(--font-mono);
    font-size: var(--text-small);
    color: var(--text-dim);
    text-align: right;
  }
  .owned {
    color: var(--good);
  }
  .time {
    color: var(--text-faint);
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
