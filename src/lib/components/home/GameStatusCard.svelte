<script lang="ts">
  // Home's "can I play?" card: one row per installed channel — the build,
  // whether its game data is ready, and a column per enabled module that
  // decorates installs (Text Patching's patch state). The LIVE row also says
  // whether it is on the newest version the patch notes announce. Reads
  // stores only; fetches nothing itself.
  import { onDestroy, onMount } from "svelte";
  import { StatusBadge, type StatusMap } from "@veelume/ui";
  import { scStore } from "$lib/state/sc.svelte";
  import { dataStore } from "$lib/state/data.svelte";
  import { settingsStore } from "$lib/state/settings.svelte";
  import { activityFor, activityLabel } from "$lib/state/activities.svelte";
  import { installStatuses } from "$lib/home/installStatus";
  import { patchNotesStore, watchPatchNotes } from "$lib/state/patchnotes.svelte";
  import { upToDate } from "$lib/home/version";

  const installs = $derived(scStore.status?.installs ?? []);
  const decorations = $derived(installStatuses(settingsStore.current?.enabled_modules ?? []));

  const rows = $derived(
    installs.map((install) => {
      const key = install.channel.toLowerCase();
      return {
        key,
        // Display casing the way the rest of the app shows it ("Live").
        name: key.charAt(0).toUpperCase() + key.slice(1),
        version: install.version,
        data: dataStore.statuses.find((s) => s.channel.toLowerCase() === key),
      };
    }),
  );

  const latest = $derived(patchNotesStore.latestLive);

  // Versions only (see version.ts): a hotfix build of the announced version
  // is current.
  const patchMap: StatusMap = {
    current: { label: () => `Current · ${latest?.version}`, tone: "success" },
    behind: { label: () => `${latest?.version} is out`, tone: "warning" },
    ahead: { label: () => `Newer than the notes (${latest?.version})`, tone: "neutral" },
  };

  let stop: (() => void) | undefined;
  onMount(() => (stop = watchPatchNotes()));
  onDestroy(() => stop?.());

  // A processed snapshot on disk loads in about a second — as good as loaded.
  const dataMap: StatusMap = {
    ready: { label: () => "Ready", tone: "success" },
    pending: { label: () => "Not prepared yet", tone: "neutral" },
  };
</script>

{#if !scStore.loaded}
  <p class="dim">Looking for Star Citizen…</p>
{:else if rows.length === 0}
  <p class="dim">
    No Star Citizen installation found. Install it through the RSI launcher, then run setup again
    (Settings → Re-run setup) if Starlume still doesn't see it.
  </p>
{:else}
  <table class="data">
    <thead>
      <tr>
        <th>Channel</th>
        <th>Build</th>
        <th>Game data</th>
        {#if latest?.version}<th>Patch</th>{/if}
        {#each decorations as d (d.key)}<th>{d.label}</th>{/each}
      </tr>
    </thead>
    <tbody>
      {#each rows as row (row.key)}
        {@const work = activityFor("data-load", row.key)}
        <tr>
          <td class="channel">{row.name}</td>
          <td class="mono dim">{row.version}</td>
          <td>
            {#if work}
              <span class="activity-note">⟳ {activityLabel(work)}</span>
            {:else if row.data}
              <StatusBadge
                status={row.data.loaded || row.data.predicted_tier === "processed" ? "ready" : "pending"}
                map={dataMap}
              />
            {:else}
              <span class="dim">—</span>
            {/if}
          </td>
          {#if latest?.version}
            <td title={latest.name ? `${latest.title}: ${latest.name}` : latest.title}>
              {#if row.key === "live"}
                <StatusBadge status={upToDate(row.version, latest.version)} map={patchMap} />
              {/if}
            </td>
          {/if}
          {#each decorations as d (d.key)}
            <td><d.component channelKey={row.key} /></td>
          {/each}
        </tr>
      {/each}
    </tbody>
  </table>
{/if}

<style>
  .channel {
    font-family: var(--font-display);
    font-weight: var(--weight-bold);
  }
</style>
