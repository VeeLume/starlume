<script lang="ts">
  // A post's text. Long bodies clamp to `lines`; expanding one marks the
  // post read (so does opening it on Spectrum).
  import type { DevPostView } from "$lib/bindings";
  import { markRead } from "$lib/state/devtracker.svelte";
  import PostText from "./PostText.svelte";

  let { post, lines = 4 }: { post: DevPostView; lines?: number } = $props();

  let expanded = $state(false);
  const long = $derived(post.body.length > lines * 80 || post.body.split("\n").length > lines);

  function toggle() {
    expanded = !expanded;
    if (expanded && post.unread) void markRead([post.key]);
  }
</script>

{#if long}
  <button
    type="button"
    class="body clamp"
    class:expanded
    style:--lines={lines}
    onclick={toggle}
    title={expanded ? "Collapse" : "Show all"}><PostText text={post.body} /></button
  >
{:else}
  <p class="body"><PostText text={post.body} /></p>
{/if}

<style>
  .body {
    display: block;
    margin: 0;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    text-align: left;
    color: var(--text-body);
  }
  .clamp {
    cursor: pointer;
  }
  .clamp:not(.expanded) {
    display: -webkit-box;
    -webkit-line-clamp: var(--lines);
    line-clamp: var(--lines);
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
</style>
