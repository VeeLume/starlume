<script lang="ts">
  // The charge bar in numbered steps in the game's colours (nothing below
  // the window, green inside, red above), matte like every other bar, each
  // step's instruction beside it with the game's ◀ marker (the window's
  // size is said once, beside How to crack it's title). `acts` replaces
  // the instructions (a crew support's), `height` lets it grow with its
  // card. The overlay will reuse the steps along the in-game bar.
  import { stepLabel, type Step } from "./format";

  let {
    steps,
    acts = null,
    height = 320,
  }: {
    steps: Step[];
    /** One instruction per step, in place of the steps' own. */
    acts?: string[] | null;
    height?: number;
  } = $props();

  const W = 330;
  const PAD = 10;
  const X = 40;
  const BW = 36;
  const LX = X + BW + 40;
  const GAP = 34;

  const H = $derived(Math.max(height, steps.length * GAP + 40));
  const y = (c: number) => PAD + (1 - c) * (H - 2 * PAD);

  const segs = $derived(
    steps.map((s, i) => {
      const top = y(s.to) + (s.to < 1 ? 1.5 : 0);
      const bot = y(s.from) - (s.from > 0 ? 1.5 : 0);
      return { s, i, top, bot, mid: (top + bot) / 2, act: acts?.[i] ?? s.act };
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
  // Ticks closer than 12 px merge, so close steps never overprint.
  const ticks = $derived.by(() => {
    const all = [...new Set([0, ...steps.flatMap((s) => [s.from, s.to]), 1])].sort((a, b) => a - b);
    const out: number[] = [];
    for (const c of all) if (!out.length || y(out[out.length - 1]) - y(c) >= 12) out.push(c);
    return out;
  });
  // The window's ring: around both halves when it is kept in two settings.
  const windowSeg = $derived.by(() => {
    const good = segs.filter((g) => g.s.tone === "good");
    return good.length ? { top: Math.min(...good.map((g) => g.top)), bot: Math.max(...good.map((g) => g.bot)) } : null;
  });
</script>

<svg viewBox="0 0 {W} {H}" style:height="{H}px" role="img" aria-label="Charge bar in {steps.length} steps">
  {#each ticks as c (c)}
    <line class="tk" x1={X - 6} x2={X} y1={y(c)} y2={y(c)} />
    <text class="tl" x={X - 9} y={y(c) + 3.5} text-anchor="end">{Math.round(c * 100)}</text>
  {/each}
  {#each segs as g (g.i)}
    <rect class="s-{g.s.tone}" x={X} y={g.top} width={BW} height={Math.max(2, g.bot - g.top)} rx="3" />
    {#if g.bot - g.top > 15}
      <text class="num" x={X + BW / 2} y={g.mid + 4} text-anchor="middle">{stepLabel(g.s)}</text>
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
    <path class="lead" d="M{X + BW + 4},{g.mid} L{X + BW + 14},{g.mid} L{LX - 18},{g.ly}" />
    <path class="tri t-{g.s.tone}" d="M{LX - 6},{g.ly - 5} L{LX - 13},{g.ly} L{LX - 6},{g.ly + 5} Z" />
    <text class="act t-{g.s.tone}" x={LX} y={g.ly + 1}>{g.act}</text>
  {/each}
</svg>

<style>
  svg {
    display: block;
    width: 100%;
    max-width: 330px;
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
    fill: color-mix(in srgb, var(--text) 12%, transparent);
  }
  .s-good {
    fill: color-mix(in srgb, var(--good) 70%, transparent);
  }
  .s-warn1 {
    fill: color-mix(in srgb, var(--bad) 32%, transparent);
  }
  .s-warn2 {
    fill: color-mix(in srgb, var(--bad) 50%, transparent);
  }
  .s-bad {
    fill: color-mix(in srgb, var(--bad) 70%, transparent);
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
</style>
