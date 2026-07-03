<script lang="ts">
  // The Me page — identity + the unified visibility/sharing hub. Modules
  // contribute sections via `ModuleDescriptor.meSections` (rendered while the
  // module is enabled), so "what do I share, with whom" lives in ONE place
  // instead of scattered across module pages.
  import { onMount } from "svelte";
  import { moduleRegistry } from "$lib/modules/registry";
  import { settingsStore } from "$lib/state/settings.svelte";
  import { authStore, loadAuth } from "$lib/state/auth.svelte";
  import Avatar from "$lib/components/Avatar.svelte";

  const auth = $derived(authStore.current);

  const meSections = $derived(
    moduleRegistry
      .filter((d) => settingsStore.current?.enabled_modules.includes(d.id))
      .flatMap((d) =>
        (d.meSections ?? []).map((s) => ({ ...s, moduleName: d.name, key: `${d.id}:${s.id}` })),
      ),
  );

  onMount(() => void loadAuth());
</script>

<h1>Me</h1>

{#if !auth?.logged_in}
  <p class="dim">Sign in (Settings → Account) to see your profile and sharing settings.</p>
{:else}
  <div class="profile">
    {#if authStore.profile}
      <Avatar
        text={authStore.profile.username.charAt(0).toUpperCase()}
        src={authStore.profile.avatar_url}
        size="3rem"
      />
      <div>
        <div class="username">{authStore.profile.username}</div>
        <div class="dim">via Discord</div>
      </div>
    {:else}
      <Avatar text="✓" size="3rem" />
      <div>
        <div class="username">Signed in</div>
        <div class="dim">profile unavailable</div>
      </div>
    {/if}
  </div>

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

<style>
  .profile {
    display: flex;
    align-items: center;
    gap: 14px;
    margin-bottom: 24px;
  }

  .username {
    font-size: 17px;
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
</style>
