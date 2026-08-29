<script lang="ts">
  import { ask } from "@tauri-apps/plugin-dialog";
  import { commands, type GrpcFeatureInfo } from "$lib/bindings";
  import { moduleRegistry } from "$lib/modules/registry";
  import { settingsStore, applySettings, loadSettings } from "$lib/state/settings.svelte";
  import { authStore, loadAuth } from "$lib/state/auth.svelte";
  import { openOnboarding } from "$lib/state/onboarding.svelte";
  import { checkForUpdates } from "$lib/updater";
  import Switch from "$lib/components/Switch.svelte";
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

  // Dev profile mode returns the sign-in URL instead of opening the browser
  // (two-account testing — paste it into the browser session that holds the
  // right Discord account).
  let manualLoginUrl = $state("");

  async function login() {
    error = "";
    manualLoginUrl = "";
    const result = await commands.loginStart();
    if (result.status === "error") {
      error = result.error.message;
    } else if (result.data) {
      manualLoginUrl = result.data;
      await navigator.clipboard.writeText(result.data).catch(() => {});
    }
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
    <Switch
      checked={settings.close_to_tray}
      onchange={(v) => apply({ close_to_tray: v })}
    >
      Close to tray (keep running in the background)
    </Switch>
    <Switch
      checked={settings.minimize_to_tray}
      onchange={(v) => apply({ minimize_to_tray: v })}
    >
      Minimize to tray (instead of the taskbar)
    </Switch>
    <Switch
      checked={settings.start_minimized}
      onchange={(v) => apply({ start_minimized: v })}
    >
      Start minimized to tray
    </Switch>
    <Switch checked={settings.autostart} onchange={(v) => apply({ autostart: v })}>
      Run at login
    </Switch>
    <Switch
      checked={settings.native_notifications}
      onchange={(v) => apply({ native_notifications: v })}
    >
      Windows notifications while hidden to tray
    </Switch>
    <Switch
      checked={settings.auto_load_game_data}
      onchange={(v) => apply({ auto_load_game_data: v })}
    >
      Load game data at startup (local only; heavy work only after a game patch)
    </Switch>
    <div class="row-buttons">
      <button class="btn" onclick={() => checkForUpdates(true)}>Check for updates</button>
      <button class="btn" onclick={openOnboarding}>Re-run setup</button>
    </div>
  </section>

  <section>
    <h2>Modules</h2>
    {#if moduleRegistry.length === 0}
      <p class="dim">No feature modules are available in this build yet.</p>
    {:else}
      {#each moduleRegistry as m (m.id)}
        <Switch
          checked={settings.enabled_modules.includes(m.id)}
          onchange={(v) => toggleModule(m.id, v)}
        >
          {m.icon} {m.name} <span class="dim">— {m.description}</span>
        </Switch>
      {/each}
    {/if}
  </section>

  <section>
    <h2>Online</h2>
    <Switch
      checked={settings.online_enabled}
      onchange={(v) => apply({ online_enabled: v })}
    >
      Enable online features
      <span class="dim">— master switch; off = no network calls (except update checks)</span>
    </Switch>
    <Switch
      checked={settings.grpc_enabled}
      disabled={!settings.online_enabled}
      onchange={(v) => toggleGrpc(v)}
    >
      Allow game-services (gRPC) calls
      <span class="dim">— ToS-grey, read-only, opt-in per feature below</span>
    </Switch>
    {#if grpcFeatures.length === 0}
      <p class="dim indent">
        No game-services features in this build yet — per-feature toggles appear here as
        they land (blueprints, missions, …).
      </p>
    {:else}
      {#each grpcFeatures as f (f.id)}
        <div class="indent">
          <Switch
            checked={settings.grpc_features.includes(f.id)}
            disabled={!settings.online_enabled || !settings.grpc_enabled}
            onchange={(v) => toggleGrpcFeature(f.id, v)}
          >
            {f.name} <span class="dim">— {f.description}</span>
          </Switch>
        </div>
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
        <button class="btn" onclick={logout}>Sign out</button>
      {:else if auth.server_configured}
        <button class="btn" onclick={login}>Sign in with Discord</button>
        {#if auth.dev_profile}
          <p class="dim">
            Dev profile <code>{auth.dev_profile}</code> — the sign-in link is shown here
            instead of opening the browser.
          </p>
          {#if manualLoginUrl}
            <p class="manual-url">
              Copied to clipboard — open it in the browser session with the right Discord
              account:<br /><code>{manualLoginUrl}</code>
            </p>
          {/if}
        {/if}
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

  .manual-url {
    font-size: 12px;
    color: var(--text-dim);
    max-width: 480px;
    word-break: break-all;
  }

  .manual-url code {
    color: var(--accent);
  }

  .error {
    color: var(--bad);
  }
</style>
