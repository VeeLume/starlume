<script lang="ts">
  // Range, as the mining HUD's distance gauge: distance from the rock's
  // surface, your lasers' full power, the falloff to none, the crack's blast
  // striped below (an overcharge's fainter), and where to sit when the ship
  // has too much power up close. The marker is your beam distance: drag it
  // and the throttles and the crack follow (committed on release). The text
  // says only what's off: too much or too little power, a sit band
  // narrower than the distance wanders, or inside the crack's blast. The
  // switch says where the rock is: the distance drifts differently on a
  // surface (the ship hovers) and in space (the rock turns), and the
  // simulated cracks include that drift.
  import { Segmented } from "@veelume/ui";
  import type { YourSetupView } from "$lib/bindings";
  import NowCard from "$lib/components/NowCard.svelte";
  import { miningStore, setDistance, setSurface } from "$lib/state/mining.svelte";

  let { yours, distance }: { yours: YourSetupView; distance: number | null } = $props();

  const range = $derived(yours.range);
  const blast = $derived(yours.blast);
  // While dragging, the marker and the text follow the hand; the solve
  // follows on release.
  let drag = $state<number | null>(null);

  const full = $derived(range?.fullM ?? 0);
  const zero = $derived(range?.zeroM ?? 0);
  const back = $derived(range?.backOff ?? null);
  const crackBlast = $derived(blast.cleanMaxM > 0 && blast.cleanClearM > 0 ? blast.cleanClearM : 0);
  const overBlast = $derived(Math.max(0, blast.fullClearM));
  const STEPS = [5, 10, 20, 25, 50, 100, 200];
  const axis = $derived.by(() => {
    const far = Math.max(zero, overBlast, back?.toM ?? 0, distance ?? 0) * 1.08;
    const step = STEPS.find((s) => far / s <= 8) ?? 250;
    return { top: Math.max(step, Math.ceil(far / step) * step), step };
  });
  const ticks = $derived(Array.from({ length: Math.round(axis.top / axis.step) + 1 }, (_, i) => i * axis.step));
  const x = (m: number) => `${(Math.min(Math.max(m, 0), axis.top) / axis.top) * 100}%`;
  const w = (a: number, b: number) => `${((Math.min(b, axis.top) - Math.min(a, axis.top)) / axis.top) * 100}%`;
  const m = (v: number) => `${Math.round(v)} m`;

  const d = $derived(drag ?? distance ?? full);
  const set = $derived(drag !== null || distance !== null);
  const share = $derived(d <= full ? 1 : d >= zero ? 0 : 1 - (d - full) / (zero - full));
  const warnings = $derived.by(() => {
    const out: { tone: "warn" | "bad"; head: string; text: string }[] = [];
    if (back) {
      const band = `${Math.round(back.fromM)}–${Math.round(back.toM)} m`;
      if (d < back.fromM) out.push({ tone: "warn", head: "Too much power:", text: `back off to ${band}.` });
      else if (d > back.toM) out.push({ tone: "warn", head: "Too little power:", text: `come closer, to ${band}.` });
      else if (back.tight)
        out.push({
          tone: "warn",
          head: "Narrow band:",
          text: `${band} is less than the ±${Math.round(back.marginM)} m your distance can wander. Watch the gauge.`,
        });
    }
    if (yours.guide.tooMuchPower && !back)
      out.push({
        tone: "warn",
        head: "No distance tames it:",
        text: `between ${m(full)} and ${m(zero)} the minimum throttle still climbs.`,
      });
    if (crackBlast > 0 && d < crackBlast)
      out.push({ tone: "bad", head: "Inside the crack's blast:", text: `${Math.round(blast.cleanDamage)} damage to your hull.` });
    return out;
  });
  const offBand = $derived(back !== null && (d < back.fromM || d > back.toM));

  function commit(v: number) {
    drag = null;
    void setDistance(v <= full ? null : Math.round(v * 2) / 2);
  }
</script>

{#if range}
  <NowCard
    hue="var(--hue-teal)"
    chip="◇ RANGE"
    version={set ? `At ${m(d)}` : `Within ${m(full)}`}
    name={share >= 0.995 ? "full power" : `${Math.round(share * 100)}% power`}
  >
    {#snippet top()}
      <Segmented
        class="env"
        options={[
          { value: "space", label: "In space" },
          { value: "surface", label: "On a surface" },
        ]}
        value={miningStore.rock.surface ? "surface" : "space"}
        onchange={(v) => setSurface(v === "surface")}
      />
    {/snippet}
    <div class="gauge" role="img" aria-label="Distance from the rock">
      <span class="cap">rock</span>
      <div class="trk">
        <span class="lab top" style:left="0">full power to {m(full)}</span>
        {#if back}<span class="lab top acc" style:left={x((back.fromM + back.toM) / 2)}>sit here</span>{/if}
        <div class="band">
          <i class="pw" style:left="0" style:width={x(full)}></i>
          {#if zero > full}<i class="fade" style:left={x(full)} style:width={w(full, zero)}></i>{/if}
          {#if back}<i class="back" style:left={x(back.fromM)} style:width={w(back.fromM, back.toM)}></i>{/if}
          <span class="lab in" style:left="calc({x(zero)} + 6px)">none from {m(zero)}</span>
        </div>
        <div class="blast">
          {#if crackBlast > 0}<i class="cl" style:left="0" style:width={x(crackBlast)}></i>{/if}
          {#if overBlast > crackBlast}<i class="ov" style:left={x(crackBlast)} style:width={w(crackBlast, overBlast)}></i>{/if}
        </div>
        {#if crackBlast > 0}<span class="lab bl" style:left={x(crackBlast)}>crack blast {m(crackBlast)}</span>{/if}
        {#if overBlast > 0}<span class="lab bl" style:left={x(overBlast)}>overcharge {m(overBlast)}</span>{/if}
        <div class="axis">
          {#each ticks as t, i (t)}<span style:left={x(t)}>{t}{i === ticks.length - 1 ? " m" : ""}</span>{/each}
        </div>
        <div class="you" style:left={x(d)}><i class="ln"></i><i class="ct">▲</i><b>you, {m(d)}</b></div>
        <input
          type="range"
          min="0"
          max={axis.top}
          step="0.5"
          value={d}
          aria-label="Your beam distance to the rock, m"
          oninput={(e) => (drag = Number(e.currentTarget.value))}
          onchange={(e) => commit(Number(e.currentTarget.value))}
        />
      </div>
    </div>
    {#if distance !== null}
      <div><button type="button" class="link" onclick={() => void setDistance(null)}>Back to optimal range</button></div>
    {/if}
    {#each warnings as wn (wn.head)}
      <p class="say"><b class={wn.tone}>{wn.head}</b> {wn.text}</p>
    {/each}
    {#if back && offBand}
      <div><button type="button" class="btn" onclick={() => void setDistance(back.bestM)}>Sit at {m(back.bestM)}</button></div>
    {/if}
  </NowCard>
{/if}

<style>
  :global(.env) {
    font-size: var(--text-small);
  }
  .link {
    padding: 0;
    border: 0;
    background: none;
    font: inherit;
    font-size: var(--text-small);
    color: var(--accent);
    cursor: pointer;
  }
  .gauge {
    display: flex;
    align-items: flex-start;
    padding-top: 18px;
  }
  .cap {
    flex: none;
    width: 34px;
    height: 30px;
    display: grid;
    place-items: center;
    border-radius: 5px 0 0 5px;
    font-size: 0.62rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--hue-sand) 80%, var(--text));
    background: color-mix(in srgb, var(--hue-sand) 25%, transparent);
  }
  .trk {
    position: relative;
    flex: 1;
    height: 116px;
    margin-right: 18px;
  }
  .band {
    position: absolute;
    inset: 0 0 auto 0;
    height: 30px;
    border-radius: 0 5px 5px 0;
    background: color-mix(in srgb, var(--text) 4%, transparent);
  }
  .band > i,
  .blast > i {
    position: absolute;
    top: 0;
    height: 100%;
    display: block;
  }
  .pw {
    background: color-mix(in srgb, var(--good) 45%, transparent);
  }
  .fade {
    background: linear-gradient(90deg, color-mix(in srgb, var(--good) 45%, transparent), transparent);
  }
  .band > i.back {
    top: -4px;
    height: calc(100% + 8px);
    border-radius: 5px;
    border: 1.5px solid var(--accent);
    background: color-mix(in srgb, var(--accent) 16%, transparent);
  }
  .blast {
    position: absolute;
    inset: 35px 0 auto 0;
    height: 10px;
  }
  .cl {
    background: repeating-linear-gradient(45deg, color-mix(in srgb, var(--bad) 85%, transparent) 0 3px, transparent 3px 6px);
  }
  .ov {
    background: repeating-linear-gradient(45deg, color-mix(in srgb, var(--bad) 35%, transparent) 0 2px, transparent 2px 6px);
  }
  .lab {
    position: absolute;
    font-size: 0.72rem;
    color: var(--text-faint);
    white-space: nowrap;
  }
  .lab.top {
    top: -18px;
    color: var(--text-dim);
  }
  .lab.acc {
    transform: translateX(-50%);
    color: var(--accent);
    font-weight: var(--weight-semibold);
  }
  .lab.in {
    top: 7px;
  }
  .lab.bl {
    top: 48px;
    transform: translateX(-100%);
  }
  .axis {
    position: absolute;
    inset: 68px 0 auto 0;
    border-top: 1px solid var(--border);
  }
  .axis span {
    position: absolute;
    top: 4px;
    transform: translateX(-50%);
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: 0.66rem;
    color: var(--text-faint);
  }
  .you {
    position: absolute;
    top: 0;
    width: 0;
    height: 100%;
    pointer-events: none;
  }
  .you .ln {
    position: absolute;
    left: -1px;
    top: -6px;
    width: 2px;
    height: 58px;
    border-radius: 1px;
    background: var(--text);
  }
  .you .ct {
    position: absolute;
    left: 0;
    top: 88px;
    transform: translateX(-50%);
    font-style: normal;
    font-size: 0.6rem;
    line-height: 1;
    color: var(--text);
  }
  .you b {
    position: absolute;
    left: 0;
    top: 99px;
    transform: translateX(-50%);
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: 0.74rem;
  }
  input[type="range"] {
    position: absolute;
    left: 0;
    top: -8px;
    width: 100%;
    height: 60px;
    margin: 0;
    opacity: 0;
    cursor: ew-resize;
  }
  .say {
    margin: 0;
    font-size: 0.86rem;
    color: var(--text-dim);
  }
  .say .warn {
    color: var(--warn);
  }
  .say .bad {
    color: var(--bad);
  }
  .btn {
    padding: 3px 10px;
  }
</style>
