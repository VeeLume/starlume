<script lang="ts">
  // How to crack it. Solo: what to do in the window as the title, the
  // charge bar in steps and how far one wave lifts the charge. With a crew
  // (more than one laser): "for" picks whose steps the bar and the title
  // show — All (the lead's, everyone's wave), or one operator (their steps,
  // their column, their own wave table) — beside the crew plan. The bar
  // grows with the card, capped at the window's height, and stays in view
  // while a long plan scrolls. Sharing the plan with the crew's apps waits
  // for the server.
  import { Segmented } from "@veelume/ui";
  import type { LoadoutView, MiningCatalogView, YourSetupView } from "$lib/bindings";
  import Monogram from "$lib/components/Monogram.svelte";
  import NowCard from "$lib/components/NowCard.svelte";
  import { distinctHues } from "$lib/monogram";
  import { miningStore, miningUi, setLead } from "$lib/state/mining.svelte";
  import ChargeBar from "./ChargeBar.svelte";
  import CrewPlan from "./CrewPlan.svelte";
  import { operatorNames, operatorPlace, steps as stepsOf } from "./format";
  import TotalWave from "./TotalWave.svelte";
  import WaveTable from "./WaveTable.svelte";

  let { catalog, yours, loadout }: { catalog: MiningCatalogView; yours: YourSetupView; loadout: LoadoutView } =
    $props();

  let besideHeight = $state(0);
  let viewHeight = $state(900);

  const guide = $derived(yours.guide);
  const steps = $derived(stepsOf(guide));
  const win = $derived(guide.windowHigh - guide.windowLow);
  const p = (v: number) => `${Math.round(v * 100)} %`;
  const barHeight = $derived(Math.round(Math.max(320, Math.min(besideHeight, viewHeight - 140))));

  const ops = $derived(yours.operators);
  const crew = $derived(ops.length > 1);
  const names = $derived(operatorNames(loadout.lasers));
  const hues = $derived(distinctHues(names));
  const saved = $derived(miningStore.state?.saved ?? []);
  const places = $derived(
    loadout.lasers.map((_, i) => operatorPlace(loadout.lasers, i, (id) => saved.find((s) => s.id === id)?.name)),
  );
  const mods = $derived(loadout.lasers.map((l) => l.modules));
  const me = $derived(miningUi.me >= 0 && miningUi.me < ops.length ? miningUi.me : -1);
  const leadIndex = $derived(Math.max(0, ops.findIndex((o) => o.lead)));

  const short = (a: string) => a.replace("Throttle up to", "Up to").replace("Keep throttle at", "Keep at");
  const supportCalls = (hold: number, full: boolean) =>
    steps.map((s) =>
      s.tone === "warn2" ? "Cut the beam" : s.tone === "bad" ? (full ? "It will overcharge" : "") : `Hold at ${p(hold)}`,
    );
  const calls = $derived(ops.map((o) => (o.lead ? steps.map((s) => short(s.act)) : supportCalls(o.hold ?? 1, false))));
  const cutAt = $derived(steps.findIndex((s) => s.tone === "warn2") + 1);

  const shown = $derived(me >= 0 ? ops[me] : null);
  const acts = $derived(shown && !shown.lead ? supportCalls(shown.hold ?? 1, true) : null);
  const holdStep = $derived(steps.find((s) => s.tone === "good") ?? null);
  const soloTitle = $derived(holdStep?.act ?? (guide.balance === null ? "Can't raise the charge" : "Climb"));
</script>

<svelte:window bind:innerHeight={viewHeight} />

<NowCard hue="var(--good)" chip="▲ HOW TO CRACK" version={crew ? "" : soloTitle} name={crew ? "" : `window, ${p(win)} of the bar`}>
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
  {#if crew}
    <div class="title">
      {#if shown}
        <Monogram name={names[me]} hue={hues[me]} size={32} />
        <span class="big">{shown.lead ? soloTitle : `Hold at ${p(shown.hold ?? 1)}`}</span>
        <span class="sub">{shown.lead ? `window, ${p(win)} of the bar` : cutAt ? `cut the beam at step ${cutAt}` : ""}</span>
      {:else}
        <span class="avs">{#each names as n, k (k)}<Monogram name={n} hue={hues[k]} size={28} />{/each}</span>
        <span class="big">{names[leadIndex] === "You" ? "You lead" : `${names[leadIndex]} leads`}</span>
        <span class="sub">the others hold, window {p(win)}</span>
      {/if}
    </div>
  {/if}
  {#if steps.length}
    <div class="fly" class:crew>
      <div class="bar"><ChargeBar {steps} {acts} {win} height={barHeight} /></div>
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
            <WaveTable rows={shown.jumps} {win} who={names[me]} />
          {:else}
            <TotalWave rows={yours.total} {win} {names} {hues} />
          {/if}
        {:else if guide.jumps.length}
          <WaveTable rows={guide.jumps} {win} />
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
    margin-top: -6px;
  }
  .big {
    font-family: var(--font-display);
    font-weight: var(--weight-bold);
    font-size: 1.7rem;
    line-height: 1;
    color: color-mix(in srgb, var(--good) 70%, var(--text));
  }
  .sub {
    font-weight: var(--weight-medium);
    color: var(--text-dim);
  }
  .avs {
    display: inline-flex;
  }
  .avs :global(.monogram) {
    box-shadow: 0 0 0 2px var(--bg-raised);
  }
  .avs :global(.monogram + .monogram) {
    margin-left: -8px;
  }
  .fly {
    display: grid;
    grid-template-columns: minmax(0, 330px) minmax(0, 560px);
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
