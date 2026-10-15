<script lang="ts">
  // The dev tracker's card on the Patch Notes page: the current PTU cycle
  // from the Patch Notes forum threads in its staff feed (complete notes,
  // unlike Spectrum's LIVE notices, which the page leaves to RSI's
  // articles). Muted once LIVE has caught up with the cycle's version.
  import { onDestroy, onMount } from "svelte";
  import { Button } from "@veelume/ui";
  import { ExternalLink } from "lucide-svelte";
  import { devtrackerStore, openPost, watchDevtracker } from "$lib/state/devtracker.svelte";
  import { patchNotesStore } from "$lib/state/patchnotes.svelte";
  import { compareVersions } from "$lib/home/version";
  import NowCard from "$lib/components/patchnotes/NowCard.svelte";
  import GateNotice from "./GateNotice.svelte";

  const overview = $derived(devtrackerStore.overview);
  const notes = $derived((overview?.patch_notes ?? []).filter((n) => n.channel && n.channel !== "LIVE"));
  const latest = $derived(notes[0] ?? null);
  // The cycle: every build of the latest note's version and channel.
  const cycle = $derived(
    latest ? notes.filter((n) => n.version === latest.version && n.channel === latest.channel) : [],
  );
  const live = $derived(patchNotesStore.latestLive?.version ?? null);
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
    version={[latest.version, latest.stage].filter(Boolean).join(" ")}
    date={day(latest.time_ms)}
    muted={released}
  >
    {#if released}<div class="dim">Released to LIVE as {live}.</div>{/if}
    {#if latest.build}<div class="dim small">build <span class="mono">{latest.build}</span></div>{/if}
    <div class="small">
      {cycle.map((n) => `${n.stage ?? n.channel} ${day(n.time_ms)}`).join(" · ")}
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
  p {
    margin: 0;
  }
</style>
