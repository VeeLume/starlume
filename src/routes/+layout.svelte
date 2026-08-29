<script lang="ts">
  import "../app.css";
  import { onMount, onDestroy } from "svelte";
  import { page } from "$app/state";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { moduleRegistry } from "$lib/modules/registry";
  import { settingsStore, loadSettings } from "$lib/state/settings.svelte";
  import { authStore, loadAuth, listenForAuthChanges } from "$lib/state/auth.svelte";
  import { scStore, loadSc } from "$lib/state/sc.svelte";
  import {
    loadStatus,
    ensureChannel,
    listenForDataProgress,
    clearAllLoading,
  } from "$lib/state/data.svelte";
  import { prefetchCatalogs, invalidateCatalogs } from "$lib/state/catalog.svelte";
  import { listen } from "@tauri-apps/api/event";
  import { onboarding, maybeStartOnboarding } from "$lib/state/onboarding.svelte";
  import { listenForNotifications, syncNotifications } from "$lib/state/notifications.svelte";
  import { Notify, setKitContext } from "@veelume/ui";
  import Onboarding from "$lib/components/Onboarding.svelte";
  import Avatar from "$lib/components/Avatar.svelte";
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

  // Home + Friends + Game Data (shell-level) + one entry per enabled module
  // (registry order).
  const nav = $derived([
    { href: "/", label: "Home", icon: "✦" },
    { href: "/friends", label: "Friends", icon: "◈" },
    { href: "/data", label: "Catalogs", icon: "▤" },
    ...moduleRegistry
      .filter((d) => settingsStore.current?.enabled_modules.includes(d.id))
      .flatMap((d) => d.nav ?? []),
  ]);

  const isActive = (href: string) =>
    href === "/" ? page.url.pathname === "/" : page.url.pathname.startsWith(href);

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

<div class="shell">
  <aside class="sidebar">
    <div class="brand">
      <span class="brand-name">Starlume</span>
      <span class="bell-wrap">
        <Notify.Bell onclick={() => (centerOpen = !centerOpen)} />
        <Notify.Center
          open={centerOpen}
          onclose={() => (centerOpen = false)}
          side="right"
          align="start"
        />
      </span>
    </div>

    <nav>
      {#each nav as item (item.href)}
        <a class="nav-item" class:active={isActive(item.href)} href={item.href}>
          <span class="nav-icon">{item.icon}</span>
          <span class="nav-label">{item.label}</span>
        </a>
      {/each}
    </nav>

    <div class="account">
      <!-- Primary identity is the RSI account (who you are in the game).
           Discord is a secondary connector, surfaced on the Me page. -->
      <a class="account-link" href="/me" title="Me">
      {#if scStore.account}
        <Avatar
          text={scStore.account.handle.charAt(0).toUpperCase()}
          title={scStore.account.handle}
        />
        <div class="account-meta">
          <span class="account-line">{scStore.account.handle}</span>
          <span class="account-sub">
            {#if scStore.account.citizen_record}
              ✓ Citizen #{scStore.account.citizen_record}
            {:else}
              RSI · unverified
            {/if}
            {#if authStore.current?.logged_in}<span class="linked" title="Discord linked">◆</span>{/if}
          </span>
        </div>
      {:else if authStore.current?.logged_in && authStore.profile}
        <!-- No SC account recognized yet, but Discord is linked. -->
        <Avatar
          text={authStore.profile.username.charAt(0).toUpperCase()}
          src={authStore.profile.avatar_url}
          title={authStore.profile.username}
        />
        <div class="account-meta">
          <span class="account-line">{authStore.profile.username}</span>
          <span class="account-sub">Discord · no SC account</span>
        </div>
      {:else}
        <Avatar text="?" muted title="Not set up" />
        <div class="account-meta">
          <span class="account-line">Not set up</span>
          <span class="account-sub">open to set up →</span>
        </div>
      {/if}
      </a>
      <a
        class="cog"
        class:active={isActive("/settings")}
        href="/settings"
        title="Settings"
        aria-label="Settings"
      >
        <svg
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="1.8"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <circle cx="12" cy="12" r="3" />
          <path
            d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"
          />
        </svg>
      </a>
    </div>
  </aside>

  <main>
    {@render children()}
  </main>
</div>

<Notify.Toasts />

{#if onboarding.open}
  <Onboarding />
{/if}

<style>
  .shell {
    display: flex;
    height: 100vh;
  }

  .sidebar {
    width: 200px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    border-right: 1px solid var(--border);
    padding: 14px 10px 10px;
  }

  .brand {
    display: flex;
    align-items: center;
    padding: 0 8px 14px;
  }

  .brand-name {
    font-family: var(--font-display);
    font-weight: var(--weight-bold);
    color: var(--accent);
    letter-spacing: 0.01em;
  }

  /* Kit Bell/Center anchor — the Popup positions against this wrapper. */
  .bell-wrap {
    position: relative;
    margin-left: auto;
    flex: 0 0 auto;
  }

  nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 9px;
    text-decoration: none;
    font-family: var(--font-display);
    padding: 7px 8px;
    border-radius: var(--radius);
    color: var(--text-dim);
    transition: background 90ms, color 90ms;
  }
  .nav-item:hover {
    background: var(--bg-raised);
    color: var(--text);
  }
  .nav-item.active {
    background: var(--accent-fill-weak);
    color: var(--accent);
    font-weight: var(--weight-bold);
  }
  .nav-icon {
    width: 1.1rem;
    text-align: center;
    font-size: 0.9rem;
  }
  .nav-label {
    font-size: 0.9rem;
  }

  .account {
    display: flex;
    align-items: center;
    gap: 9px;
    border-top: 1px solid var(--border);
    padding: 10px 6px 2px;
    margin-top: 8px;
  }

  .account-link {
    display: flex;
    align-items: center;
    gap: 9px;
    flex: 1;
    min-width: 0;
    text-decoration: none;
    color: inherit;
    border-radius: 7px;
    padding: 3px 4px;
    margin: -3px -4px;
    transition: background 90ms;
  }
  .account-link:hover {
    background: var(--bg-raised);
  }
  .account-meta {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }
  .account-line {
    font-size: 0.83rem;
    font-weight: 500;
  }
  .account-sub {
    font-size: 0.7rem;
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .linked {
    color: var(--discord);
    margin-left: 0.2rem;
  }

  .cog {
    flex: 0 0 auto;
    width: 1.9rem;
    height: 1.9rem;
    display: grid;
    place-items: center;
    border-radius: 7px;
    color: var(--text-dim);
    transition: background 90ms, color 90ms;
  }
  .cog svg {
    width: 1.05rem;
    height: 1.05rem;
    display: block;
  }
  .cog:hover {
    background: var(--bg-raised);
    color: var(--text);
  }
  .cog.active {
    background: var(--accent-fill-weak);
    color: var(--accent);
  }

  main {
    flex: 1;
    overflow: auto;
    padding: 24px;
  }
</style>
