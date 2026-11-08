<script lang="ts">
  // The Ship page's figures as label and value rows: what belongs to the
  // hull (structure, armour, flight, logistics). What changes with parts
  // and pips is on the Build page, in its boxes' header lines and the power
  // panel. All of it is computed Rust-side; this only lays it out.
  import type { ShipPageView } from "$lib/bindings";

  let { page }: { page: ShipPageView } = $props();

  const s = $derived(page.summary);
  const fmt = (n: number | null | undefined, d = 0) =>
    n === null || n === undefined ? "—" : n.toLocaleString("en-US", { maximumFractionDigits: d, minimumFractionDigits: d });
  /** A damage multiplier as the change it makes: 0.7 → −30 %. */
  const delta = (m: number) => {
    const d = Math.round((m - 1) * 100);
    return d === 0 ? "±0 %" : `${d > 0 ? "+" : "−"}${Math.abs(d)} %`;
  };
  const tone = (m: number) => (m < 0.995 ? "good" : m > 1.005 ? "bad" : "");

  const vital = $derived(s.hull.parts.filter((p) => p.vital).reduce((a, p) => a + p.hp, 0));
  const TYPES = [
    ["physical", "Physical"],
    ["energy", "Energy"],
    ["distortion", "Distortion"],
  ] as const;
  const accel = $derived(
    s.flight
      ? ([
          ["Forward", s.flight.acceleration.forward, s.flight.accelerationBoosted.forward],
          ["Retro", s.flight.acceleration.backward, s.flight.accelerationBoosted.backward],
          ["Strafe", Math.max(s.flight.acceleration.left, s.flight.acceleration.right), Math.max(s.flight.accelerationBoosted.left, s.flight.accelerationBoosted.right)],
          ["Up", s.flight.acceleration.up, s.flight.accelerationBoosted.up],
          ["Down", s.flight.acceleration.down, s.flight.accelerationBoosted.down],
        ] as const)
      : [],
  );
</script>

{#snippet head(title: string)}
  <h3 class="label">{title}</h3>
{/snippet}

<div class="figs">
  <section>
    {@render head("Structure")}
    <dl>
      {#if vital > 0}
        <dt title="The ship is lost with its vital parts">Vital hull</dt>
        <dd>{fmt(vital)}<small>HP</small></dd>
      {/if}
      <dt>Hull, all parts</dt>
      <dd>{fmt(s.hullHealth)}<small>HP</small></dd>
      {#if s.hull.dimensions}
        {@const d = s.hull.dimensions}
        <dt>Long, wide, high</dt>
        <dd>{fmt(d[1], 1)} × {fmt(d[0], 1)} × {fmt(d[2], 1)}<small>m</small></dd>
      {/if}
      {#if s.flight}
        <dt>Mass</dt>
        <dd>{fmt(s.flight.mass / 1000)}<small>t</small></dd>
      {/if}
    </dl>
  </section>
  <section>
    {@render head("Armour")}
    <dl>
      {#if s.armor}
        <dt>Armour</dt>
        <dd>{fmt(s.armor.health)}<small>HP</small></dd>
        {#if s.armor.deflection}
          <dt title="A hit at or below this does nothing">Deflects</dt>
          <dd>{fmt(s.armor.deflection.physical)}<small>physical</small> {fmt(s.armor.deflection.energy)}<small>energy</small></dd>
        {/if}
      {/if}
      {#if s.shield}
        <dt>Shield back up</dt>
        <dd>{fmt(s.shield.damagedDelay, 1)}<small>s after a hit</small> {fmt(s.shield.downedDelay, 1)}<small>s when down</small></dd>
      {/if}
    </dl>
    {#if s.armor?.hullDamage || s.armor?.durability}
      {@const a = s.armor}
      <div class="tb" role="table" aria-label="Damage taken">
        <span class="th" role="columnheader">Damage taken</span>
        {#each TYPES as [, label] (label)}<span class="th r" role="columnheader">{label.toLowerCase()}</span>{/each}
        {#if a.hullDamage}
          {@const h = a.hullDamage}
          <span role="cell">Hull</span>
          {#each TYPES as [k] (k)}<span class="v {tone(h[k])}" role="cell">{delta(h[k])}</span>{/each}
        {/if}
        {#if a.durability}
          {@const d = a.durability}
          <span role="cell">Armour</span>
          {#each TYPES as [k] (k)}<span class="v {tone(d[k])}" role="cell">{delta(d[k])}</span>{/each}
        {/if}
      </div>
    {/if}
  </section>
  {#if s.flight}
    {@const f = s.flight}
    <section>
      {@render head("Flight")}
      <dl>
        <dt>SCM, boost forward</dt>
        <dd>{fmt(f.scmSpeed)}<small>m/s</small> {fmt(f.boostForward)}<small>m/s</small></dd>
        <dt>NAV</dt>
        <dd>{fmt(f.navSpeed)}<small>m/s</small></dd>
        <dt>Pitch, yaw, roll</dt>
        <dd>{f.rotation.map((v) => fmt(v)).join(" / ")}<small>°/s</small></dd>
        <dt>Boosted</dt>
        <dd>{f.rotationBoosted.map((v) => fmt(v)).join(" / ")}<small>°/s</small></dd>
      </dl>
      <div class="tb acc" role="table" aria-label="Acceleration">
        <span class="th" role="columnheader">Acceleration</span><span class="th r" role="columnheader">G</span><span class="th r" role="columnheader">boosted</span>
        {#each accel as [label, a, b] (label)}
          <span role="cell">{label}</span><span class="v" role="cell">{fmt(a, 1)}</span><span class="v dim" role="cell">{fmt(b, 1)}</span>
        {/each}
      </div>
    </section>
  {/if}
  <section>
    {@render head("Logistics")}
    <dl>
      {#if s.flight}
        <dt>Hydrogen</dt>
        <dd>{fmt(s.flight.hydrogenCapacity, 1)}<small>SCU</small></dd>
      {/if}
      {#if s.quantum}
        <dt>Quantum fuel</dt>
        <dd>{fmt(s.quantum.fuelCapacity, 1)}<small>SCU</small></dd>
      {/if}
      {#if s.storage}
        <dt>Cargo</dt>
        <dd>{fmt(s.storage.cargo)}<small>SCU</small></dd>
      {/if}
      {#if s.insurance}
        <dt>Insurance claim</dt>
        <dd>{fmt(s.insurance.waitMinutes, 1)}<small>min</small></dd>
        <dt>Expedited</dt>
        <dd>{fmt(s.insurance.mandatoryMinutes, 1)}<small>min for</small> {fmt(s.insurance.expediteFee)}<small>aUEC</small></dd>
      {:else}
        <dt>Insurance claim</dt>
        <dd>—</dd>
      {/if}
    </dl>
  </section>
</div>

<style>
  .figs {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  h3 {
    margin: 0;
    padding-bottom: 4px;
    border-bottom: 1px solid var(--border);
  }
  dl {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 4px 14px;
    margin: 0;
    font-size: var(--text-small);
  }
  dt {
    color: var(--text-dim);
  }
  dd {
    margin: 0;
    text-align: right;
    font-family: var(--font-mono);
    color: var(--text-strong);
  }
  small {
    margin: 0 8px 0 4px;
    font-family: var(--font-sans);
    font-size: 0.7rem;
    color: var(--text-faint);
  }
  small:last-child {
    margin-right: 0;
  }
  .tb {
    display: grid;
    grid-template-columns: minmax(0, 1fr) repeat(3, 72px);
    gap: 3px 10px;
    font-size: var(--text-small);
    color: var(--text-dim);
  }
  .tb.acc {
    grid-template-columns: minmax(0, 1fr) repeat(2, 72px);
  }
  .th {
    font-size: 0.68rem;
    font-weight: 600;
    color: var(--text-faint);
  }
  .r,
  .v {
    text-align: right;
  }
  .v {
    font-family: var(--font-mono);
    color: var(--text-strong);
  }
  .v.dim {
    color: var(--text-dim);
  }
  .good {
    color: var(--good);
  }
  .bad {
    color: var(--bad);
  }
</style>
