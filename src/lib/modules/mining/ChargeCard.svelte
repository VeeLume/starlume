<script lang="ts">
  // How to fly it: the charge bar in numbered steps in the game's colours
  // (nothing below the window, green inside, red above), each step's
  // instruction beside it with the game's ◀ marker; under it, how far one
  // wave lifts the charge at the throttles that matter, against the window
  // it has to stay inside. All from the throttle guide (mod-mining guide.rs).
  // The overlay will reuse the steps along the in-game bar.
  import type { GuideView } from "$lib/bindings";
  import { steps as stepsOf } from "./format";

  let { guide }: { guide: GuideView } = $props();

  const W = 400;
  const PAD = 10;
  const X = 46;
  const BW = 38;
  const LX = X + BW + 44;
  const GAP = 34;

  const steps = $derived(stepsOf(guide));
  const H = $derived(Math.max(320, steps.length * GAP + 40));
  const y = (c: number) => PAD + (1 - c) * (H - 2 * PAD);
  const win = $derived(guide.windowHigh - guide.windowLow);
  const p = (v: number) => `${Math.round(v * 100)} %`;

  const segs = $derived(
    steps.map((s, i) => {
      const top = y(s.to) + (s.to < 1 ? 1.5 : 0);
      const bot = y(s.from) - (s.from > 0 ? 1.5 : 0);
      return { s, i, top, bot, mid: (top + bot) / 2 };
    }),
  );
  // Labels at their step's middle, pushed apart so they never overlap.
  const labels = $derived.by(() => {
    const lab = segs.map((g) => ({ ...g, ly: g.mid })).sort((a, b) => a.mid - b.mid);
    for (let k = 1; k < lab.length; k++) lab[k].ly = Math.max(lab[k].ly, lab[k - 1].ly + GAP);
    const over = (lab.at(-1)?.ly ?? 0) - (H - PAD - 10);
    if (over > 0) {
      lab[lab.length - 1].ly -= over;
      for (let k = lab.length - 2; k >= 0; k--) lab[k].ly = Math.min(lab[k].ly, lab[k + 1].ly - GAP);
    }
    return lab;
  });
  const ticks = $derived([...new Set([0, ...steps.flatMap((s) => [s.from, s.to]), 1])]);
  const windowSeg = $derived(segs.find((g) => g.s.tone === "good") ?? null);

  // The wave-lift table, scaled so the window line sits inside every bar.
  const scale = $derived(Math.max(win * 1.6, ...guide.jumps.map((j) => j.big)) * 1.05);
  const share = (v: number) => `${Math.min(100, (v / scale) * 100)}%`;
</script>

<section class="card charge">
  <p class="label">How to fly it</p>
  {#if steps.length}
    <svg viewBox="0 0 {W} {H}" role="img" aria-label="Charge bar in {steps.length} steps">
      <defs>
        <linearGradient id="charge-sheen" x1="0" x2="1">
          <stop offset="0" stop-color="var(--text)" stop-opacity="0.10" />
          <stop offset="0.5" stop-color="var(--text)" stop-opacity="0" />
        </linearGradient>
      </defs>
      {#each ticks as c (c)}
        <line class="tk" x1={X - 6} x2={X} y1={y(c)} y2={y(c)} />
        <text class="tl" x={X - 9} y={y(c) + 3.5} text-anchor="end">{Math.round(c * 100)}</text>
      {/each}
      {#each segs as g (g.i)}
        {@const h = Math.max(2, g.bot - g.top)}
        <rect class="s-{g.s.tone}" x={X} y={g.top} width={BW} height={h} rx="3" />
        <rect x={X} y={g.top} width={BW} height={h} rx="3" fill="url(#charge-sheen)" />
        {#if g.bot - g.top > 15}
          <text class="num" x={X + BW / 2} y={g.mid + 4} text-anchor="middle">{g.i + 1}</text>
        {/if}
      {/each}
      {#if windowSeg}
        <rect
          class="win-ring"
          x={X - 2.5}
          y={windowSeg.top - 2.5}
          width={BW + 5}
          height={windowSeg.bot - windowSeg.top + 5}
          rx="4"
        />
      {/if}
      {#each labels as g (g.i)}
        <path class="lead" d="M{X + BW + 4},{g.mid} L{X + BW + 16},{g.mid} L{LX - 18},{g.ly}" />
        <path class="tri t-{g.s.tone}" d="M{LX - 6},{g.ly - 5} L{LX - 13},{g.ly} L{LX - 6},{g.ly + 5} Z" />
        <text class="act t-{g.s.tone}" x={LX} y={g.ly + 1}>{g.s.act}</text>
        {#if g.s.tone === "good"}
          <text class="sub" x={LX} y={g.ly + 16}>window, {p(win)} of the bar</text>
        {/if}
      {/each}
    </svg>
  {:else}
    <p class="dim">Even full throttle can't raise the charge.</p>
  {/if}

  {#if guide.jumps.length}
    <p class="label wavehead">How far one wave lifts the charge</p>
    <div class="jumps">
      <span class="h">Throttle</span><span class="h">Big wave</span><span class="h"></span><span class="h"
        >Usual wave</span
      ><span class="h"></span>
      {#each guide.jumps as j (j.throttle)}
        <span class="th"><b>{p(j.throttle)}</b>{#if j.label}<small>{j.label}</small>{/if}</span>
        {#each [j.big, j.usual] as v, k (k)}
          <span class="jb"
            ><i class:over={v > win} style:width={share(v)}></i><span class="win" style:left={share(win)}></span></span
          >
          <span class="jv" class:over={v > win}>{p(v)}</span>
        {/each}
      {/each}
    </div>
    <p class="key"><span class="win-k"></span>window, {p(win)} of the bar</p>
  {/if}
</section>

<style>
  .charge {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
    background: var(--bg-raised);
  }
  .label {
    margin: 0;
  }
  svg {
    display: block;
    width: 100%;
    max-width: 440px;
    height: auto;
  }
  .tk,
  .lead {
    stroke: var(--border);
    fill: none;
  }
  .tl {
    font-family: var(--font-mono);
    font-size: 10px;
    fill: var(--text-faint);
  }
  .s-climb {
    fill: color-mix(in srgb, var(--text-dim) 16%, var(--bg));
  }
  .s-good {
    fill: color-mix(in srgb, var(--good) 62%, var(--bg));
  }
  .s-warn1 {
    fill: color-mix(in srgb, var(--bad) 34%, var(--bg));
  }
  .s-warn2 {
    fill: color-mix(in srgb, var(--bad) 52%, var(--bg));
  }
  .s-bad {
    fill: color-mix(in srgb, var(--bad) 75%, var(--bg));
  }
  .win-ring {
    fill: none;
    stroke: var(--good);
    stroke-width: 1.5;
  }
  .num {
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: var(--weight-bold);
    fill: var(--text);
  }
  .act {
    font-family: var(--font-body);
    font-size: 14px;
    font-weight: var(--weight-semibold);
    fill: var(--text);
  }
  .sub {
    font-family: var(--font-body);
    font-size: 11px;
    fill: var(--text-faint);
  }
  .tri {
    fill: var(--text-dim);
  }
  .t-good {
    fill: var(--good);
  }
  .t-warn1,
  .t-warn2 {
    fill: var(--warn);
  }
  .t-bad {
    fill: var(--bad);
  }
  .wavehead {
    padding-top: 10px;
    border-top: 1px solid var(--border);
  }
  .jumps {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) 40px minmax(0, 1fr) 40px;
    gap: 8px 10px;
    align-items: center;
  }
  .h {
    font-size: 0.68rem;
    color: var(--text-faint);
    text-transform: var(--label-transform);
    letter-spacing: 0.08em;
  }
  .th {
    display: flex;
    flex-direction: column;
    line-height: 1.15;
  }
  .th b {
    font-family: var(--font-mono);
    font-size: 0.9rem;
    font-variant-numeric: tabular-nums;
  }
  .th small {
    font-size: 0.7rem;
    color: var(--text-faint);
  }
  .jb {
    position: relative;
    display: block;
    height: 10px;
    border-radius: 3px;
    background: var(--bg);
    border: 1px solid var(--border);
  }
  .jb i {
    display: block;
    height: 100%;
    border-radius: 2px;
    background: color-mix(in srgb, var(--good) 65%, transparent);
  }
  .jb i.over {
    background: color-mix(in srgb, var(--warn) 70%, transparent);
  }
  .jb .win {
    position: absolute;
    top: -4px;
    bottom: -4px;
    width: 2px;
    margin-left: -1px;
    background: var(--good);
    border-radius: 1px;
  }
  .jv {
    font-family: var(--font-mono);
    font-size: 0.78rem;
    text-align: right;
    color: var(--text-dim);
    font-variant-numeric: tabular-nums;
  }
  .jv.over {
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
