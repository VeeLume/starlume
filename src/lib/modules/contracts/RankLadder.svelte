<script lang="ts">
  // A faction's rank ladder: one cell per rank. With `window` (a mission's
  // ladder) the mission's window is filled and your rank ringed; the window
  // stays grey until your rank is known and turns green when you fit.
  // Without it (a faction's own ladder) the ladder is a progress bar: ranks
  // you passed fill, yours fills by how far you are toward the next, and no
  // ring (it only repeated the fill). `named` adds each rank's name and the
  // rep it starts at under its cell, and a "you" tag over yours.
  import type { Ladder } from "./model";

  let {
    ladder,
    you = -1,
    score = null,
    window = true,
    named = false,
    counts = null,
    label = "",
  }: {
    ladder: Ladder;
    /** Your rank index, -1 when not known. */
    you?: number;
    /** Your score: fills your rank's cell on a faction's own ladder. */
    score?: number | null;
    /** Fill the mission's window (lo..hi); off for a faction's own ladder. */
    window?: boolean;
    named?: boolean;
    /** Per rank, a figure under the name instead of the rep (missions per rank). */
    counts?: string[] | null;
    label?: string;
  } = $props();

  const fits = $derived(you >= ladder.lo && you <= ladder.hi);
  const fmt = (n: number) => n.toLocaleString("en-US");
  /** How far into your rank you are, 0..1; full at the top or without a score. */
  const part = $derived.by(() => {
    if (you < 0 || score == null) return 1;
    const lo = ladder.reps[you];
    const hi = ladder.reps[you + 1];
    if (hi == null || hi <= lo) return 1;
    return Math.min(1, Math.max(0, (score - lo) / (hi - lo)));
  });
  const fill = (i: number) => (window ? undefined : i < you ? "100%" : i === you ? `${part * 100}%` : "0%");
</script>

{#if named}
  <div class="named" class:own={!window} role="img" aria-label={label}>
    {#each ladder.ranks as name, i (i)}
      <div class:in={window && i >= ladder.lo && i <= ladder.hi} class:passed={!window && i < you} class:you={i === you} style:--p={fill(i)}>
        <i></i><span>{name}</span><small>{counts ? counts[i] : fmt(ladder.reps[i])}</small>
      </div>
    {/each}
  </div>
{:else}
  <span class="row" class:own={!window} role="img" aria-label={label}>
    {#each ladder.ranks as _, i (i)}<i
        class:in={window && i >= ladder.lo && i <= ladder.hi}
        class:ok={window && fits}
        class:you={window && i === you}
        class:passed={!window && i < you}
        style:--p={fill(i)}
      ></i>{/each}
  </span>
{/if}

<style>
  .row,
  .named {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: minmax(0, 1fr);
  }
  .row {
    gap: 2px;
    width: 112px;
  }
  .named {
    gap: 3px;
    padding-top: 16px;
  }
  i {
    display: block;
    height: 8px;
    border-radius: 2px;
    background: var(--bg-sunk);
    border: 1px solid var(--border);
    box-sizing: border-box;
  }
  .named i {
    height: 12px;
  }
  /* The window is grey until your rank is known; green when you fit. */
  .row i.in {
    background: color-mix(in srgb, var(--text-dim) 30%, transparent);
    border-color: color-mix(in srgb, var(--text-dim) 40%, transparent);
  }
  .row i.in.ok,
  .named .in i {
    background: color-mix(in srgb, var(--good) 40%, transparent);
    border-color: color-mix(in srgb, var(--good) 50%, transparent);
  }
  .row i.you,
  .named:not(.own) .you i {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  /* A faction's own ladder: a progress bar, --p of each cell filled. */
  .own i {
    background: linear-gradient(
      to right,
      color-mix(in srgb, var(--good) 40%, transparent) var(--p),
      var(--bg-sunk) var(--p)
    );
  }
  .row.own i.passed,
  .named.own .passed i {
    border-color: color-mix(in srgb, var(--good) 50%, transparent);
  }
  .named div {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 5px;
    min-width: 0;
  }
  .named .you::before {
    content: "you";
    position: absolute;
    top: -16px;
    left: 0;
    font-size: 0.68rem;
    font-weight: 600;
    color: var(--accent);
  }
  .named span {
    font-size: 0.68rem;
    line-height: 1.2;
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .named .you span {
    color: var(--text-strong);
    font-weight: 600;
  }
  .named small {
    font-size: 0.68rem;
    color: var(--text-faint);
    font-variant-numeric: tabular-nums;
  }
</style>
