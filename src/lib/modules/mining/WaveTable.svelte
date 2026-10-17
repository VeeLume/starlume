<script lang="ts">
  // How far one wave lifts the charge: one row per throttle that matters,
  // a big wave (full height, longest period) and a usual one (half height,
  // mean period) against the window they have to stay inside. With a crew,
  // the rows are one operator's throttles, the others at theirs.
  import type { JumpRowView } from "$lib/bindings";

  let { rows, win, who = null }: { rows: JumpRowView[]; win: number; who?: string | null } = $props();

  const p = (v: number) => `${Math.round(v * 100)} %`;
  // Scaled so the window line sits inside every bar.
  const scale = $derived(Math.max(win * 1.6, ...rows.map((r) => r.big)) * 1.05);
  const share = (v: number) => `${Math.min(100, (v / scale) * 100)}%`;
  const whose = $derived(who === null ? "" : who === "You" ? ", your throttle" : `, ${who}'s throttle`);
</script>

<div class="waves">
  <p class="label">How far one wave lifts the charge{whose}</p>
  {#each rows as r (r.throttle)}
    <div class="row">
      <span class="th"><b>{p(r.throttle)}</b>{#if r.label}<small>{r.label}</small>{/if}</span>
      {#each [["big", r.big], ["usual", r.usual]] as [k, v] (k)}
        <span class="k">{k}</span>
        <span class="bar"
          ><i class:over={Number(v) > win} style:width={share(Number(v))}></i><span class="win" style:left={share(win)}
          ></span></span
        >
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
  .row {
    display: grid;
    grid-template-columns: 64px 38px minmax(0, 1fr) 38px;
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
    line-height: 1.15;
  }
  .th b {
    font-family: var(--font-mono);
    font-size: 0.9rem;
  }
  .th small {
    font-size: 0.7rem;
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
    display: block;
    height: 8px;
    border-radius: 3px;
    background: color-mix(in srgb, var(--text) 6%, transparent);
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
