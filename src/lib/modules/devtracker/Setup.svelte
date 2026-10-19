<script lang="ts">
  // The Dev Tracker's setup (ModuleDescriptor.setup): what reaches you while
  // Starlume is in the tray. An onboarding step and the module's What's-new
  // page. Following staff stays on the page: the feed is empty until the
  // unofficial-API feature is allowed and the first poll ran.
  import { onMount } from "svelte";
  import { Settings, Switch } from "@veelume/ui";
  import type { OnboardingStepProps } from "../types";
  import { devtrackerStore, loadDevtracker, setNotifyOps } from "$lib/state/devtracker.svelte";

  // Typed for the step contract; nothing here is required.
  const {}: OnboardingStepProps = $props();

  const overview = $derived(devtrackerStore.overview);

  onMount(() => void loadDevtracker());
</script>

{#if overview}
  <Settings.Section>
    <Settings.Row
      label="Notify me about game-ops posts"
      hint="A server restart, a PTU wave, the HOTFIX channel opening."
    >
      <Switch
        label="Notify me about game-ops posts"
        checked={overview.notify_ops}
        onchange={(v) => void setNotifyOps(v)}
      />
    </Settings.Row>
  </Settings.Section>
{:else}
  <p class="dim">Loading…</p>
{/if}
