<script lang="ts">
  // A crack share as a short bar and its number, with the change against
  // what it would replace. `adds` reads it as what a crew mate adds ("+38").
  // `null`: can't break; `undefined`: still being worked out.
  let {
    value,
    delta = null,
    adds = false,
  }: {
    /** 0..100. */
    value: number | null | undefined;
    delta?: number | null;
    adds?: boolean;
  } = $props();
</script>

<span class="ck">
  <span class="bar"><i style:width="{Math.max(0, Math.min(100, value ?? 0))}%"></i></span>
  <span class="n">
    {#if value === undefined}<span class="faint">…</span>
    {:else if value === null}<span class="faint">—</span>
    {:else}{adds ? `+${value}` : value}{#if delta}<small class:up={delta > 0} class:dn={delta < 0}
          >{delta > 0 ? "+" : "−"}{Math.abs(delta)}</small
        >{/if}{/if}
  </span>
</span>

<style>
  .ck {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 56px;
    gap: 6px;
    align-items: center;
    min-width: 0;
  }
  .bar {
    display: block;
    height: 6px;
    border-radius: 3px;
    background: color-mix(in srgb, var(--text) 8%, transparent);
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    background: color-mix(in srgb, var(--good) 70%, transparent);
  }
  .n {
    font-family: var(--font-mono);
    font-size: 0.8rem;
    text-align: right;
    white-space: nowrap;
  }
  small {
    font-size: 0.66rem;
    margin-left: 3px;
  }
  .up {
    color: var(--good);
  }
  .dn {
    color: var(--bad);
  }
  .faint {
    color: var(--text-faint);
  }
</style>
