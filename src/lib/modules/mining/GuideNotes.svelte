<script lang="ts">
  // Beside the charge bar: how the rock flies (the category), what the wave
  // does, and what to do after an overshoot. All from the throttle guide
  // (mod-mining's guide.rs); the jump sizes take the hit queue's strongest
  // tick as the worst case.
  import type { GuideView } from "$lib/bindings";

  let { guide }: { guide: GuideView } = $props();

  const pct = (v: number) => `${Math.round(v * 100)} %`;
  const secs = (v: number | null) => (v === null ? "—" : `${Math.round(v)} s`);
  const window = $derived(guide.windowHigh - guide.windowLow);

  const category = $derived(
    guide.category === "safe"
      ? { tone: "good", label: "Safe", text: "Even a maximum wave peak can't jump the window: climb and hold freely." }
      : guide.category === "okay"
        ? { tone: "warn", label: "Okay", text: "Average peaks stay inside the window; the big ones can overshoot." }
        : guide.category === "dangerous"
          ? { tone: "bad", label: "Dangerous", text: "Average peaks already jump the window and can overcharge hard." }
          : guide.category === "tooStrong"
            ? {
                tone: "bad",
                label: "Too strong",
                text: "Even at minimum throttle the charge keeps climbing: it can't be held, only pulsed.",
              }
            : null,
  );

  // Pulsing at minimum throttle (laser too strong): on-time share, the fall
  // while off (the decay: share = decay / gain, climb = gain - decay), and
  // the on-time that pairs with an off-tap short enough to keep the window.
  const pulse = $derived.by(() => {
    const share = guide.pulseOnShare;
    const climb = guide.climbAtMinimum;
    if (share === null || climb === null || share >= 1) return null;
    return {
      share,
      fall: (share * climb) / (1 - share),
      onS: (guide.modifierPersistenceS * share) / (1 - share),
    };
  });

  // Which way down after an overshoot, in one sentence.
  const wayDown = $derived.by(() => {
    const g = guide;
    const lost = g.lostFrom ?? 1;
    const minUntil = Math.min(g.keepOnBelow ?? g.windowHigh, lost);
    if (minUntil >= lost) return "Keep the laser on at minimum throttle at any height: cutting it drops the window and costs more.";
    if (minUntil <= g.windowHigh) {
      if (g.drainAtMinimum === null) return "Cut the beam at once: at minimum throttle the charge doesn't come down.";
      if (g.windowHighOff > g.windowHigh)
        return `Cut the beam at once: it falls fastest, and without the laser the window even widens (top ${pct(g.windowHighOff)}).`;
      return `Cut the beam at once: the window top only drops to ${pct(g.windowHighOff)} and the charge falls fastest.`;
    }
    return `Below ${pct(minUntil)} drop to minimum throttle (the window stays); above it cut the beam (it falls faster).`;
  });
</script>

{#if category}
  <div class="notes">
    <p class="head">
      <span class="status-pill {category.tone}"><i></i>{category.label}</span>
      <span>{category.text}</span>
    </p>

    <dl>
      <dt>The wave</dt>
      <dd>
        A peak multiplies the climb by up to ×{guide.waveMax.toFixed(0)}{guide.damping > 0
          ? ` (steady aim takes off ${pct(guide.damping)})`
          : ""}; a new wave every {guide.wavePeriodS - (guide.wavePeriodMaxS - guide.wavePeriodS)}–{guide.wavePeriodMaxS} s. It
        only acts while the charge rises.
      </dd>

      {#if guide.tooStrong && guide.climbAtMinimum !== null}
        <dt>At minimum throttle ({pct(guide.throttleMinimum)})</dt>
        <dd>
          The charge still climbs {(guide.climbAtMinimum * 100).toFixed(1)} % of the bar per second{guide.windowCrossS !== null
            ? `, crossing the window in about ${secs(guide.windowCrossS)}`
            : ""}{guide.timeInWindowS !== null ? ` — a clean crack needs ${secs(guide.timeInWindowS)} inside` : ""}.
          {#if pulse}
            Stay at minimum throttle and pulse the beam: on about {pct(pulse.share)} of the time (on it climbs
            {(guide.climbAtMinimum * 100).toFixed(1)} %/s, off it falls {(pulse.fall * 100).toFixed(1)} %/s). Keep each
            off-tap under {guide.modifierPersistenceS} s — about {pulse.onS.toFixed(2)} s on, {guide.modifierPersistenceS} s
            off — and the laser's window stays; a longer cut drops the window top to {pct(guide.windowHighOff)} (instead
            of {pct(guide.windowHigh)}) while it's off, and the first moments after resuming wobble more.
          {/if}
          A weaker laser or fewer power modules fit this rock better.
        </dd>
      {:else if guide.balance !== null}
        <dt>At balance ({pct(guide.balance)})</dt>
        <dd>
          One peak adds up to {pct(guide.peakJumpAtBalance ?? 0)} of the bar{guide.peakJumpAvgAtBalance !== null
            ? `, an average one ${pct(guide.peakJumpAvgAtBalance)}`
            : ""} — the window is {pct(window)}.
          {#if guide.decline !== null}Below {pct(guide.decline)} the wave can't lift the charge any more.{/if}
        </dd>
      {/if}

      <dt>After an overshoot</dt>
      <dd>
        {wayDown}
        Then stay inside about {Math.round(guide.paybackEdge ?? 3)}× as long as you were above ({Math.round(
          guide.paybackTop ?? 12,
        )}× if it went near the top) before the danger meter is empty again.
        {#if guide.lostFrom !== null}Past {pct(guide.lostFrom)} it can't be saved.{/if}
      </dd>

      {#if guide.crackAboveS !== null && guide.timeInWindowS !== null && guide.crackAboveS < guide.timeInWindowS}
        <dt>Sweet spot</dt>
        <dd>
          Just above the window the rock cracks faster ({secs(guide.crackAboveS)} vs {secs(guide.timeInWindowS)} inside).
          Shallow overshoots for up to about {Math.round(100 / (1 + (guide.paybackEdge ?? 3)))} % of the time speed it up
          without piling up danger.
        </dd>
      {/if}
    </dl>

    <p class="dim small">
      Jump sizes are the worst case: when the beam's hits and the rock's updates run at different rates, one update
      can take in up to 1.2× the mean gain. At one hit per update the gain is even and the jumps are much smaller;
      which one the game runs isn't pinned yet.
    </p>
  </div>
{/if}

<style>
  .notes {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 0;
  }
  .head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-2);
    margin: 0;
  }
  dl {
    margin: 0;
    display: grid;
    gap: 6px;
  }
  dt {
    font-family: var(--font-mono);
    font-size: var(--text-small);
    text-transform: var(--label-transform);
    letter-spacing: var(--label-tracking);
    color: var(--text-dim);
  }
  dd {
    margin: 0 0 4px;
    font-size: 0.86rem;
    line-height: 1.45;
  }
  .small {
    font-size: var(--text-small);
    margin: 0;
  }
</style>
