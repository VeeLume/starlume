<script lang="ts">
  import "../app.css";
  import { onMount, onDestroy } from "svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { moduleRegistry } from "$lib/modules/registry";
  import { settingsStore, loadSettings } from "$lib/state/settings.svelte";
  import { authStore, loadAuth, listenForAuthChanges } from "$lib/state/auth.svelte";
  import { scStore, loadSc } from "$lib/state/sc.svelte";
  import {
    dataStore,
    loadStatus,
    ensureChannel,
    listenForDataProgress,
    clearAllLoading,
  } from "$lib/state/data.svelte";
  import { prefetchCatalogs, invalidateCatalogs } from "$lib/state/catalog.svelte";
  import { listen } from "@tauri-apps/api/event";
  import { onboarding, maybeStartOnboarding } from "$lib/state/onboarding.svelte";
  import { listenForNotifications, syncNotifications } from "$lib/state/notifications.svelte";
  import { Notify, Progress, Shell, setKitContext, type NavGroup } from "@veelume/ui";
  import { House, Library, Settings, Users } from "lucide-svelte";
  import Onboarding from "$lib/components/Onboarding.svelte";
  import { checkForUpdates } from "$lib/updater";

  let { children } = $props();

  // The kit's only channel into app state. Two locales on purpose: the UI is
  // English while formatting follows the user's system (24h clock, 1.234,56
  // for a German user — the split the kit context exists for).
  setKitContext({
    messageLocale: () => "en",
    formattingLocale: () => (typeof navigator !== "undefined" ? navigator.language : "en"),
  });

  let unlistenAuth: UnlistenFn | undefined;
  let unlistenNotify: UnlistenFn | undefined;
  let unlistenDataProgress: UnlistenFn | undefined;
  let unlistenDataChanged: UnlistenFn | undefined;

  // Startup hydration (docs/frontend.md rule 3): refresh install statuses
  // and prefetch the default channel's catalogs so pages render from cache.
  // Cheap when nothing changed; also the focus/`data:changed` catch-up.
  async function hydrateData() {
    await loadStatus();
    const channel = await ensureChannel();
    if (channel) prefetchCatalogs(channel);
  }

  // Notification center (sidebar bell). The kit Center marks everything
  // read on open; the per-session log stays in the panel.
  let centerOpen = $state(false);

  // Home + Friends + Catalogs (shell-level) + one entry per enabled module
  // (registry order). Kit NavItems — the shell owns active-state matching.
  const navGroups: NavGroup[] = $derived([
    {
      items: [
        { path: "/", label: "Home", icon: House },
        { path: "/friends", label: "Friends", icon: Users },
        { path: "/data", label: "Catalogs", icon: Library },
        ...moduleRegistry
          .filter((d) => settingsStore.current?.enabled_modules.includes(d.id))
          .flatMap((d) => d.nav ?? [])
          .map((e) => ({ path: e.href, label: e.label, icon: e.icon })),
      ],
    },
  ]);

  // Account footer — RSI-primary identity (who you are in the game);
  // Discord is a secondary connector, surfaced on the Me page.
  const account = $derived.by(() => {
    if (scStore.account) {
      const a = scStore.account;
      return {
        name: a.handle,
        detail: a.citizen_record ? `✓ Citizen #${a.citizen_record}` : "RSI · unverified",
        char: a.handle.charAt(0).toUpperCase(),
        img: null as string | null,
        muted: false,
      };
    }
    if (authStore.current?.logged_in && authStore.profile) {
      return {
        name: authStore.profile.username,
        detail: "Discord · no SC account",
        char: authStore.profile.username.charAt(0).toUpperCase(),
        img: authStore.profile.avatar_url ?? null,
        muted: false,
      };
    }
    return {
      name: "Not set up",
      detail: "open to set up →",
      char: "?",
      img: null,
      muted: true,
    };
  });

  onMount(() => {
    void (async () => {
      // The single notification funnel — live events plus a hydration pass
      // for anything raised before mount (or while the webview was
      // suspended; the focus handler below covers later suspensions).
      unlistenNotify = await listenForNotifications();
      await syncNotifications();
      // Settings first — the online master switch gates loadAuth's profile
      // fetch (backend-side). The update check is exempt by design.
      await loadSettings();
      // RSI identity is local-only (no online gate) — the primary identity
      // for the sidebar banner. Discord is a secondary connector.
      void loadSc();
      await loadAuth();
      unlistenAuth = await listenForAuthChanges();
      await maybeStartOnboarding();
      void checkForUpdates();
    })();
    // Game-data hydration runs independently of the auth chain — local only.
    void (async () => {
      unlistenDataProgress = await listenForDataProgress();
      unlistenDataChanged = await listen("data:changed", () => {
        clearAllLoading();
        invalidateCatalogs();
        void hydrateData();
      });
      await hydrateData();
    })();
  });

  onDestroy(() => {
    unlistenAuth?.();
    unlistenNotify?.();
    unlistenDataProgress?.();
    unlistenDataChanged?.();
  });
</script>

<svelte:window
  onfocus={() => {
    void syncNotifications();
    void hydrateData();
  }}
/>

<Shell.Root groups={navGroups}>
  <!-- The rail speaks the display face (Lekton — nav/tabs/buttons per the
       type roles); the account footer drops back to sans (body role). -->
  <Shell.Rail class="font-display">
    {#snippet header({ showLabels })}
      <div
        class="relative flex items-center"
        class:w-full={showLabels}
        class:justify-center={!showLabels}
      >
        {#if showLabels}
          <span class="brand-name min-w-0 flex-1 truncate px-3">Starlume</span>
        {/if}
        <Notify.Bell onclick={() => (centerOpen = !centerOpen)} />
        <Notify.Center
          open={centerOpen}
          onclose={() => (centerOpen = false)}
          side="right"
          align="start"
        />
      </div>
    {/snippet}
    {#snippet footer({ showLabels })}
      <div class="w-full font-sans">
        <Shell.AccountFooter
        name={account.name}
        detail={account.detail}
        href="/me"
        settingsIcon={Settings}
        {showLabels}
      >
        {#snippet avatar({ size })}
          {#if account.img}
            <img
              src={account.img}
              alt=""
              class="shrink-0 rounded-full object-cover"
              style="width: {size}px; height: {size}px"
            />
          {:else}
            <span
              class="avatar-fallback"
              class:muted={account.muted}
              style="width: {size}px; height: {size}px"
              aria-hidden="true"
            >
              {account.char}
            </span>
          {/if}
        {/snippet}
      </Shell.AccountFooter>
      </div>
    {/snippet}
  </Shell.Rail>
  <Shell.Content>
    {#snippet banner()}
      <!-- Game-data work made visible: the cold parse takes minutes and runs
           once per game patch — invisible background work reads as a hang.
           One strip per channel currently loading (dataStore.loading is fed
           by the data:progress events the root layout subscribes to). -->
      {#each Object.entries(dataStore.loading) as [channel, stage] (channel)}
        <div class="data-banner">
          <Progress label="Preparing Star Citizen data ({channel})" detail={stage} />
          <p class="data-banner-hint">
            The heavy parse happens once per game patch — catalogs and text patching pick
            it up automatically when it finishes.
          </p>
        </div>
      {/each}
    {/snippet}
    {@render children()}
  </Shell.Content>
</Shell.Root>

<Notify.Toasts />

{#if onboarding.open}
  <Onboarding />
{/if}

<style>
  .brand-name {
    font-family: var(--font-display);
    font-weight: 700;
    color: var(--accent);
    letter-spacing: 0.01em;
  }

  .avatar-fallback {
    display: flex;
    flex-shrink: 0;
    align-items: center;
    justify-content: center;
    border-radius: 999px;
    background: var(--accent-fill);
    color: var(--accent);
    font-size: 0.8rem;
    font-weight: 700;
  }
  .avatar-fallback.muted {
    background: var(--bg-raised);
    color: var(--text-dim);
  }

  .data-banner {
    padding: 10px var(--content-pad) 8px;
    border-bottom: 1px solid var(--border);
    background: var(--accent-fill-faint);
  }
  .data-banner-hint {
    margin: 4px 0 0;
    font-size: 0.78rem;
    color: var(--text-dim);
  }
</style>
