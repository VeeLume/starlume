<script lang="ts">
  // The dev tracker's card on the News page: the test channel — PTU, EPTU,
  // Hotfix or Tech Preview — from the Patch Notes forum threads in its staff
  // feed (complete notes, unlike Spectrum's LIVE notices, which the page
  // leaves to RSI's articles). Open (a build ahead of LIVE): full size in
  // the channel's hue, the thread title, the build and what LIVE is on.
  // Quiet (LIVE caught up): a compact card, one line. testChannel.ts holds
  // the rules.
  import { onDestroy, onMount } from "svelte";
  import { Button } from "@veelume/ui";
  import { ExternalLink } from "lucide-svelte";
  import { devtrackerStore, openPost, watchDevtracker } from "$lib/state/devtracker.svelte";
  import { patchNotesStore } from "$lib/state/patchnotes.svelte";
  import { shortDate } from "$lib/components/patchnotes/format";
  import NowCard from "$lib/components/patchnotes/NowCard.svelte";
  import GateNotice from "./GateNotice.svelte";
  import { testChannel } from "./testChannel";

  const overview = $derived(devtrackerStore.overview);
  const live = $derived(patchNotesStore.latestLive);
  const test = $derived(
    testChannel(
      overview?.patch_notes ?? [],
      live ? { version: live.version, changelist: live.changelist } : null,
    ),
  );
  const note = $derived(test?.note ?? null);

  const day = (ms: number) => new Date(ms).toLocaleDateString([], { month: "short", day: "numeric" });

  let stop: (() => void) | undefined;
  onMount(() => (stop = watchDevtracker()));
  onDestroy(() => stop?.());
</script>

{#if overview?.gate.state === "closed"}
  <NowCard channel="PTU" version="—" size="compact">
    <GateNotice reason={overview.gate.reason} compact />
  </NowCard>
{:else if test?.open && note}
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
{:else if note}
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
