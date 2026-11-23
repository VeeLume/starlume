<script lang="ts">
  // One item's recipe: the blueprint you pick and whether you own it, a
  // quality per material, what it comes out as, and which missions grant
  // it. Every variant of a family shares the recipe (the cook guarantees
  // it), so only the owned mark and the granting missions are per variant.
  // The sliders are page-local: they explore, and record nothing until Plan
  // or Bookmark (the header's pair) or "Save to plan" takes them as the
  // plan's minimum per material. A slider stops at the qualities its
  // material comes in ($lib/quality). Design pass 8: the header with the
  // plan stays while the body scrolls, so plan, sliders and stats are in
  // view together.
  import { Check } from "lucide-svelte";
  import { Segmented, Select, type SelectOption } from "@veelume/ui";
  import PlanPair from "$lib/components/PlanPair.svelte";
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
    heldShort,
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
    choices,
  }: {
    family: Family;
    channel: string;
    /** A variant to show, chosen outside (an armour set's colourway grid). */
    pick?: string | null;
    /** The blueprints list shows only the variant in view: the caller
     *  already lists the others (the set's colourway grid). */
    single?: boolean;
    /** The variants the header's picker offers, from the caller (an armour
     *  set: every piece's colourways); without it, the family's own. */
    choices?: { options: SelectOption[]; value: string; onchange: (value: string) => void; owned: string };
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
  const many = $derived(family.variants.length > 1 && !single);
  const owned = $derived(craftingStore.owns(variant.guid));
  const inGame = $derived(craftingStore.ownedInGame(variant.guid));
  const variantLabel = $derived(family.shelf.main === "Armour" ? "Colourway" : "Variant");
  /** The header's picker: the caller's, else the family's variants. One
   *  option is no choice, so it shows as text (Valerie, 2026-11-23). */
  const pickFrom = $derived(
    choices ?? {
      options: family.variants.map((v) => ({
        value: v.guid,
        label: v.variant ?? "Standard",
        detail: craftingStore.owns(v.guid) ? "owned" : "",
      })),
      value: variant.guid,
      onchange: (g: string) => (focus = g),
      owned: family.variants.length > 1 ? `${family.ownedCount} of ${family.variants.length} owned` : "",
    },
  );

  // The plan this sheet talks to: the variant on its own ("" here), or its
  // entry in a project. The sliders as they stand are what Plan stores, one
  // minimum per material; a planned variant starts on its plan's minimums
  // and "Save to plan" writes the sliders back.
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
    qualities = d.slots.map((s, i) => {
      // Base aims at the stock quality (the neutral point) where the material
      // moves stats, snapped up so it never lands below it; a material that
      // moves nothing needs no more than the recipe's floor (Valerie,
      // 2026-11-16). Mirrors SlotNeed::aim on the Rust side.
      if (next === "Base") {
        return s.modifiers.length === 0
          ? snapUp(stops[i] ?? [], s.material.minQuality)
          : snapUp(stops[i] ?? [], Math.max(d.defaultQuality, s.material.minQuality));
      }
      return snapNearest(stops[i] ?? [], presetQuality(next, s.material, d.defaultQuality, craftingStore.stock.get(s.material.name)));
    });
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
  <!-- The header holds the plan and stays while the body scrolls (design
       pass 8): the item and its Plan / Bookmark pair, the variant, then
       the count and where it goes. Every control is always there, so
       planning moves nothing. -->
  <header class="top">
    <div class="hero">
      <span class="tile"><Icon size={20} /></span>
      <div class="title">
        <h2>{family.title}</h2>
        <div class="kicker">
          {#if variant.manufacturer}<span class="maker">{variant.manufacturer}</span>{/if}
          {#if sized}<span>Size {variant.size}</span>{/if}
          {#if classText(variant)}<span>{classText(variant)}</span>{/if}
          <span class="time">{fmtTime(variant.craftSeconds)} to craft</span>
        </div>
      </div>
      <PlanPair
        plan={holder?.plan}
        add={(parked) => (filled ? planCraft(dest || null, variant.guid, planCount, [...qualities], parked) : Promise.resolve(false))}
        what="{planCount} {variant.name} with these minimums"
        blocked={filled ? "" : "Loading the recipe"}
        markOff={dest ? "A craft in a project is planned with the project" : ""}
      />
    </div>

    <!-- One layout for every item: the same three lines, the same slots,
         whatever the item has (Valerie, 2026-11-23). -->
    <div class="line">
      <span class="lbl">{variantLabel}</span>
      <span class="slot-pick">
        {#if pickFrom.options.length > 1}
          <Select class="pick" label={variantLabel} options={pickFrom.options} value={pickFrom.value} onchange={pickFrom.onchange} />
        {:else}
          <span class="fixed">{pickFrom.options[0]?.label ?? "Standard"}</span>
        {/if}
      </span>
      <button
        type="button"
        class="own"
        class:set={owned}
        disabled={inGame}
        aria-pressed={owned}
        title={inGame ? "Owned in the game" : owned ? "You marked this blueprint as yours" : "Mark this blueprint as yours"}
        onclick={() => void toggleMark("owned", variant.guid)}><Check size={12} />Owned</button
      >
      <span class="grow"></span>
      <span class="faint">{pickFrom.owned}</span>
    </div>

    {#if plans}
      <div class="line">
        <span class="lbl">Count</span>
        <span class="step">
          <button type="button" aria-label="One less" disabled={planCount <= 1} onclick={() => (countDraft = Math.max(1, planCount - 1))}>−</button>
          <input type="number" min="1" value={planCount} aria-label="Count" onchange={(e) => setCount(e.currentTarget.value)} />
          <button type="button" aria-label="One more" onclick={() => (countDraft = planCount + 1)}>+</button>
        </span>
        <span class="lbl">In</span>
        <span class="slot-dest">
          {#if plansStore.projects.length > 0}
            <Select
              class="dest"
              label="Where the craft goes"
              options={[
                { value: "", label: "A plan of its own" },
                ...plansStore.projects.map((p) => ({ value: p.id, label: p.name || "Project", group: "Projects" })),
              ]}
              value={dest}
              onchange={(v) => {
                dest = v;
                countDraft = null;
              }}
            />
          {:else}
            <span class="fixed" title="Start a project on Plans to gather crafts in it">a plan of its own</span>
          {/if}
        </span>
        <!-- Always reserved, so a change that appears here moves nothing. -->
        <span class="save">
          {#if changed}
            <button type="button" class="sb quiet" title="Put the sliders back on the plan's minimums" onclick={toPlan}>Back</button>
            <button type="button" class="sb main" title="Take the count and the qualities below as the plan's minimums" onclick={() => void savePlan()}
              >Save</button
            >
          {/if}
        </span>
      </div>
    {/if}
  </header>

  {#if error}<p class="error">{error}</p>{/if}

  {#if detail}
    <section>
      <div class="head">
        <h3>Materials</h3>
        <span class="grow"></span>
        {#if filled && craftingStore.stockRead}<span class="faint">{ready} of {detail.slots.length} held at these</span>{/if}
      </div>
      <div class="presets">
        <Segmented options={PRESETS.map((p) => ({ value: p, label: p }))} value={preset ?? ""} onchange={(v) => apply(v as Preset)} />
      </div>
      {#each detail.slots as slot, i (i)}
        <div class="slot">
          <div class="slot-head">
            {#if slot.name}<span class="slot-name">{slot.name}</span>{/if}
            <span class="material">{slot.material.name}</span>
            <span class="need">{fmtNeed(slot.material)}</span>
            <span class="band" title="The game gives every {slot.material.name} in this quality band the one quality shown"
              >{bandText(slot.bands, qualities[i] ?? 0)}</span
            >
            <span class="qval">Q{qualities[i] ?? 0}</span>
          </div>
          <div class="q">
            <span class="stock" style="left: {stockAt(i, slot.material.minQuality, detail.defaultQuality)}" title="Stock quality"></span>
            {#if planned}
              <span class="mine" style="left: {stockAt(i, slot.material.minQuality, planned[i])}" title="Your plan's minimum, Q{planned[i]}"></span>
            {/if}
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
              <span
                class="have"
                title={holdingText(craftingStore.stock.get(slot.material.name), slot.material, qualities[i] ?? 0, craftingStore.stockRead)}
              >
                {#if craftingStore.stockRead}<i style="background: {HUE[stateOf(i)]}"></i>{/if}
                {heldShort(craftingStore.stock.get(slot.material.name), slot.material, qualities[i] ?? 0, craftingStore.stockRead)}
              </span>
              <span class="effects">
                {#each slot.modifiers as m, mi (mi)}
                  {@const e = effectText(m, qualities[i] ?? 0)}
                  {#if e}
                    <span class="effect" title="{m.name ?? 'Effect'}: {reachText(m)}"
                      >{m.name ?? "Effect"}<b style="color: {effectTone(m.property, e.up)}">{e.pct == null ? e.factor : fmtPct(e.pct)}</b></span
                    >
                  {/if}
                {/each}
              </span>
            </div>
          {/if}
        </div>
      {/each}
    </section>

    {#if sections.length > 0}
      <section>
        <div class="head"><h3>Comes out as</h3><span class="grow"></span><span class="faint">crafted, change from stock</span></div>
        <div class="stats">
          {#each sections as sec, si (si)}
            {#if sec.group}<div class="sub">{sec.group}</div>{/if}
            {#each sec.stats as s (s.label)}
              {@const r = evalStat(s, detail.slots, qualities)}
              <div class="stat" title="Stock {s.base == null ? '—' : fmtValue(s.base, s.unit)}">
                <span class="k">{s.label}</span>
                <span class="v">{r.value == null ? "—" : fmtValue(r.value, s.unit)}</span>
                <span class="d" style="color: {tone(r.improved)}">{r.pct == null ? "same" : fmtPct(r.pct)}</span>
              </div>
            {/each}
          {/each}
        </div>
      </section>
    {/if}

    <section>
      <div class="head">
        <h3>Granted by</h3>
        {#if many}<span class="faint">{variant.variant ?? "Standard"}</span>{/if}
        <span class="grow"></span>
        {#if detail.grantedBy.length > 0}
          <a class="faint link" href="/contracts?bp={encodeURIComponent(variant.guid)}">Show in Contracts</a>
        {/if}
      </div>
      {#if detail.grantedBy.length === 0}
        <p class="none">No mission in this build grants it.</p>
      {:else}
        <ul>
          {#each detail.grantedBy as m (m.missionId)}
            <li><a href="/contracts?bp={encodeURIComponent(variant.guid)}&q={encodeURIComponent(m.title)}">{m.title}</a></li>
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
    gap: 12px;
    min-width: 0;
    /* The stats go to two columns only where the pane is wide enough for
       their labels whole. */
    container-type: inline-size;
  }
  /* The pane pads 16px and scrolls: the header pulls out over that padding
   * and sticks at the pane's top edge, the body scrolling under it. */
  .top {
    position: sticky;
    top: -16px;
    z-index: 2;
    margin: -16px -16px 0;
    padding: 12px 16px 10px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    border-bottom: 1px solid var(--border);
    background:
      linear-gradient(160deg, color-mix(in srgb, var(--hue) 10%, var(--bg-sunk)), var(--bg-sunk) 70%),
      var(--bg);
  }
  .hero {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .tile {
    flex: 0 0 auto;
    display: grid;
    place-items: center;
    width: 38px;
    height: 38px;
    border-radius: var(--radius);
    border: 1px solid color-mix(in srgb, var(--hue) 32%, transparent);
    color: color-mix(in srgb, var(--hue) 75%, var(--text-strong));
  }
  .title {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.2rem;
    font-weight: var(--weight-bold);
    line-height: 1.15;
    color: var(--text-strong);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .kicker {
    display: flex;
    gap: 0 10px;
    overflow: hidden;
    white-space: nowrap;
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
  .line {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 28px;
    min-width: 0;
  }
  .lbl {
    flex: 0 0 auto;
    font-size: var(--text-small);
    color: var(--text-dim);
  }
  /* Fixed slots: a picker or its one option as text take the same room. */
  .slot-pick {
    flex: 0 1 160px;
    min-width: 0;
  }
  .slot-dest {
    flex: 1 1 0;
    min-width: 0;
  }
  .slot-pick :global(.pick),
  .slot-dest :global(.dest) {
    width: 100%;
  }
  .fixed {
    display: block;
    padding: 0 2px;
    font-size: var(--text-control);
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .faint {
    font-size: var(--text-small);
    color: var(--text-faint);
    white-space: nowrap;
  }
  .own {
    all: unset;
    flex: 0 0 auto;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    box-sizing: border-box;
    height: 24px;
    padding: 0 7px;
    border: 1px solid var(--border);
    border-radius: var(--radius-chip);
    font-size: var(--text-badge);
    white-space: nowrap;
    color: var(--text-faint);
    cursor: pointer;
  }
  .own:hover:not(:disabled) {
    border-color: var(--accent);
    color: var(--text);
  }
  .own:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .own:disabled {
    cursor: default;
  }
  .own.set {
    color: var(--good);
    border-color: color-mix(in srgb, var(--good) 45%, transparent);
    background: color-mix(in srgb, var(--good) 12%, transparent);
  }
  .step {
    flex: 0 0 auto;
    display: inline-flex;
    align-items: center;
    height: 28px;
    box-sizing: border-box;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg);
  }
  .step button {
    all: unset;
    width: 22px;
    text-align: center;
    color: var(--text-dim);
    cursor: pointer;
  }
  .step button:hover:not(:disabled) {
    color: var(--text-strong);
  }
  .step button:disabled {
    cursor: default;
    opacity: 0.4;
  }
  .step input {
    all: unset;
    width: 3ch;
    text-align: center;
    font-family: var(--font-mono);
    color: var(--text-strong);
  }
  .step input::-webkit-inner-spin-button {
    display: none;
  }
  /* One width whatever it holds, so Save appearing moves nothing. */
  .save {
    flex: 0 0 108px;
    display: flex;
    justify-content: flex-end;
    gap: 4px;
  }
  .sb {
    all: unset;
    box-sizing: border-box;
    display: inline-flex;
    align-items: center;
    height: 26px;
    padding: 0 9px;
    border-radius: var(--radius);
    font-size: var(--text-small);
    font-weight: var(--weight-medium);
    white-space: nowrap;
    cursor: pointer;
  }
  .sb.main {
    background: var(--accent);
    color: var(--accent-foreground, var(--bg));
  }
  .sb.quiet {
    color: var(--text-dim);
  }
  .sb.quiet:hover {
    color: var(--text);
  }
  .sb:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 8px;
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
  .head {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 28px;
  }
  .slot {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-sunk);
  }
  .slot-head {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
    overflow: hidden;
  }
  .slot-name {
    font-family: var(--font-mono);
    font-size: var(--text-badge);
    color: var(--text-faint);
    text-transform: var(--label-transform);
    letter-spacing: var(--label-tracking);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }
  .material {
    color: var(--text-strong);
    font-weight: var(--weight-medium);
    white-space: nowrap;
  }
  .need,
  .band {
    font-family: var(--font-mono);
    font-size: var(--text-small);
    color: var(--text-faint);
    white-space: nowrap;
  }
  .qval {
    margin-left: auto;
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
  .stock,
  .mine {
    position: absolute;
    top: -2px;
    bottom: -2px;
    width: 1px;
    background: var(--text-faint);
    pointer-events: none;
  }
  .mine {
    top: -4px;
    bottom: -4px;
    width: 2px;
    background: var(--accent);
  }
  .held {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 2px 12px;
    font-size: var(--text-small);
    color: var(--text-dim);
    min-width: 0;
  }
  .effects {
    margin-left: auto;
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 2px 12px;
    min-width: 0;
  }
  .presets {
    display: flex;
  }
  .have {
    white-space: nowrap;
  }
  .have i {
    display: inline-block;
    width: 6px;
    height: 6px;
    margin-right: 5px;
    border-radius: 50%;
    transform: translateY(-1px);
  }
  .effect {
    white-space: nowrap;
  }
  .effect b {
    margin-left: 6px;
    font-family: var(--font-mono);
    font-weight: var(--weight-medium);
  }
  .stats {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 1px 20px;
  }
  @container (min-width: 560px) {
    .stats {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
  .sub {
    grid-column: 1 / -1;
    margin-top: 4px;
    font-family: var(--font-mono);
    font-size: var(--text-badge);
    color: var(--text-faint);
    text-transform: var(--label-transform);
    letter-spacing: var(--label-tracking);
  }
  .stat {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto 60px;
    gap: 8px;
    align-items: baseline;
    padding: 2px 0;
    font-size: var(--text-control);
  }
  .k {
    min-width: 0;
    color: var(--text-dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .v,
  .d {
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
    text-align: right;
    white-space: nowrap;
  }
  .v {
    color: var(--text-strong);
  }
  .d {
    font-size: var(--text-small);
  }
  .none {
    margin: 0;
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .link:hover {
    color: var(--text);
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
