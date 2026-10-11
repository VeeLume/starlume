<script lang="ts">
  // What a catalog page shows when no build is browsable yet. While the
  // data is being prepared (startup warm, post-patch cook) it says so with
  // the live stage — that is the normal first-run case, and the page fills
  // itself when the cook lands (`data:changed`). Only with nothing running
  // does it point at Settings → Game data, where loading is managed.
  import { activityStore, activityLabel } from "$lib/state/activities.svelte";

  const preparing = $derived(activityStore.items.find((a) => a.kind === "data-load"));
</script>

{#if preparing}
  <p class="dim">⟳ Preparing game data — {activityLabel(preparing)}</p>
{:else}
  <p class="dim">
    No game data loaded yet — load it in <a href="/settings/game-data">Settings → Game data</a>.
  </p>
{/if}
