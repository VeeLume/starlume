<script lang="ts">
  // One blueprint's recipe: a quality per material, what the item comes out
  // as, and where the blueprint is from. The sliders are page-local: they
  // explore, they don't record anything.
  import { Button, Segmented } from "@veelume/ui";
  import type { BlueprintDetailView, BlueprintRowView, BlueprintStatView } from "$lib/bindings";
  import { getRecipe } from "$lib/state/catalog.svelte";
  import { craftingStore, toggleMark } from "$lib/state/crafting.svelte";
  import {
    PRESETS,
    classText,
    effectText,
    evalStat,
    fmtNeed,
    fmtPct,
    fmtTime,
    fmtValue,
    heldAt,
    presetQuality,
    reachText,
    readiness,
    shelfOf,
    type Preset,
    type Readiness,
  } from "./model";

  let { row, channel }: { row: BlueprintRowView; channel: string } = $props();

  let detail = $state<BlueprintDetailView | null>(null);
  let error = $state("");
  let preset = $state<Preset | null>("Base");
  let qualities = $state<number[]>([]);

  const sized = $derived(shelfOf(row).sub.startsWith("Size"));
  const owned = $derived(craftingStore.owns(row.guid));
  const inGame = $derived(craftingStore.ownedInGame(row.guid));

  $effect(() => {
    const guid = row.guid;
    void (async () => {
      const r = await getRecipe(channel, guid);
      if (guid !== row.guid) return;
      if (typeof r === "string") {
        error = r;
        detail = null;
      } else {
        error = "";
        detail = r;
        apply("Base");
      }
    })();
  });

  function apply(next: Preset) {
    if (!detail) return;
    const d = detail;
    preset = next;
    qualities = d.slots.map((s) =>
      presetQuality(next, s.material, d.defaultQuality, craftingStore.stock.get(s.material.name)),
    );
  }

  function setQuality(i: number, value: number) {
    qualities[i] = value;
    preset = null;
  }

  const stateOf = (i: number): Readiness => {
    const s = detail!.slots[i];
    return readiness(craftingStore.stock.get(s.material.name), s.material, qualities[i] ?? 0);
  };
  const ready = $derived(detail && qualities.length === detail.slots.length ? detail.slots.filter((_, i) => stateOf(i) === "good").length : 0);

  const HUE: Record<Readiness, string> = { good: "var(--good)", short: "var(--warn)", none: "var(--bad)" };

  /** What you hold for a slot, in a few words. */
  function holding(i: number): string {
    const s = detail!.slots[i];
    const stacks = craftingStore.stock.get(s.material.name);
    if (!stacks?.length) return "none held";
    const best = stacks.reduce((a, b) => (b.quality > a.quality ? b : a));
    const q = qualities[i] ?? 0;
    const held = heldAt(stacks, q);
    const unit = best.bulk ? " SCU" : " pcs";
    return `${held.toFixed(best.bulk ? 2 : 0)}${unit} at Q${q}+ · best Q${best.quality} · ${best.place}`;
  }

  /** Stat rows under their sections, in sheet order. */
  const sections = $derived.by(() => {
    const out: { group: string | null; stats: BlueprintStatView[] }[] = [];
    for (const s of detail?.stats ?? []) {
      const last = out.at(-1);
      if (last && last.group === s.group) last.stats.push(s);
      else out.push({ group: s.group, stats: [s] });
    }
    return out;
  });

  const tone = (improved: boolean | null) => (improved == null ? "var(--text-faint)" : improved ? "var(--good)" : "var(--bad)");
  /** A slot's effect, judged by the stat it moves: recoil is better down. */
  function effectTone(property: string | null, up: boolean | null) {
    if (up == null) return "var(--text-faint)";
    const better = detail?.stats.find((s) => s.property === property)?.higherIsBetter;
    return better == null ? "var(--text-dim)" : up === better ? "var(--good)" : "var(--bad)";
  }
</script>

<div class="sheet">
  <header>
    <h2>{row.name}</h2>
    <div class="meta">
      {#if row.manufacturer}<span>{row.manufacturer}</span>{/if}
      {#if sized}<span>Size {row.size}</span>{/if}
      {#if classText(row)}<span>{classText(row)}</span>{/if}
      <span class="hue-chip" style="--hue: {owned ? 'var(--good)' : 'var(--warn)'}">{owned ? "owned" : "not owned"}</span>
      {#if detail}
        <span class="hue-chip" style="--hue: {ready === detail.slots.length ? 'var(--good)' : 'var(--text-dim)'}">
          materials {ready} / {detail.slots.length}
        </span>
      {/if}
    </div>
    {#if !inGame}
      <div class="acts">
        <Button variant={owned ? "outline" : "primary"} onclick={() => void toggleMark("owned", row.guid)}>
          {owned ? "Mark not owned" : "Mark owned"}
        </Button>
      </div>
    {/if}
  </header>

  {#if error}<p class="error">{error}</p>{/if}

  {#if detail}
    <section>
      <div class="head">
        <h3>Materials</h3>
        <Segmented options={PRESETS.map((p) => ({ value: p, label: p }))} value={preset ?? ""} onchange={(v) => apply(v as Preset)} />
      </div>
      {#each detail.slots as slot, i (i)}
        <div class="slot">
          <div class="slot-head">
            {#if slot.name}<span class="slot-name">{slot.name}</span>{/if}
            <span class="material">{slot.material.name}</span>
            <span class="need">{fmtNeed(slot.material)}</span>
          </div>
          {#if qualities.length === detail.slots.length}
            <div class="held"><span class="dot" style="background: {HUE[stateOf(i)]}"></span>{holding(i)}</div>
          {/if}
          <div class="q">
            <input
              type="range"
              min={slot.material.minQuality}
              max="1000"
              value={qualities[i] ?? 0}
              aria-label="Quality of {slot.material.name}"
              oninput={(e) => setQuality(i, e.currentTarget.valueAsNumber)}
            />
            <span class="qval">Q{qualities[i] ?? 0}</span>
          </div>
          {#each slot.modifiers as m, mi (mi)}
            {@const e = effectText(m, qualities[i] ?? 0)}
            {#if e}
              <div class="effect">
                <span class="grow">{m.name ?? "Effect"}</span>
                <span class="mono pct" style="color: {effectTone(m.property, e.up)}">{e.pct == null ? e.factor : fmtPct(e.pct)}</span>
                <span class="mono reach">{reachText(m)}</span>
              </div>
            {/if}
          {/each}
        </div>
      {/each}
      <div class="effect"><span class="grow">Craft time</span><span class="mono">{fmtTime(row.craftSeconds)}</span></div>
    </section>

    {#if sections.length > 0}
      <section>
        <div class="head"><h3>Product stats</h3><span class="dim">stock → crafted</span></div>
        {#each sections as sec, si (si)}
          {#if sec.group}<div class="sub">{sec.group}</div>{/if}
          {#each sec.stats as s (s.label)}
            {@const r = evalStat(s, detail.slots, qualities)}
            <div class="stat">
              <span class="k">{s.label}</span>
              <span class="v base">{s.base == null ? "—" : fmtValue(s.base, s.unit)}</span>
              <span class="v now">{r.value == null ? "—" : fmtValue(r.value, s.unit)}</span>
              <span class="v" style="color: {tone(r.improved)}">{r.pct == null ? "" : fmtPct(r.pct)}</span>
            </div>
          {/each}
        {/each}
      </section>
    {/if}

    <section>
      <div class="head"><h3>Granted by</h3></div>
      {#if detail.grantedBy.length === 0}
        <p class="dim">No mission in this build grants it.</p>
      {:else}
        <ul>
          {#each detail.grantedBy as m (m.missionId)}
            <li><a href="/data/missions?q={encodeURIComponent(m.title)}">{m.title}</a></li>
          {/each}
        </ul>
      {/if}
    </section>
  {:else if !error}
    <p class="dim">Loading…</p>
  {/if}
</div>

<style>
  .sheet {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: var(--space-3);
  }
  h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.4rem;
    line-height: 1.15;
    color: var(--text-strong);
  }
  h3 {
    margin: 0;
    font-family: var(--font-mono);
    font-size: var(--text-small);
    font-weight: var(--weight-medium);
    color: var(--text-dim);
    text-transform: var(--label-transform);
    letter-spacing: var(--label-tracking);
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 10px;
    margin-top: 6px;
    font-size: var(--text-small);
    color: var(--text-dim);
  }
  .acts {
    margin-top: 10px;
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 8px;
  }
  .slot {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-sunk);
  }
  .slot-head {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 4px 8px;
  }
  .slot-name {
    font-family: var(--font-mono);
    font-size: var(--text-small);
    color: var(--text-faint);
    text-transform: var(--label-transform);
    letter-spacing: var(--label-tracking);
  }
  .material {
    color: var(--text);
    font-weight: var(--weight-medium);
  }
  .need {
    font-family: var(--font-mono);
    font-size: var(--text-small);
    color: var(--text-dim);
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .held {
    display: flex;
    align-items: baseline;
    gap: 6px;
    font-size: var(--text-small);
    color: var(--text-dim);
  }
  .dot {
    flex-shrink: 0;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    transform: translateY(-1px);
  }
  .q {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 56px;
    align-items: center;
    gap: 10px;
  }
  .q input {
    width: 100%;
    margin: 0;
    accent-color: var(--hue-quality);
  }
  .qval {
    font-family: var(--font-mono);
    font-weight: var(--weight-bold);
    text-align: right;
    color: color-mix(in srgb, var(--hue-quality) 70%, var(--text-strong));
  }
  .effect {
    display: flex;
    align-items: baseline;
    gap: 10px;
    font-size: var(--text-small);
    color: var(--text-dim);
  }
  .pct {
    min-width: 62px;
    text-align: right;
  }
  .reach {
    color: var(--text-faint);
  }
  .sub {
    margin-top: 4px;
    font-family: var(--font-mono);
    font-size: var(--text-small);
    color: var(--text-faint);
    text-transform: var(--label-transform);
    letter-spacing: var(--label-tracking);
  }
  .stat {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 64px 64px 66px;
    gap: 10px;
    align-items: baseline;
    font-size: var(--text-control);
  }
  .k {
    color: var(--text-dim);
  }
  .v {
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
    text-align: right;
  }
  .v.base {
    color: var(--text-dim);
  }
  .v.now {
    color: var(--text-strong);
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: var(--text-control);
  }
</style>
