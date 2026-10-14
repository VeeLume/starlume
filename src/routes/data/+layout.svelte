<script lang="ts">
  // Catalogs frame — the tab nav shared by the catalog pages (items /
  // resources / missions / manufacturers) plus the browse-channel picker.
  // These catalogs are app-framework reference surfaces over svc-data, not
  // a module (see README module rules). Install and cache management lives
  // in Settings → Game data.
  import { page } from "$app/state";
  import { dataStore, setChannel } from "$lib/state/data.svelte";

  let { children } = $props();

  const tabs = [
    { href: "/data/items", label: "Items" },
    { href: "/data/resources", label: "Resources" },
    { href: "/data/missions", label: "Missions" },
    { href: "/data/manufacturers", label: "Manufacturers" },
  ];

  const isActive = (href: string) => page.url.pathname.startsWith(href);
</script>

<div class="tabs">
  {#each tabs as tab (tab.href)}
    <a class="tab" class:active={isActive(tab.href)} href={tab.href}>{tab.label}</a>
  {/each}
  <!-- Which install the catalogs show: a picker only when there is a
       choice (Live + PTU both cooked), a quiet label otherwise. -->
  {#if dataStore.browsable.length > 1 && dataStore.channel}
    <select
      class="select tab-channel"
      aria-label="Channel"
      value={dataStore.channel}
      onchange={(e) => setChannel(e.currentTarget.value)}
    >
      {#each dataStore.browsable as channel (channel)}
        <option value={channel}>{channel}</option>
      {/each}
    </select>
  {:else if dataStore.channel}
    <span class="tab-channel dim">{dataStore.channel}</span>
  {/if}
</div>

{@render children()}

<style>
  .tab-channel {
    margin-left: auto;
    font-size: 0.78rem;
  }
</style>
