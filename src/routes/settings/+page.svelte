<script lang="ts">
  import { commands } from "$lib/bindings";
  import { moduleRegistry } from "$lib/modules/registry";
  import { settingsStore, applySettings, loadSettings } from "$lib/state/settings.svelte";
  import { authStore, loadAuth } from "$lib/state/auth.svelte";
  import { openOnboarding } from "$lib/state/onboarding.svelte";
  import { checkForUpdates } from "$lib/updater";
  import { onMount } from "svelte";

  let serverUrlInput = $state("");
  let error = $state("");

  const settings = $derived(settingsStore.current);
  const auth = $derived(authStore.current);

  onMount(async () => {
    const s = await loadSettings();
    serverUrlInput = s.server_url ?? "";
    await loadAuth();
  });

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
      {#if auth.logged_in}
        <p>
          Signed in{authStore.profile ? ` as ${authStore.profile.username}` : ""} on this
          device.
        </p>
        <button onclick={logout}>Sign out</button>
      {:else if auth.server_configured}
        <button onclick={login}>Sign in with Discord</button>
      {:else}
        <p class="dim">Online features are off — no server configured.</p>
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

  .error {
    color: #e06c6c;
  }
</style>
