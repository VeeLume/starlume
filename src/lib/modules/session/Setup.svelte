<script lang="ts">
  // Session Log's setup (ModuleDescriptor.setup): the two sources it reads,
  // both off by default. An onboarding step and the module's What's-new page,
  // so turning the module on is never a page that stays empty. The same
  // opt-ins, the status and Remove live on Settings → Game data.
  import { onMount } from "svelte";
  import { Button, Settings, Switch } from "@veelume/ui";
  import type { OnboardingStepProps } from "../types";
  import { applySettings, loadSettings, settingsStore } from "$lib/state/settings.svelte";
  import { installTelemetryEnv, telemetryStore, watchTelemetry } from "$lib/state/telemetry.svelte";

  // Typed for the step contract; nothing here is required.
  const {}: OnboardingStepProps = $props();

  const settings = $derived(settingsStore.current);
  let error = $state("");
  let envBusy = $state(false);

  onMount(() => {
    if (!settingsStore.current) void loadSettings();
    return watchTelemetry();
  });

  async function apply(patch: Parameters<typeof applySettings>[0]) {
    error = "";
    const err = await applySettings(patch);
    if (err) error = err.message;
  }

  async function install() {
    envBusy = true;
    await installTelemetryEnv();
    envBusy = false;
  }
</script>

{#if settings}
  <Settings.Section>
    <Settings.Row
      label="Follow the game's log"
      hint="Shards, places, contracts and purchases, from Game.log in your install folder."
    >
      <Switch
        label="Follow the game's log"
        checked={settings.gamelog_enabled}
        onchange={(v) => void apply({ gamelog_enabled: v })}
      />
    </Settings.Row>
    <Settings.Row
      label="Read the game's telemetry"
      hint="Your ship and your aUEC balance, from the running game on this PC."
    >
      <Switch
        label="Read the game's telemetry"
        checked={settings.telemetry_enabled}
        onchange={(v) => void apply({ telemetry_enabled: v })}
      />
    </Settings.Row>
    {#if settings.telemetry_enabled && telemetryStore.env === "not_installed"}
      <Settings.Row
        label="Make the game send it"
        hint="The game sends telemetry only when the launcher starts with GRPC_CLIENT_SERVER_ENABLED=true. Install sets it for your Windows user; restart the launcher afterwards. Or start the launcher from Starlume's Home."
      >
        <Button variant="outline" disabled={envBusy} onclick={() => void install()}>Install</Button>
      </Settings.Row>
    {/if}
  </Settings.Section>
  <p class="dim">Both read files and the game on this PC only; nothing is sent anywhere.</p>
  {#if error || telemetryStore.error}
    <p class="text-sm text-destructive">{error || telemetryStore.error}</p>
  {/if}
{:else}
  <p class="dim">Loading…</p>
{/if}
