<script lang="ts">
  // What a post answers, as a sunk inset above it: the forum reply (or the
  // thread opener) it replies to, or the mentioned member's last chat
  // message. Tinted in the reply / chat hue; clamps to three lines, a click
  // shows all.
  import { AtSign, Reply } from "lucide-svelte";
  import type { QuoteView } from "$lib/bindings";
  import PostText from "./PostText.svelte";

  let { quote }: { quote: QuoteView } = $props();

  let open = $state(false);
  const label = $derived(
    quote.kind === "opener"
      ? `${quote.author}, opening the thread`
      : quote.kind === "mention"
        ? `${quote.author}, before`
        : `${quote.author} wrote`,
  );
  const deleted = $derived(quote.erased || !quote.text);
</script>

<button
  type="button"
  class="quote"
  class:open
  class:mention={quote.kind === "mention"}
  title={open ? "Collapse" : "Show all"}
  onclick={() => (open = !open)}
>
  <span class="who">
    {#if quote.kind === "mention"}<AtSign size={12} />{:else}<Reply size={12} />{/if}
    {label}
  </span>
  <span class="text">{#if deleted}(deleted){:else}<PostText text={quote.text} />{/if}</span>
</button>

<style>
  .quote {
    --hue: var(--hue-reply);
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 100%;
    padding: 6px 10px;
    border-radius: var(--radius);
    text-align: left;
    font-size: var(--text-small);
    color: var(--text-dim);
    background: color-mix(in srgb, var(--hue) 7%, var(--bg-sunk));
    border: 1px solid color-mix(in srgb, var(--hue) 18%, transparent);
  }
  .quote.mention {
    --hue: var(--hue-chat);
  }
  .quote:hover {
    border-color: color-mix(in srgb, var(--hue) 35%, transparent);
  }
  .who {
    display: flex;
    align-items: center;
    gap: 6px;
    font-weight: var(--weight-medium);
    color: color-mix(in srgb, var(--hue) 70%, var(--text));
  }
  .text {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .quote:not(.open) .text {
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
</style>
