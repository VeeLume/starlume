<script lang="ts">
  // Plain post text with @mentions set off — split in script, never {@html}.
  import { mentionParts } from "./feed";

  let { text }: { text: string } = $props();
  const parts = $derived(mentionParts(text));
</script>

{#each parts as p, i (i)}{#if p.mention}<span class="mention">{p.text}</span>{:else}{p.text}{/if}{/each}

<style>
  .mention {
    font-weight: var(--weight-medium);
    color: var(--text-strong);
  }
</style>
