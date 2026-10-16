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
          : null,
  );

  // Which way down after an overshoot, in one sentence.
  const wayDown = $derived.by(() => {
    const g = guide;
    const lost = g.lostFrom ?? 1;
    const minUntil = Math.min(g.keepOnBelow ?? g.windowHigh, lost);
    if (minUntil >= lost) return "Keep the laser on at minimum throttle at any height: cutting it drops the window and costs more.";
    if (minUntil <= g.windowHigh) return `Cut the beam at once: the window top only drops to ${pct(g.windowHighOff)} and the charge falls fastest.`;
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

      {#if guide.balance !== null}
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
