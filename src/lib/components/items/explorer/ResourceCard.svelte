<script lang="ts">
  // One material in a container: its total, the quality mix, best quality
  // and how much of it is at Q500 or better.
  import { Pickaxe } from "lucide-svelte";
  import { fmtAmount, type Material } from "../model/resources";
  import Card from "./Card.svelte";
  import Quality from "./Quality.svelte";
  import QualityMix from "./QualityMix.svelte";

  let { m, on, onclick }: { m: Material; on: boolean; onclick: () => void } = $props();

  const note = $derived(
    m.good === 0 ? "none at Q500+" : m.good >= m.total - 1e-9 ? "all Q500+" : `${fmtAmount(m.bulk, m.good)} at Q500+`,
  );
</script>

<Card icon={Pickaxe} {on} {onclick}>
  {#snippet name()}{m.name}{/snippet}
  {#snippet right()}{fmtAmount(m.bulk, m.total)}{/snippet}
  <QualityMix list={m.nodes} bulk={m.bulk} height={12} />
  {#if m.best > 0}
    <span class="meta"><Quality q={m.best} /><span class="note">{note}</span></span>
  {/if}
</Card>

<style>
  .meta {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .note {
    font-family: var(--font-mono);
    font-size: 0.66rem;
    color: var(--text-faint);
  }
</style>
