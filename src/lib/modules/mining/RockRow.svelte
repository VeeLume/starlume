<script lang="ts">
  // One rock row in the finder's cards: the rock and where (or its share),
  // with a loadout the ship's class split and good share, and under it the
  // mass range on the shared scale. The ore card and the place card list the
  // same rows, so a rock reads the same wherever it shows.
  import type { ClassSharesView, MassRangeView } from "$lib/bindings";
  import ClassBar from "./ClassBar.svelte";
  import MassSpan from "./MassSpan.svelte";

  let {
    name,
    where = "",
    figure,
    figureTitle = undefined,
    classes,
    withClasses,
    mass,
    on,
    disabled = false,
    title = undefined,
    onclick,
  }: {
    name: string;
    /** Its places, in the ore card. */
    where?: string;
    /** Its share there, or the ore's share of it. */
    figure: string;
    figureTitle?: string;
    classes: ClassSharesView | null;
    /** The card shows the ship's classes (a loadout with a laser). */
    withClasses: boolean;
    mass: MassRangeView | null;
    on: boolean;
    disabled?: boolean;
    title?: string;
    onclick: () => void;
  } = $props();

  const good = $derived(classes ? Math.round((classes.easy + classes.manageable) * 100) : null);
</script>

<button class="rk" class:cls={withClasses} class:on {disabled} {title} {onclick}>
  <span class="nm"><b>{name}</b>{#if where}<span>{where}</span>{/if}</span>
  <span class="n" title={figureTitle}>{figure}</span>
  {#if withClasses}
    {#if classes}<ClassBar shares={classes} />{:else}<span></span>{/if}
    <span class="n g" class:w={good !== null && good < 65} class:b={good !== null && good < 40}>{good ?? "—"}</span>
  {/if}
  {#if mass}<span class="mrow"><MassSpan {mass} /></span>{/if}
</button>

<style>
  .rk {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 54px;
    gap: 4px 12px;
    align-items: center;
    padding: 8px;
    margin-inline: -8px;
    border: 0;
    border-top: 1px solid color-mix(in srgb, var(--text) 7%, transparent);
    border-radius: 6px;
    background: none;
    font: inherit;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .rk.cls {
    grid-template-columns: minmax(0, 1fr) 54px minmax(70px, 1fr) 30px;
  }
  :global(.rk:first-child) {
    border-top-color: transparent;
  }
  .rk:hover:not(:disabled) {
    background: color-mix(in srgb, var(--text) 4%, transparent);
  }
  .rk.on {
    background: var(--accent-fill);
  }
  .rk:disabled {
    color: var(--text-faint);
    cursor: default;
  }
  .nm {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
  }
  .nm b {
    font-size: 0.92rem;
    font-weight: var(--weight-semibold);
    white-space: nowrap;
  }
  .nm span {
    min-width: 0;
    font-size: var(--text-small);
    color: var(--text-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .n {
    font-family: var(--font-mono);
    font-size: 0.8rem;
    font-variant-numeric: tabular-nums;
    text-align: right;
    color: var(--text-dim);
  }
  .n.g {
    color: var(--good);
    font-weight: var(--weight-semibold);
  }
  .n.g.w {
    color: var(--warn);
  }
  .n.g.b {
    color: var(--bad);
  }
  .mrow {
    grid-column: 1 / -1;
  }
</style>
