<script lang="ts">
  // Onboarding: online + gRPC privacy posture. A framework step (not
  // module-specific) — every SC feature respects these switches. Nothing is
  // required here, so the wizard's Next stays enabled; we only surface the
  // choice early so the posture is deliberate.
  import { ask } from "@tauri-apps/plugin-dialog";
  import { commands, type GrpcFeatureInfo } from "$lib/bindings";
  import type { OnboardingStepProps } from "$lib/modules/types";
  import { settingsStore, applySettings } from "$lib/state/settings.svelte";
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
      const consent = await ask(
        "Live game-services sync connects to CIG's backend using your launcher " +
          "session. This is not an official API — it sits in a ToS-grey area. " +
          "Calls are read-only, manual or startup-only, never polled. " +
          "Your account, your risk.\n\nEnable game-services calls?",
        { title: "Game-services (gRPC) consent", kind: "warning" },
      );
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
    <input
      type="checkbox"
      checked={settings.online_enabled}
      onchange={(e) => applySettings({ online_enabled: e.currentTarget.checked })}
    />
    <span>
      <strong>Enable online features</strong>
      <span class="dim">Discord sign-in, RSI profile lookups, community. Off = no network at all (except update checks).</span>
    </span>
  </label>

  <label class="row" class:disabled={!settings.online_enabled}>
    <input
      type="checkbox"
      checked={settings.grpc_enabled}
      disabled={!settings.online_enabled}
      onchange={(e) => toggleGrpc(e.currentTarget.checked)}
    />
    <span>
      <strong>Allow game-services (gRPC) calls</strong>
      <span class="dim">Reads live account data from CIG's backend. ToS-grey, read-only, opt-in per feature.</span>
    </span>
  </label>

  {#if grpcFeatures.length > 0}
    <div class="features" class:disabled={!settings.online_enabled || !settings.grpc_enabled}>
      {#each grpcFeatures as f (f.id)}
        <label class="feature">
          <input
            type="checkbox"
            checked={settings.grpc_features.includes(f.id)}
            disabled={!settings.online_enabled || !settings.grpc_enabled}
            onchange={(e) => toggleGrpcFeature(f.id, e.currentTarget.checked)}
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
