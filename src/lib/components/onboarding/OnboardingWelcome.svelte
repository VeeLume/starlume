<script lang="ts">
  // Onboarding: what Starlume found, instead of asking. The installs, the
  // RSI launcher account and the default channel's game data, all from
  // stores the root layout already hydrates (frontend rule "onboarding reads
  // stores"): it renders at once and refreshes silently. Nothing here gates
  // Next; no install is one line, not a blocker. Verifying the account moved
  // to the privacy step, where the network is decided.
  import { onMount } from "svelte";
  import { scStore, loadSc } from "$lib/state/sc.svelte";
  import { dataStore, loadStatus } from "$lib/state/data.svelte";
  import { activityFor, activityLabel } from "$lib/state/activities.svelte";
  import { channelHue } from "$lib/channels";
  import { installVersion } from "$lib/home/version";

  const status = $derived(scStore.status);
  const installs = $derived(status?.installs ?? []);
  const handle = $derived(scStore.account?.handle ?? status?.launcher_handle ?? null);
  const data = $derived(dataStore.statuses.find((s) => s.is_default) ?? null);
  const cooking = $derived(data ? activityFor("data-load", data.channel) : undefined);
  const ready = $derived(data && (data.loaded || data.predicted_tier === "processed"));

  onMount(() => {
    void loadSc();
    void loadStatus();
  });
</script>

<section class="found" class:none={installs.length === 0} style:--hue={installs[0] ? channelHue(installs[0].channel) : undefined}>
  <p class="label">Found on this PC</p>
  {#if !scStore.loaded}
    <p class="dim">Looking for Star Citizen…</p>
  {:else if installs.length === 0}
    <div class="line">
      <span class="mark faint">–</span>
      <span>No Star Citizen install.<small>News and the Dev Tracker work without it. Game features start once it's installed.</small></span>
    </div>
  {:else}
    {#each installs as i (i.build_id)}
      <div class="line">
        <span class="mark ok">✓</span>
        <span>
          <span class="hue-chip" style:--hue={channelHue(i.channel)}>{i.channel.toUpperCase()}</span>
          <b class="ver">{installVersion(i.version)}</b>
          <small class="path">{i.directory}</small>
        </span>
      </div>
    {/each}
    {#if handle}
      <div class="line">
        <span class="mark ok">✓</span>
        <span>Signed into the RSI launcher as <b>{handle}</b><small>Your data is kept under this account.</small></span>
      </div>
    {:else}
      <div class="line">
        <span class="mark faint">–</span>
        <span>No RSI launcher account.<small>Sign into the launcher once; Starlume picks it up.</small></span>
      </div>
    {/if}
    {#if cooking}
      <div class="line">
        <span class="mark busy">⟳</span>
        <span>Preparing {data?.channel} {installVersion(data?.version ?? "")} game data<small>{activityLabel(cooking)}. Runs in the background; continue meanwhile.</small></span>
      </div>
    {:else if ready}
      <div class="line">
        <span class="mark ok">✓</span>
        <span>{data?.channel} {installVersion(data?.version ?? "")} game data ready</span>
      </div>
    {/if}
  {/if}
</section>

<style>
  .found {
    --hue: var(--hue-live);
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 16px 18px;
    border-radius: var(--radius-lg);
    border: 1px solid color-mix(in srgb, var(--hue) 30%, var(--border));
    background: linear-gradient(
      135deg,
      color-mix(in srgb, var(--hue) 10%, var(--bg-raised)),
      var(--bg-raised) 60%
    );
  }
  .found.none {
    --hue: var(--text-faint);
  }
  .found > .label {
    margin: 0;
  }
  .line {
    display: grid;
    grid-template-columns: 18px minmax(0, 1fr);
    gap: 10px;
    align-items: baseline;
  }
  .line small {
    display: block;
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .line b {
    color: var(--text-strong);
  }
  .ver {
    margin-left: 4px;
    font-family: var(--font-mono);
  }
  .path {
    font-family: var(--font-mono);
    word-break: break-all;
  }
  .mark.ok {
    color: var(--good);
  }
  .mark.faint {
    color: var(--text-faint);
  }
  .mark.busy {
    color: var(--accent);
    display: inline-block;
    animation: spin 1.6s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .mark.busy {
      animation: none;
    }
  }
</style>
