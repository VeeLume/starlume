<script lang="ts">
  // Text Patching's column on Home's Game status card: the install's patch
  // state, or the work in flight for it. Off-channels say so quietly.
  import { onDestroy, onMount } from "svelte";
  import { StatusBadge } from "@veelume/ui";
  import type { InstallStatusProps } from "../types";
  import { langpatchStore, watchLangpatch } from "$lib/state/langpatch.svelte";
  import { activityFor, activityLabel } from "$lib/state/activities.svelte";
  import { installStateMap } from "./status";

  let { channelKey }: InstallStatusProps = $props();

  const install = $derived(langpatchStore.overview?.installs.find((i) => i.channel_key === channelKey));
  // Same precedence as the Text Patching page: live work beats the stored state.
  const work = $derived(
    install?.selected
      ? (activityFor("langpatch", channelKey) ??
          (install.state === "stale" ? activityFor("data-load", channelKey) : undefined))
      : undefined,
  );

  let stop: (() => void) | undefined;
  onMount(() => (stop = watchLangpatch()));
  onDestroy(() => stop?.());
</script>

<!-- The overview's first read can take seconds while a startup reconcile
     runs; later visits render the cached one at once (frontend rule 2). -->
{#if !langpatchStore.overview}
  <span class="dim">…</span>
{:else if install}
  {#if !install.selected}
    <span class="dim">Off</span>
  {:else if work}
    <span class="activity-note">⟳ {activityLabel(work)}</span>
  {:else}
    <StatusBadge status={install.state} map={installStateMap} />
  {/if}
{/if}
