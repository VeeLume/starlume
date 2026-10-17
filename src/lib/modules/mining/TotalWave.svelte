<script lang="ts">
  // The wave for everyone: at each moment of the plan (everyone's throttle
  // under its name), how far one wave lifts the charge, each operator's
  // share stacked in their colour against the window. The rock has one
  // wave (a second ship makes it taller, more lasers on one ship split
  // it); the shares are each operator's share of the gain.
  import type { TotalRowView } from "$lib/bindings";

  let {
    rows,
    win,
    names,
    hues,
  }: { rows: TotalRowView[]; win: number; names: string[]; hues: string[] } = $props();

  const p = (v: number) => `${Math.round(v * 100)} %`;
  const scale = $derived(Math.max(win * 1.6, ...rows.map((r) => r.big)) * 1.05);
  const w = (v: number) => `${Math.min(100, (v / scale) * 100)}%`;
</script>

<div class="waves">
  <p class="label">How far one wave lifts the charge, everyone</p>
  <div class="keys">
    {#each names as n, k (k)}<span><i style:--hue={hues[k]}></i>{n}</span>{/each}
  </div>
  {#each rows as r, i (i)}
    <div class="row">
      <span class="th"
        ><b>{r.label ?? p(r.throttles[0] ?? 0)}</b><small
          >{r.throttles.map((t) => Math.round(t * 100)).join(", ")} %</small
        ></span
      >
      {#each [["big", r.big], ["usual", r.usual]] as [k, v] (k)}
        <span class="k">{k}</span>
        <span class="bar">
          {#each r.shares as s, j (j)}
            {#if s * Number(v) > 0}<i style:width={w(s * Number(v))} style:--hue={hues[j]}></i>{/if}
          {/each}
          <span class="win" style:left={w(win)}></span>
        </span>
        <span class="v" class:over={Number(v) > win}>{p(Number(v))}</span>
      {/each}
    </div>
  {/each}
  <p class="key"><span class="win-k"></span>window, {p(win)} of the bar</p>
</div>

<style>
  .waves {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
  }
  .label {
    margin: 0;
  }
  .keys {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 12px;
    margin-top: -4px;
    font-size: 0.72rem;
    color: var(--text-dim);
  }
  .keys span {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .keys i {
    width: 8px;
    height: 8px;
    border-radius: 2px;
    background: color-mix(in srgb, var(--hue) 70%, transparent);
  }
  .row {
    display: grid;
    grid-template-columns: 110px 38px minmax(0, 1fr) 38px;
    grid-template-rows: auto auto;
    gap: 3px 8px;
    align-items: center;
    padding-top: 8px;
    border-top: 1px solid color-mix(in srgb, var(--text) 8%, transparent);
  }
  .th {
    grid-row: span 2;
    display: flex;
    flex-direction: column;
    line-height: 1.2;
  }
  .th b {
    font-family: var(--font-mono);
    font-size: 0.86rem;
  }
  .th small {
    font-size: 0.68rem;
    color: var(--text-faint);
  }
  .k {
    font-size: 0.66rem;
    color: var(--text-faint);
    text-transform: var(--label-transform);
    letter-spacing: 0.06em;
  }
  .bar {
    position: relative;
    display: flex;
    gap: 1px;
    height: 8px;
    border-radius: 3px;
    background: color-mix(in srgb, var(--text) 6%, transparent);
  }
  .bar i {
    flex: none;
    display: block;
    height: 100%;
    background: color-mix(in srgb, var(--hue) 70%, transparent);
  }
  .bar i:first-child {
    border-radius: 2px 0 0 2px;
  }
  .bar .win {
    position: absolute;
    top: -4px;
    bottom: -4px;
    width: 2px;
    margin-left: -1px;
    background: var(--good);
    border-radius: 1px;
  }
  .v {
    font-family: var(--font-mono);
    font-size: 0.75rem;
    text-align: right;
    color: var(--text-dim);
  }
  .v.over {
    color: var(--warn);
  }
  .key {
    margin: 2px 0 0;
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.78rem;
    color: var(--text-faint);
  }
  .win-k {
    width: 2px;
    height: 14px;
    background: var(--good);
    border-radius: 1px;
    flex: none;
  }
</style>
