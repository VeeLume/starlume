<script lang="ts">
  // The dev tracker's section on the Patch Notes page: the Patch Notes forum
  // threads from its staff feed. PTU / EPTU notes there are complete; the
  // LIVE thread is only a notice (the full LIVE notes are the page's own,
  // from RSI's patch-note articles). Behind the dev tracker's opt-in.
  import { onDestroy, onMount } from "svelte";
  import { devtrackerStore, watchDevtracker } from "$lib/state/devtracker.svelte";
  import GateNotice from "./GateNotice.svelte";
  import PatchNotes from "./PatchNotes.svelte";

  const overview = $derived(devtrackerStore.overview);

  let stop: (() => void) | undefined;
  onMount(() => (stop = watchDevtracker()));
  onDestroy(() => stop?.());
</script>

{#if overview?.gate.state === "closed"}
  <GateNotice reason={overview.gate.reason} compact />
{:else if overview}
  <PatchNotes notes={overview.patch_notes} />
{/if}
