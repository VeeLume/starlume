<script lang="ts">
  // Earlier LIVE releases, two lines each: version, name and date, and under
  // the name a bar for the fix count (scaled to the biggest release listed,
  // so a large patch shows at a glance). A parsed release expands to its highlights and
  // counts; older articles (not parsed) are lines with only the link.
  // Expansion is page-local (frontend rule 7), never the URL.
  import { onDestroy, onMount } from "svelte";
  import { Button, Expand, createExpansion } from "@veelume/ui";
  import { ExternalLink } from "lucide-svelte";
  import { openPatchNote, patchNotesStore, watchPatchNotes } from "$lib/state/patchnotes.svelte";
  import Highlights from "./Highlights.svelte";
  import { shortDate } from "./format";

  const overview = $derived(patchNotesStore.overview);
  const latestKey = $derived(patchNotesStore.latestLive?.key);
  const releases = $derived((overview?.releases ?? []).filter((r) => r.key !== latestKey));
  const expanded = createExpansion("many");
  const maxFixes = $derived(Math.max(1, ...releases.map((r) => r.bug_fixes ?? 0)));

  const counts = (fixes: number | null, known: number | null) =>
    [fixes != null ? `${fixes} fixes` : null, known != null ? `${known} known issues` : null]
      .filter(Boolean)
      .join(" · ");

  // Subscribe itself (ref-counted), like every store surface.
  let stop: (() => void) | undefined;
  onMount(() => (stop = watchPatchNotes()));
  onDestroy(() => stop?.());
</script>

{#if releases.length || overview?.older.length}
  <div class="list">
    {#each releases as r (r.key)}
      <Expand.Row
        title={r.name ?? r.title}
        open={expanded.has(r.key)}
        ontoggle={() => expanded.toggle(r.key)}
      >
        {#snippet gutter()}<span class="mono version">{r.version ?? "—"}</span>{/snippet}
        {#snippet meta()}
          {#if r.bug_fixes != null}
            <span class="fixes">
              <span class="track"><i style:width="{Math.max(4, (r.bug_fixes / maxFixes) * 100)}%"></i></span>
              {r.bug_fixes} fixes
            </span>
          {:else}
            <span class="fixes">no fix count</span>
          {/if}
        {/snippet}
        {#snippet right()}<span class="dim date">{shortDate(r.date, r.date_text)}</span>{/snippet}
        {#snippet actions()}
          <Button variant="ghost" size="icon" title="Read on RSI" onclick={() => void openPatchNote(r.article_url)}>
            <ExternalLink size={14} />
          </Button>
        {/snippet}
        <div class="detail">
          {#if r.highlights.length}<Highlights highlights={r.highlights} />{/if}
          {#if counts(r.bug_fixes, r.known_issues)}
            <div class="dim small">{counts(r.bug_fixes, r.known_issues)}</div>
          {/if}
        </div>
      </Expand.Row>
    {/each}
    {#each overview?.older ?? [] as a (a.url)}
      <Expand.Row title={a.title}>
        {#snippet gutter()}<span class="mono version">{a.version ?? "—"}</span>{/snippet}
        {#snippet right()}<span class="dim date">{a.age_text}</span>{/snippet}
        {#snippet actions()}
          <Button variant="ghost" size="icon" title="Read on RSI" onclick={() => void openPatchNote(a.url)}>
            <ExternalLink size={14} />
          </Button>
        {/snippet}
      </Expand.Row>
    {/each}
  </div>
{/if}

<style>
  .version {
    display: inline-block;
    min-width: 4.5em;
    font-weight: var(--weight-bold);
    color: color-mix(in srgb, var(--hue-live) 65%, var(--text));
  }
  .fixes {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    font-family: var(--font-mono);
    font-size: 0.7rem;
    color: var(--text-faint);
    white-space: nowrap;
  }
  .track {
    display: block;
    flex: 0 1 140px;
  }
  .track i {
    display: block;
    height: 4px;
    border-radius: 2px;
    background: color-mix(in srgb, var(--hue-live) 55%, transparent);
  }
  .date {
    font-size: var(--text-small);
    white-space: nowrap;
  }
  .detail {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 4px 0 8px;
  }
  .small {
    font-size: var(--text-small);
  }
</style>
