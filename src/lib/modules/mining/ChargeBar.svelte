<script lang="ts">
  // The rock's charge bar, upright and coloured as in game (nothing below the
  // window, green inside, red above), with what to do at each charge level:
  // the throttle on the way up, the hold in the window, and the way back
  // from an overshoot. The bracket on the left is how far one wave peak at
  // the balance throttle can carry the charge (worst case, and an average
  // one). The overlay will reuse these markers along the in-game bar.
  import type { GuideView } from "$lib/bindings";

  let { guide }: { guide: GuideView } = $props();

  const H = 360;
  const PAD = 8;
  const BAR_X = 64;
  const BAR_W = 30;
  const LABEL_X = BAR_X + BAR_W + 38;
  const W = 330;
  const GAP = 17;

  const y = (charge: number) => PAD + (1 - Math.min(1, Math.max(0, charge))) * (H - 2 * PAD);
  const pct = (v: number) => `${Math.round(v * 100)} %`;

  type Tone = "plain" | "good" | "warn" | "bad";
  interface Zone {
    from: number;
    to: number;
    text: string;
    tone: Tone;
  }

  const zones = $derived.by(() => {
    const g = guide;
    const out: Zone[] = [];
    if (g.balance === null) return out;
    // The way up: full throttle, then zones up to each 10 % step and the
    // window. Each zone names the throttle that is safe all the way to its
    // top marker (the step's throttle; the safe climb at the window).
    let at = 0;
    if (g.fullUntil !== null && g.fullUntil > 0) {
      out.push({ from: 0, to: g.fullUntil, text: "Full throttle", tone: "plain" });
      at = g.fullUntil;
    }
    const tops = [
      ...[...g.climb].sort((a, b) => a.untilCharge - b.untilCharge).map((s) => ({ at: s.untilCharge, t: s.throttle })),
      { at: g.windowLow, t: g.safeClimb ?? 0 },
    ];
    for (const top of tops) {
      if (top.at <= at) continue;
      const text = top.t <= g.throttleMinimum ? "Minimum" : `≤ ${pct(top.t)}`;
      out.push({ from: at, to: top.at, text, tone: "plain" });
      at = top.at;
    }
    // The window.
    if (g.holdThrottle !== null) {
      out.push({
        from: g.windowLow,
        to: g.windowHigh,
        text: g.riding ? `Ride the waves ≤ ${pct(g.holdThrottle)}` : `Hold ${pct(g.holdThrottle)}`,
        tone: "good",
      });
    }
    // Above: drop to minimum, cut the beam, too late.
    const lost = g.lostFrom ?? 1;
    const minUntil = Math.min(g.keepOnBelow ?? g.windowHigh, lost);
    if (minUntil > g.windowHigh) out.push({ from: g.windowHigh, to: minUntil, text: "Drop to minimum", tone: "warn" });
    if (lost > minUntil) out.push({ from: minUntil, to: lost, text: "Cut the beam", tone: "warn" });
    if (lost < 1) out.push({ from: lost, to: 1, text: "Too late: overcharges", tone: "bad" });
    return out;
  });

  // Labels at their zone's middle, pushed apart so they never overlap.
  const labels = $derived.by(() => {
    const placed = zones.map((z) => ({ ...z, mid: y((z.from + z.to) / 2), ly: y((z.from + z.to) / 2) }));
    placed.sort((a, b) => a.mid - b.mid);
    for (let i = 1; i < placed.length; i++) placed[i].ly = Math.max(placed[i].ly, placed[i - 1].ly + GAP);
    const overflow = (placed.at(-1)?.ly ?? 0) - (H - PAD);
    if (overflow > 0) {
      placed.at(-1)!.ly -= overflow;
      for (let i = placed.length - 2; i >= 0; i--) placed[i].ly = Math.min(placed[i].ly, placed[i + 1].ly - GAP);
    }
    return placed;
  });

  const bounds = $derived([...new Set(zones.flatMap((z) => [z.from, z.to]))].filter((c) => c > 0 && c < 1));
  const peak = $derived(guide.peakJumpAtBalance);
  const avgPeak = $derived(guide.peakJumpAvgAtBalance);
</script>

<svg viewBox="0 0 {W} {H}" width={W} height={H} role="img" aria-label="Charge bar with what to do at each level">
  <!-- The bar, as in game. -->
  <rect class="bar-empty" x={BAR_X} y={y(guide.windowLow)} width={BAR_W} height={y(0) - y(guide.windowLow)} />
  <rect class="bar-window" x={BAR_X} y={y(guide.windowHigh)} width={BAR_W} height={y(guide.windowLow) - y(guide.windowHigh)} />
  <rect class="bar-danger" x={BAR_X} y={y(1)} width={BAR_W} height={y(guide.windowHigh) - y(1)} />
  <rect class="bar-frame" x={BAR_X} y={y(1)} width={BAR_W} height={y(0) - y(1)} />
  {#if guide.windowHighOff < guide.windowHigh - 0.005}
    <line class="off-line" x1={BAR_X} x2={BAR_X + BAR_W} y1={y(guide.windowHighOff)} y2={y(guide.windowHighOff)}>
      <title>Window top with the beam off ({pct(guide.windowHighOff)})</title>
    </line>
  {/if}
  {#each bounds as c (c)}
    <line class="tick" x1={BAR_X + BAR_W} x2={BAR_X + BAR_W + 6} y1={y(c)} y2={y(c)} />
    <text class="tick-label" x={BAR_X - 4} y={y(c) + 3} text-anchor="end">{pct(c)}</text>
  {/each}

  <!-- How far one peak at balance reaches from the window's bottom. -->
  {#if peak !== null && peak > 0.005}
    {@const top = y(guide.windowLow + peak)}
    <g class="peak">
      <line x1={BAR_X - 34} x2={BAR_X - 34} y1={y(guide.windowLow)} y2={top} />
      <line x1={BAR_X - 38} x2={BAR_X - 30} y1={top} y2={top} />
      {#if avgPeak !== null && avgPeak > 0.005}
        <line class="avg" x1={BAR_X - 38} x2={BAR_X - 30} y1={y(guide.windowLow + avgPeak)} y2={y(guide.windowLow + avgPeak)} />
      {/if}
      <title>
        One wave peak at the balance throttle can add up to {pct(peak)} of the bar{avgPeak !== null
          ? `; an average one ${pct(avgPeak)}`
          : ""}.
      </title>
    </g>
  {/if}

  <!-- What to do there. -->
  {#each labels as l (l.from)}
    <path class="leader" d="M{BAR_X + BAR_W + 6},{l.mid} L{LABEL_X - 6},{l.ly}" />
    <text class="label-{l.tone}" x={LABEL_X} y={l.ly + 4}>{l.text}</text>
  {/each}
</svg>

<style>
  svg {
    display: block;
    flex: none;
    max-width: 100%;
    height: auto;
  }
  .bar-empty {
    fill: color-mix(in srgb, var(--text-dim) 8%, transparent);
  }
  .bar-window {
    fill: color-mix(in srgb, var(--good) 55%, transparent);
  }
  .bar-danger {
    fill: color-mix(in srgb, var(--bad) 45%, transparent);
  }
  .bar-frame {
    fill: none;
    stroke: var(--border);
  }
  .off-line {
    stroke: var(--text);
    stroke-dasharray: 3 3;
    opacity: 0.7;
  }
  .tick,
  .leader {
    stroke: var(--border);
    fill: none;
  }
  .tick-label {
    font-family: var(--font-mono);
    font-size: 10px;
    fill: var(--text-dim);
  }
  .peak line {
    stroke: var(--warn);
    stroke-width: 2;
  }
  .peak .avg {
    stroke-width: 1;
    stroke-dasharray: 2 2;
  }
  text[class^="label-"] {
    font-size: 12.5px;
  }
  .label-plain {
    fill: var(--text);
  }
  .label-good {
    fill: var(--good);
  }
  .label-warn {
    fill: var(--warn);
  }
  .label-bad {
    fill: var(--bad);
  }
</style>
