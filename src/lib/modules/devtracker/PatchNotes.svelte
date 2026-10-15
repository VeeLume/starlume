<script lang="ts">
  // Patch notes: the "Patch Notes" forum threads in the staff feed, read
  // from their titles. The newest per channel sits on top (what LIVE / PTU
  // are on right now), the full list below. Opens on Spectrum.
  import type { PatchNoteView } from "$lib/bindings";
  import { openPost } from "$lib/state/devtracker.svelte";
  import { postAge } from "./time";

  let { notes }: { notes: PatchNoteView[] } = $props();

  // Newest note per channel, in the order they were last updated.
  const latest = $derived.by(() => {
    const seen = new Set<string>();
    return notes.filter((n) => {
      const ch = n.channel ?? "";
      if (!ch || seen.has(ch)) return false;
      seen.add(ch);
      return true;
    });
  });

  const label = (n: PatchNoteView) => [n.version, n.stage].filter(Boolean).join(" ");
</script>

{#if notes.length === 0}
  <p class="dim">
    No patch notes in the cached feed yet. They appear here as CIG posts them; the cache reaches
    back as far as the tracker has been reading.
  </p>
{:else}
  <div class="latest">
    {#each latest as n (n.key)}
      <button class="card current" onclick={() => void openPost(n.key)} title={n.subject}>
        <span class="channel">{n.channel}</span>
        <span class="version">{label(n) || "—"}</span>
        <span class="dim small">
          {#if n.build}<span class="mono">{n.build}</span> ·{/if}
          {postAge(n.time_ms)}
        </span>
      </button>
    {/each}
  </div>

  <table class="data clickable">
    <thead>
      <tr><th>Channel</th><th>Version</th><th>Build</th><th>Title</th><th>Posted</th></tr>
    </thead>
    <tbody>
      {#each notes as n (n.key)}
        <tr onclick={() => void openPost(n.key)} title="Open on Spectrum">
          <td>{n.channel ?? "—"}</td>
          <td class="mono">{label(n) || "—"}</td>
          <td class="mono dim">{n.build ?? ""}</td>
          <td>{n.subject}</td>
          <td class="dim nowrap">{postAge(n.time_ms)}</td>
        </tr>
      {/each}
    </tbody>
  </table>
{/if}

<style>
  .latest {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    margin-bottom: 14px;
  }
  .current {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    min-width: 180px;
    text-align: left;
  }
  .current:hover {
    border-color: var(--accent-border);
  }
  .channel {
    font-family: var(--font-display);
    font-weight: var(--weight-bold);
    color: var(--accent);
  }
  .version {
    font-family: var(--font-mono);
    font-size: 1.1rem;
    color: var(--text-strong);
  }
  .small {
    font-size: var(--text-small);
  }
  .nowrap {
    white-space: nowrap;
  }
</style>
