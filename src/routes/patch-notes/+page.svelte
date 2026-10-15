<script lang="ts">
  // Patch Notes — an app page, not a module's (decision 2026-10-15): LIVE
  // release notes are reference data from RSI's public patch-note articles,
  // gated by the online switch only, and Home reads them too.
  //
  // Layout: the top row is "now" — the LIVE card, plus cards modules add
  // while enabled (`ModuleDescriptor.patchNotes`, e.g. the dev tracker's PTU
  // cycle). Below, earlier releases one line each, expandable to their
  // highlights. A short overview only; the full notes are on RSI's site.
  import { onDestroy, onMount } from "svelte";
  import { Button } from "@veelume/ui";
  import { RefreshCw } from "lucide-svelte";
  import { moduleRegistry } from "$lib/modules/registry";
  import { settingsStore } from "$lib/state/settings.svelte";
  import { patchNotesStore, refreshPatchNotes, watchPatchNotes } from "$lib/state/patchnotes.svelte";
  import LiveNow from "$lib/components/patchnotes/LiveNow.svelte";
  import ReleaseList from "$lib/components/patchnotes/ReleaseList.svelte";
  import { postAge } from "$lib/modules/devtracker/time";

  const overview = $derived(patchNotesStore.overview);
  const cards = $derived(
    moduleRegistry
      .filter((d) => settingsStore.current?.enabled_modules.includes(d.id))
      .flatMap((d) => (d.patchNotes ?? []).map((c) => ({ ...c, key: `${d.id}:${c.id}` }))),
  );

  let stop: (() => void) | undefined;
  onMount(() => (stop = watchPatchNotes()));
  onDestroy(() => stop?.());
</script>

<div class="page">
  <header class="bar">
    <span class="grow"></span>
    {#if overview?.fetched_at}
      <span class="dim small">Checked {postAge(overview.fetched_at * 1000)}</span>
    {/if}
    {#if overview?.online}
      <Button variant="ghost" disabled={patchNotesStore.refreshing} onclick={() => void refreshPatchNotes()}>
        <RefreshCw size={14} /> Check now
      </Button>
    {/if}
  </header>

  {#if patchNotesStore.error}<p class="error">{patchNotesStore.error}</p>{/if}
  {#if overview?.last_error}<p class="error">{overview.last_error}</p>{/if}

  <div class="now">
    <LiveNow />
    {#each cards as c (c.key)}<c.component />{/each}
  </div>

  {#if overview?.releases.length || overview?.older.length}
    <h2>Earlier releases</h2>
    <ReleaseList />
  {/if}
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 36px;
  }
  .grow {
    flex: 1;
  }
  .small {
    font-size: var(--text-small);
  }
  .now {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(340px, 1fr));
    gap: 12px;
    align-items: stretch;
  }
  h2 {
    margin: 8px 0 0;
    font-size: 16px;
  }
</style>
