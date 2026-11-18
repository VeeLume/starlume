<script lang="ts">
  // Contracts' sub pages on the Ships page's tab row (a view switch is
  // navigation, never the Segmented input). Contracts shows only its own
  // concern: next steps and plan content belong to Plans, which links here.
  import type { Snippet } from "svelte";
  import { page } from "$app/state";

  let { children }: { children: Snippet } = $props();

  const TABS = [
    { href: "/contracts", label: "Catalog" },
    { href: "/contracts/standing", label: "Standing" },
  ];
</script>

<div class="contracts">
  <nav class="tabs" aria-label="Contracts">
    {#each TABS as t (t.href)}
      <a class="tab" class:active={page.url.pathname === t.href} aria-current={page.url.pathname === t.href ? "page" : undefined} href={t.href}>
        {t.label}
      </a>
    {/each}
  </nav>
  {@render children()}
</div>

<style>
  .contracts {
    display: flex;
    flex-direction: column;
    gap: 16px;
    height: 100%;
    min-height: 0;
  }
  .tabs {
    margin-bottom: 0;
  }
  .tab {
    text-decoration: none;
  }
</style>
