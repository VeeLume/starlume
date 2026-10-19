<script lang="ts">
  // One crafted stat delta: "Impact Force +14.8%", green when it is the
  // better roll, red when worse, plain when nobody knows which way helps.
  import type { StatDelta } from "../model/crafted";

  let { d }: { d: StatDelta } = $props();
</script>

<span class="delta" class:good={d.good > 0} class:bad={d.good < 0}>{d.name} {d.pct > 0 ? "+" : d.pct < 0 ? "−" : ""}{Math.abs(d.pct)}%</span>

<style>
  .delta {
    font-family: var(--font-mono);
    font-size: 0.72rem;
    white-space: nowrap;
    color: var(--text-dim);
  }
  .good {
    color: var(--good);
  }
  .bad {
    color: var(--bad);
  }
</style>
