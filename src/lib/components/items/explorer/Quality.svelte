<script lang="ts">
  // A quality chip: the quality hue from the stock line (Q500) up, neutral
  // below. Data, not a control — so not violet (design pass).
  import { STOCK_QUALITY } from "../model/resources";

  let { q, prefix = "Q", title = "Quality (0–1000; 500 = stock)" }: { q: number; prefix?: string; title?: string } =
    $props();
</script>

<span class="hue-chip q" class:hi={q >= STOCK_QUALITY} {title}>{prefix} {q}</span>

<style>
  .q {
    font-family: var(--font-mono);
  }
  .q.hi {
    --hue: var(--hue-quality);
  }
</style>
