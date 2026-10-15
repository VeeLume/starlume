<script lang="ts">
  // The newest comm-links, one line each: type, title, when. A line expands
  // to its summary; the link opens it on RSI. Times are when Starlume first
  // saw the item (estimated from its age on the first fill).
  import { onDestroy, onMount } from "svelte";
  import { Button, Expand, createExpansion } from "@veelume/ui";
  import { ExternalLink } from "lucide-svelte";
  import { commLinksStore, openCommLink, watchCommLinks } from "$lib/state/commlinks.svelte";
  import { activityFor, activityLabel } from "$lib/state/activities.svelte";
  import { postAge } from "$lib/modules/devtracker/time";

  const overview = $derived(commLinksStore.overview);
  const reading = $derived(activityFor("comm-links", ""));
  const expanded = createExpansion("many");

  const channelName = (slug: string | null) =>
    slug ? slug.replace(/-/g, " ").replace(/^./, (c) => c.toUpperCase()) : null;

  // Subscribe itself (ref-counted), like every store surface.
  let stop: (() => void) | undefined;
  onMount(() => (stop = watchCommLinks()));
  onDestroy(() => stop?.());
</script>

{#if commLinksStore.error}<p class="error">{commLinksStore.error}</p>{/if}
{#if overview?.last_error}<p class="error">{overview.last_error}</p>{/if}

{#if overview?.items.length}
  <div class="list">
    {#each overview.items as c (c.id)}
      <Expand.Row
        title={c.title}
        open={expanded.has(c.id)}
        ontoggle={c.summary ? () => expanded.toggle(c.id) : undefined}
      >
        {#snippet gutter()}<span class="kind">{c.kind}</span>{/snippet}
        {#snippet right()}
          <span class="dim when">{c.published_ms ? postAge(c.published_ms) : ""}</span>
        {/snippet}
        {#snippet actions()}
          <Button variant="ghost" size="icon" title="Read on RSI" onclick={() => void openCommLink(c.url)}>
            <ExternalLink size={14} />
          </Button>
        {/snippet}
        {#if c.summary}
          <div class="detail">
            <p>{c.summary}</p>
            {#if channelName(c.channel)}<div class="dim small">{channelName(c.channel)}</div>{/if}
          </div>
        {/if}
      </Expand.Row>
    {/each}
  </div>
{:else if reading}
  <p class="dim">⟳ {activityLabel(reading)}</p>
{:else if overview && !overview.online}
  <p class="dim">
    Online features are off, so Starlume doesn't read RSI's comm-links
    (<a href="/settings/online">Settings → Online</a>).
  </p>
{:else if overview}
  <p class="dim">Not read yet. The first check runs shortly after start.</p>
{/if}

<style>
  .kind {
    display: inline-block;
    min-width: 4.5em;
    font-size: var(--text-small);
    color: var(--text-faint);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .when {
    font-size: var(--text-small);
    white-space: nowrap;
  }
  .detail {
    padding: 4px 0 8px;
  }
  .detail p {
    margin: 0 0 4px;
    max-width: 90ch;
  }
  .small {
    font-size: var(--text-small);
  }
</style>
