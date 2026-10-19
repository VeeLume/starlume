<script lang="ts">
  // Onboarding: online + unofficial-API privacy posture. A framework step (not
  // module-specific) — every SC feature respects these switches. Nothing is
  // required here, so the wizard's Next stays enabled; we only surface the
  // choice early so the posture is deliberate.
  import { ask } from "@tauri-apps/plugin-dialog";
  import { Switch } from "@veelume/ui";
  import { commands, type GrpcFeatureInfo } from "$lib/bindings";
  import type { OnboardingStepProps } from "$lib/modules/types";
  import { settingsStore, applySettings } from "$lib/state/settings.svelte";
  import { UNOFFICIAL_CONSENT } from "$lib/unofficial";
  import { onMount } from "svelte";

  // Typed for the step contract; this step never blocks Next (all optional).
  const {}: OnboardingStepProps = $props();

  const settings = $derived(settingsStore.current);
  let grpcFeatures = $state<GrpcFeatureInfo[]>([]);

  onMount(async () => {
    grpcFeatures = await commands.listGrpcFeatures();
  });

  async function toggleGrpc(on: boolean) {
    if (on && settings && !settings.grpc_consented) {
      const consent = await ask(UNOFFICIAL_CONSENT.message, {
        title: UNOFFICIAL_CONSENT.title,
        kind: "warning",
      });
      if (!consent) return;
    }
    await applySettings({ grpc_enabled: on });
  }

  function toggleGrpcFeature(id: string, on: boolean) {
    const current = settings?.grpc_features ?? [];
    const next = on ? [...current, id] : current.filter((f) => f !== id);
    void applySettings({ grpc_features: next });
  }
</script>

{#if settings}
  <p class="lead">
    Starlume works fully offline. You decide what — if anything — it may reach
    out to the network for. You can change all of this later in Settings.
  </p>

  <label class="row">
    <Switch
      label="Enable online features"
      checked={settings.online_enabled}
      onchange={(v) => applySettings({ online_enabled: v })}
    />
    <span>
      <strong>Enable online features</strong>
      <span class="dim">RSI profile lookups and the unofficial-API features you opt into below. Off = no network at all (except update checks).</span>
    </span>
  </label>

  <label class="row" class:disabled={!settings.online_enabled}>
    <Switch
      label="Allow unofficial API calls"
      checked={settings.grpc_enabled}
      disabled={!settings.online_enabled}
      onchange={(v) => toggleGrpc(v)}
    />
    <span>
      <strong>Allow unofficial API calls</strong>
      <span class="dim">Reads your data from RSI and CIG services Starlume is not an official client of. ToS-grey, read-only, opt-in per feature.</span>
    </span>
  </label>

  {#if grpcFeatures.length > 0}
    <div class="features" class:disabled={!settings.online_enabled || !settings.grpc_enabled}>
      {#each grpcFeatures as f (f.id)}
        <label class="feature">
          <Switch
            label={f.name}
            checked={settings.grpc_features.includes(f.id)}
            disabled={!settings.online_enabled || !settings.grpc_enabled}
            onchange={(v) => toggleGrpcFeature(f.id, v)}
          />
          {f.name} <span class="dim">— {f.description}</span>
        </label>
      {/each}
    </div>
  {/if}
{/if}

<style>
  .lead {
    margin-top: 0;
  }
  .row {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    padding: 8px 0;
  }
  .row span {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .features {
    margin-left: 26px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .feature {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .disabled {
    opacity: 0.55;
  }
  .dim {
    color: var(--text-dim);
    font-size: 0.85em;
  }
</style>
