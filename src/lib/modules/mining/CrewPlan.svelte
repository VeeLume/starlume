<script lang="ts">
  // The crew plan: one column per operator. On top what each brings (the
  // laser and where it sits, modules, power with its share, and R/I/W —
  // their change to the rock's resistance, instability and window); below,
  // what each does at each step. The lead follows the steps; the supports
  // hold one throttle and cut the beam with everyone at step 4.
  import type { MiningCatalogView, OperatorView } from "$lib/bindings";
  import Monogram from "$lib/components/Monogram.svelte";
  import { laserShort, namesOf, type Step } from "./format";

  let {
    catalog,
    operators,
    names,
    places,
    mods,
    hues,
    steps,
    calls,
    me,
    onlead,
  }: {
    catalog: MiningCatalogView;
    operators: OperatorView[];
    names: string[];
    places: string[];
    /** Per operator, the module ids fitted to their laser. */
    mods: string[][];
    hues: string[];
    steps: Step[];
    /** Per operator, one call per step (short wording). */
    calls: string[][];
    /** The operator picked in "for"; -1: none. */
    me: number;
    onlead: (index: number) => void;
  } = $props();

  const catalogNames = $derived(namesOf(catalog));
  const name = (id: string) => catalogNames.get(id) ?? "Unknown";
  const total = $derived(operators.reduce((a, o) => a + o.effect.power, 0));
  const rel = (v: number) => {
    const r = Math.round(v * 100);
    return r === 0 ? "—" : `${r > 0 ? "+" : "−"}${Math.abs(r)}`;
  };
  const tone = (v: number, goodWhenUp: boolean) =>
    Math.round(v * 100) === 0 ? "zero" : v > 0 === goodWhenUp ? "good" : "bad";
  const laserOf = (o: OperatorView) => catalog.lasers.find((l) => l.id === o.laser);
  const activeIds = $derived(new Set(catalog.modules.filter((m) => m.active).map((m) => m.id)));
</script>

<div class="plan" style:--n={operators.length}>
  <p class="label">Crew plan</p>
  <div class="r h">
    <span></span>
    {#each operators as o, k (o.index)}
      <span class="oh" class:me={k === me}>
        <Monogram name={names[k]} hue={hues[k]} size={22} />
        <b>{names[k]}</b>
        {#if o.lead}
          <span class="role lead">lead</span>
        {:else}
          <button class="role" onclick={() => onlead(o.index)} title="Make {names[k]} the lead">support</button>
        {/if}
      </span>
    {/each}
  </div>
  <div class="r lo">
    <span class="rl">laser</span>
    {#each operators as o, k (o.index)}
      <span class="cl" class:me={k === me}
        ><b>{laserShort(laserOf(o)?.name ?? "Unknown laser")}</b>{#if places[k]}<span class="faint">{places[k]}</span
          >{/if}</span
      >
    {/each}
  </div>
  <div class="r lo">
    <span class="rl">modules</span>
    {#each operators as o, k (o.index)}
      <span class="cl tags" class:me={k === me}>
        {#each mods[k] ?? [] as m, j (j)}<span class="tag" class:act={activeIds.has(m)}>{name(m)}</span>{/each}
      </span>
    {/each}
  </div>
  <div class="r lo">
    <span class="rl">power</span>
    {#each operators as o, k (o.index)}
      <span class="cl" class:me={k === me}>
        <span class="pw">{Math.round(o.effect.power).toLocaleString("en-US")}</span>
        <span class="share"><i style:width="{total > 0 ? (o.effect.power / total) * 100 : 0}%"></i></span>
      </span>
    {/each}
  </div>
  <div class="r lo last">
    <span class="rl">on the rock</span>
    {#each operators as o, k (o.index)}
      <span class="cl fx" class:me={k === me}>
        <span class={tone(o.effect.resistance, false)} title="resistance"><small>R</small>{rel(o.effect.resistance)}</span>
        <span class={tone(o.effect.instability, false)} title="instability"
          ><small>I</small>{rel(o.effect.instability)}</span
        >
        <span class={tone(o.effect.window, true)} title="window"><small>W</small>{rel(o.effect.window)}</span>
      </span>
    {/each}
  </div>
  {#each steps as s, i (i)}
    <div class="r">
      <span class="sn s-{s.tone}">{i + 1}</span>
      {#if s.tone === "bad"}
        <span class="c t-bad all">It will overcharge</span>
      {:else}
        {#each operators as o, k (o.index)}
          <span class="c t-{s.tone}" class:me={k === me}>{calls[k]?.[i] ?? ""}</span>
        {/each}
      {/if}
    </div>
  {/each}
</div>

<style>
  .plan {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .label {
    margin: 0 0 6px;
  }
  .r {
    display: grid;
    grid-template-columns: 74px repeat(var(--n), minmax(0, 1fr));
    gap: 0 10px;
    align-items: center;
    padding: 7px 0;
    border-top: 1px solid color-mix(in srgb, var(--text) 8%, transparent);
  }
  .r.h {
    border-top: 0;
    padding-top: 0;
  }
  .lo {
    padding: 5px 0;
    align-items: start;
  }
  .lo.last {
    padding-bottom: 10px;
    border-bottom: 1px solid color-mix(in srgb, var(--text) 14%, transparent);
  }
  .lo + .r:not(.lo) {
    border-top: 0;
  }
  .rl {
    font-size: 0.6rem;
    color: var(--text-faint);
    text-transform: var(--label-transform);
    letter-spacing: 0.08em;
  }
  .oh {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    padding: 4px 6px;
    border-radius: 6px;
  }
  .oh b {
    font-size: 0.86rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .oh.me,
  .c.me,
  .cl.me {
    background: var(--accent-fill-weak);
  }
  .role {
    font-family: var(--font-mono);
    font-size: 0.58rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    padding: 0 5px;
    border-radius: var(--radius-pill);
    border: 1px solid var(--border);
    background: none;
    color: var(--text-dim);
    white-space: nowrap;
  }
  button.role {
    cursor: pointer;
  }
  button.role:hover {
    border-color: var(--accent-border);
    color: var(--text);
  }
  .role.lead {
    color: color-mix(in srgb, var(--good) 85%, var(--text));
    border-color: color-mix(in srgb, var(--good) 40%, transparent);
  }
  .cl {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    padding: 2px 6px;
    border-radius: 6px;
    font-size: 0.84rem;
  }
  .cl b {
    font-weight: var(--weight-semibold);
  }
  .cl .faint {
    font-size: 0.7rem;
    color: var(--text-faint);
  }
  .cl.tags {
    flex-direction: row;
    flex-wrap: wrap;
    gap: 3px;
  }
  .cl.tags .tag {
    font-size: 0.72rem;
  }
  .tag.act {
    box-shadow: inset 0 0 0 1px var(--accent-border);
  }
  .pw {
    font-family: var(--font-mono);
    font-size: 0.82rem;
  }
  .share {
    display: block;
    height: 3px;
    border-radius: 2px;
    background: color-mix(in srgb, var(--text) 8%, transparent);
    overflow: hidden;
  }
  .share i {
    display: block;
    height: 100%;
    background: color-mix(in srgb, var(--hue-sky) 70%, transparent);
  }
  .fx {
    flex-direction: row;
    flex-wrap: wrap;
    gap: 2px 10px;
    font-family: var(--font-mono);
    font-size: 0.78rem;
  }
  .fx small {
    font-size: 0.58rem;
    color: var(--text-faint);
    margin-right: 3px;
  }
  .good {
    color: var(--good);
  }
  .bad {
    color: var(--warn);
  }
  .zero {
    color: var(--text-faint);
  }
  .sn {
    width: 22px;
    height: 22px;
    border-radius: 4px;
    display: grid;
    place-items: center;
    font: var(--weight-bold) 0.7rem var(--font-mono);
    color: var(--text);
  }
  .s-climb {
    background: color-mix(in srgb, var(--text) 12%, transparent);
  }
  .s-good {
    background: color-mix(in srgb, var(--good) 70%, transparent);
  }
  .s-warn1 {
    background: color-mix(in srgb, var(--bad) 32%, transparent);
  }
  .s-warn2 {
    background: color-mix(in srgb, var(--bad) 50%, transparent);
  }
  .s-bad {
    background: color-mix(in srgb, var(--bad) 70%, transparent);
  }
  .c {
    font-size: 0.86rem;
    font-weight: var(--weight-semibold);
    padding: 4px 6px;
    border-radius: 6px;
    min-width: 0;
  }
  .t-good {
    color: var(--good);
  }
  .t-warn1,
  .t-warn2 {
    color: var(--warn);
  }
  .t-bad {
    color: var(--bad);
  }
  .all {
    grid-column: 2 / -1;
  }
</style>
