<script lang="ts">
  // A release's headline items: one line per group ("Gameplay: A · B · C"),
  // plain titles as their own line.
  import { highlightGroup } from "./format";

  let { highlights }: { highlights: string[] } = $props();
  const groups = $derived(highlights.map(highlightGroup));
</script>

<ul>
  {#each groups as g, i (i)}
    <li>
      {#if g.label}<span class="label">{g.label}:</span>{/if}
      {g.items.join(" · ")}
    </li>
  {/each}
</ul>

<style>
  ul {
    margin: 0;
    padding-left: 18px;
    list-style: disc;
  }
  li + li {
    margin-top: 2px;
  }
  .label {
    color: var(--text-dim);
  }
</style>
