<script lang="ts">
  // The follow list: your accounts, each Always show / Filter by content /
  // Mute on top of the content filter, plus the staff the feed saw that are
  // not on it. "Posts" opens the feed on that account (filtered-out posts
  // included) to gauge whether it is worth following. Counts cover the
  // cached feed's last 30 days.
  import { onMount } from "svelte";
  import { Button, Segmented } from "@veelume/ui";
  import { Plus, X } from "lucide-svelte";
  import {
    devtrackerStore,
    follow,
    followHandle,
    loadAuthors,
    resetAuthors,
    setAuthorMode,
    unfollow,
  } from "$lib/state/devtracker.svelte";

  let { onview }: { onview: (id: string, nickname: string) => void } = $props();

  const options = [
    { value: "always", label: "Always" },
    { value: "filter", label: "Filter" },
    { value: "mute", label: "Mute" },
  ];

  const listed = $derived(devtrackerStore.authors?.filter((a) => a.listed) ?? []);
  const others = $derived(devtrackerStore.authors?.filter((a) => !a.listed) ?? []);

  let handle = $state("");
  let adding = $state(false);
  let addError = $state("");

  async function addHandle() {
    if (!handle.trim() || adding) return;
    adding = true;
    addError = (await followHandle(handle)) ?? "";
    if (!addError) handle = "";
    adding = false;
  }

  onMount(() => void loadAuthors());
</script>

<div class="head">
  <p class="dim">
    <strong>Filter</strong> lets the content filter decide (the default). <strong>Always</strong>
    shows every post from that account, small talk included. <strong>Mute</strong> hides it.
    Taking an account off the list sends it back to the content filter.
  </p>
  <form
    class="add"
    onsubmit={(e) => {
      e.preventDefault();
      void addHandle();
    }}
  >
    <input class="input" placeholder="Spectrum handle" bind:value={handle} disabled={adding} />
    <Button variant="outline" type="submit" disabled={adding || !handle.trim()}>
      <Plus size={14} /> Add
    </Button>
  </form>
</div>
{#if addError}<p class="error">{addError}</p>{/if}

{#if devtrackerStore.authors}
  <table class="data">
    <thead>
      <tr>
        <th>Account</th>
        <th class="num">Posts</th>
        <th class="num">Kept</th>
        <th>Mode</th>
        <th></th>
      </tr>
    </thead>
    <tbody>
      {#each listed as a (a.id)}
        <tr>
          <td>{a.nickname}</td>
          <td class="num mono">{a.posts || "–"}</td>
          <td class="num mono">{a.posts ? a.kept : "–"}</td>
          <td>
            <Segmented {options} value={a.mode} onchange={(m) => void setAuthorMode(a.id, m)} />
          </td>
          <td class="actions">
            <Button variant="ghost" disabled={!a.posts} onclick={() => onview(a.id, a.nickname)}>
              Posts
            </Button>
            <Button variant="ghost" size="icon" title="Take off the list" onclick={() => void unfollow(a.id)}>
              <X size={15} />
            </Button>
          </td>
        </tr>
      {:else}
        <tr><td colspan="5" class="dim">Nobody on the list. Add accounts below or by handle.</td></tr>
      {/each}
    </tbody>
  </table>

  {#if others.length}
    <h3>Other staff in the feed</h3>
    <table class="data">
      <tbody>
        {#each others as a (a.id)}
          <tr>
            <td>{a.nickname}</td>
            <td class="num mono">{a.posts}</td>
            <td class="num mono">{a.kept}</td>
            <td class="actions">
              <Button variant="ghost" onclick={() => onview(a.id, a.nickname)}>Posts</Button>
              <Button variant="outline" onclick={() => void follow(a.id, a.nickname)}>
                <Plus size={14} /> Follow
              </Button>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}

  <div class="foot">
    <Button variant="ghost" onclick={() => void resetAuthors()}>Reset list to defaults</Button>
  </div>
{/if}

<style>
  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 8px;
  }
  .head p {
    margin: 0;
    max-width: 75ch;
  }
  .add {
    display: flex;
    gap: 8px;
    flex-shrink: 0;
  }
  .num {
    text-align: right;
  }
  .actions {
    text-align: right;
    white-space: nowrap;
  }
  h3 {
    margin: 20px 0 4px;
    font-size: var(--text-small);
    color: var(--text-faint);
    font-family: var(--font-display);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }
  .foot {
    margin-top: 12px;
  }
</style>
