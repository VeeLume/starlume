<script lang="ts">
  // One staff post. Long bodies clamp; expanding one marks it read, and so
  // does opening it on Spectrum. Filtered-out posts (with the page toggle)
  // render dimmed with the filter's reason.
  import { ExternalLink } from "lucide-svelte";
  import type { DevPostView } from "$lib/bindings";
  import { markRead, openPost } from "$lib/state/devtracker.svelte";
  import { postAge } from "./time";

  let { post }: { post: DevPostView } = $props();

  let expanded = $state(false);
  const long = $derived(post.body.length > 320 || post.body.split("\n").length > 4);

  function toggle() {
    expanded = !expanded;
    if (expanded && post.unread) void markRead([post.key]);
  }
</script>

<article class="post" class:filtered={!post.shown} class:unread={post.unread}>
  <header>
    <span class="dot" aria-hidden="true"></span>
    <span class="author">{post.author}</span>
    {#if post.place}<span class="dim">in {post.place}</span>{/if}
    {#if post.ops}<span class="badge accent" title="Game-ops signal">ops</span>{/if}
    {#if !post.shown}<span class="badge" title="Why it is filtered out">{post.reason}</span>{/if}
    <span class="grow"></span>
    <time class="dim" datetime={new Date(post.time_ms).toISOString()}>{postAge(post.time_ms)}</time>
    <button class="open" title="Open on Spectrum" onclick={() => void openPost(post.key)}>
      <ExternalLink size={14} />
    </button>
  </header>
  {#if post.subject}<div class="subject">{post.subject}</div>{/if}
  {#if long}
    <button class="body clamp" class:expanded onclick={toggle} title={expanded ? "Collapse" : "Show all"}
      >{post.body}</button
    >
  {:else}
    <p class="body">{post.body}</p>
  {/if}
</article>

<style>
  .post {
    padding: 10px 0 12px;
    border-bottom: 1px solid var(--border-soft);
  }
  .post.filtered {
    opacity: 0.55;
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--text-small);
    min-width: 0;
  }
  .dot {
    width: 6px;
    height: 6px;
    flex-shrink: 0;
    border-radius: var(--radius-pill);
    background: transparent;
  }
  .unread .dot {
    background: var(--accent);
  }
  .author {
    font-weight: var(--weight-semibold);
    color: var(--text-strong);
  }
  .grow {
    flex: 1;
  }
  .open {
    display: inline-flex;
    color: var(--text-dim);
    padding: 2px;
    border-radius: var(--radius-chip);
  }
  .open:hover {
    color: var(--accent);
  }
  .subject {
    margin: 4px 0 0 14px;
    font-weight: var(--weight-medium);
  }
  .body {
    display: block;
    margin: 4px 0 0 14px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    text-align: left;
    color: var(--text-body);
  }
  .clamp:not(.expanded) {
    display: -webkit-box;
    -webkit-line-clamp: 4;
    line-clamp: 4;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .clamp {
    cursor: pointer;
  }
</style>
