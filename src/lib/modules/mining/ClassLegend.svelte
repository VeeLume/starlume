<script lang="ts">
  // The classes' colours, in the order a comfort zone stacks them.
  import { CLASSES } from "./format";

  const order = ["easy", "manageable", "hard", "tooMuchPower", "impossible"];
  const shown = $derived(order.map((id) => CLASSES.find((c) => c.id === id)!));
</script>

<div class="lg">
  {#each shown as c (c.id)}<span><i style:--hue={c.hue}></i>{c.label}</span>{/each}
</div>

<style>
  .lg {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 14px;
    font-size: 0.72rem;
    color: var(--text-dim);
  }
  span {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  i {
    width: 10px;
    height: 10px;
    border-radius: 2px;
    background: color-mix(in srgb, var(--hue) 75%, transparent);
  }
</style>
