<script lang="ts" module>
  // A low-poly mark for a plan or a step, tinted in its `--hue` (set on an
  // ancestor): decoration that tells kinds apart at a glance, not data.
  // `large` is the same idea as the hero card's art.
  export type GlyphShape = "ship" | "blueprint" | "mine" | "craft" | "crate" | "mission" | "find";

  const SHAPES: Record<GlyphShape, [string, string, string]> = {
    ship: ["4,26 14,16 30,14 37,20 32,27 12,29", "14,16 24,10 30,14 22,20", "4,26 12,29 10,34 2,30"],
    blueprint: ["8,6 32,6 32,34 8,34", "8,6 32,6 20,18", "8,34 20,18 32,34"],
    mine: ["20,3 34,13 30,33 10,33 6,13", "20,3 34,13 20,18", "10,33 20,18 30,33"],
    craft: ["8,22 22,8 32,18 18,32", "22,8 32,18 26,12", "8,22 18,32 12,30"],
    crate: ["6,12 34,12 34,34 6,34", "6,12 20,4 34,12 20,18", "6,34 20,22 34,34"],
    mission: ["20,4 34,20 20,36 6,20", "20,4 34,20 20,20", "6,20 20,20 20,36"],
    find: ["6,14 34,14 34,32 6,32", "6,14 20,6 34,14", "6,32 20,22 34,32"],
  };
</script>

<script lang="ts">
  let { shape, large = false }: { shape: GlyphShape; large?: boolean } = $props();
  const points = $derived(SHAPES[shape]);
</script>

<svg class:large viewBox="0 0 40 40" aria-hidden="true">
  <polygon class="f1" points={points[0]} />
  <polygon class="f2" points={points[1]} />
  <polygon class="f4" points={points[2]} />
</svg>

<style>
  svg {
    display: block;
    width: 40px;
    height: 40px;
  }
  svg.large {
    width: 200px;
    height: 200px;
    opacity: 0.55;
  }
  polygon {
    stroke: color-mix(in srgb, var(--hue) 45%, transparent);
    stroke-width: 1;
    stroke-linejoin: round;
    vector-effect: non-scaling-stroke;
  }
  .f1 {
    fill: color-mix(in srgb, var(--hue) 14%, var(--bg-raised));
  }
  .f2 {
    fill: color-mix(in srgb, var(--hue) 24%, var(--bg-raised));
  }
  .f4 {
    fill: color-mix(in srgb, var(--hue) 18%, var(--bg-raised));
  }
</style>
