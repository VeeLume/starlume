<script lang="ts">
  // "What if it weighs": the loadout's classes along the mass axis for the
  // scanned composition, and a handle above the band (the mass with a
  // caret) so no part of the band — the narrow safe zone included — is
  // ever covered. The scanned mass is marked under the band. Dragging
  // anywhere on the track tries that mass.
  import type { MassBandView } from "$lib/bindings";
  import { CLASSES } from "./format";

  let {
    band,
    value,
    scan,
    onchange,
  }: {
    band: MassBandView;
    value: number;
    scan: number;
    onchange: (mass: number) => void;
  } = $props();

  const hueOf = (id: string) => CLASSES.find((c) => c.id === id)?.hue ?? "var(--text-faint)";
  const at = (m: number) => `${(Math.min(Math.max(m, 0), band.top) / band.top) * 100}%`;
  const step = $derived(band.top / 400);
  const ticks = $derived.by(() => {
    const unit = band.top <= 5000 ? 1000 : band.top <= 20000 ? 2000 : 5000;
    const out: number[] = [];
    for (let t = 0; t <= band.top + 1; t += unit) out.push(t);
    return out;
  });
  const label = (t: number) => (t === 0 ? "0" : `${t / 1000}k`);
  const fmt = (m: number) => `${Math.round(m).toLocaleString("en-US")} t`;
</script>

<div class="band">
  <div class="track">
    <span class="bubble" style:left={at(value)}>{fmt(value)}</span>
    <span class="needle" style:left={at(value)}></span>
    <span class="rail">
      {#each band.segments as s, i (i)}
        <i style:left={at(s.from)} style:width="calc({at(s.to)} - {at(s.from)})" style:--hue={hueOf(s.class)}></i>
      {/each}
    </span>
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
    {#each CLASSES as c (c.id)}<span><i style:--hue={c.hue}></i>{c.label}</span>{/each}
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
  .rail {
    position: absolute;
    left: 0;
    right: 0;
    top: 28px;
    height: 10px;
    border-radius: 5px;
    overflow: hidden;
    background: color-mix(in srgb, var(--text) 6%, transparent);
  }
  .rail i {
    position: absolute;
    top: 0;
    bottom: 0;
    display: block;
    background: color-mix(in srgb, var(--hue) 70%, transparent);
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
</style>
