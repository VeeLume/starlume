<script lang="ts">
  // The Me page — your identity hub + the unified visibility/sharing surface.
  //
  // Identity model: the **RSI account is primary** (who you are in the game).
  // **Discord is a secondary connector** — it exists for the guild/org
  // community layer, NOT for reading friends or chats (Discord OAuth can't).
  // Modules contribute sharing sections via `ModuleDescriptor.meSections`.
  import { onMount, onDestroy } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { commands } from "$lib/bindings";
  import { moduleRegistry } from "$lib/modules/registry";
  import { settingsStore } from "$lib/state/settings.svelte";
  import { authStore, loadAuth } from "$lib/state/auth.svelte";
  import { scStore, loadSc, verifyAccount } from "$lib/state/sc.svelte";
  import Avatar from "$lib/components/Avatar.svelte";

  const settings = $derived(settingsStore.current);
  const auth = $derived(authStore.current);
  const account = $derived(scStore.account);

  let verifying = $state(false);
  let manualLoginUrl = $state("");
  let error = $state("");
  let unlisten: UnlistenFn | undefined;

  const meSections = $derived(
    moduleRegistry
      .filter((d) => settingsStore.current?.enabled_modules.includes(d.id))
      .flatMap((d) =>
        (d.meSections ?? []).map((s) => ({ ...s, moduleName: d.name, key: `${d.id}:${s.id}` })),
      ),
  );

  onMount(async () => {
    await Promise.all([loadSc(), loadAuth()]);
    unlisten = await listen("auth-changed", () => void loadAuth());
  });
  onDestroy(() => unlisten?.());

  async function verify() {
    if (!account) return;
    error = "";
    verifying = true;
    const err = await verifyAccount(account.handle);
    if (err) error = err;
    verifying = false;
  }

  async function connectDiscord() {
    error = "";
    manualLoginUrl = "";
    const result = await commands.loginStart();
    if (result.status === "error") error = result.error.message;
    else if (result.data) {
      manualLoginUrl = result.data;
      await navigator.clipboard.writeText(result.data).catch(() => {});
    }
  }

  async function disconnectDiscord() {
    await commands.logout();
    await loadAuth();
  }
</script>

<h1>Me</h1>

<!-- Primary identity: RSI account -->
<section class="identity">
  {#if account}
    <Avatar text={account.handle.charAt(0).toUpperCase()} size="3.2rem" />
    <div class="id-meta">
      <div class="handle">{account.handle}</div>
      {#if account.citizen_record}
        <div class="dim">
          ✓ Verified · UEE Citizen #{account.citizen_record}
          {#if account.enlisted}· enlisted {account.enlisted}{/if}
        </div>
        {#if account.primary_org_sid}
          <div class="dim">Main org: {account.primary_org_sid}</div>
        {/if}
      {:else}
        <div class="dim">Recognized from the RSI launcher · not yet verified</div>
        {#if settings?.online_enabled}
          <button onclick={verify} disabled={verifying}>
            {verifying ? "Verifying…" : "Verify with RSI profile"}
          </button>
        {:else}
          <div class="dim">Enable online features (Settings) to verify.</div>
        {/if}
      {/if}
    </div>
  {:else}
    <Avatar text="?" muted size="3.2rem" />
    <div class="id-meta">
      <div class="handle dim">No Star Citizen account</div>
      <div class="dim">
        Sign into the RSI launcher, or run setup again (Settings → Re-run setup) once
        Star Citizen is installed.
      </div>
    </div>
  {/if}
</section>

<!-- Secondary: connected accounts (Discord, for community/guilds) -->
<h2>Connected accounts</h2>
<section class="connections">
  <div class="conn">
    <div class="conn-head">
      <span class="conn-name">Discord</span>
      {#if auth?.logged_in}
        <span class="conn-status ok">connected{authStore.profile ? ` · ${authStore.profile.username}` : ""}</span>
      {:else}
        <span class="conn-status">not connected</span>
      {/if}
    </div>
    <p class="dim">
      Used for the community layer — guilds and orgs are scoped by your Discord server
      membership. (Discord can't share your friends or chats, so it's only for community
      access, not your friends list here.)
    </p>
    {#if auth?.logged_in}
      <button onclick={disconnectDiscord}>Disconnect</button>
    {:else if !settings?.online_enabled}
      <span class="dim">Enable online features (Settings) to connect.</span>
    {:else if auth?.server_configured}
      <button onclick={connectDiscord}>Connect Discord</button>
      {#if auth.dev_profile && manualLoginUrl}
        <p class="manual-url">Copied to clipboard:<br /><code>{manualLoginUrl}</code></p>
      {/if}
    {:else}
      <span class="dim">Set a server URL (Settings → Account) to connect.</span>
    {/if}
  </div>
</section>

<h2>Sharing &amp; visibility</h2>
{#if meSections.length === 0}
  <p class="dim">
    Nothing is shared yet. As feature modules land, each one adds a section here showing
    what of yours it shares and with whom (friends, groups) — one place to review it all.
  </p>
{:else}
  {#each meSections as s (s.key)}
    <section>
      <h3>{s.title} <span class="module-tag">{s.moduleName}</span></h3>
      <s.component />
    </section>
  {/each}
{/if}

{#if error}<p class="error">{error}</p>{/if}

<style>
  .identity {
    display: flex;
    align-items: center;
    gap: 16px;
    margin-bottom: 28px;
  }
  .id-meta {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .handle {
    font-size: 20px;
    font-weight: 600;
  }

  h2 {
    font-size: 16px;
    margin: 0 0 8px;
  }
  h3 {
    font-size: 14px;
    margin: 0 0 6px;
  }

  .connections {
    margin-bottom: 28px;
  }
  .conn {
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 12px 14px;
    max-width: 480px;
  }
  .conn-head {
    display: flex;
    align-items: baseline;
    gap: 10px;
    margin-bottom: 4px;
  }
  .conn-name {
    font-weight: 600;
  }
  .conn-status {
    font-size: 12px;
    color: var(--text-dim);
  }
  .conn-status.ok {
    color: var(--good);
  }

  .module-tag {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--accent);
    margin-left: 6px;
  }

  section {
    margin-bottom: 20px;
  }

  .manual-url {
    word-break: break-all;
    font-size: 12px;
  }
  .manual-url code {
    color: var(--accent);
  }

  .dim {
    color: var(--text-dim);
  }
  .error {
    color: #e06c6c;
  }
</style>
