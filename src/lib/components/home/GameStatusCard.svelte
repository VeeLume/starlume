<script lang="ts">
  // Home's "can I play?" hero (a frameless card): one row per installed
  // channel over the LIVE patch-note image — the channel in its hue, the
  // version large (the first row also names the patch), and the checks as
  // pills: game data ready, where the build stands against the LIVE notes
  // (a newer build of the noted version is a hotfix that shipped after
  // them), and each enabled module's decoration (Text Patching's state).
  // Reads stores only; fetches nothing itself.
  import { onDestroy, onMount } from "svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { Button } from "@veelume/ui";
  import { ExternalLink } from "lucide-svelte";
  import { scStore } from "$lib/state/sc.svelte";
  import { dataStore } from "$lib/state/data.svelte";
  import { settingsStore } from "$lib/state/settings.svelte";
  import { activityFor, activityLabel } from "$lib/state/activities.svelte";
  import { installStatuses } from "$lib/home/installStatus";
  import { openPatchNote, patchNotesStore, watchPatchNotes } from "$lib/state/patchnotes.svelte";
  import { buildStatus, installChangelist, installVersion, type BuildStatus } from "$lib/home/version";
  import { channelHue } from "$lib/channels";

  const installs = $derived(scStore.status?.installs ?? []);
  const decorations = $derived(installStatuses(settingsStore.current?.enabled_modules ?? []));
  const latest = $derived(patchNotesStore.latestLive);
  const image = $derived(latest?.image ? convertFileSrc(latest.image) : null);

  const rows = $derived(
    installs.map((install) => {
      const key = install.channel.toLowerCase();
      const data = dataStore.statuses.find((s) => s.channel.toLowerCase() === key);
      return {
        key,
        channel: install.channel.toUpperCase(),
        version: installVersion(install.version),
        build: installChangelist(install.version),
        ready: data ? data.loaded || data.predicted_tier === "processed" : null,
        // Only LIVE compares with the LIVE notes.
        patch:
          key === "live" && latest?.version
            ? buildStatus(install.version, latest.version, latest.changelist)
            : null,
      };
    }),
  );

  const patchText = (s: BuildStatus, build: string | null): string =>
    s === "current"
      ? `Current · ${latest?.version}`
      : s === "behind"
        ? `${latest?.version} is out`
        : s === "hotfix"
          ? `Hotfix build${build ? ` ${build}` : ""}`
          : `Newer than the notes (${latest?.version})`;

  let stop: (() => void) | undefined;
  onMount(() => (stop = watchPatchNotes()));
  onDestroy(() => stop?.());
</script>

<section class="status" class:with-image={!!image}>
  {#if image}<img class="backdrop" src={image} alt="" />{/if}
  <div class="body">
    <p class="label">Game status</p>

    {#if !scStore.loaded}
      <p class="dim">Looking for Star Citizen…</p>
    {:else if rows.length === 0}
      <p class="dim">
        No Star Citizen installation found. Install it through the RSI launcher, then run setup again
        (Settings → Re-run setup) if Starlume still doesn't see it.
      </p>
    {/if}

    {#each rows as row, i (row.key)}
      {@const work = activityFor("data-load", row.key)}
      <div class="install" class:primary={i === 0} style:--hue={channelHue(row.key)}>
        <div class="line">
          <span class="hue-chip">{row.channel}</span>
          <span class="version">{row.version}</span>
          {#if row.key === "live" && latest?.name && row.patch !== "behind"}
            <span class="name">{latest.name}</span>
          {/if}
        </div>
        <div class="pills">
          {#if work}
            <span class="status-pill busy"><i></i>{activityLabel(work)}</span>
          {:else if row.ready != null}
            <span class="status-pill" class:good={row.ready}><i></i>{row.ready ? "Game data ready" : "Game data not prepared yet"}</span>
          {/if}
          {#if row.patch}
            <span
              class="status-pill"
              class:good={row.patch === "current"}
              class:warn={row.patch === "behind"}
              class:hotfix={row.patch === "hotfix"}
              title={row.patch === "hotfix"
                ? `Build ${row.build} is newer than the build in the ${latest?.version} notes (${latest?.changelist}): a hotfix shipped after them.`
                : undefined}><i></i>{patchText(row.patch, row.build)}</span
            >
          {/if}
          {#each decorations as d (d.key)}
            <d.component channelKey={row.key} />
          {/each}
          {#if row.build && row.patch !== "hotfix"}<span class="build">build {row.build}</span>{/if}
        </div>
      </div>
    {/each}

    {#if latest}
      <div>
        <Button variant="outline" onclick={() => void openPatchNote(latest.article_url)}>
          Patch notes <ExternalLink size={14} />
        </Button>
      </div>
    {/if}
  </div>
</section>

<style>
  .status {
    --hue: var(--hue-live);
    position: relative;
    overflow: hidden;
    border-radius: var(--radius-lg);
    background: var(--bg-raised);
    border: 1px solid color-mix(in srgb, var(--hue) 30%, var(--border));
  }
  .backdrop {
    position: absolute;
    inset: 0 0 0 auto;
    width: 62%;
    height: 100%;
    object-fit: cover;
  }
  .with-image::after {
    content: "";
    position: absolute;
    inset: 0;
    background: linear-gradient(
      90deg,
      color-mix(in srgb, var(--hue) 10%, var(--bg-raised)) 38%,
      color-mix(in srgb, var(--bg-raised) 70%, transparent) 62%,
      transparent
    );
  }
  .body {
    position: relative;
    z-index: 1;
    display: flex;
    flex-direction: column;
    gap: 12px;
    max-width: 620px;
    padding: 14px 18px 16px;
  }
  .label {
    margin: 0;
    font-family: var(--font-display);
    font-size: 0.75rem;
    color: var(--text-faint);
    text-transform: var(--label-transform);
    letter-spacing: var(--label-tracking);
  }
  .install {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .install + .install {
    padding-top: 10px;
    border-top: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
  }
  .line {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 6px 14px;
  }
  .version {
    font-family: var(--font-display);
    font-weight: var(--weight-bold);
    font-size: 1.5rem;
    line-height: 1;
    color: color-mix(in srgb, var(--hue) 55%, var(--text-strong));
  }
  .primary .version {
    font-size: 2.4rem;
  }
  .name {
    font-weight: var(--weight-semibold);
    font-size: 1.05rem;
    color: var(--text-strong);
  }
  .pills {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 8px;
  }
  .status-pill.hotfix {
    --dot: var(--hue-hotfix);
    color: color-mix(in srgb, var(--hue-hotfix) 85%, var(--text));
    border-color: color-mix(in srgb, var(--hue-hotfix) 40%, transparent);
  }
  .build {
    font-family: var(--font-mono);
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  @container (width < 640px) {
    .backdrop {
      width: 100%;
      opacity: 0.3;
    }
  }
</style>
