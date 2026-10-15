<script lang="ts">
  // A run of chat messages (feed.ts `groupFeed`): one developer, one lobby,
  // close together — one header, then each message on its own line with its
  // time, oldest first. Flat, no card: chat stays light next to the forum
  // cards. A message's quote (the mentioned member's last message) sits
  // above it; unread messages carry a dot.
  import { ExternalLink } from "lucide-svelte";
  import { openPost } from "$lib/state/devtracker.svelte";
  import type { ChatRun } from "./feed";
  import Monogram from "./Monogram.svelte";
  import PostBody from "./PostBody.svelte";
  import PostQuote from "./PostQuote.svelte";
  import { clock, postAge } from "./time";

  let { run }: { run: ChatRun } = $props();
  const first = $derived(run.messages[0]);
  const last = $derived(run.messages[run.messages.length - 1]);
</script>

<div class="run">
  <Monogram name={run.author} />
  <div class="head">
    <span class="author">{run.author}</span>
    {#if run.place}<span class="hue-chip" style:--hue="var(--hue-chat)"># {run.place}</span>{/if}
    <span class="span" title={postAge(last.time_ms)}>
      {clock(first.time_ms)}{#if run.messages.length > 1}–{clock(last.time_ms)}{/if}
    </span>
  </div>
  <div class="messages">
    {#each run.messages as m (m.key)}
      <div class="message" class:filtered={!m.shown}>
        <span class="dot" class:unread={m.unread} title={m.unread ? "Unread" : undefined}></span>
        <div class="content">
          {#if m.quote}<PostQuote quote={m.quote} />{/if}
          <PostBody post={m} />
        </div>
        <div class="meta">
          {#if m.ops}<span class="badge accent" title="Game-ops signal">ops</span>{/if}
          {#if !m.shown}<span class="badge" title="Why it is filtered out">{m.reason}</span>{/if}
          <time datetime={new Date(m.time_ms).toISOString()}>{clock(m.time_ms)}</time>
          <button type="button" class="open" title="Open on Spectrum" onclick={() => void openPost(m.key)}>
            <ExternalLink size={13} />
          </button>
        </div>
      </div>
    {/each}
  </div>
</div>

<style>
  .run {
    display: grid;
    grid-template-columns: 32px minmax(0, 1fr);
    gap: 4px 12px;
    padding: 6px 0;
  }
  .head {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
    min-height: 32px;
  }
  .author {
    font-weight: var(--weight-semibold);
    color: var(--text-strong);
  }
  .span {
    font-size: var(--text-small);
    color: var(--text-faint);
    font-variant-numeric: tabular-nums;
  }
  .messages {
    grid-column: 2;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .message {
    display: grid;
    grid-template-columns: 6px minmax(0, 1fr) auto;
    align-items: start;
    gap: 10px;
    margin-left: -16px;
    padding: 3px 6px;
    border-radius: var(--radius);
  }
  .message:hover {
    background: color-mix(in srgb, var(--text) 4%, transparent);
  }
  .message.filtered {
    opacity: 0.55;
  }
  .dot {
    width: 6px;
    height: 6px;
    margin-top: 0.55em;
    border-radius: var(--radius-pill);
  }
  .dot.unread {
    background: var(--accent);
  }
  .content {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .meta {
    display: flex;
    align-items: center;
    gap: 6px;
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
    opacity: 0;
  }
  .message:hover .open,
  .open:focus-visible {
    opacity: 1;
  }
  .open:hover {
    color: var(--accent);
  }
</style>
