<script lang="ts">
  // A one-off pill with a computed label ("Q 874", "3 off saved") on the
  // kit's StatusBadge, so it reads exactly like every other status pill.
  import { StatusBadge, type StatusTone } from "@veelume/ui";

  let { label, tone = "neutral", title }: { label: string; tone?: StatusTone; title?: string } = $props();
  const map = $derived({ [label]: { label: () => label, tone } });
</script>

<span {title} class="tag"><StatusBadge status={label} {map} /></span>

<style>
  .tag {
    display: inline-flex;
    font-family: var(--font-mono);
  }
</style>
