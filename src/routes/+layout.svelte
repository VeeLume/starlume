<script lang="ts">
  import "../app.css";
  import { onMount, onDestroy } from "svelte";
  import { goto } from "$app/navigation";
  import { commands } from "$lib/bindings";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { moduleRegistry } from "$lib/modules/registry";
  import { settingsStore, loadSettings } from "$lib/state/settings.svelte";
  import { authStore, loadAuth, listenForAuthChanges } from "$lib/state/auth.svelte";
  import { scStore, loadSc } from "$lib/state/sc.svelte";
  import { loadStatus, ensureChannel, refreshBuildsIfShown } from "$lib/state/data.svelte";
  import { listenForActivities, syncActivities } from "$lib/state/activities.svelte";
  import { prefetchCatalogs, invalidateCatalogs } from "$lib/state/catalog.svelte";
  import { listen } from "@tauri-apps/api/event";
  import { onboarding, maybeStartOnboarding } from "$lib/state/onboarding.svelte";
  import { listenForNotifications, syncNotifications } from "$lib/state/notifications.svelte";
  import { loadOwnedBlueprints } from "$lib/state/blueprints.svelte";
  import { Notify, Shell, setKitContext, type NavGroup } from "@veelume/ui";
  import { House, Library, Newspaper, Settings, Users } from "lucide-svelte";
  import brandMark from "../../src-tauri/icons/source/icon-small.svg?url";
  import Onboarding from "$lib/components/Onboarding.svelte";
  import { checkForUpdates } from "$lib/updater";
  import { SHOW_UNRELEASED } from "$lib/unreleased";

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
  let unlistenActivities: UnlistenFn | undefined;
  let unlistenDataChanged: UnlistenFn | undefined;
  let unlistenBlueprints: UnlistenFn | undefined;

  // Startup hydration (frontend rule, CLAUDE.md): refresh install statuses
  // and prefetch the default channel's catalogs so pages render from cache.
  // Cheap when nothing changed; also the focus/`data:changed` catch-up.
  // No prefetch while the catalog pages are unreleased — nothing would read it.
  async function hydrateData() {
    await loadStatus();
    refreshBuildsIfShown();
    const channel = await ensureChannel();
    if (channel && SHOW_UNRELEASED) prefetchCatalogs(channel);
  }

  // Notification center (sidebar bell). The kit Center marks everything
  // read on open; the per-session log stays in the panel.
  let centerOpen = $state(false);

  // Three groups, divided: what's going on (Home, News, and module pages
  // in the `news` section — the dev tracker), the enabled modules' tools
  // (registry order), then Friends + Catalogs (shell-level). Friends and
  // Catalogs are unreleased. Kit NavItems — the shell owns active-state
  // matching; rows render through the rail's `item` snippet below.
  const moduleNav = $derived(
    moduleRegistry
      .filter((d) => settingsStore.current?.enabled_modules.includes(d.id))
      .flatMap((d) => d.nav ?? []),
  );
  const navGroups: NavGroup[] = $derived(
    [
      [
        { path: "/", label: "Home", icon: House },
        { path: "/news", label: "News", icon: Newspaper },
        ...moduleNav.filter((e) => e.section === "news").map((e) => ({ path: e.href, label: e.label, icon: e.icon })),
      ],
      moduleNav.filter((e) => e.section !== "news").map((e) => ({ path: e.href, label: e.label, icon: e.icon })),
      SHOW_UNRELEASED
        ? [
            { path: "/friends", label: "Friends", icon: Users },
            { path: "/data", label: "Catalogs", icon: Library },
          ]
        : [],
    ]
      .filter((items) => items.length > 0)
      .map((items) => ({ items })),
  );

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
    // A window closed while hidden (window.rs) comes back on its page.
    void commands.takeRestoreRoute().then((route) => {
      if (route && route !== location.pathname + location.search + location.hash) {
        void goto(route, { replaceState: true });
      }
    });
    void (async () => {
      // The single notification funnel — live events plus a hydration pass
      // for anything raised before mount (or while the webview was
      // suspended; the focus handler below covers later suspensions).
      unlistenNotify = await listenForNotifications();
      await syncNotifications();
      // Background activity — same funnel shape: live snapshots plus a
      // catch-up read. The center's "Now" section, the bell's busy mark and
      // the inline surfaces (catalog card, langpatch row) all read this
      // store; there is no page-wide banner.
      unlistenActivities = await listenForActivities();
      await syncActivities();
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
      unlistenDataChanged = await listen("data:changed", () => {
        invalidateCatalogs();
        void hydrateData();
      });
      await hydrateData();
    })();
    // Owned blueprints: render the cached set immediately, and re-load when
    // the backend's startup/manual fetch changes it (catalog + text marks).
    void (async () => {
      await loadOwnedBlueprints();
      unlistenBlueprints = await listen("blueprints:changed", () => void loadOwnedBlueprints());
    })();
  });

  onDestroy(() => {
    unlistenAuth?.();
    unlistenNotify?.();
    unlistenActivities?.();
    unlistenDataChanged?.();
    unlistenBlueprints?.();
  });
</script>

<svelte:window
  onfocus={() => {
    void syncNotifications();
    void syncActivities();
    void hydrateData();
  }}
/>

<Shell.Root groups={navGroups}>
  <!-- The brand speaks the display face (Lekton); the nav rows and the
       account footer the body face, at 16 px icons (design pass). -->
  <Shell.Rail>
    {#snippet header({ showLabels })}
      <div
        class="relative flex items-center"
        class:w-full={showLabels}
        class:justify-center={!showLabels}
      >
        {#if showLabels}
          <span class="brand min-w-0 flex-1 px-2">
            <img src={brandMark} alt="" />
            <span class="brand-name truncate">Starlume</span>
          </span>
        {:else}
          <img class="brand-mark-only" src={brandMark} alt="Starlume" />
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
    {#snippet item({ item: nav, active, showLabels })}
      {@const Icon = nav.icon}
      <a
        href={nav.path}
        class="nav-row"
        class:active
        class:icon-only={!showLabels}
        aria-current={active ? "page" : undefined}
        title={showLabels ? undefined : nav.label}
      >
        {#if Icon}<Icon size={16} />{/if}
        {#if showLabels}<span class="truncate">{nav.label}</span>{/if}
      </a>
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
    {@render children()}
  </Shell.Content>
</Shell.Root>

<Notify.Toasts />

{#if onboarding.open}
  <Onboarding />
{/if}

<style>
  .brand {
    display: flex;
    align-items: center;
    gap: 9px;
  }
  .brand img,
  .brand-mark-only {
    width: 22px;
    height: 22px;
    flex: none;
    border-radius: 6px;
  }
  .brand-name {
    font-family: var(--font-display);
    font-size: 1.05rem;
    font-weight: 700;
    color: var(--text-strong);
    letter-spacing: 0.04em;
  }
  /* Nav rows (the rail's `item` snippet): body face, 16 px icons, a soft
   * violet tint on the active page — the design-pass mockup. */
  .nav-row {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    height: 34px;
    padding: 0 10px;
    border-radius: var(--radius);
    font-family: var(--font-body);
    font-size: 0.93rem;
    color: var(--text-dim);
    text-decoration: none;
    transition: background-color var(--transition-fast) var(--easing);
  }
  .nav-row :global(svg) {
    flex: none;
  }
  .nav-row:hover {
    background: color-mix(in srgb, var(--text) 5%, transparent);
    color: var(--text);
  }
  .nav-row.active {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    color: var(--text-strong);
  }
  .nav-row.icon-only {
    justify-content: center;
    width: 40px;
    padding: 0;
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
</style>
