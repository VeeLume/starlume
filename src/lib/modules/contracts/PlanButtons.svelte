<script lang="ts">
  // Plan a mission or a rank from Contracts: as a goal (Plans derives the
  // path to it) or as a bookmark (a parked plan: shown, no steps). Contracts
  // only creates the plan; the path lives in Plans (Contracts is not Plans).
  // Hidden while the Plans module is off; once planned, a link there.
  import { Button } from "@veelume/ui";
  import type { Plan } from "$lib/bindings";
  import { settingsStore } from "$lib/state/settings.svelte";
  import { loadPlans, plansStore } from "$lib/state/plans.svelte";

  let {
    planned,
    add,
    what = "it",
  }: {
    /** The plan it already has, if any. */
    planned: Plan | undefined;
    add: (parked: boolean) => Promise<boolean>;
    /** What is planned, for the button labels ("it", "Guild Member"). */
    what?: string;
  } = $props();

  const on = $derived(settingsStore.current?.enabled_modules.includes("plans") ?? false);
  $effect(() => {
    if (on && plansStore.plans === null) void loadPlans();
  });
  let busy = $state(false);
  async function run(parked: boolean) {
    busy = true;
    await add(parked);
    busy = false;
  }
</script>

{#if on}
  <span class="plan-act">
    {#if planned}
      <Button variant="outline" href="/plans">
        {planned.parked ? "Bookmarked in Plans" : "In your plans"}
      </Button>
    {:else}
      <Button variant="primary" disabled={busy} onclick={() => void run(false)}>Plan {what}</Button>
      <Button variant="outline" disabled={busy} onclick={() => void run(true)}>Bookmark</Button>
    {/if}
  </span>
{/if}

<style>
  .plan-act {
    display: inline-flex;
    gap: 8px;
    align-items: center;
  }
</style>
