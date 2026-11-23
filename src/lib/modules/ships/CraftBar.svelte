<script lang="ts">
  // The part picker's "Fitted part" bar (design board ShipsCraftQuality,
  // Valerie 2026-11-23): as sold, or crafted at a minimum quality per stat
  // the recipe moves. You craft for one stat (a gun's Impact Force) and take
  // whatever Integrity you have, so each recipe slot that moves a stat is a
  // slider of its own, named by what it moves; the material is the line
  // under it. A slot that moves nothing stays at its floor. Crafted starts
  // the main stats at stock quality (the figures start unchanged) and
  // Integrity at its floor. The sliders are drawn as sold too, dimmed, so
  // nothing moves as it toggles. Plan takes these minimums, one per slot.
  import { Segmented } from "@veelume/ui";
  import type { BlueprintDetailView, Plan } from "$lib/bindings";
  import PlanPair from "$lib/components/PlanPair.svelte";
  import { markAt, snapUp, stopsOf } from "$lib/quality";
  import { effectText, fmtPct, heldShort } from "$lib/recipe";
  import { getRecipe } from "$lib/state/catalog.svelte";
  import { craftingStore, loadStock } from "$lib/state/crafting.svelte";

  let {
    craft,
    channel,
    count,
    plans = null,
    oncraft,
    onplan,
  }: {
    /** The fitted part's blueprint and its mark: `quality` null while it
     *  is as sold; `slots` a quality per recipe slot, empty when all are
     *  at `quality`. */
    craft: { blueprint: string; quality: number | null; slots: number[] };
    channel: string | null;
    /** How many ports the part is fitted to. */
    count: number;
    plans?: { plan: Plan | undefined } | null;
    /** Mark it crafted at a quality per slot; null: as sold. */
    oncraft: (slots: number[] | null) => void;
    /** Plan it crafted at these minimums: a plan, or `parked` a bookmark. */
    onplan: (slots: number[], parked: boolean) => Promise<boolean>;
  } = $props();

  let recipe = $state<BlueprintDetailView | null>(null);
  $effect(() => {
    if (channel && !craftingStore.stockRead) void loadStock(channel);
  });
  $effect(() => {
    const bp = craft.blueprint;
    if (!channel) return;
    void getRecipe(channel, bp).then((r) => {
      if (craft.blueprint === bp) recipe = typeof r === "string" ? null : r;
    });
  });

  const crafted = $derived(craft.quality !== null);
  /** Integrity is what you take as it comes: it starts at the floor and
   *  sorts after the stats you craft for. */
  const minor = (names: string[]) => names.length > 0 && names.every((n) => /integrity/i.test(n));

  /** Each recipe slot: its stops, its floor, what it moves. */
  const slots = $derived(
    (recipe?.slots ?? []).map((s, i) => {
      const stops = stopsOf(s.bands, s.material.minQuality);
      const names = s.modifiers.map((m) => m.name ?? "Effect");
      const floor = stops[0] ?? s.material.minQuality;
      const stock = snapUp(stops, Math.max(recipe!.defaultQuality, s.material.minQuality));
      return { i, slot: s, stops, names, floor, start: s.modifiers.length === 0 || minor(names) ? floor : stock };
    }),
  );
  /** The qualities in effect: the mark's, else where Crafted starts. */
  const current = $derived(
    slots.map((s) => (crafted ? snapUp(s.stops, craft.slots[s.i] ?? craft.quality ?? s.start) : s.start)),
  );
  /** A slider being dragged, until the mark takes it. */
  let draft = $state<number[] | null>(null);
  $effect(() => {
    void craft.quality;
    void craft.slots;
    draft = null;
  });
  const shown = $derived(draft ?? current);
  /** The sliders: slots that move a stat, the ones you craft for first. */
  const sliders = $derived(
    slots.filter((s) => s.slot.modifiers.length > 0).sort((a, b) => Number(minor(a.names)) - Number(minor(b.names))),
  );

  function set(i: number, q: number, commit: boolean) {
    const next = [...shown];
    next[i] = q;
    if (commit) {
      draft = null;
      oncraft(next);
    } else draft = next;
  }

  const label = (s: (typeof slots)[number], q: number) => (q <= s.floor && s.floor <= 1 ? "any" : `Q${q}+`);
  const at = (s: (typeof slots)[number], q: number) =>
    `${(s.stops.length > 1 ? markAt(s.stops, q) : (q - s.floor) / Math.max(1, 1000 - s.floor)) * 100}%`;
  /** What the slot does to its stats at `q`, as the game reads it. */
  function effectOf(s: (typeof slots)[number], q: number): { text: string; tone: string } {
    const e = s.slot.modifiers.map((m) => effectText(m, q)).find((x) => x && x.up !== null);
    if (!e) return { text: "stock", tone: "var(--text-faint)" };
    return { text: e.pct == null ? e.factor : fmtPct(e.pct), tone: e.up ? "var(--good)" : "var(--bad)" };
  }

  const planWhat = $derived(
    sliders.map((s) => `${s.names.join(", ")} ${label(s, shown[s.i])}`).join(", ") || "at its floor",
  );
</script>

<!-- The same controls in both states, so nothing moves as it toggles. -->
<div class="craft">
  <span class="clbl">Fitted part</span>
  <Segmented
    options={[
      { value: "sold", label: "As sold" },
      { value: "crafted", label: "Crafted" },
    ]}
    value={crafted ? "crafted" : "sold"}
    onchange={(v) => oncraft(v === "sold" ? null : [...current])}
  />
  <span class="sep" aria-hidden="true"></span>
  <div class="stats" class:off={!crafted}>
    {#if !recipe}
      <span class="faint">Reading the recipe…</span>
    {/if}
    {#each sliders as s (s.i)}
      {@const q = shown[s.i]}
      {@const e = effectOf(s, q)}
      <div class="st" title="{s.names.join(', ')}: the minimum quality of its {s.slot.material.name}">
        <span class="st-n" class:main={!minor(s.names)}><b>{s.names.join(", ")}</b><small>{s.slot.material.name}</small></span>
        <span class="q">
          <span class="stock" style:left={at(s, snapUp(s.stops, recipe?.defaultQuality ?? 500))} title="Stock quality"></span>
          {#if s.stops.length > 1}
            <input
              type="range"
              min="0"
              max={s.stops.length - 1}
              step="1"
              value={Math.max(0, s.stops.indexOf(q))}
              disabled={!crafted}
              aria-label="Minimum quality for {s.names.join(', ')}"
              aria-valuetext={label(s, q)}
              oninput={(ev) => set(s.i, s.stops[ev.currentTarget.valueAsNumber], false)}
              onchange={(ev) => set(s.i, s.stops[ev.currentTarget.valueAsNumber], true)}
            />
          {:else}
            <input
              type="range"
              min={s.floor}
              max="1000"
              step="10"
              value={q}
              disabled={!crafted}
              aria-label="Minimum quality for {s.names.join(', ')}"
              oninput={(ev) => set(s.i, ev.currentTarget.valueAsNumber, false)}
              onchange={(ev) => set(s.i, ev.currentTarget.valueAsNumber, true)}
            />
          {/if}
        </span>
        <span class="st-q">{label(s, q)}</span>
        <span class="st-e"
          ><span>{heldShort(craftingStore.stock.get(s.slot.material.name), s.slot.material, q, craftingStore.stockRead)}</span><span
            style:color={e.tone}>{e.text}</span
          ></span
        >
      </div>
    {/each}
  </div>
  {#if plans}
    <PlanPair
      plan={plans.plan}
      add={(parked) => onplan([...shown], parked)}
      what={crafted ? `${count > 1 ? `${count} of them` : "it"} crafted, ${planWhat}` : ""}
      blocked={crafted ? (recipe ? "" : "Reading the recipe") : "Mark it crafted to plan it"}
    />
  {/if}
</div>

<style>
  .craft {
    display: flex;
    align-items: center;
    gap: 14px;
    min-height: 58px;
    margin: -2px 0 10px;
    padding: 8px 10px;
    border-radius: var(--radius);
    background: var(--accent-fill-faint);
    font-size: var(--text-small);
  }
  .clbl {
    color: var(--text-dim);
  }
  .sep {
    align-self: stretch;
    width: 1px;
    background: var(--border);
  }
  .stats {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 24px;
  }
  .stats.off {
    opacity: 0.4;
  }
  .faint {
    color: var(--text-faint);
  }
  .st {
    display: grid;
    grid-template-columns: auto 150px 56px;
    grid-template-rows: auto auto;
    column-gap: 10px;
    row-gap: 1px;
    align-items: center;
  }
  .st-n {
    grid-row: 1 / 3;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .st-n b {
    font-weight: var(--weight-semibold);
    color: var(--text-strong);
    white-space: nowrap;
  }
  .st-n.main b {
    color: color-mix(in srgb, var(--hue-quality) 60%, var(--text-strong));
  }
  .st-n small {
    font-size: var(--text-badge);
    color: var(--text-faint);
    white-space: nowrap;
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
  .st-q {
    text-align: right;
    font-family: var(--font-mono);
    font-weight: var(--weight-bold);
    color: color-mix(in srgb, var(--hue-quality) 70%, var(--text-strong));
  }
  .st-e {
    grid-column: 2 / 4;
    display: flex;
    justify-content: space-between;
    gap: 8px;
    font-size: var(--text-badge);
    color: var(--text-faint);
    white-space: nowrap;
  }
</style>
