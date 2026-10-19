<script lang="ts">
  // "What if it weighs": how the loadout fares along the mass axis for the
  // scanned composition, and a handle above the band (the mass with a
  // caret) so no part of the band is ever covered. Where the simulated
  // player decides, the band shades by the share of tries that crack
  // without losing a piece, hard → manageable → easy with the class
  // colours as anchors (20 / 50 / 80 %), blending from slice to slice: the
  // 1 % throttle steps make that share ripple from one mass to the next,
  // which hard class blocks showed as stripes. Too much power and can't
  // break are decided exactly and stay solid. The scanned mass is marked
  // under the band. Dragging anywhere on the track tries that mass.
  import type { BandSegmentView, MassBandView } from "$lib/bindings";
  import { CLASSES } from "./format";

  let {
    band,
    stale = false,
    value,
    scan,
    onchange,
  }: {
    band: MassBandView;
    /** Computed for another rock or loadout: the current one is on its way. */
    stale?: boolean;
    value: number;
    scan: number;
    onchange: (mass: number) => void;
  } = $props();

  const hueOf = (id: string) => CLASSES.find((c) => c.id === id)?.hue ?? "var(--text-faint)";
  /** A simulated share as a colour between the class anchors. A window
   *  too narrow to rate easy stays short of the easy green. */
  function shareColour(seg: BandSegmentView): string {
    const v = Math.max(0, Math.min(1, seg.class === "manageable" ? Math.min(seg.share ?? 0, 0.79) : (seg.share ?? 0)));
    const mix = (a: string, b: string, t: number) => `color-mix(in srgb, ${b} ${Math.round(t * 100)}%, ${a})`;
    const c =
      v <= 0.2 ? "var(--bad)" : v < 0.5 ? mix("var(--bad)", "var(--warn)", (v - 0.2) / 0.3) : v < 0.8 ? mix("var(--warn)", "var(--good)", (v - 0.5) / 0.3) : "var(--good)";
    return `color-mix(in srgb, ${c} 70%, transparent)`;
  }
  const solid = (seg: BandSegmentView) => `color-mix(in srgb, ${hueOf(seg.class)} 70%, transparent)`;
  const pct = (m: number) => (Math.min(Math.max(m, 0), band.top) / band.top) * 100;
  // One gradient over the rail: a stop at each simulated slice's centre (so
  // neighbours blend), hard stops at the edges of decided stretches.
  const gradient = $derived.by(() => {
    const segs = band.segments;
    const stops: string[] = [];
    segs.forEach((seg, i) => {
      if (seg.share === null) {
        stops.push(`${solid(seg)} ${pct(seg.from)}%`, `${solid(seg)} ${pct(seg.to)}%`);
        return;
      }
      const c = shareColour(seg);
      if (i === 0 || segs[i - 1].share === null) stops.push(`${c} ${pct(seg.from)}%`);
      stops.push(`${c} ${pct((seg.from + seg.to) / 2)}%`);
      if (i === segs.length - 1 || segs[i + 1].share === null) stops.push(`${c} ${pct(seg.to)}%`);
    });
    return stops.length ? `linear-gradient(to right, ${stops.join(", ")})` : "none";
  });
  // What the handle sits on, for the bubble.
  const under = $derived(band.segments.find((seg) => value >= seg.from && value < seg.to) ?? band.segments.at(-1) ?? null);
  const label2 = $derived(
    under === null
      ? ""
      : under.share !== null
        ? ` · ${Math.round(under.share * 100)}% no loss`
        : ` · ${CLASSES.find((c) => c.id === under.class)?.label ?? ""}`,
  );
  const at = (m: number) => `${(Math.min(Math.max(m, 0), band.top) / band.top) * 100}%`;
  const step = $derived(band.top / 400);
  const ticks = $derived.by(() => {
    // 1, 2 or 5 times a power of ten, about eight along the axis (the
    // backend ends the axis on this step: solve::axis_step)
    const raw = Math.max(band.top / 8, 1);
    const ten = 10 ** Math.floor(Math.log10(raw));
    const unit = [1, 2, 5, 10].map((k) => k * ten).find((s) => s >= raw - 1e-6) ?? 10 * ten;
    const out: number[] = [];
    for (let t = 0; t <= band.top + 1; t += unit) out.push(t);
    return out;
  });
  const label = (t: number) => (t === 0 ? "0" : `${t / 1000}k`);
  const fmt = (m: number) => `${Math.round(m).toLocaleString("en-US")} t`;
</script>

<div class="band" class:stale aria-busy={stale}>
  <div class="track">
    <span class="bubble" style:left={at(value)}>{fmt(value)}<span class="sh">{stale ? " · updating…" : label2}</span></span>
    <span class="needle" style:left={at(value)}></span>
    <span class="rail" style:background-image={gradient}></span>
    <span class="scan" style:left={at(scan)} title="Scanned mass"></span>
    <input
      type="range"
      min="0"
      max={band.top}
      {step}
      value={Math.min(value, band.top)}
      aria-label="Rock mass"
      oninput={(e) => onchange(Number(e.currentTarget.value))}
    />
  </div>
  <div class="axis">
    {#each ticks as t, i (t)}
      <span class:first={i === 0} class:last={i === ticks.length - 1} style:left={at(t)}
        >{label(t)}{i === ticks.length - 1 ? " t" : ""}</span
      >
    {/each}
    <span class="scan-l" style:left={at(scan)}>scan</span>
  </div>
  <div class="keys">
    {#each CLASSES.filter((c) => c.id === "tooMuchPower") as c (c.id)}<span><i style:--hue={c.hue}></i>{c.label}</span>{/each}
    <span class="ramp-key"
      ><i class="ramp"></i><span class="ends"><b>hard</b><b>manageable</b><b>easy</b></span></span
    >
    {#each CLASSES.filter((c) => c.id === "impossible") as c (c.id)}<span><i style:--hue={c.hue}></i>{c.label}</span>{/each}
  </div>
</div>

<style>
  .band {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .track {
    position: relative;
    height: 46px;
  }
  .bubble {
    position: absolute;
    top: 0;
    transform: translateX(-50%);
    padding: 1px 6px;
    border-radius: 4px;
    font-family: var(--font-mono);
    font-size: 0.72rem;
    white-space: nowrap;
    color: var(--text-strong);
    background: var(--bg);
    border: 1px solid var(--border);
    pointer-events: none;
  }
  .needle {
    position: absolute;
    top: 20px;
    width: 0;
    height: 0;
    margin-left: -5px;
    border: 5px solid transparent;
    border-top: 6px solid var(--text);
    border-bottom: 0;
    pointer-events: none;
  }
  .stale .rail {
    opacity: 0.35;
  }
  .rail {
    transition: opacity 0.15s;
    position: absolute;
    left: 0;
    right: 0;
    top: 28px;
    height: 10px;
    border-radius: 5px;
    overflow: hidden;
    background: color-mix(in srgb, var(--text) 6%, transparent);
  }
  .sh {
    color: var(--text-dim);
  }
  .scan {
    position: absolute;
    top: 40px;
    width: 0;
    height: 0;
    margin-left: -5px;
    border: 5px solid transparent;
    border-bottom: 6px solid var(--text-dim);
    border-top: 0;
  }
  input {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 46px;
    margin: 0;
    opacity: 0;
    cursor: grab;
  }
  input:active {
    cursor: grabbing;
  }
  .track:has(input:focus-visible) .bubble {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .axis {
    position: relative;
    height: 16px;
    font-family: var(--font-mono);
    font-size: 0.66rem;
    color: var(--text-faint);
  }
  .axis span {
    position: absolute;
    top: 0;
    transform: translateX(-50%);
    white-space: nowrap;
  }
  .axis .first {
    transform: none;
  }
  .axis .last {
    transform: translateX(-100%);
  }
  .axis .scan-l {
    color: var(--text-dim);
  }
  .keys {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 12px;
    font-size: 0.72rem;
    color: var(--text-faint);
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
  /* The gradient's key: hard at 20 %, manageable at 50 %, easy from 80 %
     (stops at the same anchors as the band, on a 0-100 % scale). */
  .ramp-key {
    display: inline-flex;
    flex-direction: column;
    gap: 1px;
  }
  .keys i.ramp {
    width: 120px;
    height: 6px;
    background: linear-gradient(
      to right,
      color-mix(in srgb, var(--bad) 70%, transparent) 20%,
      color-mix(in srgb, var(--warn) 70%, transparent) 50%,
      color-mix(in srgb, var(--good) 70%, transparent) 80%
    );
  }
  .ends {
    display: flex;
    justify-content: space-between;
    width: 120px;
    font-size: 0.62rem;
  }
  .ends b {
    font-weight: var(--weight-normal);
  }
</style>
