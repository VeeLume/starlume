<script lang="ts">
  // The Me page — your identity hub + the unified visibility/sharing surface.
  //
  // Identity model: the **RSI account is primary** (who you are in the game).
  // Connectors (Discord) and game-account data (owned blueprints) are
  // settings, under Settings → Account / Online & privacy.
  // Modules contribute sharing sections via `ModuleDescriptor.meSections`.
  import { onMount } from "svelte";
  import { moduleRegistry } from "$lib/modules/registry";
  import { settingsStore } from "$lib/state/settings.svelte";
  import { scStore, loadSc, verifyAccount } from "$lib/state/sc.svelte";
  import Avatar from "$lib/components/Avatar.svelte";
  import { SHOW_UNRELEASED } from "$lib/unreleased";
  import { Button } from "@veelume/ui";

  const settings = $derived(settingsStore.current);
  const account = $derived(scStore.account);

  let verifying = $state(false);
  let error = $state("");

  const meSections = $derived(
    moduleRegistry
      .filter((d) => settingsStore.current?.enabled_modules.includes(d.id))
      .flatMap((d) =>
        (d.meSections ?? []).map((s) => ({ ...s, moduleName: d.name, key: `${d.id}:${s.id}` })),
      ),
  );

  onMount(() => void loadSc());

  async function verify() {
    if (!account) return;
    error = "";
    verifying = true;
    const err = await verifyAccount(account.handle);
    if (err) error = err;
    verifying = false;
  }
</script>

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
          <Button variant="outline" onclick={verify} disabled={verifying}>
            {verifying ? "Verifying…" : "Verify with RSI profile"}
          </Button>
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

<!-- Sharing scopes are friends/groups — unreleased. -->
{#if SHOW_UNRELEASED}
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

  .dim {
    color: var(--text-dim);
  }
  .error {
    color: var(--bad);
  }
</style>
