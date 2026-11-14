<script lang="ts">
  // Past sessions on the kit surface: one flat row per finished session,
  // newest first; a row opens that session on the session page (`?s=<id>`).
  // Reads the session store only (frontend rules 1, 2).
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { Surface, Expand, createBrowseState, type Status } from "@veelume/ui";
  import type { SessionSummary } from "$lib/bindings";
  import { sessionStore, watchSession } from "$lib/state/session.svelte";
  import { dayTime, signedAuec, summaryLine } from "$lib/modules/session/model";

  type HistoryRow = SessionSummary & { key: string; title: string };

  const browse = createBrowseState({
    q: { kind: "text" },
    sort: { kind: "one", default: "newest", narrows: false },
  });

  const descriptor = {
    sources: () => sessionStore.history ?? [],
    derive: (sessions: SessionSummary[]): HistoryRow[] =>
      sessions.map((s) => ({ ...s, key: s.id, title: dayTime(s.started_at) })),
    searchIn: (r: HistoryRow) => [r.title, r.channel, r.branch, r.build, ...r.shards],
    sorts: [
      {
        value: "newest",
        label: "Newest first",
        compare: (a: HistoryRow, b: HistoryRow) => b.started_at - a.started_at,
      },
      {
        value: "net",
        label: "Net change",
        compare: (a: HistoryRow, b: HistoryRow) => b.net - a.net,
      },
    ],
  };

  const status = $derived<Status>(sessionStore.history === null ? "loading" : "ready");

  onMount(() => watchSession());
</script>

<div class="flex h-full min-h-0 flex-col gap-3">
  <header class="head-line">
    <h1>Past sessions</h1>
    <a class="back" href="/session">Current session</a>
  </header>
  {#if sessionStore.error}
    <p class="error">{sessionStore.error}</p>
  {/if}

  <Surface.Root {descriptor} {browse} class="min-h-0 flex-1">
    <Surface.List {status}>
      {#snippet row(r: HistoryRow)}
        <Expand.Row
          title={r.title}
          subtitle={summaryLine(r)}
          onselect={() => goto(`/session?s=${encodeURIComponent(r.id)}`)}
        >
          {#snippet meta()}
            {#if r.channel}
              <span class="badge">{r.channel}</span>
            {/if}
            {#if !r.sources.telemetry}
              <span class="badge">no telemetry</span>
            {:else if !r.sources.log}
              <span class="badge">no Game.log</span>
            {/if}
          {/snippet}
          {#snippet right()}
            {#if r.opening_balance != null}
              <span class="metric">{signedAuec(r.net)}</span>
            {/if}
          {/snippet}
        </Expand.Row>
      {/snippet}
      {#snippet empty()}
        <p class="dim">No finished session yet. A session is kept when the game closes.</p>
      {/snippet}
    </Surface.List>
  </Surface.Root>
</div>

<style>
  .head-line {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .back {
    margin-left: auto;
  }
</style>
