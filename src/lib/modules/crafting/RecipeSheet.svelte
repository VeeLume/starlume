<script lang="ts">
  // One item's recipe: which of its blueprints you own and want, a quality
  // per material, what it comes out as, and which missions grant it. Every
  // variant of a family shares the recipe (the cook guarantees it), so only
  // the marks and the granting missions are per variant. The sliders are
  // page-local: they explore, and record nothing until "Add to plan" or
  // "Update plan" takes them as the plan's minimum per material. A slider
  // stops at the qualities its material comes in ($lib/quality).
  import { goto } from "$app/navigation";
  import { Check, Flag, Heart, Target } from "lucide-svelte";
  import { Segmented } from "@veelume/ui";
  import type { BlueprintDetailView, BlueprintStatView } from "$lib/bindings";
  import { getRecipe } from "$lib/state/catalog.svelte";
  import { craftingStore, toggleMark } from "$lib/state/crafting.svelte";
  import { loadPlans, planCraft, plansStore, updateCraft } from "$lib/state/plans.svelte";
  import { settingsStore } from "$lib/state/settings.svelte";
  import { bandText, markAt, slotMinimums, snapNearest, snapUp, stopsOf } from "$lib/quality";
  import {
    PRESETS,
    classText,
    effectText,
    evalStat,
    fmtNeed,
    fmtPct,
    fmtTime,
    fmtValue,
    holdingText,
    presetQuality,
    reachText,
    readiness,
    type Family,
    type Preset,
    type Readiness,
  } from "./model";
  import { lookOf } from "./shelf";

  let {
    family,
    channel,
    pick = null,
    single = false,
  }: {
    family: Family;
    channel: string;
    /** A variant to show, chosen outside (an armour set's colourway grid). */
    pick?: string | null;
    /** The blueprints list shows only the variant in view: the caller
     *  already lists the others (the set's colourway grid). */
    single?: boolean;
  } = $props();

  let detail = $state<BlueprintDetailView | null>(null);
  let error = $state("");
  let preset = $state<Preset | null>("Base");
  let qualities = $state<number[]>([]);
  let focus = $state<string | null>(null);
  $effect(() => {
    if (pick) focus = pick;
  });

  /** Plans is a module of its own: no way there while it is off. */
  const plans = $derived(settingsStore.current?.enabled_modules.includes("plans") ?? false);
  $effect(() => {
    if (plans && plansStore.plans === null) void loadPlans();
  });

  const look = $derived(lookOf(family.shelf.main));
  const Icon = $derived(look.icon);
  const variant = $derived(family.variants.find((v) => v.guid === focus) ?? family.variants[0]);
  const sized = $derived(family.shelf.sized);
  const many = $derived(family.variants.length > 1);

  // The plan this sheet talks to: the variant on its own ("" here), or its
  // entry in a project. The sliders as they stand are what "Add to plan"
  // stores, one minimum per material; a planned variant starts on its
  // plan's minimums and "Update plan" writes the sliders back.
  let dest = $state("");
  let countDraft = $state<number | null>(null);
  const holder = $derived(
    plansStore.holdersOf(variant.guid).find((h) => (dest === "" ? h.plan.goal.kind === "craft" : h.plan.id === dest)) ?? null,
  );
  const planCount = $derived(countDraft ?? holder?.craft.count ?? 1);

  /** Each slot's stops: the qualities its material comes in, from the
   *  slot's own minimum up. Empty: the data gives none, the slider is free. */
  const stops = $derived((detail?.slots ?? []).map((s) => stopsOf(s.bands, s.material.minQuality)));
  /** The plan's minimum per slot, as the sliders show it. */
  const planned = $derived(
    holder && detail
      ? slotMinimums(holder.craft, detail.slots.length).map((q, i) => snapUp(stops[i], Math.max(q, detail!.slots[i].material.minQuality)))
      : null,
  );
  const changed = $derived(
    !!holder && !!planned && (planCount !== holder.craft.count || planned.some((q, i) => q !== qualities[i])),
  );

  function setCount(raw: string) {
    const value = Math.round(Number(raw));
    if (raw.trim() !== "" && Number.isFinite(value)) countDraft = Math.max(1, value);
  }

  async function savePlan() {
    if (!filled) return;
    const slots = [...qualities];
    const ok = holder
      ? await updateCraft(holder.plan.id, variant.guid, planCount, slots)
      : await planCraft(dest || null, variant.guid, planCount, slots);
    if (ok) countDraft = null;
  }

  /** Put the sliders back on the plan's minimums. */
  function toPlan() {
    if (!planned) return;
    qualities = [...planned];
    countDraft = null;
    preset = null;
  }

  // Plain, not $state: only tells a new family from a new variant of this one.
  let shownFamily: string | null = null;

  $effect(() => {
    const guid = variant.guid;
    const key = family.key;
    void (async () => {
      const r = await getRecipe(channel, guid);
      if (guid !== variant.guid) return;
      if (typeof r === "string") {
        error = r;
        detail = null;
        return;
      }
      error = "";
      detail = r;
      // A variant shares the recipe: the sliders stay where they are.
      if (key !== shownFamily) {
        shownFamily = key;
        countDraft = null;
        focus = pick && family.variants.some((v) => v.guid === pick) ? pick : null;
        // A planned variant opens on its plan (the first that has it).
        const first = plansStore.holdersOf(variant.guid)[0];
        dest = first && first.plan.goal.kind === "project" ? first.plan.id : "";
        if (first) toPlan();
        else apply("Base");
      }
    })();
  });

  function apply(next: Preset) {
    if (!detail) return;
    const d = detail;
    preset = next;
    qualities = d.slots.map((s, i) =>
      snapNearest(stops[i] ?? [], presetQuality(next, s.material, d.defaultQuality, craftingStore.stock.get(s.material.name))),
    );
  }

  function setQuality(i: number, value: number) {
    qualities[i] = value;
    preset = null;
  }

  const filled = $derived(detail !== null && qualities.length === detail.slots.length);
  const stateOf = (i: number): Readiness => {
    const s = detail!.slots[i];
    return readiness(craftingStore.stock.get(s.material.name), s.material, qualities[i] ?? 0);
  };
  const ready = $derived(filled ? detail!.slots.filter((_, i) => stateOf(i) === "good").length : 0);

  const HUE: Record<Readiness, string> = { good: "var(--good)", short: "var(--warn)", none: "var(--bad)" };

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

  /** Where stock quality sits on a slider: along its stops when it has
   *  them, else along the free scale. */
  const stockAt = (i: number, min: number, base: number) =>
    `${(stops[i]?.length ? markAt(stops[i], base) : (base - min) / (1000 - min)) * 100}%`;
</script>

<div class="sheet" style="--hue: {look.hue}">
  <header class="hero">
    <span class="tile"><Icon size={26} /></span>
    <div class="title">
      <div class="kicker">
        {#if variant.manufacturer}<span class="maker">{variant.manufacturer}</span>{/if}
        {#if sized}<span>Size {variant.size}</span>{/if}
        {#if classText(variant)}<span>{classText(variant)}</span>{/if}
        <span class="time">{fmtTime(variant.craftSeconds)} to craft</span>
      </div>
      <h2>{family.title}</h2>
      {#if filled && craftingStore.stockRead}
        <div class="have">You have {ready} of {detail!.slots.length} materials at these qualities</div>
      {/if}
    </div>
  </header>

  {#if error}<p class="error">{error}</p>{/if}

  <section>
    <div class="head">
      <h3>
        {many && !single ? "Blueprints" : "Blueprint"}
        {#if many && !single}<span class="owned-count">{family.ownedCount} of {family.variants.length} owned</span>{/if}
      </h3>
      <!-- A plan is for the variant in view, on its own or in a project.
           The sliders below are what it stores: one minimum per material.
           Its steps and materials are worked out on Plans. Every control
           here is always present, so planning moves nothing. -->
      {#if plans}
        <div class="plan-set">
          <label
            >Count
            <input class="input" type="number" min="1" value={planCount} onchange={(e) => setCount(e.currentTarget.value)} /></label
          >
          <label class="dest" title="A plan of its own, or a craft in one of your projects"
            >In
            <select class="select" bind:value={dest} onchange={() => (countDraft = null)}>
              <option value="">a plan of its own</option>
              {#each plansStore.projects as p (p.id)}
                <option value={p.id}>{p.name || "Project"}</option>
              {/each}
            </select></label
          >
          <button
            type="button"
            class="mark plan reset"
            disabled={!changed}
            title="Put the sliders back on the plan's minimums"
            onclick={toPlan}>Back to plan</button
          >
          <button
            type="button"
            class="mark plan main"
            class:set={!!holder && !changed}
            disabled={!filled}
            title={!holder
              ? `Plan to craft the ${variant.name} with the qualities below as each material's minimum`
              : changed
                ? "Take the count and the qualities below as the plan's minimums"
                : `${variant.name} is planned with these minimums: open Plans`}
            onclick={() => (holder && !changed ? void goto("/plans") : void savePlan())}
            ><Target size={12} />{!holder ? "Add to plan" : changed ? "Update plan" : "In plans"}</button
          >
        </div>
      {/if}
    </div>
    <div class="variants" class:many={many && !single}>
      {#each single ? [variant] : family.variants as v (v.guid)}
        {@const owned = craftingStore.owns(v.guid)}
        {@const inGame = craftingStore.ownedInGame(v.guid)}
        {@const wantBp = craftingStore.wantsBlueprint(v.guid)}
        {@const wantItem = craftingStore.wantsItem(v.guid)}
        <div class="variant" class:on={many && !single && v.guid === variant.guid}>
          {#if many}
            <button type="button" class="label" title={v.name} onclick={() => (focus = v.guid)}>{v.variant ?? "Standard"}</button>
          {:else}
            <span class="label plain">{v.name}</span>
          {/if}
          <button
            type="button"
            class="mark own"
            class:set={owned}
            disabled={inGame}
            aria-pressed={owned}
            title={inGame ? "Owned in the game" : "You have this blueprint"}
            onclick={() => void toggleMark("owned", v.guid)}><Check size={12} />Owned</button
          >
          <button
            type="button"
            class="mark bp"
            class:set={wantBp}
            disabled={owned}
            aria-pressed={wantBp}
            title={owned ? "You already have this blueprint" : "You want to get this blueprint"}
            onclick={() => void toggleMark("wantBlueprint", v.guid)}><Flag size={12} />Want blueprint</button
          >
          <button
            type="button"
            class="mark item"
            class:set={wantItem}
            aria-pressed={wantItem}
            title="You want a crafted one of this item"
            onclick={() => void toggleMark("wantItem", v.guid)}><Heart size={12} />Want item</button
          >
        </div>
      {/each}
    </div>
  </section>

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
            <span class="plan-min" title="The minimum your plan sets for this material">{planned ? `plan Q${planned[i]}+` : ""}</span>
            <span class="band" title="The game gives every {slot.material.name} in this quality band the one quality shown"
              >{bandText(slot.bands, qualities[i] ?? 0)}</span
            >
            <span class="qval">Q{qualities[i] ?? 0}</span>
          </div>
          <div class="q">
            <span class="stock" style="left: {stockAt(i, slot.material.minQuality, detail.defaultQuality)}" title="Stock quality"></span>
            {#if stops[i]?.length}
              <!-- One stop per quality the material comes in. -->
              <input
                type="range"
                min="0"
                max={stops[i].length - 1}
                step="1"
                value={Math.max(0, stops[i].indexOf(qualities[i] ?? 0))}
                aria-label="Quality of {slot.material.name}"
                aria-valuetext="Q{qualities[i] ?? 0}"
                oninput={(e) => setQuality(i, stops[i][e.currentTarget.valueAsNumber])}
              />
            {:else}
              <input
                type="range"
                min={slot.material.minQuality}
                max="1000"
                value={qualities[i] ?? 0}
                aria-label="Quality of {slot.material.name}"
                oninput={(e) => setQuality(i, e.currentTarget.valueAsNumber)}
              />
            {/if}
          </div>
          {#if filled}
            <div class="held">
              {#if craftingStore.stockRead}<i style="background: {HUE[stateOf(i)]}"></i>{/if}
              {holdingText(craftingStore.stock.get(slot.material.name), slot.material, qualities[i] ?? 0, craftingStore.stockRead)}
            </div>
          {/if}
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
    </section>

    {#if sections.length > 0}
      <section>
        <div class="head"><h3>Comes out as</h3><span class="cols">stock → crafted</span></div>
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
      <h3>{many ? `Granted by · ${variant.variant ?? "Standard"}` : "Granted by"}</h3>
      {#if detail.grantedBy.length === 0}
        <p class="none">No mission in this build grants it.</p>
      {:else}
        <ul>
          {#each detail.grantedBy as m (m.missionId)}
            <li><a href="/data/missions?q={encodeURIComponent(m.title)}">{m.title}</a></li>
          {/each}
        </ul>
      {/if}
    </section>
  {:else if !error}
    <p class="none">Loading…</p>
  {/if}
</div>

<style>
  .sheet {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .hero {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 14px;
    border: 1px solid color-mix(in srgb, var(--hue) 24%, var(--border));
    border-radius: var(--radius-lg);
    background:
      radial-gradient(120% 160% at 90% 0%, color-mix(in srgb, var(--hue) 26%, transparent), transparent 62%),
      linear-gradient(160deg, color-mix(in srgb, var(--hue) 12%, var(--bg-sunk)), var(--bg-sunk) 75%);
  }
  .tile {
    flex: 0 0 auto;
    display: grid;
    place-items: center;
    width: 60px;
    height: 60px;
    border-radius: var(--radius);
    border: 1px solid color-mix(in srgb, var(--hue) 32%, transparent);
    color: color-mix(in srgb, var(--hue) 75%, var(--text-strong));
    background: color-mix(in srgb, var(--bg-sunk) 60%, transparent);
  }
  .title {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .kicker {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 10px;
    font-size: var(--text-small);
    color: var(--text-dim);
  }
  .maker {
    font-family: var(--font-mono);
    font-weight: var(--weight-medium);
    color: var(--hue-sand);
  }
  .time {
    font-family: var(--font-mono);
  }
  h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.5rem;
    font-weight: var(--weight-bold);
    line-height: 1.15;
    color: var(--text-strong);
  }
  .have {
    font-size: var(--text-small);
    color: var(--text-dim);
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 14px;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--bg-raised);
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
  /* The count beside a heading: its own quiet element, not joined on. */
  .owned-count {
    margin-left: 8px;
    font-family: var(--font-sans);
    font-weight: 400;
    text-transform: none;
    letter-spacing: 0;
    color: var(--text-faint);
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 8px;
  }
  .cols {
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .variants {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .variants.many {
    max-height: 214px;
    overflow-y: auto;
  }
  .variant {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 2px 4px;
    border-radius: var(--radius-chip);
  }
  .variant.on {
    background: var(--accent-fill-weak);
  }
  .label {
    all: unset;
    flex: 1;
    min-width: 0;
    font-size: var(--text-control);
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    cursor: pointer;
  }
  .label.plain {
    cursor: default;
  }
  .label:focus-visible {
    outline: 2px solid var(--accent);
  }
  .mark {
    all: unset;
    flex: 0 0 auto;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    box-sizing: border-box;
    height: 24px;
    padding: 0 6px;
    border: 1px solid var(--border);
    border-radius: var(--radius-chip);
    font-size: var(--text-badge);
    white-space: nowrap;
    color: var(--text-faint);
    cursor: pointer;
  }
  .mark:hover:not(:disabled) {
    border-color: var(--accent);
    color: var(--text);
  }
  .mark:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .mark:disabled {
    cursor: default;
    opacity: 0.55;
  }
  .mark.own.set {
    opacity: 1;
    color: var(--good);
    border-color: color-mix(in srgb, var(--good) 45%, transparent);
    background: color-mix(in srgb, var(--good) 14%, transparent);
  }
  .mark.bp.set {
    color: var(--hue-sky);
    border-color: color-mix(in srgb, var(--hue-sky) 45%, transparent);
    background: color-mix(in srgb, var(--hue-sky) 14%, transparent);
  }
  /* Its own full-width row under the heading, wrapping inside the sheet:
     the sidebar is narrower than the four controls in one line. */
  .plan-set {
    flex: 1 1 100%;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: flex-end;
    gap: 6px 8px;
    min-width: 0;
  }
  .plan-set label {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--text-small);
    color: var(--text-dim);
  }
  .plan-set label.dest {
    flex: 1 1 160px;
    min-width: 0;
  }
  .plan-set .input {
    width: 60px;
    height: 24px;
  }
  .plan-set .select {
    flex: 1;
    min-width: 0;
    height: 24px;
    padding-block: 0;
  }
  /* One width whatever it says, so adding to a plan moves nothing. */
  .mark.plan.main {
    min-width: 104px;
    justify-content: center;
  }
  .mark.plan.set {
    color: var(--accent);
    border-color: var(--accent-line);
    background: var(--accent-fill);
  }
  .mark.item.set {
    color: var(--hue-rose);
    border-color: color-mix(in srgb, var(--hue-rose) 45%, transparent);
    background: color-mix(in srgb, var(--hue-rose) 14%, transparent);
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
    align-items: baseline;
    gap: 8px;
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
  .plan-min {
    margin-left: auto;
    font-family: var(--font-mono);
    font-size: var(--text-small);
    color: var(--accent);
  }
  .band {
    font-family: var(--font-mono);
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .qval {
    min-width: 5ch;
    text-align: right;
    font-family: var(--font-mono);
    font-weight: var(--weight-bold);
    color: color-mix(in srgb, var(--hue-quality) 70%, var(--text-strong));
  }
  .q {
    position: relative;
    display: flex;
  }
  .q input {
    width: 100%;
    margin: 0;
    accent-color: var(--hue-quality);
  }
  .stock {
    position: absolute;
    top: -2px;
    bottom: -2px;
    width: 1px;
    background: var(--text-faint);
    pointer-events: none;
  }
  .held {
    display: flex;
    align-items: baseline;
    gap: 6px;
    font-size: var(--text-small);
    color: var(--text-dim);
  }
  .held i {
    flex: 0 0 auto;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    transform: translateY(-1px);
  }
  .effect {
    display: flex;
    align-items: baseline;
    gap: 10px;
    font-size: var(--text-small);
    color: var(--text-dim);
  }
  .grow {
    flex: 1;
    min-width: 0;
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
  .none {
    margin: 0;
    font-size: var(--text-small);
    color: var(--text-faint);
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
