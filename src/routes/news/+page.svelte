<script lang="ts">
  // News — an app page, not a module's (decisions 2026-10-15): LIVE patch
  // notes and comm-links are public RSI pages, gated by the online switch
  // only; Home reads the patch notes too.
  //
  // Layout: the top row is "now" — the LIVE card, plus cards modules add
  // while enabled (`ModuleDescriptor.patchNotes`, e.g. the dev tracker's PTU
  // cycle). Below, side by side on a wide window: the newest comm-links and
  // the earlier releases, one line each, expandable. A short overview only;
  // the full texts are on RSI's site.
  import { onDestroy, onMount } from "svelte";
  import { Button } from "@veelume/ui";
  import { RefreshCw } from "lucide-svelte";
  import { moduleRegistry } from "$lib/modules/registry";
  import { settingsStore } from "$lib/state/settings.svelte";
  import { patchNotesStore, refreshPatchNotes, watchPatchNotes } from "$lib/state/patchnotes.svelte";
  import LiveNow from "$lib/components/patchnotes/LiveNow.svelte";
  import ReleaseList from "$lib/components/patchnotes/ReleaseList.svelte";
  import CommLinkList from "$lib/components/news/CommLinkList.svelte";
  import { refreshCommLinks, watchCommLinks } from "$lib/state/commlinks.svelte";
  import { postAge } from "$lib/modules/devtracker/time";

  const overview = $derived(patchNotesStore.overview);
  const cards = $derived(
    moduleRegistry
      .filter((d) => settingsStore.current?.enabled_modules.includes(d.id))
      .flatMap((d) => (d.patchNotes ?? []).map((c) => ({ ...c, key: `${d.id}:${c.id}` }))),
  );

  let checking = $state(false);
  async function checkNow() {
    checking = true;
    await Promise.all([refreshPatchNotes(), refreshCommLinks()]);
    checking = false;
  }

  let stops: (() => void)[] = [];
  onMount(() => (stops = [watchPatchNotes(), watchCommLinks()]));
  onDestroy(() => stops.forEach((s) => s()));
</script>

<div class="page">
  <header class="bar">
    <span class="grow"></span>
    {#if overview?.fetched_at}
      <span class="dim small">Checked {postAge(overview.fetched_at * 1000)}</span>
    {/if}
    {#if overview?.online}
      <Button variant="ghost" disabled={checking} onclick={() => void checkNow()}>
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

  <div class="columns">
    <section>
      <h2>Comm-links</h2>
      <CommLinkList />
    </section>
    {#if overview?.releases.length || overview?.older.length}
      <section>
        <h2>Earlier releases</h2>
        <ReleaseList />
      </section>
    {/if}
  </div>
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
  .columns {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(420px, 1fr));
    gap: 20px;
    align-items: start;
  }
  section {
    min-width: 0;
  }
  h2 {
    margin: 8px 0 6px;
    font-size: 16px;
  }
</style>
