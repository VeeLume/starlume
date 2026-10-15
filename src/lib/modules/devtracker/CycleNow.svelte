<script lang="ts">
  // The dev tracker's card on the News page, beside LIVE: the dev cycle.
  // A stepper (PTU → RC → LIVE → Hotfixes) says where the newest version
  // stands; below, a timeline of the last 30 days across PTU waves and
  // release candidates (Patch Notes threads), LIVE releases (RSI's notes)
  // and hotfixes (Hotfix Central), each in its channel's hue and opening
  // its source. cycle.ts holds the rules.
  import { onDestroy, onMount } from "svelte";
  import {
    devtrackerStore,
    openHotfixCentral,
    openPost,
    watchDevtracker,
  } from "$lib/state/devtracker.svelte";
  import { openPatchNote, patchNotesStore, watchPatchNotes } from "$lib/state/patchnotes.svelte";
  import { channelHue } from "$lib/channels";
  import NowCard from "$lib/components/patchnotes/NowCard.svelte";
  import GateNotice from "./GateNotice.svelte";
  import { STAGES, devCycle, type CycleEvent, type CycleStage } from "./cycle";

  /** Timeline rows shown. */
  const MAX_EVENTS = 7;

  const STAGE: Record<CycleStage, { step: string; name: string; channel: string; hue: string }> = {
    ptu: { step: "PTU", name: "On PTU", channel: "PTU", hue: "var(--hue-ptu)" },
    rc: { step: "RC", name: "Release candidates", channel: "PTU", hue: "var(--hue-ptu)" },
    live: { step: "LIVE", name: "Released", channel: "LIVE", hue: "var(--hue-live)" },
    hotfix: { step: "Hotfixes", name: "Hotfixes", channel: "HOTFIX", hue: "var(--hue-hotfix)" },
  };

  const overview = $derived(devtrackerStore.overview);
  const cycle = $derived(
    devCycle(
      overview?.patch_notes ?? [],
      patchNotesStore.overview?.releases ?? [],
      overview?.hotfix ?? null,
      Date.now(),
    ),
  );
  const current = $derived(cycle ? STAGES.indexOf(cycle.stage) : -1);

  const hue = (e: CycleEvent) =>
    e.kind === "hotfix" ? "var(--hue-hotfix)" : (channelHue(e.channel) ?? "var(--text-dim)");
  const day = (ms: number) => new Date(ms).toLocaleDateString([], { month: "short", day: "numeric" });
  function open(e: CycleEvent) {
    if (e.target.type === "post") void openPost(e.target.key);
    else if (e.target.type === "notes") void openPatchNote(e.target.url);
    else void openHotfixCentral();
  }

  // Subscribe itself to both stores (ref-counted), like every surface.
  let stops: (() => void)[] = [];
  onMount(() => (stops = [watchDevtracker(), watchPatchNotes()]));
  onDestroy(() => stops.forEach((s) => s()));
</script>

{#if overview?.gate.state === "closed"}
  <NowCard channel="PTU" version="—" size="compact">
    <GateNotice reason={overview.gate.reason} compact />
  </NowCard>
{:else if cycle}
  {@const stage = STAGE[cycle.stage]}
  <NowCard channel={stage.channel} version={cycle.version ?? "?"} name={stage.name}>
    <ol class="stepper" aria-label="Dev cycle">
      {#each STAGES as s, i (s)}
        <li
          class:done={i < current}
          class:current={i === current}
          style:--hue={STAGE[s].hue}
          aria-current={i === current ? "step" : undefined}
        >
          <i></i><span>{STAGE[s].step}</span>
        </li>
      {/each}
    </ol>
    {#if cycle.hotfixOpen}
      <div><span class="hue-chip" style:--hue="var(--hue-hotfix)">HOTFIX channel open</span></div>
    {/if}
    <ol class="timeline">
      {#each cycle.events.slice(0, MAX_EVENTS) as e (e.key)}
        <li style:--hue={hue(e)}>
          <button type="button" onclick={() => open(e)} title={e.detail ?? e.title}>
            <span class="date">{day(e.time_ms)}</span>
            <span class="dot"></span>
            <span class="txt">
              <span class="title">{e.title}</span>
              {#if e.detail}<span class="detail">{e.detail}</span>{/if}
            </span>
          </button>
        </li>
      {/each}
    </ol>
  </NowCard>
{:else if overview}
  <NowCard channel="PTU" version="—" size="compact">
    <p class="dim small">No PTU, release or hotfix news in the last 30 days yet.</p>
  </NowCard>
{/if}

<style>
  .small {
    font-size: var(--text-small);
  }
  p {
    margin: 0;
  }

  /* Stepper: dots on a line, done steps filled, the current one ringed. */
  .stepper {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .stepper li {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 5px;
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .stepper li::before {
    content: "";
    position: absolute;
    top: 5px;
    right: 50%;
    width: 100%;
    height: 2px;
    background: var(--border);
  }
  .stepper li:first-child::before {
    display: none;
  }
  .stepper li.done::before,
  .stepper li.current::before {
    background: color-mix(in srgb, var(--hue) 55%, var(--border));
  }
  .stepper i {
    position: relative;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--bg-raised);
    border: 2px solid var(--border);
  }
  .stepper li.done i {
    background: color-mix(in srgb, var(--hue) 70%, transparent);
    border-color: transparent;
  }
  .stepper li.done {
    color: var(--text-dim);
  }
  .stepper li.current i {
    background: var(--hue);
    border-color: var(--hue);
    box-shadow: 0 0 0 4px color-mix(in srgb, var(--hue) 22%, transparent);
  }
  .stepper li.current {
    font-weight: var(--weight-semibold);
    color: color-mix(in srgb, var(--hue) 80%, var(--text));
  }

  /* Timeline: date, a dot on a vertical rule, title and one detail line. */
  .timeline {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .timeline li {
    position: relative;
  }
  .timeline li::before {
    content: "";
    position: absolute;
    top: 0;
    bottom: 0;
    left: calc(52px + 12px + 4px);
    width: 1px;
    background: var(--border);
  }
  .timeline li:first-child::before {
    top: 12px;
  }
  .timeline li:last-child::before {
    bottom: calc(100% - 12px);
  }
  .timeline button {
    all: unset;
    box-sizing: border-box;
    position: relative;
    display: grid;
    grid-template-columns: 52px 9px minmax(0, 1fr);
    align-items: start;
    gap: 0 12px;
    width: 100%;
    padding: 5px 4px 5px 0;
    border-radius: var(--radius);
    cursor: pointer;
  }
  .timeline button:hover {
    background: color-mix(in srgb, var(--text) 4%, transparent);
  }
  .timeline button:focus-visible {
    outline: 2px solid var(--accent);
  }
  .date {
    font-size: var(--text-small);
    color: var(--text-faint);
    text-align: right;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .dot {
    width: 9px;
    height: 9px;
    margin-top: 4px;
    border-radius: 50%;
    background: var(--hue);
    box-shadow: 0 0 0 3px var(--bg-raised);
  }
  .txt {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .title {
    font-size: 0.9rem;
    font-weight: var(--weight-medium);
    color: color-mix(in srgb, var(--hue) 45%, var(--text-strong));
  }
  .detail {
    font-size: var(--text-small);
    color: var(--text-faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
