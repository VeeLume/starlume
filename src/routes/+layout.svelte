<script lang="ts">
  import "../app.css";
  import { onMount, onDestroy } from "svelte";
  import { page } from "$app/state";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { moduleRegistry } from "$lib/modules/registry";
  import { settingsStore, loadSettings } from "$lib/state/settings.svelte";
  import { authStore, loadAuth, listenForAuthChanges } from "$lib/state/auth.svelte";
  import { onboarding, maybeStartOnboarding } from "$lib/state/onboarding.svelte";
  import {
    notifications,
    markAllRead,
    listenForNotifications,
    syncNotifications,
  } from "$lib/state/notifications.svelte";
  import Onboarding from "$lib/components/Onboarding.svelte";
  import Avatar from "$lib/components/Avatar.svelte";
  import Toasts from "$lib/components/Toasts.svelte";
  import NotificationCenter from "$lib/components/NotificationCenter.svelte";
  import { checkForUpdates } from "$lib/updater";

  let { children } = $props();

  let unlistenAuth: UnlistenFn | undefined;
  let unlistenNotify: UnlistenFn | undefined;

  // Notification center (sidebar bell). Opening it marks everything read so
  // the bell badge clears; the per-session log stays in the panel.
  let centerOpen = $state(false);
  function toggleCenter() {
    centerOpen = !centerOpen;
    if (centerOpen) markAllRead();
  }

  // Home + Friends (shell-level) + one entry per enabled module (registry
  // order).
  const nav = $derived([
    { href: "/", label: "Home", icon: "✦" },
    { href: "/friends", label: "Friends", icon: "◈" },
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
      await loadAuth();
      unlistenAuth = await listenForAuthChanges();
      await maybeStartOnboarding();
      void checkForUpdates();
    })();
  });

  onDestroy(() => {
    unlistenAuth?.();
    unlistenNotify?.();
  });
</script>

<svelte:window onfocus={() => void syncNotifications()} />

<div class="shell">
  <aside class="sidebar">
    <div class="brand">
      <span class="brand-name">Starlume</span>
      <button
        class="bell"
        class:open={centerOpen}
        onclick={toggleCenter}
        title="Notifications"
        aria-label="Notifications"
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
          <path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9" />
          <path d="M13.73 21a2 2 0 0 1-3.46 0" />
        </svg>
        {#if notifications.unread > 0}
          <span class="bell-badge">{notifications.unread > 9 ? "9+" : notifications.unread}</span>
        {/if}
      </button>
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
      <a class="account-link" href="/me" title="Me">
      {#if authStore.current?.logged_in}
        {#if authStore.profile}
          <Avatar
            text={authStore.profile.username.charAt(0).toUpperCase()}
            src={authStore.profile.avatar_url}
            title={authStore.profile.username}
          />
          <div class="account-meta">
            <span class="account-line">{authStore.profile.username}</span>
            <span class="account-sub">via Discord</span>
          </div>
        {:else}
          <!-- Signed in but the server is unreachable right now. -->
          <Avatar text="✓" title="Signed in" />
          <div class="account-meta">
            <span class="account-line">Signed in</span>
            <span class="account-sub">profile unavailable</span>
          </div>
        {/if}
      {:else}
        <Avatar text="?" muted title="Not signed in" />
        <div class="account-meta">
          <span class="account-line">Not signed in</span>
          <span class="account-sub">
            {authStore.current?.server_configured ? "sign in via Settings" : "offline"}
          </span>
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

<NotificationCenter open={centerOpen} onClose={() => (centerOpen = false)} />
<Toasts />

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
    font-weight: 600;
    color: var(--accent);
    letter-spacing: 0.01em;
  }

  .bell {
    position: relative;
    margin-left: auto;
    flex: 0 0 auto;
    display: grid;
    place-items: center;
    padding: 0.25rem 0.3rem;
    background: transparent;
    border: none;
    cursor: pointer;
    color: var(--text-dim);
    transition: color 90ms, transform 90ms;
  }
  .bell svg {
    display: block;
    width: 1.05rem;
    height: 1.05rem;
  }
  .bell:hover {
    color: var(--text);
    transform: scale(1.12);
  }
  .bell.open {
    color: var(--accent);
  }
  .bell-badge {
    position: absolute;
    top: -0.1rem;
    right: -0.1rem;
    min-width: 1rem;
    height: 1rem;
    padding: 0 0.2rem;
    display: grid;
    place-items: center;
    border-radius: 999px;
    background: var(--accent);
    color: var(--on-accent);
    font-size: 0.6rem;
    font-weight: 700;
    line-height: 1;
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
    padding: 7px 8px;
    border-radius: 7px;
    color: var(--text-dim);
    transition: background 90ms, color 90ms;
  }
  .nav-item:hover {
    background: var(--bg-raised);
    color: var(--text);
  }
  .nav-item.active {
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    color: var(--accent);
    font-weight: 500;
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
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    color: var(--accent);
  }

  main {
    flex: 1;
    overflow: auto;
    padding: 24px;
  }
</style>
