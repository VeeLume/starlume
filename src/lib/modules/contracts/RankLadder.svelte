<script lang="ts">
  // A faction's rank ladder: one cell per rank, the mission's window filled,
  // your rank ringed. `named` adds each rank's name and the rep it starts
  // at under its cell, and a "you" tag over yours (the bare ladder was hard
  // to read at first). Without your rank the window stays grey; it turns
  // green when you fit.
  import type { Ladder } from "./model";

  let {
    ladder,
    you = -1,
    window = true,
    named = false,
    counts = null,
    label = "",
  }: {
    ladder: Ladder;
    /** Your rank index, -1 when not known. */
    you?: number;
    /** Fill the mission's window (lo..hi); off for a faction's own ladder. */
    window?: boolean;
    named?: boolean;
    /** Per rank, a figure under the name instead of the rep (missions per rank). */
    counts?: string[] | null;
    label?: string;
  } = $props();

  const fits = $derived(you >= ladder.lo && you <= ladder.hi);
  const fmt = (n: number) => n.toLocaleString("en-US");
</script>

{#if named}
  <div class="named" role="img" aria-label={label}>
    {#each ladder.ranks as name, i (i)}
      <div class:in={window ? i >= ladder.lo && i <= ladder.hi : i <= you} class:you={i === you}>
        <i></i><span>{name}</span><small>{counts ? counts[i] : fmt(ladder.reps[i])}</small>
      </div>
    {/each}
  </div>
{:else}
  <span class="row" role="img" aria-label={label}>
    {#each ladder.ranks as _, i (i)}<i
        class:in={window ? i >= ladder.lo && i <= ladder.hi : i <= you}
        class:ok={window ? fits : true}
        class:you={i === you}
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
  .named .you i {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
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
