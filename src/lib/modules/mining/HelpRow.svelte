<script lang="ts">
  // One change that would do better: what it is, the same outcome bar as
  // the outcome card, cracks and time, and the button that applies it.
  import Monogram from "$lib/components/Monogram.svelte";
  import type { Alternative } from "./alternatives";
  import { secs } from "./format";
  import OutcomeBar from "./OutcomeBar.svelte";

  let { alt, onapply }: { alt: Alternative; onapply?: () => void } = $props();

  const d = $derived(alt.eval.difficulty);
  const glyph = $derived(alt.group === "Gadgets" ? "◆" : alt.group === "Modules" ? "▤" : "+");
</script>

<div class="row">
  {#if alt.who}
    <span class="ico"><Monogram name={alt.who} size={32} /></span>
  {:else}
    <span class="ico glyph" aria-hidden="true">{glyph}</span>
  {/if}
  <div class="nm"><b>{alt.name}</b><span>{alt.kind}</span></div>
  <button
    class="btn"
    onclick={() => {
      void alt.apply();
      onapply?.();
    }}>{alt.verb}</button
  >
  <div class="res">
    <OutcomeBar {d} height={8} />
    {#if d}
      <span class="n"><b>{Math.round(d.controlled * 100)}</b> crack</span>
      <span class="t">{secs(d.medianTimeS)}</span>
    {:else}
      <span class="n">can't break</span><span class="t"></span>
    {/if}
  </div>
</div>

<style>
  .row {
    display: grid;
    grid-template-columns: 32px minmax(0, 1fr) auto;
    grid-template-areas: "ico nm btn" "ico res res";
    gap: 6px 10px;
    align-items: center;
    padding: 10px 0;
    border-top: 1px solid var(--border);
  }
  .row:first-child {
    border-top: 0;
    padding-top: 2px;
  }
  .ico {
    grid-area: ico;
    align-self: start;
  }
  .glyph {
    width: 32px;
    height: 32px;
    border-radius: 8px;
    display: grid;
    place-items: center;
    font-size: 0.8rem;
    color: color-mix(in srgb, var(--hue-sand) 85%, var(--text));
    background: color-mix(in srgb, var(--hue-sand) 14%, var(--bg));
    border: 1px solid color-mix(in srgb, var(--hue-sand) 30%, transparent);
  }
  .nm {
    grid-area: nm;
    display: flex;
    flex-direction: column;
    line-height: 1.3;
    min-width: 0;
  }
  .nm b {
    font-size: 0.92rem;
  }
  .nm span {
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .btn {
    grid-area: btn;
    padding: 3px 12px;
  }
  .res {
    grid-area: res;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto 44px;
    gap: 10px;
    align-items: center;
  }
  .n {
    font-size: var(--text-small);
    color: var(--text-dim);
    white-space: nowrap;
  }
  .n b {
    color: var(--good);
    font-family: var(--font-mono);
  }
  .t {
    font-family: var(--font-mono);
    font-size: var(--text-small);
    color: var(--text-dim);
    text-align: right;
  }
</style>
