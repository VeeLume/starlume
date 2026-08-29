<script lang="ts">
  // Game Data section frame — one heading + catalog tab nav shared by the
  // overview (cache/load) and the catalog pages (items / resources /
  // missions / manufacturers). These catalogs are app-framework reference
  // surfaces over svc-data, not a module (see README module rules).
  import { page } from "$app/state";
  import { dataStore } from "$lib/state/data.svelte";

  let { children } = $props();

  const tabs = [
    { href: "/data", label: "Overview" },
    { href: "/data/items", label: "Items" },
    { href: "/data/resources", label: "Resources" },
    { href: "/data/missions", label: "Missions" },
    { href: "/data/manufacturers", label: "Manufacturers" },
  ];

  const isActive = (href: string) =>
    href === "/data" ? page.url.pathname === "/data" : page.url.pathname.startsWith(href);
</script>

<div class="tabs">
  {#each tabs as tab (tab.href)}
    <a class="tab" class:active={isActive(tab.href)} href={tab.href}>{tab.label}</a>
  {/each}
  {#if dataStore.channel}
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
