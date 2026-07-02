<script lang="ts">
  import "../app.css";
  import { onMount, onDestroy } from "svelte";
  import { page } from "$app/state";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { moduleRegistry } from "$lib/modules/registry";
  import { settingsStore, loadSettings } from "$lib/state/settings.svelte";
  import { authStore, loadAuth, listenForAuthChanges } from "$lib/state/auth.svelte";
  import { onboarding, maybeStartOnboarding } from "$lib/state/onboarding.svelte";
  import Onboarding from "$lib/components/Onboarding.svelte";
  import Avatar from "$lib/components/Avatar.svelte";
  import { checkForUpdates } from "$lib/updater";

  let { children } = $props();

  let unlistenAuth: UnlistenFn | undefined;

  // Home + one entry per enabled module (registry order).
  const nav = $derived([
    { href: "/", label: "Home", icon: "✦" },
    ...moduleRegistry
      .filter((d) => settingsStore.current?.enabled_modules.includes(d.id))
      .flatMap((d) => d.nav ?? []),
  ]);

  const isActive = (href: string) =>
    href === "/" ? page.url.pathname === "/" : page.url.pathname.startsWith(href);

  onMount(() => {
    void (async () => {
      await loadSettings();
      await loadAuth();
      unlistenAuth = await listenForAuthChanges();
      await maybeStartOnboarding();
      void checkForUpdates();
    })();
  });

  onDestroy(() => unlistenAuth?.());
</script>

<div class="shell">
  <aside class="sidebar">
    <div class="brand">Starlume</div>

    <nav>
      {#each nav as item (item.href)}
        <a class="nav-item" class:active={isActive(item.href)} href={item.href}>
          <span class="nav-icon">{item.icon}</span>
          <span class="nav-label">{item.label}</span>
        </a>
      {/each}
    </nav>

    <div class="account">
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
    font-weight: 600;
    color: var(--accent);
    padding: 0 8px 14px;
    letter-spacing: 0.01em;
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
