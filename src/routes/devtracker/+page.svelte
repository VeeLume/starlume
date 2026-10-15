<script lang="ts">
  // Dev Tracker — CIG staff posts from Spectrum, chat included, filtered by
  // content. Three views: the feed, the patch notes (Patch Notes forum
  // threads, from the same feed), and the filter settings (notifications,
  // follow list) — the module's settings live on its page, not in Settings.
  // Renders the cached feed at once; the poll runs Rust-side.
  import { onDestroy, onMount } from "svelte";
  import { Button, Switch } from "@veelume/ui";
  import { RefreshCw, X } from "lucide-svelte";
  import {
    devtrackerStore,
    markAllRead,
    refreshDevtracker,
    setNotifyOps,
    watchDevtracker,
  } from "$lib/state/devtracker.svelte";
  import { activityFor, activityLabel } from "$lib/state/activities.svelte";
  import GateNotice from "$lib/modules/devtracker/GateNotice.svelte";
  import PostItem from "$lib/modules/devtracker/PostItem.svelte";
  import FollowList from "$lib/modules/devtracker/FollowList.svelte";
  import PatchNotes from "$lib/modules/devtracker/PatchNotes.svelte";
  import { dayLabel, postAge } from "$lib/modules/devtracker/time";

  type Tab = "feed" | "patches" | "filters";
  let tab = $state<Tab>("feed");
  let showFiltered = $state(false);
  // One account's posts (from the follow list), filtered-out ones included.
  let author = $state<{ id: string; nickname: string } | null>(null);

  const overview = $derived(devtrackerStore.overview);
  const gateOpen = $derived(overview?.gate.state === "open");
  const posts = $derived(
    (overview?.posts ?? []).filter((p) =>
      author ? p.author_id === author.id : showFiltered || p.shown,
    ),
  );

  function viewAuthor(id: string, nickname: string) {
    author = { id, nickname };
    tab = "feed";
  }
  const hiddenCount = $derived((overview?.posts ?? []).filter((p) => !p.shown).length);
  const filling = $derived(activityFor("devtracker", ""));

  // Day groups, newest first (posts arrive sorted).
  const days = $derived.by(() => {
    const out: { label: string; posts: typeof posts }[] = [];
    for (const p of posts) {
      const label = dayLabel(p.time_ms);
      if (out.at(-1)?.label !== label) out.push({ label, posts: [] });
      out.at(-1)!.posts.push(p);
    }
    return out;
  });

  let stop: (() => void) | undefined;
  onMount(() => (stop = watchDevtracker()));
  onDestroy(() => stop?.());
</script>

<div class="page">
  <header class="bar">
    <div class="tabs">
      <button class="tab" class:active={tab === "feed"} onclick={() => (tab = "feed")}>
        Feed
        {#if overview?.unread}<span class="count-badge owned">{overview.unread}</span>{/if}
      </button>
      <button class="tab" class:active={tab === "patches"} onclick={() => (tab = "patches")}>
        Patch notes
      </button>
      <button class="tab" class:active={tab === "filters"} onclick={() => (tab = "filters")}>
        Filters &amp; follow list
      </button>
    </div>
    <span class="grow"></span>
    {#if tab === "feed" && gateOpen}
      <span class="dim status">
        {#if filling}
          ⟳ {activityLabel(filling)}
        {:else if overview?.throttled_until}
          Spectrum is rate-limiting; paused for a moment
        {:else if overview?.fetched_at}
          Updated {postAge(overview.fetched_at * 1000)}
        {/if}
      </span>
      <Button variant="ghost" disabled={!overview?.unread} onclick={() => void markAllRead()}>
        Mark all read
      </Button>
      <Button
        variant="outline"
        disabled={devtrackerStore.refreshing}
        title="Check for new posts"
        onclick={() => void refreshDevtracker()}
      >
        <RefreshCw size={14} /> Refresh
      </Button>
    {/if}
  </header>

  {#if devtrackerStore.error}<p class="error">{devtrackerStore.error}</p>{/if}
  {#if overview?.last_error && tab === "feed"}
    <p class="error">The last check failed: {overview.last_error}</p>
  {/if}

  {#if !overview}
    <!-- First read of the local cache: a moment, no spinner. -->
  {:else if tab === "filters"}
    <section class="card settings">
      <div class="row">
        <div>
          <div>Notify about ops posts</div>
          <div class="dim">
            Maintenance, PTU waves, hotfixes, known issues: a notification when a new post carries
            one, even while Starlume sits in the tray.
          </div>
        </div>
        <Switch
          label="Notify about ops posts"
          checked={overview.notify_ops}
          onchange={(v) => void setNotifyOps(v)}
        />
      </div>
      <p class="dim filter-note">
        The content filter keeps every forum post, and the staff chat that carries game news: patch
        and PTU talk, versions and builds, maintenance, known issues, “the team is investigating”.
        It drops greetings, moderation and channel policing. It gets about nine in ten right; “Show
        filtered-out” on the feed shows what it dropped.
      </p>
    </section>
    <h2>Follow list</h2>
    <FollowList onview={viewAuthor} />
  {:else if tab === "patches"}
    <PatchNotes notes={overview.patch_notes} />
  {:else if overview.gate.state === "closed"}
    <GateNotice reason={overview.gate.reason} />
  {:else}
    <div class="feed-tools">
      {#if author}
        <span class="chip">
          All posts by <strong>{author.nickname}</strong>, filtered-out included
          <button title="Back to the feed" onclick={() => (author = null)}><X size={13} /></button>
        </span>
        <span class="grow"></span>
      {/if}
      <label class="toggle" class:hidden={author}>
        <Switch
          label="Show filtered-out"
          checked={showFiltered}
          onchange={(v) => (showFiltered = v)}
        />
        Show filtered-out <span class="dim">({hiddenCount})</span>
      </label>
    </div>
    {#if posts.length === 0}
      <p class="dim">
        {#if overview.fetched_at}
          No staff posts in the last 30 days{showFiltered ? "" : " made it through the filter"}.
        {:else if !filling}
          Nothing read yet. The first check runs shortly, or press Refresh.
        {/if}
      </p>
    {/if}
    {#each days as day (day.label)}
      <h3 class="day">{day.label}</h3>
      {#each day.posts as post (post.key)}
        <PostItem {post} />
      {/each}
    {/each}
  {/if}
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .grow {
    flex: 1;
  }
  .status {
    font-size: var(--text-small);
  }
  .feed-tools {
    display: flex;
    align-items: center;
    justify-content: flex-end;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 2px 4px 2px 10px;
    border: 1px solid var(--accent-border);
    border-radius: var(--radius-pill);
    background: var(--accent-fill-faint);
    font-size: var(--text-small);
  }
  .chip button {
    display: inline-flex;
    color: var(--text-dim);
  }
  .chip button:hover {
    color: var(--accent);
  }
  .hidden {
    display: none;
  }
  .toggle {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: var(--text-small);
  }
  .day {
    margin: 14px 0 0;
    font-family: var(--font-display);
    font-size: var(--text-small);
    color: var(--text-faint);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }
  .settings .row {
    display: flex;
    align-items: center;
    gap: 16px;
    justify-content: space-between;
  }
  .filter-note {
    margin: 12px 0 0;
    max-width: 75ch;
  }
  h2 {
    font-size: 16px;
    margin: 12px 0 0;
  }
</style>
