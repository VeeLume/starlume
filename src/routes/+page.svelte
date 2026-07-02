<script lang="ts">
  import { moduleRegistry } from "$lib/modules/registry";
  import { settingsStore } from "$lib/state/settings.svelte";

  const enabled = $derived(
    moduleRegistry.filter((d) =>
      settingsStore.current?.enabled_modules.includes(d.id),
    ),
  );
</script>

<h1>Starlume</h1>
<p class="dim">The Star Citizen companion.</p>

{#if moduleRegistry.length === 0}
  <p class="dim">
    No feature modules are compiled in yet. The first one to land is the cargo
    planner (see the migration order in the design doc).
  </p>
{:else if enabled.length === 0}
  <p class="dim">No modules enabled — pick some in Settings or re-run setup.</p>
{:else}
  <ul>
    {#each enabled as m (m.id)}
      <li>{m.icon} {m.name} — {m.description}</li>
    {/each}
  </ul>
{/if}

<style>
  .dim {
    color: var(--text-dim);
  }
</style>
