<script lang="ts">
  // The dev tracker's card on the News page: the test channel. HOTFIX when
  // Hotfix Central says its channel is open — full size in the hotfix hue,
  // with what is on it. Else PTU, EPTU or Tech Preview from the Patch Notes
  // forum threads in the staff feed: open (a build ahead of LIVE) at full
  // size with the thread title, the build and what LIVE is on; quiet (LIVE
  // caught up) as a compact card with one line — or, when a hotfix went to
  // LIVE after the notes, that hotfix. testChannel.ts holds the rules.
  import { onDestroy, onMount } from "svelte";
  import { Button } from "@veelume/ui";
  import { ExternalLink } from "lucide-svelte";
  import {
    devtrackerStore,
    openHotfixCentral,
    openPost,
    watchDevtracker,
  } from "$lib/state/devtracker.svelte";
  import { patchNotesStore } from "$lib/state/patchnotes.svelte";
  import { shortDate } from "$lib/components/patchnotes/format";
  import NowCard from "$lib/components/patchnotes/NowCard.svelte";
  import GateNotice from "./GateNotice.svelte";
  import { liveHotfixSince, testChannel } from "./testChannel";

  /** Hotfix lines on the open HOTFIX card before "and N more". */
  const MAX_ITEMS = 5;

  const overview = $derived(devtrackerStore.overview);
  const live = $derived(patchNotesStore.latestLive);
  const hotfix = $derived(overview?.hotfix ?? null);
  const test = $derived(
    testChannel(
      overview?.patch_notes ?? [],
      live ? { version: live.version, changelist: live.changelist } : null,
      hotfix,
    ),
  );
  const liveHotfix = $derived(liveHotfixSince(hotfix, live?.date ?? null));

  const day = (ms: number) => new Date(ms).toLocaleDateString([], { month: "short", day: "numeric" });

  let stop: (() => void) | undefined;
  onMount(() => (stop = watchDevtracker()));
  onDestroy(() => stop?.());
</script>

{#if overview?.gate.state === "closed"}
  <NowCard channel="PTU" version="—" size="compact">
    <GateNotice reason={overview.gate.reason} compact />
  </NowCard>
{:else if test?.kind === "hotfix"}
  <NowCard
    channel="HOTFIX"
    version={test.hotfix.version ?? "?"}
    name="Channel open"
    date={test.entry?.date ? shortDate(test.entry.date) : ""}
  >
    {#if test.entry}
      <div class="small headline">{test.entry.headline}</div>
      {#if test.entry.items.length}
        <ul>
          {#each test.entry.items.slice(0, MAX_ITEMS) as item, i (i)}<li>{item}</li>{/each}
        </ul>
        {#if test.entry.items.length > MAX_ITEMS}
          <div class="dim small">and {test.entry.items.length - MAX_ITEMS} more</div>
        {/if}
      {/if}
    {/if}
    {#snippet action()}
      <Button variant="outline" onclick={() => void openHotfixCentral()}>
        Hotfix Central <ExternalLink size={14} />
      </Button>
    {/snippet}
  </NowCard>
{:else if test?.open}
  {@const note = test.note}
  <NowCard
    channel={note.channel ?? "PTU"}
    version={note.version ?? "?"}
    name={note.stage ?? ""}
    date={day(note.time_ms)}
  >
    <div class="dim small subject">{note.subject}</div>
    <div class="stat-tiles">
      {#if note.build}<div class="stat-tile"><b>{note.build}</b><span>build</span></div>{/if}
      {#if live?.version}
        <div class="stat-tile live"><b>{live.version}</b><span>on LIVE</span></div>
      {/if}
    </div>
    {#snippet action()}
      <Button variant="outline" onclick={() => void openPost(note.key)}>
        Open on Spectrum <ExternalLink size={14} />
      </Button>
    {/snippet}
  </NowCard>
{:else if test}
  {@const note = test.note}
  <NowCard
    channel={note.channel ?? "PTU"}
    version={note.version ?? "?"}
    name={note.stage ?? ""}
    size="compact"
  >
    <div class="quiet">
      <span>
        Quiet · <span class="live-text">LIVE</span>
        {live?.date ? `since ${shortDate(live.date)}` : "caught up"}
      </span>
      <button type="button" class="open" title="Open the last notes on Spectrum" onclick={() => void openPost(note.key)}>
        <ExternalLink size={13} />
      </button>
    </div>
    {#if liveHotfix}
      <button type="button" class="hotfix-line" title="Open Hotfix Central" onclick={() => void openHotfixCentral()}>
        Hotfix {liveHotfix.builds[0] ?? ""} on LIVE{liveHotfix.date ? ` · ${shortDate(liveHotfix.date)}` : ""}
      </button>
    {/if}
  </NowCard>
{:else if overview}
  <NowCard channel="PTU" version="—" size="compact">
    <p class="dim small">No test-channel patch notes in the dev tracker's feed yet.</p>
  </NowCard>
{/if}

<style>
  .small {
    font-size: var(--text-small);
  }
  .subject {
    overflow-wrap: anywhere;
  }
  .headline {
    color: var(--text-dim);
  }
  ul {
    margin: 0;
    padding-left: 18px;
    list-style: disc;
    color: var(--text-dim);
  }
  li + li {
    margin-top: 2px;
  }
  .stat-tile.live b,
  .live-text {
    color: color-mix(in srgb, var(--hue-live) 80%, var(--text));
  }
  .quiet {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    font-size: var(--text-small);
    color: var(--text-dim);
  }
  .hotfix-line {
    text-align: left;
    font-size: var(--text-small);
    color: color-mix(in srgb, var(--hue-hotfix) 80%, var(--text));
  }
  .hotfix-line:hover {
    text-decoration: underline;
  }
  .open {
    display: inline-flex;
    color: var(--text-dim);
    padding: 2px;
    border-radius: var(--radius-chip);
  }
  .open:hover {
    color: var(--accent);
  }
  p {
    margin: 0;
  }
</style>
