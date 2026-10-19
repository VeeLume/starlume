<script lang="ts">
  // How to crack it, in one vocabulary: the climb is always "Up to x %",
  // the window always "Keep at x %", then Drop to minimum, Cut the beam, It
  // will overcharge. The title is the shown operator's step in the window
  // (solo: yours; a crew: the lead's, with their monogram; a support picked
  // in "for": theirs, kept the whole time), and the window's size is said
  // once, beside it. When the window can't be kept (a usual wave there is
  // wider than the window), one strip says so and offers what widens it.
  // The charge bar in steps beside the wave table (and the crew plan); the
  // bar grows with the card, capped at the window's height, and stays in
  // view while a long plan scrolls. Sharing the plan with the crew's apps
  // waits for the server.
  import { Segmented } from "@veelume/ui";
  import type { LoadoutView, MiningCatalogView, MiningReportView, YourSetupView } from "$lib/bindings";
  import Monogram from "$lib/components/Monogram.svelte";
  import NowCard from "$lib/components/NowCard.svelte";
  import { distinctHues } from "$lib/monogram";
  import { miningStore, miningUi, setDistance, setLead } from "$lib/state/mining.svelte";
  import { alternatives, byOutcome, improves } from "./alternatives";
  import ChargeBar from "./ChargeBar.svelte";
  import CrewPlan from "./CrewPlan.svelte";
  import { operatorNames, operatorPlace, ownLasers, stepLabel, stepNumbers, steps as stepsOf } from "./format";
  import WaveTable from "./WaveTable.svelte";

  let {
    catalog,
    report,
    yours,
    loadout,
  }: { catalog: MiningCatalogView; report: MiningReportView; yours: YourSetupView; loadout: LoadoutView } = $props();

  let besideHeight = $state(0);
  let viewHeight = $state(900);

  const guide = $derived(yours.guide);
  const steps = $derived(stepsOf(guide));
  const numbers = $derived(stepNumbers(steps, guide));
  const win = $derived(guide.windowHigh - guide.windowLow);
  const p = (v: number) => `${Math.round(v * 100)}%`;
  const barHeight = $derived(Math.round(Math.max(320, Math.min(besideHeight, viewHeight - 140))));

  const ops = $derived(yours.operators);
  const crew = $derived(ops.length > 1);
  // One entry per operator of the report. The loadout changes first and the
  // report follows a solve later, so the two can disagree on how many
  // lasers there are (a crew ship just taken off): never index past either.
  const laserNames = $derived(operatorNames(loadout.lasers));
  const names = $derived(ops.map((_, k) => laserNames[k] ?? "?"));
  const hues = $derived(distinctHues(names));
  const saved = $derived(miningStore.state?.saved ?? []);
  const places = $derived(
    ops.map((_, i) =>
      i < loadout.lasers.length ? operatorPlace(loadout.lasers, i, (id) => saved.find((s) => s.id === id)?.name) : "",
    ),
  );
  const mods = $derived(ops.map((_, i) => loadout.lasers[i]?.modules ?? []));
  const me = $derived(miningUi.me >= 0 && miningUi.me < ops.length ? miningUi.me : -1);
  const leadIndex = $derived(Math.max(0, ops.findIndex((o) => o.lead)));
  const ships = $derived(new Set(loadout.lasers.map((l) => l.owner)).size > 1);

  /** A support goes Up to their throttle in the climb, Keeps at it through
   *  the window and the drop, and cuts with everyone. */
  const supportCalls = (hold: number, full: boolean) =>
    steps.map((s) =>
      s.tone === "climb"
        ? `Up to ${p(hold)}`
        : s.tone === "warn2" || (guide.tooMuchPower && s.half === "high")
          ? s.act
          : s.tone === "bad"
            ? full
              ? "It will overcharge"
              : ""
            : `Keep at ${p(hold)}`,
    );
  const calls = $derived(ops.map((o) => (o.lead ? steps.map((s) => s.act) : supportCalls(o.hold ?? 1, false))));
  // a pulse cuts in the window's upper half, everyone at once
  const cutStep = $derived(steps.find((s) => s.tone === "warn2" || (guide.tooMuchPower && s.half === "high")) ?? null);

  const shown = $derived(me >= 0 ? ops[me] : null);
  const acts = $derived(shown && !shown.lead ? supportCalls(shown.hold ?? 1, true) : null);
  const windowStep = $derived(steps.find((s) => s.n === numbers.keep) ?? null);
  // The window kept in two settings reads as their range: "Keep at 49–53 %".
  const keepRange = $derived(
    !guide.tooMuchPower && guide.keepLow !== null && guide.keepHigh !== null
      ? `Keep at ${Math.round(guide.keepHigh * 100)}–${p(guide.keepLow)}`
      : null,
  );
  const leadTitle = $derived(
    guide.unholdable
      ? "Can't keep it"
      : (keepRange ?? (guide.tooMuchPower ? "Pulse at minimum" : null) ?? windowStep?.act ?? (guide.balance === null ? "Can't raise the charge" : (steps[0]?.act ?? ""))),
  );
  // The throttles assume your beam distance, when one is set (the Range card).
  const distance = $derived(ownLasers(loadout).find((l) => l.distanceM !== null)?.distanceM ?? null);
  const pct1 = (v: number) => `${(v * 100).toFixed(1)}%`;
  const windowLine = $derived(`window, ${pct1(win)} of the bar${distance !== null ? ` · at ${Math.round(distance)} m` : ""}`);
  const title = $derived(shown && !shown.lead ? `Keep at ${p(shown.hold ?? 1)}` : leadTitle);
  const titleSub = $derived(shown && !shown.lead ? (cutStep ? `cut the beam at step ${stepLabel(cutStep)}` : "") : windowLine);
  const monogram = $derived(crew ? (shown ? me : leadIndex) : -1);

  // When the plan doesn't work (too much power, a window that can't be
  // kept, out of reach) the card leads with why, from the simulated tries,
  // and the changes that do better: backing off first (a slower climb even
  // when it can't be held), then gadgets, modules and crew, best first.
  const loss = $derived(yours.eval.difficulty ? Math.round(yours.eval.difficulty.controlled * 100) : null);
  const short = $derived(Math.max(0, yours.eval.need - yours.eval.power));
  const trouble = $derived(guide.tooMuchPower || guide.unholdable || guide.balance === null);
  const backOff = $derived(guide.tooMuchPower ? (yours.range?.backOff ?? null) : null);
  const fixes = $derived(
    trouble ? alternatives(report, loadout, catalog).filter((a) => improves(a.eval, yours.eval)).sort(byOutcome).slice(0, 3) : [],
  );
</script>

<svelte:window bind:innerHeight={viewHeight} />

<NowCard hue="var(--good)" chip="▲ HOW TO CRACK" version="">
  {#snippet top()}
    {#if crew}
      <span class="for">for</span>
      <Segmented
        class="who"
        options={[{ value: "-1", label: "All" }, ...ops.map((_, k) => ({ value: String(k), label: names[k] }))]}
        value={String(me)}
        onchange={(v) => (miningUi.me = Number(v))}
      />
      <button class="btn share" disabled title="Each player's app opens the same rock (needs the server)"
        >Share with crew <span class="soon">later</span></button
      >
    {/if}
  {/snippet}
  <div class="title">
    {#if monogram >= 0}<Monogram name={names[monogram]} hue={hues[monogram]} size={32} />{/if}
    <span class="big" class:warn={guide.unholdable && !(shown && !shown.lead)}>{title}</span>
    {#if titleSub}<span class="sub">{titleSub}</span>{/if}
  </div>
  {#if trouble}
    <div class="fix">
      <span>
        {#if guide.tooMuchPower}
          <b>Too much power: even at the minimum the charge keeps climbing.</b> Pulsed, {loss ?? 0} of 100 tries
          crack without losing a piece.
        {:else if guide.balance === null}
          <b>Even 100% can't raise the charge: {Math.round(short).toLocaleString("en-US")} power short.</b>
        {:else}
          <b>Flying the plan, {loss ?? 0} of 100 tries crack without losing a piece.</b> The waves throw the charge out of
          the {pct1(win)} window.
        {/if}
        {#if backOff || fixes.length}Better:{/if}
      </span>
      {#if backOff || fixes.length}
        <div class="fixes">
          {#if backOff}
            <button type="button" class="btn" onclick={() => void setDistance(backOff.bestM)}>
              Back off to {Math.round(backOff.bestM)} m <span class="w">a slower climb</span>
            </button>
          {/if}
          {#each fixes as f (f.key)}
            <button type="button" class="btn" onclick={() => void f.apply()}>
              {#if f.who}<Monogram name={f.who} size={20} />{/if}
              {f.verb === "Add" ? `Bring ${f.name}` : f.name}
              {#if guide.unholdable}<span class="w">window {pct1(f.eval.windowHigh - f.eval.windowLow)}</span>{/if}
              <b class="gn">{Math.round((f.eval.difficulty?.controlled ?? 0) * 100)}</b>
            </button>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
  {#if steps.length}
    <div class="fly" class:crew>
      <div class="bar"><ChargeBar {steps} {acts} height={barHeight} /></div>
      <div class="beside" bind:clientHeight={besideHeight}>
        {#if crew}
          <CrewPlan
            {catalog}
            operators={ops}
            {names}
            {places}
            {mods}
            {hues}
            {steps}
            {calls}
            {me}
            onlead={(i) => void setLead(i)}
          />
          {#if shown}
            <WaveTable rows={shown.jumps} {win} who={names[me]} keepStep={numbers.keep} upStep={numbers.up} />
          {:else}
            <WaveTable total={yours.total} {win} {names} {hues} {ships} keepStep={numbers.keep} upStep={numbers.up} />
          {/if}
        {:else if guide.jumps.length}
          <WaveTable rows={guide.jumps} {win} keepStep={numbers.keep} upStep={numbers.up} />
        {/if}
      </div>
    </div>
  {:else}
    <p class="dim none">Even full throttle can't raise the charge.</p>
  {/if}
</NowCard>

<style>
  .for {
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  :global(.who) {
    font-size: var(--text-small);
  }
  .share {
    padding: 3px 9px;
    font-size: var(--text-small);
  }
  .soon {
    font-family: var(--font-mono);
    font-size: 0.58rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-faint);
  }
  .title {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px 12px;
    margin-top: -4px;
  }
  .big {
    font-family: var(--font-display);
    font-weight: var(--weight-bold);
    font-size: 1.9rem;
    line-height: 1;
    color: color-mix(in srgb, var(--good) 70%, var(--text));
  }
  .big.warn {
    color: color-mix(in srgb, var(--warn) 75%, var(--text));
  }
  .sub {
    font-weight: var(--weight-medium);
    color: var(--text-dim);
  }
  .fix {
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-width: 760px;
    padding: 9px 12px;
    border-radius: 9px;
    font-size: var(--text-small);
    color: var(--text-dim);
    background: color-mix(in srgb, var(--warn) 7%, transparent);
    border: 1px solid color-mix(in srgb, var(--warn) 26%, transparent);
  }
  .fix b {
    color: var(--text);
  }
  .fixes {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .fixes .btn {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 4px 10px;
  }
  .w {
    font-size: 0.74rem;
    color: var(--text-faint);
  }
  .gn {
    font-family: var(--font-mono);
    color: var(--good);
  }
  .fly {
    display: grid;
    grid-template-columns: minmax(0, 330px) minmax(0, 600px);
    gap: 40px;
    align-items: start;
    margin-top: 4px;
  }
  .fly.crew {
    grid-template-columns: minmax(0, 330px) minmax(0, 1fr);
    gap: 36px;
  }
  .bar {
    position: sticky;
    top: 12px;
    min-width: 0;
  }
  .beside {
    display: flex;
    flex-direction: column;
    gap: 18px;
    min-width: 0;
  }
  .none {
    margin: 0;
  }
  @media (max-width: 1000px) {
    .fly,
    .fly.crew {
      grid-template-columns: minmax(0, 1fr);
    }
    .bar {
      position: static;
    }
  }
</style>
