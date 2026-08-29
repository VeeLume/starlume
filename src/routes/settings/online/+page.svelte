<script lang="ts">
  import { ask } from "@tauri-apps/plugin-dialog";
  import { Settings, Switch } from "@veelume/ui";
  import { commands, type GrpcFeatureInfo } from "$lib/bindings";
  import { settingsStore, applySettings, loadSettings } from "$lib/state/settings.svelte";
  import { onMount } from "svelte";

  const settings = $derived(settingsStore.current);
  let error = $state("");
  let grpcFeatures = $state<GrpcFeatureInfo[]>([]);

  onMount(async () => {
    await loadSettings();
    grpcFeatures = await commands.listGrpcFeatures();
  });

  async function apply(patch: Parameters<typeof applySettings>[0]) {
    error = "";
    const err = await applySettings(patch);
    if (err) error = err.message;
  }

  async function toggleGrpc(on: boolean) {
    // First enable shows the one-time ToS consent (the Hearth pattern);
    // the backend records grpc_consented on the transition.
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
    await apply({ grpc_enabled: on });
  }

  function toggleGrpcFeature(id: string, on: boolean) {
    const current = settings?.grpc_features ?? [];
    const next = on ? [...current, id] : current.filter((f) => f !== id);
    void apply({ grpc_features: next });
  }
</script>

<Settings.Page title="Online & privacy">
  {#if settings}
    <Settings.Section>
      <Settings.Row
        label="Enable online features"
        hint="Master switch — off means no network calls at all (update checks excepted)."
      >
        <Switch
          label="Enable online features"
          checked={settings.online_enabled}
          onchange={(v) => apply({ online_enabled: v })}
        />
      </Settings.Row>
    </Settings.Section>

    <Settings.Section title="Game services (gRPC)">
      <Settings.Row
        label="Allow game-services calls"
        hint="ToS-grey, read-only, opt-in per feature below."
      >
        <Switch
          label="Allow game-services calls"
          checked={settings.grpc_enabled}
          disabled={!settings.online_enabled}
          onchange={(v) => toggleGrpc(v)}
        />
      </Settings.Row>
      {#if grpcFeatures.length === 0}
        <p class="text-sm text-muted-foreground">
          No game-services features in this build yet — per-feature toggles appear here as
          they land (blueprints, missions, …).
        </p>
      {:else}
        {#each grpcFeatures as f (f.id)}
          <Settings.Row label={f.name} hint={f.description}>
            <Switch
              label={f.name}
              checked={settings.grpc_features.includes(f.id)}
              disabled={!settings.online_enabled || !settings.grpc_enabled}
              onchange={(v) => toggleGrpcFeature(f.id, v)}
            />
          </Settings.Row>
        {/each}
      {/if}
    </Settings.Section>

    {#if error}
      <p class="text-sm text-destructive">{error}</p>
    {/if}
  {/if}
</Settings.Page>
