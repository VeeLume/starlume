<script lang="ts">
  // Text Patching's pill on Home's Game status card: the install's patch
  // state, or the work in flight for it, as one `.status-pill` like the
  // card's other checks. Off-channels say so quietly.
  import { onDestroy, onMount } from "svelte";
  import type { InstallStatusProps } from "../types";
  import { langpatchStore, watchLangpatch } from "$lib/state/langpatch.svelte";
  import { activityFor, activityLabel } from "$lib/state/activities.svelte";

  let { channelKey }: InstallStatusProps = $props();

  // The page's badge labels (status.ts) read in a table column under
  // "Text"; a pill stands alone, so it names its subject.
  const PILLS: Record<string, { text: string; tone: "good" | "warn" | "" }> = {
    "up-to-date": { text: "Text patched", tone: "good" },
    stale: { text: "Text re-patch pending", tone: "warn" },
    foreign: { text: "Text patch paused — modified by another tool", tone: "warn" },
    unpatched: { text: "Text not patched", tone: "" },
  };

  const install = $derived(langpatchStore.overview?.installs.find((i) => i.channel_key === channelKey));
  // Same precedence as the Text Patching page: live work beats the stored state.
  const work = $derived(
    install?.selected
      ? (activityFor("langpatch", channelKey) ??
          (install.state === "stale" ? activityFor("data-load", channelKey) : undefined))
      : undefined,
  );
  const pill = $derived(install ? (PILLS[install.state] ?? { text: `Text: ${install.state}`, tone: "" }) : null);

  let stop: (() => void) | undefined;
  onMount(() => (stop = watchLangpatch()));
  onDestroy(() => stop?.());
</script>

<!-- The overview's first read can take seconds while a startup reconcile
     runs; later visits render the cached one at once (frontend rule 2). -->
{#if install}
  {#if !install.selected}
    <span class="status-pill"><i></i>Text patching off</span>
  {:else if work}
    <span class="status-pill busy"><i></i>{activityLabel(work)}</span>
  {:else if pill}
    <span class="status-pill {pill.tone}"><i></i>{pill.text}</span>
  {/if}
{/if}
