<script lang="ts">
  // One forum post as a card: a new thread (sand tint, the subject as the
  // headline, the opening text clamped) or a reply (the quote of what it
  // answers above the answer). Chat goes through ChatRun instead. Unread
  // posts carry a dot; filtered-out ones (with the page toggle) render
  // dimmed with the filter's reason.
  import { ExternalLink } from "lucide-svelte";
  import type { DevPostView } from "$lib/bindings";
  import { openPost } from "$lib/state/devtracker.svelte";
  import Monogram from "./Monogram.svelte";
  import PostBody from "./PostBody.svelte";
  import PostQuote from "./PostQuote.svelte";
  import { clock, postAge } from "./time";

  let { post }: { post: DevPostView } = $props();
  const thread = $derived(post.kind === "thread");
</script>

<article class="post" class:thread class:filtered={!post.shown}>
  <header>
    <Monogram name={post.author} />
    <div class="who">
      <span class="author">
        {#if post.unread}<span class="dot" title="Unread"></span>{/if}{post.author}
      </span>
      <span class="where">
        {#if thread}
          new thread{#if post.place}&nbsp;in <em>{post.place}</em>{/if}
        {:else}
          replied in <em>{post.subject ?? post.place ?? "a thread"}</em>
        {/if}
      </span>
    </div>
    <div class="right">
      {#if post.ops}<span class="badge accent" title="Game-ops signal">ops</span>{/if}
      {#if !post.shown}<span class="badge" title="Why it is filtered out">{post.reason}</span>{/if}
      <span class="hue-chip" style:--hue={thread ? "var(--hue-thread)" : "var(--hue-reply)"}>
        {thread ? "Thread" : "Reply"}
      </span>
      <time datetime={new Date(post.time_ms).toISOString()} title={postAge(post.time_ms)}>{clock(post.time_ms)}</time>
      <button type="button" class="open" title="Open on Spectrum" onclick={() => void openPost(post.key)}>
        <ExternalLink size={14} />
      </button>
    </div>
  </header>
  {#if thread && post.subject}<h3>{post.subject}</h3>{/if}
  {#if post.quote}<PostQuote quote={post.quote} />{/if}
  <PostBody {post} lines={thread ? 5 : 6} />
</article>

<style>
  .post {
    --hue: var(--hue-reply);
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px 14px;
    border-radius: var(--radius-lg);
    background: var(--bg-raised);
    border: 1px solid var(--border);
  }
  .post.thread {
    --hue: var(--hue-thread);
    background: linear-gradient(
      160deg,
      color-mix(in srgb, var(--hue) 9%, var(--bg-raised)),
      var(--bg-raised) 50%
    );
    border-color: color-mix(in srgb, var(--hue) 25%, var(--border));
  }
  .post.filtered {
    opacity: 0.55;
  }
  header {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .who {
    display: flex;
    flex-direction: column;
    min-width: 0;
    line-height: 1.3;
  }
  .author {
    display: flex;
    align-items: center;
    gap: 6px;
    font-weight: var(--weight-semibold);
    color: var(--text-strong);
  }
  .dot {
    width: 6px;
    height: 6px;
    flex: none;
    border-radius: var(--radius-pill);
    background: var(--accent);
  }
  .where {
    font-size: var(--text-small);
    color: var(--text-faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .where em {
    font-style: normal;
    color: var(--text-dim);
  }
  .right {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 8px;
    flex: none;
  }
  time {
    font-size: var(--text-small);
    color: var(--text-faint);
    font-variant-numeric: tabular-nums;
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
  h3 {
    margin: 0;
    font-family: var(--font-body);
    font-size: 1.1rem;
    line-height: 1.3;
    color: var(--text-strong);
    text-wrap: balance;
  }
</style>
