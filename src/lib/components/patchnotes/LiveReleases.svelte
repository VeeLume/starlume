<script lang="ts">
  // The LIVE releases: a short overview each — patch name, highlights, build
  // info, fix counts — with a link to the RSI article (no in-app reader,
  // decision 2026-10-15). The newest release is the large card; older parsed
  // releases are compact rows; older articles are links only.
  import { onDestroy, onMount } from "svelte";
  import { Button } from "@veelume/ui";
  import { ExternalLink, RefreshCw } from "lucide-svelte";
  import type { ReleaseView } from "$lib/bindings";
  import {
    openPatchNote,
    patchNotesStore,
    refreshPatchNotes,
    watchPatchNotes,
  } from "$lib/state/patchnotes.svelte";
  import { activityFor, activityLabel } from "$lib/state/activities.svelte";
  import { postAge } from "$lib/modules/devtracker/time";

  const overview = $derived(patchNotesStore.overview);
  const newest = $derived(overview?.releases[0] ?? null);
  const rest = $derived(overview?.releases.slice(1) ?? []);
  const reading = $derived(activityFor("patch-notes", ""));

  const when = (r: ReleaseView) =>
    r.date
      ? new Date(`${r.date}T00:00:00`).toLocaleDateString([], { dateStyle: "medium" })
      : r.date_text;

  const counts = (r: ReleaseView) =>
    [
      r.bug_fixes != null ? `${r.bug_fixes} fixes` : null,
      r.known_issues != null ? `${r.known_issues} known issues` : null,
    ].filter(Boolean) as string[];

  let stop: (() => void) | undefined;
  onMount(() => (stop = watchPatchNotes()));
  onDestroy(() => stop?.());
</script>

<div class="bar">
  <span class="dim small">
    {#if reading}
      ⟳ {activityLabel(reading)}
    {:else if overview?.fetched_at}
      Checked {postAge(overview.fetched_at * 1000)}
    {/if}
  </span>
  {#if overview?.online}
    <Button
      variant="ghost"
      disabled={patchNotesStore.refreshing}
      onclick={() => void refreshPatchNotes()}
    >
      <RefreshCw size={14} /> Check now
    </Button>
  {/if}
</div>

{#if patchNotesStore.error}<p class="error">{patchNotesStore.error}</p>{/if}
{#if overview?.last_error}<p class="error">{overview.last_error}</p>{/if}

{#if overview && !overview.online && !newest}
  <p class="dim">
    Online features are off, so Starlume doesn't read RSI's patch notes. Turn them on in
    <a href="/settings/online">Settings → Online</a>.
  </p>
{:else if overview && !newest && !reading}
  <p class="dim">Not read yet. The first check runs shortly after start, or press “Check now”.</p>
{/if}

{#if newest}
  <article class="card newest">
    <header>
      <span class="channel">{newest.channel ?? "LIVE"}</span>
      <span class="version">{newest.version ?? newest.title}</span>
      {#if newest.name}<span class="name">{newest.name}</span>{/if}
      <span class="grow"></span>
      <span class="dim small">
        {when(newest)}{#if newest.changelist} · build <span class="mono">{newest.changelist}</span>{/if}
      </span>
    </header>
    {#if newest.highlights.length}
      <ul class="highlights">
        {#each newest.highlights as h (h)}<li>{h}</li>{/each}
      </ul>
    {/if}
    <footer>
      {#each newest.build_info as b (b.label)}
        <span class="badge">{b.label}: {b.value}</span>
      {/each}
      {#each counts(newest) as c (c)}<span class="badge">{c}</span>{/each}
      <span class="grow"></span>
      <Button variant="outline" onclick={() => void openPatchNote(newest.article_url)}>
        Read on RSI <ExternalLink size={14} />
      </Button>
    </footer>
  </article>
{/if}

{#if rest.length}
  <table class="data clickable">
    <tbody>
      {#each rest as r (r.key)}
        <tr onclick={() => void openPatchNote(r.article_url)} title="Read on RSI">
          <td class="mono nowrap">{r.version ?? "—"}</td>
          <td class="nowrap">{r.name ?? ""}</td>
          <td class="dim">{r.highlights.slice(0, 3).join(" · ")}</td>
          <td class="dim nowrap">{counts(r).join(" · ")}</td>
          <td class="dim nowrap">{when(r)}</td>
        </tr>
      {/each}
    </tbody>
  </table>
{/if}

{#if overview?.older.length}
  <h3>Older</h3>
  <div class="older">
    {#each overview.older as a (a.url)}
      <button class="link" onclick={() => void openPatchNote(a.url)}>
        {a.version ?? a.title} <span class="dim">· {a.age_text}</span>
      </button>
    {/each}
  </div>
{/if}

<style>
  .bar {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
    margin-bottom: 8px;
  }
  .small {
    font-size: var(--text-small);
  }
  .newest {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-bottom: 12px;
  }
  .newest header,
  .newest footer {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 10px;
  }
  .newest footer {
    align-items: center;
  }
  .grow {
    flex: 1;
  }
  .channel {
    font-family: var(--font-display);
    font-weight: var(--weight-bold);
    color: var(--accent);
  }
  .version {
    font-family: var(--font-mono);
    font-size: 1.25rem;
    color: var(--text-strong);
  }
  .name {
    font-weight: var(--weight-semibold);
  }
  .highlights {
    margin: 0;
    padding-left: 18px;
    list-style: disc;
  }
  .nowrap {
    white-space: nowrap;
  }
  h3 {
    margin: 16px 0 6px;
    font-family: var(--font-display);
    font-size: var(--text-small);
    color: var(--text-faint);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }
  .older {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 16px;
  }
  .link {
    font-size: var(--text-small);
  }
  .link:hover {
    color: var(--accent);
  }
</style>
