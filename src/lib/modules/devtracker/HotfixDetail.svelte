<script lang="ts">
  // The dev tracker's block under an install on Home's Game status card:
  // what the hotfix this install runs changed, from Hotfix Central — the
  // part many players never find. Only for an install whose build a
  // hotfix entry names; nothing otherwise (or while the tracker is off).
  import { onDestroy, onMount } from "svelte";
  import { ExternalLink } from "lucide-svelte";
  import type { InstallStatusProps } from "../types";
  import { scStore } from "$lib/state/sc.svelte";
  import { devtrackerStore, openHotfixCentral, watchDevtracker } from "$lib/state/devtracker.svelte";
  import { installChangelist } from "$lib/home/version";
  import { shortDate } from "$lib/components/patchnotes/format";
  import { hotfixFor } from "./cycle";

  let { channelKey }: InstallStatusProps = $props();

  /** Lines shown before "and N more". */
  const MAX_ITEMS = 4;

  const install = $derived(scStore.status?.installs.find((i) => i.channel.toLowerCase() === channelKey));
  const build = $derived(install ? installChangelist(install.version) : null);
  const entry = $derived(hotfixFor(devtrackerStore.overview?.hotfix ?? null, build));

  let stop: (() => void) | undefined;
  onMount(() => (stop = watchDevtracker()));
  onDestroy(() => stop?.());
</script>

{#if entry && entry.items.length}
  <div class="hotfix">
    <div class="head">
      <span class="title">What hotfix {build} changed</span>
      {#if entry.date}<span class="date">{shortDate(entry.date)}</span>{/if}
      <button type="button" class="link" onclick={() => void openHotfixCentral()}>
        Hotfix Central <ExternalLink size={12} />
      </button>
    </div>
    <ul>
      {#each entry.items.slice(0, MAX_ITEMS) as item, i (i)}<li>{item}</li>{/each}
    </ul>
    {#if entry.items.length > MAX_ITEMS}
      <div class="more">and {entry.items.length - MAX_ITEMS} more on Hotfix Central</div>
    {/if}
  </div>
{/if}

<style>
  .hotfix {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 8px 12px;
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--hue-hotfix) 7%, color-mix(in srgb, var(--bg-sunk) 75%, transparent));
    border: 1px solid color-mix(in srgb, var(--hue-hotfix) 22%, transparent);
    backdrop-filter: blur(4px);
  }
  .head {
    display: flex;
    align-items: baseline;
    gap: 8px;
    flex-wrap: wrap;
  }
  .title {
    font-size: var(--text-small);
    font-weight: var(--weight-semibold);
    color: color-mix(in srgb, var(--hue-hotfix) 80%, var(--text));
  }
  .date {
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .link {
    margin-left: auto;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: var(--text-small);
    color: var(--accent);
  }
  .link:hover {
    text-decoration: underline;
  }
  ul {
    margin: 0;
    padding-left: 18px;
    list-style: disc;
    font-size: 0.9rem;
    color: var(--text-dim);
  }
  li + li {
    margin-top: 2px;
  }
  .more {
    font-size: var(--text-small);
    color: var(--text-faint);
  }
</style>
