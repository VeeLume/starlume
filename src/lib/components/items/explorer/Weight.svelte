<script lang="ts">
  // An armour weight class: heavy red, medium amber, light green.
  import type { Weight } from "../model/classify";

  let { weight }: { weight: Weight } = $props();
</script>

{#if weight !== "Other"}<span class="wt {weight}">{weight}</span>{/if}

<style>
  .wt {
    font-family: var(--font-mono);
    font-size: 0.58rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    padding: 0 5px;
    border-radius: 3px;
    border: 1px solid var(--border);
    color: var(--text-dim);
  }
  .Heavy {
    color: var(--bad);
    border-color: var(--danger-border);
  }
  .Medium {
    color: var(--warn);
    border-color: color-mix(in srgb, var(--warn) 45%, transparent);
  }
  .Light {
    color: var(--good);
    border-color: color-mix(in srgb, var(--good) 40%, transparent);
  }
</style>
