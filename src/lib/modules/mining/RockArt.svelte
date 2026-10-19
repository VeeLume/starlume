<!-- A low-poly asteroid in the card's hue: decoration behind the scan and
     place cards, not data. -->
<svg viewBox="0 0 200 160" aria-hidden="true">
  <polygon class="f1" points="40,70 78,32 128,28 168,58 172,104 132,138 74,140 34,112" />
  <polygon class="f2" points="78,32 128,28 112,74 70,78" />
  <polygon class="f3" points="128,28 168,58 150,90 112,74" />
  <polygon class="f2" points="40,70 78,32 70,78" />
  <polygon class="f4" points="70,78 112,74 150,90 132,138 74,140" />
  <polygon class="f3" points="34,112 40,70 70,78 74,140" />
  <polygon class="f1" points="150,90 168,58 172,104 132,138" />
</svg>

<style>
  svg {
    display: block;
    width: 210px;
    height: auto;
    opacity: 0.55;
  }
  polygon {
    stroke: color-mix(in srgb, var(--hue) 45%, transparent);
    stroke-width: 1;
    stroke-linejoin: round;
  }
  .f1 {
    fill: color-mix(in srgb, var(--hue) 14%, var(--bg-raised));
  }
  .f2 {
    fill: color-mix(in srgb, var(--hue) 24%, var(--bg-raised));
  }
  .f3 {
    fill: color-mix(in srgb, var(--hue) 9%, var(--bg-raised));
  }
  .f4 {
    fill: color-mix(in srgb, var(--hue) 18%, var(--bg-raised));
  }
  @media (max-width: 560px) {
    svg {
      width: 140px;
    }
  }
</style>
