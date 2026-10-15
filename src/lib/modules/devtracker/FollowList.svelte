<script lang="ts">
  // The follow list: per staff account, Always show / Filter by content /
  // Mute on top of the content filter. Counts are the cached feed's last 30
  // days, so a user can see who the filter already handles.
  import { onMount } from "svelte";
  import { Button, Segmented } from "@veelume/ui";
  import { devtrackerStore, loadAuthors, resetAuthors, setAuthorMode } from "$lib/state/devtracker.svelte";

  const options = [
    { value: "always", label: "Always" },
    { value: "filter", label: "Filter" },
    { value: "mute", label: "Mute" },
  ];

  const authors = $derived(devtrackerStore.authors);
  const overridden = $derived(authors?.filter((a) => a.mode !== "filter").length ?? 0);

  onMount(() => void loadAuthors());
</script>

<div class="head">
  <p class="dim">
    <strong>Filter</strong> lets the content filter decide (the default). <strong>Always</strong>
    shows every post from that account, small talk included. <strong>Mute</strong> hides the
    account entirely. Counts cover the last 30 days.
  </p>
  <Button variant="ghost" disabled={overridden === 0} onclick={() => void resetAuthors()}>
    Reset to defaults
  </Button>
</div>

{#if authors}
  <table class="data">
    <thead>
      <tr><th>Account</th><th class="num">Posts</th><th class="num">Kept</th><th>Mode</th></tr>
    </thead>
    <tbody>
      {#each authors as a (a.id)}
        <tr>
          <td>{a.nickname}</td>
          <td class="num mono">{a.posts || "–"}</td>
          <td class="num mono">{a.posts ? a.kept : "–"}</td>
          <td><Segmented {options} value={a.mode} onchange={(m) => void setAuthorMode(a.id, m)} /></td>
        </tr>
      {/each}
    </tbody>
  </table>
{/if}

<style>
  .head {
    display: flex;
    align-items: flex-start;
    gap: 16px;
    margin-bottom: 8px;
  }
  .head p {
    margin: 0;
    max-width: 70ch;
  }
  .num {
    text-align: right;
  }
</style>
