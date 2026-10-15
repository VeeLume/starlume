<script lang="ts">
  // The LIVE card: the newest release from RSI's patch notes — name,
  // highlights, build info and fix count, and the link to the article.
  import { onDestroy, onMount } from "svelte";
  import { Button } from "@veelume/ui";
  import { ExternalLink } from "lucide-svelte";
  import { openPatchNote, patchNotesStore, watchPatchNotes } from "$lib/state/patchnotes.svelte";
  import { activityFor, activityLabel } from "$lib/state/activities.svelte";
  import NowCard from "./NowCard.svelte";
  import Highlights from "./Highlights.svelte";
  import { shortDate } from "./format";

  const overview = $derived(patchNotesStore.overview);
  const live = $derived(patchNotesStore.latestLive);
  const reading = $derived(activityFor("patch-notes", ""));
  const facts = $derived(
    live
      ? [
          ...live.build_info.map((b) => `${b.label}: ${b.value}`),
          ...(live.bug_fixes != null ? [`${live.bug_fixes} fixes`] : []),
          ...(live.known_issues != null ? [`${live.known_issues} known issues`] : []),
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
  <NowCard channel="LIVE" version={live.version ?? "?"} date={shortDate(live.date, live.date_text)}>
    {#if live.name}<div class="name">{live.name}</div>{/if}
    {#if live.highlights.length}<Highlights highlights={live.highlights} />{/if}
    {#if facts.length}<div class="dim facts">{facts.join(" · ")}</div>{/if}
    {#if live.changelist}<div class="dim facts">build <span class="mono">{live.changelist}</span></div>{/if}
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
  .name {
    font-weight: var(--weight-semibold);
  }
  .facts {
    font-size: var(--text-small);
  }
  p {
    margin: 0;
  }
</style>
