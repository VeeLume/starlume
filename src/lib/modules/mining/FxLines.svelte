<script lang="ts">
  // A part's modifiers in words, strongest first: green where they help
  // cracking, red where they hurt. Stacked in a tile, inline in a title.
  import { FX_COLUMNS, helps, signed, type FxKey } from "./format";

  let {
    fx,
    stacked = false,
    none = "no modifiers",
  }: { fx: Record<FxKey, number>; stacked?: boolean; none?: string } = $props();

  const lines = $derived(
    FX_COLUMNS.filter((c) => fx[c.key] !== 0)
      .map((c) => ({ key: c.key, text: `${c.word} ${signed(fx[c.key])}%`, good: helps(c.key, fx[c.key]), size: Math.abs(fx[c.key]) }))
      .sort((a, b) => b.size - a.size),
  );
</script>

<span class="fx" class:stacked>
  {#each lines as l (l.key)}<span class:good={l.good} class:bad={!l.good}>{l.text}</span>{:else}<span class="none"
      >{none}</span
    >{/each}
</span>

<style>
  .fx {
    display: inline-flex;
    flex-wrap: wrap;
    align-items: baseline;
    min-width: 0;
  }
  .fx > span {
    white-space: nowrap;
  }
  .fx:not(.stacked) > span + span::before {
    content: "·";
    margin: 0 5px;
    color: var(--text-faint);
  }
  .stacked {
    flex-direction: column;
    align-items: inherit;
  }
  .good {
    color: color-mix(in srgb, var(--good) 85%, var(--text));
  }
  .bad {
    color: color-mix(in srgb, var(--bad) 85%, var(--text));
  }
  .none {
    color: var(--text-faint);
  }
</style>
