<script lang="ts">
  // Catalogs frame — the tab nav shared by the catalog pages (items /
  // resources / missions / manufacturers) plus the browse-channel picker.
  // These catalogs are app-framework reference surfaces over svc-data, not
  // a module (see CONTRIBUTING module rules). Install and cache management lives
  // in Settings → Game data.
  import { page } from "$app/state";
  import { Select } from "@veelume/ui";
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
    <span class="tab-channel">
      <Select
        label="Channel"
        options={dataStore.browsable.map((c) => ({ value: c, label: c }))}
        value={dataStore.channel}
        onchange={(c) => setChannel(c)}
      />
    </span>
  {:else if dataStore.channel}
    <span class="tab-channel dim">{dataStore.channel}</span>
  {/if}
</div>

{@render children()}

<style>
  .tab-channel {
    margin-left: auto;
    align-self: center;
    font-size: 0.78rem;
  }
</style>
