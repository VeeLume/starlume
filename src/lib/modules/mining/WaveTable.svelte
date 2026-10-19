<script lang="ts">
  // What one wave does at the throttles that matter, one table for one
  // operator and for the whole crew. Each row: the throttle first (for a
  // crew everyone's, in their colours), under it its step in the plan (Keep
  // at, Up to) or, for any other throttle, what its own bars show, so a
  // label never contradicts them. A big wave and a usual one as bars
  // against the window, marked once in the header; for a crew each
  // operator's share stacked in their colour. The guide's balance row stays
  // out: Keep at already says where the charge stays put.
  import type { JumpRowView, TotalRowView } from "$lib/bindings";
  import { waveRole } from "./format";

  let {
    rows = null,
    total = null,
    win,
    who = null,
    names = [],
    hues = [],
    ships = false,
    keepStep,
    upStep,
  }: {
    /** One operator's throttles. */
    rows?: JumpRowView[] | null;
    /** Everyone at each moment of the plan. */
    total?: TotalRowView[] | null;
    win: number;
    /** Whose throttles `rows` are; null: solo. */
    who?: string | null;
    names?: string[];
    hues?: string[];
    /** The crew is on separate ships (their big waves can land together). */
    ships?: boolean;
    /** The plan's step numbers for Keep at and the last Up to. */
    keepStep: number | null;
    upStep: number | null;
  } = $props();

  interface Row {
    key: string;
    throttles: number[];
    label: string | null;
    big: number;
    usual: number;
    shares: number[] | null;
  }

  const p = (v: number) => `${Math.round(v * 100)}%`;
  const list = $derived<Row[]>(
    (total
      ? total.map((r, i) => ({ key: `${i}`, throttles: r.throttles, label: r.label, big: r.big, usual: r.usual, shares: r.shares }))
      : (rows ?? []).map((r) => ({ key: `${r.throttle}`, throttles: [r.throttle], label: r.label, big: r.big, usual: r.usual, shares: null }))
    ).filter((r) => r.label !== "balance"),
  );
  // Scaled so the window line sits inside every bar.
  const scale = $derived(Math.max(win * 1.6, ...list.map((r) => r.big)) * 1.05);
  const share = (v: number) => `${Math.min(100, (v / scale) * 100)}%`;
  const whose = $derived(total ? ", everyone" : who === null ? "" : who === "You" ? ", your throttle" : `, ${who}'s throttle`);
</script>

<div class="waves">
  <p class="label">What one wave does{whose}</p>
  <p class="sub">
    How far one wave throws the charge{total ? ", each operator's share in their colour" : ""}. At Keep at the drain
    and the waves even out, so the charge stays put; green stays inside the window, amber jumps out of it.{#if total}
      {ships ? " Separate ships' big waves can land together; the big wave bar is that case." : " A MOLE's heads share one wave."}{/if}
  </p>
  {#if total}
    <div class="keys">
      {#each names as n, k (k)}<span><i style:--hue={hues[k]}></i>{n}</span>{/each}
    </div>
  {/if}
  <div class="row h" class:crew={total}>
    <span>Throttle</span><span></span><span class="wcol"><span style:left={share(win)}>window</span></span><span></span>
  </div>
  {#each list as r (r.key)}
    {@const role = waveRole(r.label, r.big, r.usual, win, keepStep, upStep)}
    <div class="row" class:crew={total}>
      <span class="th">
        <b
          >{#each r.throttles as t, k (k)}{#if k}<span class="sep"> · </span>{/if}<span style:color={total ? `color-mix(in srgb, ${hues[k]} 75%, var(--text))` : null}
              >{Math.round(t * 100)}</span
            >{/each}%</b
        >
        <small class={role.tone}>{#if role.step}<span class="st {role.tone}">{role.step}</span>{/if}{role.say}</small>
      </span>
      {#each [["big wave", r.big], ["usual", r.usual]] as [k, v] (k)}
        <span class="k">{k}</span>
        <span class="bar" class:stack={r.shares !== null}>
          {#if r.shares}
            {#each r.shares as s, j (j)}
              {#if s * Number(v) > 0}<i class="part" style:width={share(s * Number(v))} style:--hue={hues[j]}></i>{/if}
            {/each}
          {:else}
            <i class:over={Number(v) > win} style:width={share(Number(v))}></i>
          {/if}
          <span class="win" style:left={share(win)}></span>
        </span>
        <span class="v" class:over={Number(v) > win}>{p(Number(v))}</span>
      {/each}
    </div>
  {/each}
</div>

<style>
  .waves {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }
  .label {
    margin: 0;
  }
  .sub {
    margin: -4px 0 0;
    font-size: 0.78rem;
    color: var(--text-faint);
  }
  .keys {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 12px;
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
    grid-template-columns: 132px 64px minmax(0, 1fr) 40px;
    grid-template-rows: auto auto;
    gap: 3px 8px;
    align-items: center;
    padding-top: 8px;
    border-top: 1px solid color-mix(in srgb, var(--text) 8%, transparent);
  }
  .row.crew {
    grid-template-columns: 150px 64px minmax(0, 1fr) 40px;
  }
  .row.h {
    grid-template-rows: auto;
    padding-top: 0;
    border-top: 0;
    font-size: 0.64rem;
    letter-spacing: 0.08em;
    text-transform: var(--label-transform);
    color: var(--text-faint);
  }
  .wcol {
    position: relative;
    height: 14px;
  }
  .wcol span {
    position: absolute;
    bottom: 0;
    transform: translateX(-50%);
    white-space: nowrap;
    color: var(--good);
  }
  .th {
    grid-row: span 2;
    display: flex;
    flex-direction: column;
    gap: 2px;
    line-height: 1.2;
    min-width: 0;
  }
  .th b {
    font-family: var(--font-mono);
    font-size: 0.9rem;
  }
  .sep {
    color: var(--text-faint);
  }
  .th small {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 0.74rem;
    color: var(--text-dim);
  }
  .th small.good {
    color: var(--good);
  }
  .th small.warn {
    color: var(--warn);
  }
  .st {
    display: inline-grid;
    place-items: center;
    flex: none;
    width: 17px;
    height: 17px;
    border-radius: 4px;
    font: var(--weight-bold) 0.64rem var(--font-mono);
    color: var(--text);
    background: color-mix(in srgb, var(--text) 14%, transparent);
  }
  .st.good {
    color: var(--bg);
    background: color-mix(in srgb, var(--good) 70%, transparent);
  }
  .k {
    font-size: 0.66rem;
    color: var(--text-faint);
    text-transform: var(--label-transform);
    letter-spacing: 0.06em;
    white-space: nowrap;
  }
  .bar {
    position: relative;
    display: block;
    height: 8px;
    border-radius: 3px;
    background: color-mix(in srgb, var(--text) 6%, transparent);
  }
  .bar.stack {
    display: flex;
    gap: 1px;
  }
  .bar i {
    display: block;
    height: 100%;
    border-radius: 2px;
    background: color-mix(in srgb, var(--good) 65%, transparent);
  }
  .bar i.over {
    background: color-mix(in srgb, var(--warn) 70%, transparent);
  }
  .bar i.part {
    flex: none;
    border-radius: 0;
    background: color-mix(in srgb, var(--hue) 70%, transparent);
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
</style>
