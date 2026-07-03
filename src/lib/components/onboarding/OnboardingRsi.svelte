<script lang="ts">
  // Onboarding: Star Citizen account recognition. Framework step (accounts
  // are app-level — every SC module needs "whose data is this"). Scans for
  // installs, reads the launcher handle, and offers an optional online verify
  // that captures the immutable RSI anchors. All optional.
  import { onMount } from "svelte";
  import type { OnboardingStepProps } from "$lib/modules/types";
  import { settingsStore } from "$lib/state/settings.svelte";
  import { scStore, loadSc, verifyAccount } from "$lib/state/sc.svelte";

  // Typed for the step contract; optional step, never blocks Next.
  const {}: OnboardingStepProps = $props();

  const settings = $derived(settingsStore.current);
  // Shared store: a verify here propagates live to the sidebar + Me page.
  const status = $derived(scStore.status);
  const account = $derived(scStore.account);

  let loading = $state(true);
  let verifying = $state(false);
  let error = $state("");

  onMount(async () => {
    await loadSc();
    loading = false;
  });

  async function verify() {
    if (!status?.launcher_handle) return;
    error = "";
    verifying = true;
    const err = await verifyAccount(status.launcher_handle);
    if (err) error = err;
    verifying = false;
  }
</script>

{#if loading}
  <p class="dim">Scanning for Star Citizen…</p>
{:else if status}
  {#if status.installs.length === 0}
    <p>
      No Star Citizen installation found. That's fine — you can still use online
      and community features. SC data features will light up once the game is
      installed.
    </p>
  {:else}
    <p class="lead">Found your Star Citizen installation:</p>
    <ul class="installs">
      {#each status.installs as i (i.build_id)}
        <li><strong>{i.channel}</strong> <span class="dim">{i.version}</span></li>
      {/each}
    </ul>
  {/if}

  {#if account}
    <div class="account">
      <div class="handle">@{account.handle}</div>
      {#if account.citizen_record}
        <div class="dim">
          Verified · Citizen #{account.citizen_record}
          {#if account.enlisted}· enlisted {account.enlisted}{/if}
        </div>
      {:else if settings?.online_enabled}
        <p class="dim">
          Recognized from the RSI launcher. Verify to capture your permanent
          account anchors (used later so a handle rename doesn't lose your data).
        </p>
        <button onclick={verify} disabled={verifying}>
          {verifying ? "Verifying…" : "Verify with RSI profile"}
        </button>
      {:else}
        <p class="dim">
          Recognized from the RSI launcher. Enable online features to verify
          your account against the public RSI profile.
        </p>
      {/if}
    </div>
  {:else if status.launcher_handle === null && status.installs.length > 0}
    <p class="dim">
      Couldn't read your RSI handle from the launcher — sign into the RSI
      launcher once, or set it later in Settings.
    </p>
  {/if}
{/if}

{#if error}<p class="error">{error}</p>{/if}

<style>
  .lead {
    margin-top: 0;
  }
  .installs {
    margin: 0 0 16px;
    padding-left: 20px;
  }
  .account {
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 12px 14px;
    max-width: 420px;
  }
  .handle {
    font-weight: 600;
    font-size: 15px;
  }
  .dim {
    color: var(--text-dim);
    font-size: 0.9em;
  }
  .error {
    color: #e06c6c;
  }
</style>
