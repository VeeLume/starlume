<script lang="ts">
  // Onboarding: Discord sign-in. Framework step, optional — community
  // features (friends, groups, later sharing) need it, but a text-patch-only
  // user can skip it entirely. Mirrors Settings → Account.
  import { onMount, onDestroy } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { commands } from "$lib/bindings";
  import type { OnboardingStepProps } from "$lib/modules/types";
  import { settingsStore, applySettings } from "$lib/state/settings.svelte";
  import { authStore, loadAuth } from "$lib/state/auth.svelte";

  // Typed for the step contract; optional step, never blocks Next.
  const {}: OnboardingStepProps = $props();

  const settings = $derived(settingsStore.current);
  const auth = $derived(authStore.current);

  let serverUrlInput = $state("");
  let manualLoginUrl = $state("");
  let error = $state("");
  let unlisten: UnlistenFn | undefined;

  onMount(async () => {
    serverUrlInput = settingsStore.current?.server_url ?? "";
    await loadAuth();
    unlisten = await listen("auth-changed", () => void loadAuth());
  });
  onDestroy(() => unlisten?.());

  async function saveServer() {
    await applySettings({
      server_url: serverUrlInput.trim() === "" ? null : serverUrlInput.trim(),
    });
    await loadAuth();
  }

  async function login() {
    error = "";
    manualLoginUrl = "";
    const result = await commands.loginStart();
    if (result.status === "error") error = result.error.message;
    else if (result.data) {
      manualLoginUrl = result.data;
      await navigator.clipboard.writeText(result.data).catch(() => {});
    }
  }

  async function logout() {
    await commands.logout();
    await loadAuth();
  }
</script>

{#if settings && !settings.online_enabled}
  <p>
    Online features are turned off, so Discord sign-in is unavailable. You can
    enable them on the previous step or later in Settings.
  </p>
{:else}
  <p class="lead">
    Sign in with Discord to use friends, groups, and (later) community sharing.
    Optional — skip it if you only want local features.
  </p>

  {#if auth?.logged_in}
    <p>
      Signed in{authStore.profile ? ` as ${authStore.profile.username}` : ""}.
    </p>
    <button onclick={logout}>Sign out</button>
  {:else}
    <label class="server">
      Server URL
      <input
        type="url"
        placeholder="https://…"
        bind:value={serverUrlInput}
        onchange={saveServer}
      />
    </label>
    {#if auth?.server_configured}
      <button class="primary" onclick={login}>Sign in with Discord</button>
      {#if auth.dev_profile}
        <p class="dim">
          Dev profile <code>{auth.dev_profile}</code> — the sign-in link is shown
          here instead of opening the browser.
        </p>
        {#if manualLoginUrl}
          <p class="manual-url">Copied to clipboard:<br /><code>{manualLoginUrl}</code></p>
        {/if}
      {/if}
    {:else}
      <p class="dim">Enter a server URL above to enable sign-in.</p>
    {/if}
  {/if}

  {#if error}<p class="error">{error}</p>{/if}
{/if}

<style>
  .lead {
    margin-top: 0;
  }
  .server {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-bottom: 12px;
    max-width: 360px;
  }
  .primary {
    border-color: var(--accent);
    color: var(--accent);
  }
  .dim {
    color: var(--text-dim);
    font-size: 0.85em;
  }
  .manual-url {
    word-break: break-all;
    font-size: 12px;
  }
  .manual-url code {
    color: var(--accent);
  }
  .error {
    color: #e06c6c;
  }
</style>
