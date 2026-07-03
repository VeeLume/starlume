<script lang="ts">
  import { ask } from "@tauri-apps/plugin-dialog";
  import { commands, type GrpcFeatureInfo } from "$lib/bindings";
  import { moduleRegistry } from "$lib/modules/registry";
  import { settingsStore, applySettings, loadSettings } from "$lib/state/settings.svelte";
  import { authStore, loadAuth } from "$lib/state/auth.svelte";
  import { openOnboarding } from "$lib/state/onboarding.svelte";
  import { checkForUpdates } from "$lib/updater";
  import { onMount } from "svelte";

  let serverUrlInput = $state("");
  let error = $state("");
  let grpcFeatures = $state<GrpcFeatureInfo[]>([]);

  const settings = $derived(settingsStore.current);
  const auth = $derived(authStore.current);

  onMount(async () => {
    const s = await loadSettings();
    serverUrlInput = s.server_url ?? "";
    grpcFeatures = await commands.listGrpcFeatures();
    await loadAuth();
  });

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

  async function apply(patch: Parameters<typeof applySettings>[0]) {
    error = "";
    const err = await applySettings(patch);
    if (err) error = err.message;
    await loadAuth(); // server_url changes flip server_configured
  }

  function toggleModule(id: string, on: boolean) {
    const current = settings?.enabled_modules ?? [];
    const next = on ? [...current, id] : current.filter((m) => m !== id);
    void apply({ enabled_modules: next });
  }

  async function login() {
    error = "";
    const result = await commands.loginStart();
    if (result.status === "error") error = result.error.message;
  }

  async function logout() {
    error = "";
    const result = await commands.logout();
    if (result.status === "error") error = result.error.message;
    await loadAuth();
  }
</script>

<h1>Settings</h1>

{#if settings}
  <section>
    <h2>App</h2>
    <label>
      <input
        type="checkbox"
        checked={settings.close_to_tray}
        onchange={(e) => apply({ close_to_tray: e.currentTarget.checked })}
      />
      Close to tray (keep running in the background)
    </label>
    <label>
      <input
        type="checkbox"
        checked={settings.minimize_to_tray}
        onchange={(e) => apply({ minimize_to_tray: e.currentTarget.checked })}
      />
      Minimize to tray (instead of the taskbar)
    </label>
    <label>
      <input
        type="checkbox"
        checked={settings.start_minimized}
        onchange={(e) => apply({ start_minimized: e.currentTarget.checked })}
      />
      Start minimized to tray
    </label>
    <label>
      <input
        type="checkbox"
        checked={settings.autostart}
        onchange={(e) => apply({ autostart: e.currentTarget.checked })}
      />
      Run at login
    </label>
    <label>
      <input
        type="checkbox"
        checked={settings.native_notifications}
        onchange={(e) => apply({ native_notifications: e.currentTarget.checked })}
      />
      Windows notifications while hidden to tray
    </label>
    <div class="row-buttons">
      <button onclick={() => checkForUpdates(true)}>Check for updates</button>
      <button onclick={openOnboarding}>Re-run setup</button>
    </div>
  </section>

  <section>
    <h2>Modules</h2>
    {#if moduleRegistry.length === 0}
      <p class="dim">No feature modules are available in this build yet.</p>
    {:else}
      {#each moduleRegistry as m (m.id)}
        <label>
          <input
            type="checkbox"
            checked={settings.enabled_modules.includes(m.id)}
            onchange={(e) => toggleModule(m.id, e.currentTarget.checked)}
          />
          {m.icon} {m.name} <span class="dim">— {m.description}</span>
        </label>
      {/each}
    {/if}
  </section>

  <section>
    <h2>Online</h2>
    <label>
      <input
        type="checkbox"
        checked={settings.online_enabled}
        onchange={(e) => apply({ online_enabled: e.currentTarget.checked })}
      />
      Enable online features
      <span class="dim">— master switch; off = no network calls (except update checks)</span>
    </label>
    <label class:disabled={!settings.online_enabled}>
      <input
        type="checkbox"
        checked={settings.grpc_enabled}
        disabled={!settings.online_enabled}
        onchange={(e) => toggleGrpc(e.currentTarget.checked)}
      />
      Allow game-services (gRPC) calls
      <span class="dim">— ToS-grey, read-only, opt-in per feature below</span>
    </label>
    {#if grpcFeatures.length === 0}
      <p class="dim indent">
        No game-services features in this build yet — per-feature toggles appear here as
        they land (blueprints, missions, …).
      </p>
    {:else}
      {#each grpcFeatures as f (f.id)}
        <label class="indent" class:disabled={!settings.online_enabled || !settings.grpc_enabled}>
          <input
            type="checkbox"
            checked={settings.grpc_features.includes(f.id)}
            disabled={!settings.online_enabled || !settings.grpc_enabled}
            onchange={(e) => toggleGrpcFeature(f.id, e.currentTarget.checked)}
          />
          {f.name} <span class="dim">— {f.description}</span>
        </label>
      {/each}
    {/if}
  </section>

  <section>
    <h2>Account</h2>
    <label class="row">
      Server URL
      <input
        type="url"
        placeholder="https://…  (no server yet — leave empty)"
        bind:value={serverUrlInput}
        onchange={() =>
          apply({ server_url: serverUrlInput.trim() === "" ? null : serverUrlInput.trim() })}
      />
    </label>
    {#if auth}
      {#if !settings.online_enabled}
        <p class="dim">Online features are disabled — sign-in unavailable.</p>
      {:else if auth.logged_in}
        <p>
          Signed in{authStore.profile ? ` as ${authStore.profile.username}` : ""} on this
          device.
        </p>
        <button onclick={logout}>Sign out</button>
      {:else if auth.server_configured}
        <button onclick={login}>Sign in with Discord</button>
      {:else}
        <p class="dim">No server configured.</p>
      {/if}
    {/if}
  </section>

  {#if error}
    <p class="error">{error}</p>
  {/if}
{:else}
  <p class="dim">Loading…</p>
{/if}

<style>
  section {
    margin-bottom: 24px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: flex-start;
  }

  h2 {
    font-size: 15px;
    margin: 0 0 4px;
  }

  label {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .row input {
    min-width: 320px;
  }

  .row-buttons {
    display: flex;
    gap: 8px;
  }

  .dim {
    color: var(--text-dim);
  }

  .indent {
    margin-left: 24px;
  }

  .disabled {
    opacity: 0.55;
  }

  .error {
    color: #e06c6c;
  }
</style>
