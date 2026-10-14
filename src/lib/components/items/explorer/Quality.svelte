<script lang="ts">
  // A quality chip: accent from the stock line (Q500) up.
  import { STOCK_QUALITY } from "../model/resources";
  import Tag from "./Tag.svelte";

  let { q, prefix = "Q", title = "Quality (0–1000; 500 = stock)" }: { q: number; prefix?: string; title?: string } =
    $props();
</script>

<Tag label={`${prefix} ${q}`} tone={q >= STOCK_QUALITY ? "primary" : "neutral"} {title} />
