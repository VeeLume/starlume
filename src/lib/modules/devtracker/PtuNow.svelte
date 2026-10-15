<script lang="ts">
  // The dev tracker's card on the News page: the current PTU cycle
  // from the Patch Notes forum threads in its staff feed (complete notes,
  // unlike Spectrum's LIVE notices, which the page leaves to RSI's
  // articles): version and stage, the thread title, the build, and what
  // LIVE is on — in the LIVE hue once LIVE has caught up with the cycle.
  import { onDestroy, onMount } from "svelte";
  import { Button } from "@veelume/ui";
  import { ExternalLink } from "lucide-svelte";
  import { devtrackerStore, openPost, watchDevtracker } from "$lib/state/devtracker.svelte";
  import { patchNotesStore } from "$lib/state/patchnotes.svelte";
  import { compareVersions } from "$lib/home/version";
  import { shortDate } from "$lib/components/patchnotes/format";
  import NowCard from "$lib/components/patchnotes/NowCard.svelte";
  import GateNotice from "./GateNotice.svelte";

  const overview = $derived(devtrackerStore.overview);
  const notes = $derived((overview?.patch_notes ?? []).filter((n) => n.channel && n.channel !== "LIVE"));
  const latest = $derived(notes[0] ?? null);
  const live = $derived(patchNotesStore.latestLive?.version ?? null);
  const liveDate = $derived(patchNotesStore.latestLive?.date ?? null);
  const released = $derived(
    !!latest?.version && !!live && compareVersions(live, latest.version) >= 0,
  );

  const day = (ms: number) => new Date(ms).toLocaleDateString([], { month: "short", day: "numeric" });

  let stop: (() => void) | undefined;
  onMount(() => (stop = watchDevtracker()));
  onDestroy(() => stop?.());
</script>

{#if overview?.gate.state === "closed"}
  <NowCard channel="PTU" version="—" muted>
    <GateNotice reason={overview.gate.reason} compact />
  </NowCard>
{:else if latest}
  <NowCard
    channel={latest.channel ?? "PTU"}
    version={latest.version ?? "?"}
    name={latest.stage ?? ""}
    date={day(latest.time_ms)}
  >
    <div class="dim small subject">{latest.subject}</div>
    <div class="stat-tiles">
      {#if latest.build}<div class="stat-tile"><b>{latest.build}</b><span>build</span></div>{/if}
      {#if released}
        <div class="stat-tile live">
          <b>LIVE</b><span>since {shortDate(liveDate, live ?? "")}</span>
        </div>
      {:else if live}
        <div class="stat-tile"><b>{live}</b><span>on LIVE</span></div>
      {/if}
    </div>
    {#snippet action()}
      <Button variant="outline" onclick={() => void openPost(latest.key)}>
        Open on Spectrum <ExternalLink size={14} />
      </Button>
    {/snippet}
  </NowCard>
{:else if overview}
  <NowCard channel="PTU" version="—" muted>
    <p class="dim">No PTU patch notes in the dev tracker's feed yet.</p>
  </NowCard>
{/if}

<style>
  .small {
    font-size: var(--text-small);
  }
  .subject {
    overflow-wrap: anywhere;
  }
  .stat-tile.live b {
    color: color-mix(in srgb, var(--hue-live) 80%, var(--text));
  }
  p {
    margin: 0;
  }
</style>
