<script lang="ts">
  // A person's initials in a circle tinted from their name — the dev
  // tracker's avatar (at no fetch: Spectrum avatars would be lab work
  // first) and a mining crew mate's.
  import { initials, nameHue } from "$lib/monogram";

  let { name, size = 32, hue = undefined }: { name: string; size?: number; hue?: string } = $props();
  // A missing name must not throw: an error in a render aborts the whole
  // update, and the page's effects (the next solve) never run.
  const safe = $derived(name ?? "");
</script>

<span class="monogram" style:--hue={hue ?? nameHue(safe)} style:--size="{size}px" aria-hidden="true">{initials(safe)}</span>

<style>
  .monogram {
    flex: none;
    display: grid;
    place-items: center;
    width: var(--size);
    height: var(--size);
    border-radius: 50%;
    font-family: var(--font-display);
    font-weight: var(--weight-bold);
    font-size: calc(var(--size) * 0.42);
    color: color-mix(in srgb, var(--hue) 80%, var(--text));
    background: color-mix(in srgb, var(--hue) 20%, var(--bg-raised));
    border: 1px solid color-mix(in srgb, var(--hue) 40%, transparent);
  }
</style>
