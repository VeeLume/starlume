<script lang="ts">
  import { Button, Settings } from "@veelume/ui";
  import { commands } from "$lib/bindings";
  import { settingsStore, applySettings, loadSettings } from "$lib/state/settings.svelte";
  import { authStore, loadAuth } from "$lib/state/auth.svelte";
  import { onMount } from "svelte";

  const settings = $derived(settingsStore.current);
  const auth = $derived(authStore.current);

  let serverUrlInput = $state("");
  let error = $state("");
  // Dev profile mode returns the sign-in URL instead of opening the browser
  // (two-account testing — paste it into the browser session that holds the
  // right Discord account).
  let manualLoginUrl = $state("");

  onMount(async () => {
    const s = await loadSettings();
    serverUrlInput = s.server_url ?? "";
    await loadAuth();
  });

  async function saveServerUrl() {
    error = "";
    const err = await applySettings({
      server_url: serverUrlInput.trim() === "" ? null : serverUrlInput.trim(),
    });
    if (err) error = err.message;
    await loadAuth(); // server_url changes flip server_configured
  }

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

<Settings.Page title="Account">
  {#if settings}
    <Settings.Section title="Server">
      <Settings.Row label="Server URL" hint="No server yet — leave empty.">
        <input
          class="server-url"
          type="url"
          placeholder="https://…"
          bind:value={serverUrlInput}
          onchange={saveServerUrl}
        />
      </Settings.Row>
    </Settings.Section>

    <Settings.Section title="Discord">
      {#if auth}
        {#if !settings.online_enabled}
          <p class="text-sm text-muted-foreground">
            Online features are disabled — sign-in unavailable.
          </p>
        {:else if auth.logged_in}
          <Settings.Row
            label="Signed in"
            hint={authStore.profile ? `as ${authStore.profile.username} on this device` : "on this device"}
          >
            <Button variant="outline" onclick={logout}>Sign out</Button>
          </Settings.Row>
        {:else if auth.server_configured}
          <Settings.Row label="Not signed in" hint="Connect Discord for the community layer.">
            <Button onclick={login}>Sign in with Discord</Button>
          </Settings.Row>
          {#if auth.dev_profile}
            <p class="text-sm text-muted-foreground">
              Dev profile <code>{auth.dev_profile}</code> — the sign-in link is shown here
              instead of opening the browser.
            </p>
            {#if manualLoginUrl}
              <p class="manual-url text-sm text-muted-foreground">
                Copied to clipboard — open it in the browser session with the right Discord
                account:<br /><code>{manualLoginUrl}</code>
              </p>
            {/if}
          {/if}
        {:else}
          <p class="text-sm text-muted-foreground">No server configured.</p>
        {/if}
      {/if}
    </Settings.Section>

    {#if error}
      <p class="text-sm text-destructive">{error}</p>
    {/if}
  {/if}
</Settings.Page>

<style>
  .server-url {
    min-width: 320px;
  }
  .manual-url {
    max-width: 480px;
    word-break: break-all;
  }
  .manual-url code {
    color: var(--accent);
  }
</style>
