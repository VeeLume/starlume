<script lang="ts">
  // Earlier LIVE releases, one line each: version, name, date. A parsed
  // release expands to its highlights and counts; older articles (not
  // parsed) are lines with only the link. Expansion is page-local
  // (frontend rule 7), never the URL.
  import { Button, Expand, createExpansion } from "@veelume/ui";
  import { ExternalLink } from "lucide-svelte";
  import { openPatchNote, patchNotesStore } from "$lib/state/patchnotes.svelte";
  import Highlights from "./Highlights.svelte";
  import { shortDate } from "./format";

  const overview = $derived(patchNotesStore.overview);
  const latestKey = $derived(patchNotesStore.latestLive?.key);
  const releases = $derived((overview?.releases ?? []).filter((r) => r.key !== latestKey));
  const expanded = createExpansion("many");

  const counts = (fixes: number | null, known: number | null) =>
    [fixes != null ? `${fixes} fixes` : null, known != null ? `${known} known issues` : null]
      .filter(Boolean)
      .join(" · ");
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
