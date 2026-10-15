<script lang="ts">
  // The LIVE card: the newest release from RSI's patch notes — version and
  // name, the headline items as tags, the facts as stat tiles, and the link
  // to the article.
  import { onDestroy, onMount } from "svelte";
  import { Button } from "@veelume/ui";
  import { ExternalLink } from "lucide-svelte";
  import { openPatchNote, patchNotesStore, watchPatchNotes } from "$lib/state/patchnotes.svelte";
  import { activityFor, activityLabel } from "$lib/state/activities.svelte";
  import NowCard from "./NowCard.svelte";
  import { highlightGroup, shortDate } from "./format";

  const overview = $derived(patchNotesStore.overview);
  const live = $derived(patchNotesStore.latestLive);
  const reading = $derived(activityFor("patch-notes", ""));
  const tags = $derived((live?.highlights ?? []).flatMap((h) => highlightGroup(h).items));
  const stats = $derived(
    live
      ? [
          ...(live.bug_fixes != null ? [{ value: String(live.bug_fixes), label: "fixes" }] : []),
          ...(live.known_issues != null ? [{ value: String(live.known_issues), label: "known issues" }] : []),
          ...live.build_info.map((b) => ({ value: b.value, label: b.label })),
          ...(live.changelist ? [{ value: live.changelist, label: "build" }] : []),
        ]
      : [],
  );

  // Subscribe itself (ref-counted), like every store surface: the card must
  // not depend on its page having loaded the store.
  let stop: (() => void) | undefined;
  onMount(() => (stop = watchPatchNotes()));
  onDestroy(() => stop?.());
</script>

{#if live}
  <NowCard
    channel="LIVE"
    version={live.version ?? "?"}
    name={live.name ?? ""}
    date={shortDate(live.date, live.date_text)}
  >
    {#if tags.length}
      <div class="tags">{#each tags as t, i (i)}<span class="tag">{t}</span>{/each}</div>
    {/if}
    {#if stats.length}
      <div class="stat-tiles">
        {#each stats as s (s.label)}
          <div class="stat-tile" title="{s.label}: {s.value}"><b>{s.value}</b><span>{s.label}</span></div>
        {/each}
      </div>
    {/if}
    {#snippet action()}
      <Button variant="outline" onclick={() => void openPatchNote(live.article_url)}>
        Read on RSI <ExternalLink size={14} />
      </Button>
    {/snippet}
  </NowCard>
{:else}
  <NowCard channel="LIVE" version="—" muted>
    <p class="dim">
      {#if reading}
        ⟳ {activityLabel(reading)}
      {:else if overview && !overview.online}
        Online features are off, so Starlume doesn't read RSI's patch notes
        (<a href="/settings/online">Settings → Online</a>).
      {:else}
        Not read yet. The first check runs shortly after start.
      {/if}
    </p>
  </NowCard>
{/if}

<style>
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .tag {
    font-size: var(--text-small);
    padding: 1px 8px;
    border-radius: var(--radius-chip);
    background: color-mix(in srgb, var(--text) 7%, transparent);
    color: var(--text-dim);
  }
  p {
    margin: 0;
  }
</style>
