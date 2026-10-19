<script lang="ts">
  // Your saved loadouts, each row compared on the calculator's rock (flown
  // on their own; your crew's live in the Crew tab). The one on the
  // calculator is marked. Selecting a row opens a sheet beside it that
  // edits with the ship card's parts: the same heads, the same picker (laid
  // over the list), the same try-out against the saved loadout. Nothing
  // changes until Save; there's no dialog.
  //
  // Measured on every rock (design pass 5): one switch in the list's header
  // measures your loadouts on every rock that spawns — or one ore's, in one
  // system — instead of the calculator's rock. Each row then carries the two
  // scores (Everyday: easy or manageable among the rocks you'd meet; Reach:
  // how much of what spawns it breaks at all) and its comfort zone; the
  // sheet draws the try-out above the saved loadout on the same rocks; a
  // part opens the sweep (every option, measured, best first); a saved
  // module's tile says what it adds; and the sheet keeps this session's
  // try-outs. The rows are the comparison: name, scores and comfort zone on
  // one line each, the parts in the sheet.
  import { untrack } from "svelte";
  import { Segmented, Select, type SelectOption } from "@veelume/ui";
  import { ArrowDown } from "lucide-svelte";
  import type { EvalView, LaserInputView, LoadoutView, MiningCatalogView, SavedLoadoutView } from "$lib/bindings";
  import Monogram from "$lib/components/Monogram.svelte";
  import NowCard from "$lib/components/NowCard.svelte";
  import {
    changesFrom,
    comfortKey,
    comfortOf,
    compareOnRock,
    deleteLoadout,
    miningStore,
    miningUi,
    requestComfort,
    saveLoadout,
    savedForm,
    useLoadout,
  } from "$lib/state/mining.svelte";
  import ClassLegend from "./ClassLegend.svelte";
  import { binsOf, comfortRows, scoresOf, topOf, type ComfortFilter, type Scores } from "./comfort";
  import ComfortZone from "./ComfortZone.svelte";
  import CrackCell from "./CrackCell.svelte";
  import HeadsEditor from "./HeadsEditor.svelte";
  import { MASS_SCALE, int, lasersLine, massAt, namesOf, oreName, ownLasers } from "./format";
  import PartPicker from "./PartPicker.svelte";
  import PartSweep from "./PartSweep.svelte";
  import Pending from "./Pending.svelte";
  import { changeLabel, withPart } from "./parts";
  import TryOut from "./TryOut.svelte";

  let { catalog, channel, loadout }: { catalog: MiningCatalogView; channel: string; loadout: LoadoutView } = $props();

  const NEW_MINE = "new:mine";

  const names = $derived(namesOf(catalog));
  const mstate = $derived(miningStore.state);
  const saved = $derived(mstate?.saved ?? []);
  const mine = $derived(saved.filter((s) => s.owner === null));
  const own = $derived(ownLasers(loadout));
  const distance = $derived(own.find((l) => l.distanceM !== null)?.distanceM ?? null);

  // ── Selection and the sheet's draft (page-local) ──
  let selected = $state("");
  let draft = $state<LaserInputView[]>([]);
  let draftName = $state("");
  let open = $state("");
  let error = $state("");

  const sel = $derived(mine.find((s) => s.id === selected) ?? null);
  const isNew = $derived(selected === NEW_MINE);

  function select(id: string) {
    selected = id;
    open = "";
    error = "";
    const s = mine.find((x) => x.id === id);
    draft = s ? structuredClone($state.snapshot(s.loadout.lasers)) : [];
    draftName = s?.name ?? "";
  }
  // Open on the loadout on the calculator, else the first one.
  $effect(() => {
    if (selected && (isNew || sel)) return;
    const first = mine.find((s) => s.id === mstate?.basedOn)?.id ?? mine[0]?.id ?? "";
    if (first) select(first);
  });

  // ── Comparing on the rock ──
  const alone = (lasers: LaserInputView[]): LoadoutView => ({
    lasers: lasers.map((l) => ({ ...l, crew: null, distanceM: distance })),
    gadget: loadout.gadget,
    lead: null,
  });
  const pct = (e: EvalView | null | undefined) => (e === undefined ? undefined : e?.difficulty ? Math.round(e.difficulty.controlled * 100) : null);

  // undefined: being worked out; null: no rock to compare on.
  let rowEvals = $state<Map<string, EvalView | null> | null | undefined>(undefined);
  $effect(() => {
    const list = mine.map((s) => alone(s.loadout.lasers));
    const ids = mine.map((s) => s.id);
    let live = true;
    void compareOnRock(channel, list).then((evals) => {
      if (!live) return;
      rowEvals = evals ? new Map(ids.map((id, k) => [id, evals[k]])) : null;
    });
    return () => (live = false);
  });
  const rowCrack = (s: SavedLoadoutView) => (rowEvals ? pct(rowEvals.get(s.id)) : rowEvals);
  /** Why a row's figure is what it is on the rock: too weak, too strong, or
   *  how its cracks go (per 100 attempts). */
  function verdict(e: EvalView | null | undefined): { text: string; tone: string } {
    if (e === undefined) return { text: "measuring…", tone: "" };
    if (!e) return { text: "", tone: "" };
    const d = e.difficulty;
    if (!d) {
      const more = e.margin > 0 && e.margin < 1 ? `: needs ${(1 / e.margin).toFixed(1)}× the power` : "";
      return { text: `can't break it${more}`, tone: "b" };
    }
    if (p100(d.controlled) === 0 && d.overcharged >= 0.5) return { text: "too much power: overcharges every time", tone: "b" };
    const parts = [`${p100(d.controlled)} clean`];
    if (p100(d.overcharged)) parts.push(`${p100(d.overcharged)} overcharge`);
    if (p100(d.timedOut)) parts.push(`${p100(d.timedOut)} run out of time`);
    return { text: `${parts.join(" · ")} in 100`, tone: "" };
  }

  // The sheet's try-out: the draft against the saved loadout.
  const build = alone;
  const changes = $derived(sel ? changesFrom(sel.loadout.lasers, draft) + (draftName.trim() !== sel.name ? 1 : 0) : 0);
  let wasEval = $state<EvalView | null | undefined>(undefined);
  let nowEval = $state<EvalView | null | undefined>(undefined);
  $effect(() => {
    if (!sel || !changes) return;
    const pair = [build(sel.loadout.lasers), build(draft)];
    wasEval = nowEval = undefined;
    let live = true;
    void compareOnRock(channel, pair).then((e) => {
      if (!live) return;
      wasEval = e?.[0] ?? null;
      nowEval = e?.[1] ?? null;
    });
    return () => (live = false);
  });

  const rock = $derived.by(() => {
    const r = miningStore.report?.rock;
    if (!r) return "";
    const src = miningStore.source;
    const name = src ? oreName(src.rockType) : "the rock";
    const gadget = loadout.gadget ? catalog.gadgets.find((g) => g.id === loadout.gadget)?.name : null;
    return `${name}, ${int(r.mass)} t${gadget ? `, ${gadget}` : ""}`;
  });

  // ── Actions ──
  async function save(id: string | null, name: string) {
    const owner = null;
    // The calculator follows a save of its loadout, unless it has a try-out of its own.
    const follow = sel !== null && id === sel.id && id === mstate?.basedOn && changesFrom(sel.loadout.lasers, own) === 0;
    error = (await saveLoadout(id, name, owner, savedForm(draft))) ?? "";
    if (error) return error;
    const made = miningStore.state?.saved.find((s) => s.owner === owner && s.name.toLowerCase() === name.trim().toLowerCase());
    if (made && follow) void useLoadout(made.id);
    if (made) select(made.id);
    return null;
  }
  function startNew() {
    selected = NEW_MINE;
    draft = [];
    draftName = "";
    open = "L0";
    error = "";
  }
  async function duplicate() {
    if (!sel) return;
    const name = `${sel.name} copy`;
    error = (await saveLoadout(null, name, sel.owner, sel.loadout)) ?? "";
    const made = miningStore.state?.saved.find((s) => s.owner === sel.owner && s.name === name);
    if (made) select(made.id);
  }
  async function remove() {
    if (!sel) return;
    await deleteLoadout(sel.id);
    selected = "";
  }
  const onCalc = (s: SavedLoadoutView) => s.id === mstate?.basedOn;

  // ── Measured on every rock (design pass 5) ──
  const all = $derived(miningUi.measure === "all");
  const filter = $derived<ComfortFilter>({ ore: miningUi.measureOre, sys: miningUi.measureSys });
  const rows = $derived(
    miningStore.areas && miningStore.places ? comfortRows(miningStore.areas.areas, miningStore.places) : [],
  );
  /** The ship on its own: no calculator gadget, no range. */
  const shipOf = (lasers: LaserInputView[]): LoadoutView => ({
    lasers: lasers.map((l) => ({ ...l, crew: null, distanceM: null })),
    gadget: null,
    lead: null,
  });
  type Measured = { s: Scores; bins: number[][] };
  function measure(lasers: LaserInputView[]): Measured | null {
    const c = comfortOf(channel, shipOf(lasers));
    return c ? { s: scoresOf(c, rows, filter), bins: binsOf(c, rows, filter) } : null;
  }
  const measured = $derived(
    all ? new Map(mine.map((s) => [s.id, measure(s.loadout.lasers)])) : new Map<string, Measured | null>(),
  );
  const partsChanged = $derived(sel ? changesFrom(sel.loadout.lasers, draft) : 0);
  const savedMeasured = $derived(all && sel ? (measured.get(sel.id) ?? null) : null);
  const drafted = $derived(all && (isNew || partsChanged) && draft.length ? measure(draft) : null);
  // One scale for every chart on the page: they share an outline (the same rocks).
  const scale = $derived.by(() => {
    const any = [...measured.values()].find((m) => m) ?? drafted;
    return any ? topOf(any.bins) : 1;
  });
  let sortBy = $state<"everyday" | "reach">("everyday");
  const sortedMine = $derived(
    all
      ? [...mine].sort((a, b) => {
          const x = measured.get(a.id)?.s;
          const y = measured.get(b.id)?.s;
          if (!x || !y) return x ? -1 : y ? 1 : 0;
          return sortBy === "reach" ? y.reach - x.reach || y.everyday - x.everyday : y.everyday - x.everyday || y.reach - x.reach;
        })
      : mine,
  );
  const empty = $derived([...measured.values()].some((m) => m && m.s.n === 0));

  // The list's solves, the sheet's first; what each saved module adds.
  const without = (i: number, s: number) => withPart(catalog, sel?.loadout.lasers ?? [], `S${i}:${s}`, null);
  $effect(() => {
    // The open one's solve also gives the Measured on list its figures.
    const open = sel ? shipOf(sel.loadout.lasers) : null;
    if (!all) {
      const ch = channel;
      if (open) untrack(() => requestComfort(ch, open));
      return;
    }
    const list = mine.map((s) => shipOf(s.loadout.lasers));
    const urgent = [...(sel ? [shipOf(sel.loadout.lasers)] : []), ...(draft.length ? [shipOf(draft)] : [])];
    const parts = sel ? sel.loadout.lasers.flatMap((l, i) => l.modules.map((_, s) => shipOf(without(i, s)))) : [];
    const ch = channel;
    untrack(() => {
      urgent.forEach((l) => requestComfort(ch, l, true));
      list.forEach((l) => requestComfort(ch, l));
      parts.forEach((l) => requestComfort(ch, l));
    });
  });
  function adds(i: number, s: number): [string, number][] | null {
    const base = savedMeasured;
    const less = measure(without(i, s));
    if (!base || !less) return null;
    // mix: too much power, easy, manageable, hard, can't break
    return [
      ["clean", base.s.everyday - less.s.everyday],
      ["power", less.s.mix[4] - base.s.mix[4]],
      ["control", less.s.mix[3] - base.s.mix[3]],
      ["less overcharge", less.s.mix[0] - base.s.mix[0]],
    ];
  }

  // What to measure on: the calculator's rock, every rock, one ore's.
  const ores = $derived(
    [
      ...new Set(
        (miningStore.areas?.areas ?? []).flatMap((a) =>
          a.rocks.filter((r) => r.rollable).flatMap((r) => r.ores.map((o) => o.element)),
        ),
      ),
    ]
      .map((id) => ({ id, name: oreName(names.get(id) ?? "?") }))
      .sort((a, b) => a.name.localeCompare(b.name)),
  );
  // Each option carries the open loadout's Everyday there (the finder's ore
  // figure), so the list reads as where it does well before picking.
  const figureOf = $derived.by(() => {
    const c = sel ? comfortOf(channel, shipOf(sel.loadout.lasers)) : null;
    if (!c) return (_: string) => ({});
    return (ore: string) => {
      const s = scoresOf(c, rows, { ore, sys: miningUi.measureSys });
      if (!s.n) return {};
      const tone = ({ g: "success", w: "warning", b: "destructive" } as const)[everydayTone(s.everyday)];
      return { meter: s.everyday, detail: String(p100(s.everyday)), tone };
    };
  });
  const measureOptions = $derived<SelectOption[]>([
    { value: "rock", label: "The calculator's rock" },
    { value: "all", label: "Every rock", ...figureOf("") },
    ...ores.map((o) => ({
      value: `ore|${o.id}`,
      label: o.name,
      group: sel ? `One ore · ${sel.name}'s everyday` : "One ore",
      ...figureOf(o.id),
    })),
  ]);
  const measureValue = $derived(!all ? "rock" : miningUi.measureOre ? `ore|${miningUi.measureOre}` : "all");
  function setMeasure(v: string) {
    open = "";
    if (v === "rock") {
      miningUi.measure = "rock";
      return;
    }
    miningUi.measure = "all";
    miningUi.measureOre = v.startsWith("ore|") ? v.slice(4) : "";
  }
  const systemOptions = $derived([
    { value: "all", label: "All systems" },
    ...(miningStore.places ?? []).map((s) => ({ value: s.name, label: s.name })),
  ]);
  const oreLabel = $derived(miningUi.measureOre ? oreName(names.get(miningUi.measureOre) ?? "?") : "");
  const scope = $derived(
    `${oreLabel ? `${oreLabel} rocks` : "every rock"}${miningUi.measureSys === "all" ? "" : ` in ${miningUi.measureSys}`}`,
  );

  // This session's try-outs of the selected loadout.
  $effect(() => {
    if (!all || !sel || !partsChanged) return;
    const lasers = $state.snapshot(draft) as LaserInputView[];
    const id = sel.id;
    untrack(() => {
      const list = miningUi.tries[id] ?? [];
      const key = JSON.stringify(lasers);
      if (!list.some((l) => JSON.stringify(l) === key)) miningUi.tries[id] = [...list, lasers].slice(-12);
    });
  });
  const tries = $derived(all && sel ? (miningUi.tries[sel.id] ?? []) : []);
  const keyOf = (l: LaserInputView[]) => JSON.stringify(l);
  function keep() {
    const k = keyOf($state.snapshot(draft) as LaserInputView[]);
    miningUi.kept = miningUi.kept.includes(k) ? miningUi.kept.filter((x) => x !== k) : [...miningUi.kept, k];
  }

  /** Where a waiting solve stands: measuring now, or queued. */
  const waiting = (lasers: LaserInputView[]) =>
    miningStore.comfortRunning === comfortKey(channel, shipOf(lasers)) ? "measuring…" : "queued";
  const p100 = (v: number) => Math.round(v * 100);
  const sgn = (x: number) => {
    const r = p100(x);
    return `${r > 0 ? "+" : r < 0 ? "−" : "±"}${Math.abs(r)}`;
  };
  const dTone = (x: number) => (p100(x) > 0 ? "up" : p100(x) < 0 ? "dn" : "z");
  const everydayTone = (v: number) => (v >= 0.65 ? "g" : v >= 0.4 ? "w" : "b");
  const reachTone = (v: number) => (v >= 0.995 ? "g" : v >= 0.9 ? "w" : "b");
</script>

{#snippet fig(a: number | undefined, b: number | undefined, percent: boolean)}
  {@const v = b ?? a}
  {#if v === undefined}
    <span class="n5 z">…</span>
  {:else}
    <span class="n5 {percent ? reachTone(v) : everydayTone(v)}"
      ><span>{p100(v)}{#if percent}<small>%</small>{/if}</span>{#if a !== undefined && b !== undefined && p100(b) !== p100(a)}<small
          class={dTone(b - a)}>{sgn(b - a)}</small
        >{/if}</span
    >
  {/if}
{/snippet}

{#snippet allRow(s: SavedLoadoutView)}
  {@const m = measured.get(s.id) ?? null}
  {@const tried = s.id === selected && partsChanged ? drafted : null}
  {@const shown = tried ?? m}
  <div class="arow" class:sel={s.id === selected}>
    <button type="button" class="nmb" onclick={() => select(s.id)}>
      <b>{s.name}</b>{#if onCalc(s)}<span class="on">on the calculator</span>{/if}{#if tried}<span class="try">try-out</span>{/if}
    </button>
    {@render fig(m?.s.everyday, tried?.s.everyday, false)}
    {@render fig(m?.s.reach, tried?.s.reach, true)}
    <span class="zone">
      {#if shown}<ComfortZone bins={shown.bins} top={scale} height={38} />{:else}<Pending height={38} label={waiting(s.loadout.lasers)} />{/if}
    </span>
  </div>
{/snippet}

{#snippet num(label: string, a: number | undefined, b: number | undefined, percent: boolean)}
  {@const v = b ?? a}
  <span class="nu">
    <small>{label}</small>
    {#if v === undefined}<b class="z">…</b>{:else}<b class={percent ? reachTone(v) : everydayTone(v)}>{p100(v)}{percent ? "%" : ""}</b
      >{#if a !== undefined && b !== undefined && p100(b) !== p100(a)}<i class={dTone(b - a)}>{sgn(b - a)}</i><em
          >saved {p100(a)}{percent ? "%" : ""}</em
        >{/if}{/if}
  </span>
{/snippet}

{#snippet tryFigure()}
  <div class="nums">
    {@render num("Everyday", savedMeasured?.s.everyday, drafted?.s.everyday, false)}
    {@render num("Reach", savedMeasured?.s.reach, drafted?.s.reach, true)}
  </div>
  {#if savedMeasured && drafted}
    <div class="mi">
      <span class="ml"><b>try-out</b><b>saved</b></span>
      <ComfortZone bins={drafted.bins} mirror={savedMeasured.bins} top={scale} height={50} axis label="Try-out above the line, saved below" />
    </div>
    <ClassLegend />
  {:else}
    <Pending height={101} label="{waiting(draft)}: the try-out on {scope}" />
  {/if}
{/snippet}

<!-- The part overlay's two views: every option measured on the rocks you'd
     meet, or the picker's stat table (what each changes on the laser). -->
{#snippet views()}
  <Segmented
    options={[
      { value: "measured", label: "Measured" },
      { value: "stats", label: "Stats" },
    ]}
    value={miningUi.partView}
    onchange={(v) => (miningUi.partView = v as typeof miningUi.partView)}
  />
{/snippet}

{#snippet slotNote(i: number, s: number)}
  {@const items = adds(i, s)}
  {#if items}
    <span class="adds">{#each items as [l, x] (l)}<span><b class={dTone(x)}>{sgn(x)}</b> {l}</span>{/each}</span>
  {/if}
{/snippet}

{#snippet row(s: SavedLoadoutView)}
  {@const v = verdict(rowEvals === undefined ? undefined : (rowEvals?.get(s.id) ?? null))}
  <button type="button" class="row" class:sel={s.id === selected} onclick={() => select(s.id)}>
    <Monogram name={s.name} hue="var(--hue-sky)" size={30} />
    <span class="nm">
      <b>{s.name}{#if onCalc(s)}<span class="on">on the calculator</span>{/if}</b>
      <small class:bad={v.tone === "b"}>{v.text}</small>
    </span>
    <CrackCell value={rowCrack(s)} adds={false} />
  </button>
{/snippet}

<div class="lo">
  <div class="list">
    {#if open && all && miningUi.partView === "measured"}
      <div class="over">
        <PartSweep
          {views}
          {catalog}
          {channel}
          target={open}
          lasers={draft}
          loadoutOf={shipOf}
          {rows}
          {filter}
          top={scale}
          onpick={(next) => {
            open = "";
            draft = next;
          }}
          onclose={() => (open = "")}
        />
      </div>
    {:else if open}
      <div class="over">
        <PartPicker
          views={all ? views : undefined}
          {catalog}
          {channel}
          target={open}
          lasers={draft}
          {build}
          minus={null}
          onpick={(next) => {
            open = "";
            draft = next;
          }}
          onclose={() => (open = "")}
        />
      </div>
    {/if}
    <section>
      <div class="gh">
        <p class="label">Yours</p>
        <span class="mo">
          <span class="label">Measured on</span>
          <Select label="Measured on" options={measureOptions} value={measureValue} onchange={setMeasure} />
          {#if all}<Select label="System" options={systemOptions} value={miningUi.measureSys} onchange={(v) => (miningUi.measureSys = v)} />{/if}
        </span>
        <span class="grow"></span>
        <button type="button" class="btn" onclick={startNew}>+ Loadout</button>
      </div>
      {#if all}
        <div class="arow h">
          <span class="srt">{mine.length} loadouts</span>
          <!-- The figures' headers sort the list (best first). -->
          {#each [["everyday", "Everyday", "Easy or manageable among the rocks you'd meet, per 100"], ["reach", "Reach", "How much of what spawns it breaks at all"]] as const as [k, label, hint] (k)}
            <button
              type="button"
              class="hh sort"
              class:on={sortBy === k}
              aria-pressed={sortBy === k}
              title="{hint} · sort by it"
              onclick={() => (sortBy = k)}>{label}<ArrowDown size={11} strokeWidth={2.25} /></button
            >
          {/each}
          <span class="hh zh" title="Comfort zone: every rock you'd meet by mass, coloured by the class it has">
            {#each MASS_SCALE.ticks as [m, l] (m)}<span style:left="{massAt(m)}%">{l}</span>{/each}
          </span>
        </div>
        {#if empty}<p class="faint none">No {oreLabel} rocks in {miningUi.measureSys}.</p>{/if}
        {#each sortedMine as s (s.id)}{@render allRow(s)}{:else}<p class="faint none">None yet.</p>{/each}
        {#if sortedMine.length}<div class="lg"><ClassLegend /></div>{/if}
      {:else}
        <small class="faint sub">{rock ? `crack on the calculator's rock: ${rock}` : "scan or roll a rock to compare them"}</small>
        {#each mine as s (s.id)}{@render row(s)}{:else}<p class="faint none">None yet.</p>{/each}
      {/if}
    </section>
  </div>

  {#if sel || isNew}
    <div class="sheet">
      <NowCard hue="var(--hue-sky)" chip="● YOURS" version="">
        {#snippet top()}{#if sel && onCalc(sel)}<span class="on">on the calculator</span>{/if}{/snippet}
        <div class="nf">
          <input class="nmi" bind:value={draftName} placeholder="Name it" aria-label="Loadout name" />
          {#if changes && !isNew}<span class="mod">modified</span>{/if}
        </div>
        {#if all}
          <small class="faint scope">on {scope}</small>
        {/if}
        {#if sel && changes}
          <TryOut
            was={wasEval}
            now={nowEval}
            {changes}
            minus={null}
            name={`${draftName} 2`}
            onrevert={() => select(sel.id)}
            onsave={() => void save(sel.id, draftName)}
            onsaveas={(n) => save(null, n)}
            figure={all && partsChanged ? tryFigure : undefined}
          />
        {:else if all}
          {@const m = sel ? savedMeasured : drafted}
          <div class="measured">
            {#if draft.length}
              <div class="nums">
                {@render num("Everyday", m?.s.everyday, undefined, false)}
                {@render num("Reach", m?.s.reach, undefined, true)}
              </div>
              {#if m}
                <ComfortZone bins={m.bins} top={scale} height={60} axis />
                <ClassLegend />
              {:else}
                <Pending height={60} label="{waiting(sel?.loadout.lasers ?? draft)} on {scope}" />
              {/if}
            {:else}
              <p class="pending">Pick a laser to measure it on {scope}.</p>
            {/if}
          </div>
        {/if}
        <HeadsEditor
          {catalog}
          lasers={draft}
          base={sel?.loadout.lasers ?? null}
          {open}
          onopen={(k) => (open = open === k ? "" : k)}
          onchange={(next) => (draft = next)}
          structure
          slotNote={all && sel && !partsChanged ? slotNote : undefined}
        />
        {#if tries.length}
          <div class="tried">
            <div class="th">
              <p class="label">Tried this session</p>
              <span class="grow"></span>
              {#if partsChanged}<button type="button" class="btn subtle" onclick={keep}
                  >{miningUi.kept.includes(keyOf($state.snapshot(draft) as LaserInputView[])) ? "★ kept" : "☆ keep"}</button
                >{/if}
            </div>
            {#each [sel?.loadout.lasers ?? [], ...tries] as t, k (k)}
              {@const tm = measure(t)}
              {@const here = keyOf($state.snapshot(draft) as LaserInputView[]) === keyOf(t)}
              <button type="button" class="tr" class:here onclick={() => (draft = structuredClone(t))}>
                <span class="tn">{miningUi.kept.includes(keyOf(t)) ? "★ " : ""}{k === 0 ? "as saved" : changeLabel(catalog, sel?.loadout.lasers ?? [], t)}</span>
                <span class="tv">{tm ? `${p100(tm.s.everyday)} · ${p100(tm.s.reach)}%` : "…"}</span>
                {#if tm}<ComfortZone bins={tm.bins} top={scale} height={16} />{:else}<span></span>{/if}
              </button>
            {/each}
          </div>
        {/if}
        {#if error}<p class="error">{error}</p>{/if}
        {#snippet action()}
          {#if isNew}
            <button type="button" class="btn subtle" onclick={() => (selected = "")}>Cancel</button>
            <button type="button" class="btn primary" disabled={!draftName.trim() || !draft.length} onclick={() => void save(null, draftName)}>Save</button>
          {:else if sel}
            <button type="button" class="btn subtle danger" onclick={() => void remove()}>Delete</button>
            <button type="button" class="btn" onclick={() => void duplicate()}>Duplicate</button>
            <span class="grow"></span>
            {#if onCalc(sel)}
              <button type="button" class="btn" onclick={() => (miningUi.view = "calc")}>Open the calculator</button>
            {:else}
              <button type="button" class="btn primary" onclick={() => void useLoadout(sel.id)}>Use on the calculator</button>
            {/if}
          {/if}
        {/snippet}
      </NowCard>
    </div>
  {/if}
</div>

<style>
  /* The rows reach 10px past their column (their content lines up with the
     headers); the inset keeps that off the page's scrollbar. */
  .lo {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 430px;
    gap: 18px;
    align-items: start;
    padding-inline: 10px;
  }
  .list {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 20px;
    min-width: 0;
  }
  .over {
    position: absolute;
    inset: 0 0 auto 0;
    z-index: 4;
  }
  section {
    display: flex;
    flex-direction: column;
  }
  .gh {
    display: flex;
    align-items: baseline;
    gap: 10px;
    margin-bottom: 6px;
  }
  .gh .label {
    margin: 0;
  }
  .gh small {
    font-size: 0.78rem;
  }
  .gh .btn {
    padding: 3px 10px;
  }
  .grow {
    flex: 1;
  }
  .faint {
    color: var(--text-faint);
  }
  .none {
    margin: 4px 0;
    font-size: var(--text-small);
  }
  .row {
    display: grid;
    grid-template-columns: 30px minmax(0, 1fr) 220px;
    gap: 0 12px;
    align-items: center;
    padding: 8px 10px;
    margin-inline: -10px;
    border: 0;
    border-top: 1px solid color-mix(in srgb, var(--text) 7%, transparent);
    border-radius: 8px;
    background: none;
    font: inherit;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .row:hover {
    background: color-mix(in srgb, var(--text) 4%, transparent);
  }
  .row.sel {
    border-top-color: transparent;
    background: var(--accent-fill-weak);
    box-shadow: inset 0 0 0 1px var(--accent-border);
  }
  .nm {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .nm b {
    display: flex;
    align-items: center;
    gap: 6px;
    font-weight: var(--weight-semibold);
  }
  .nm small {
    font-size: 0.76rem;
    color: var(--text-faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .nm small.bad {
    color: var(--bad);
  }
  .on {
    margin-left: 4px;
    padding: 1px 7px;
    border-radius: var(--radius-pill);
    font-family: var(--font-mono);
    font-size: 0.6rem;
    font-weight: var(--weight-medium);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--accent) 85%, var(--text));
    border: 1px solid var(--accent-border);
  }
  .mo {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    margin-left: 6px;
  }
  .mo .label {
    margin: 0;
    white-space: nowrap;
  }
  .sub {
    margin: -2px 0 4px;
    font-size: 0.78rem;
  }
  .arow {
    display: grid;
    grid-template-columns: minmax(8rem, 13rem) 54px 60px minmax(0, 1fr);
    gap: 0 12px;
    align-items: center;
    padding: 9px 10px;
    margin-inline: -10px;
    border-top: 1px solid color-mix(in srgb, var(--text) 7%, transparent);
    border-radius: 8px;
  }
  /* The whole row opens the loadout: the name's button reaches over it. */
  .arow:not(.h) {
    position: relative;
  }
  .arow:not(.h, .sel):hover {
    background: color-mix(in srgb, var(--text) 4%, transparent);
  }
  .nmb::after {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: inherit;
  }
  .arow.h {
    border-top: 0;
    padding-block: 0 4px;
    font-size: 0.72rem;
    color: var(--text-faint);
  }
  .arow.sel {
    border-top-color: transparent;
    background: var(--accent-fill-weak);
    box-shadow: inset 0 0 0 1px var(--accent-border);
  }
  .hh {
    font-family: var(--font-mono);
    font-size: 0.66rem;
    letter-spacing: var(--label-tracking);
    text-transform: var(--label-transform);
    text-align: right;
    white-space: nowrap;
  }
  .hh.sort {
    display: inline-flex;
    align-items: center;
    justify-content: flex-end;
    gap: 3px;
    padding: 2px 0;
    border: 0;
    background: none;
    color: var(--text-faint);
    cursor: pointer;
  }
  .hh.sort :global(svg) {
    opacity: 0;
    transition: opacity var(--transition-fast);
  }
  .hh.sort:hover {
    color: var(--text);
  }
  .hh.sort:hover :global(svg) {
    opacity: 0.5;
  }
  .hh.sort.on {
    color: var(--text);
  }
  .hh.sort.on :global(svg) {
    opacity: 1;
    color: var(--accent);
  }
  /* The comfort zone's mass scale, over the charts below it. */
  .hh.zh {
    position: relative;
    height: 1.3em;
    letter-spacing: 0;
    text-transform: none;
  }
  .zh span {
    position: absolute;
    transform: translateX(-50%);
  }
  .srt {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }
  /* The name, its marks below it when they don't fit beside it. */
  .nmb {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 2px 6px;
    min-width: 0;
    padding: 0;
    border: 0;
    background: none;
    font: inherit;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .nmb b {
    min-width: 0;
    font-weight: var(--weight-semibold);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .lg {
    padding-top: 8px;
  }
  .try {
    font-family: var(--font-mono);
    font-size: 0.62rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--warn);
  }
  .n5 {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    font-family: var(--font-mono);
    font-size: 0.95rem;
    font-weight: var(--weight-bold);
    font-variant-numeric: tabular-nums;
    line-height: 1.15;
    white-space: nowrap;
  }
  .n5 small {
    font-size: 0.68rem;
    font-weight: var(--weight-semibold);
  }
  .n5 span small {
    color: inherit;
    opacity: 0.8;
  }
  .g {
    color: var(--good);
  }
  .w {
    color: var(--warn);
  }
  .b {
    color: var(--bad);
  }
  .z {
    color: var(--text-faint);
  }
  .up {
    color: var(--good);
  }
  .dn {
    color: var(--bad);
  }
  .zone {
    min-width: 0;
  }
  .pending {
    margin: 0;
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .scope {
    margin-top: -8px;
    font-size: var(--text-small);
  }
  .measured {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .nums {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 22px;
  }
  .nu {
    display: flex;
    align-items: baseline;
    gap: 6px;
    font-family: var(--font-mono);
  }
  .nu small {
    font-size: 0.68rem;
    letter-spacing: var(--label-tracking);
    text-transform: var(--label-transform);
    color: var(--text-faint);
  }
  .nu b {
    font-size: 1.35rem;
    font-weight: var(--weight-bold);
  }
  .nu i {
    font-style: normal;
    font-size: 0.8rem;
    font-weight: var(--weight-bold);
  }
  .nu em {
    font-style: normal;
    font-size: 0.72rem;
    color: var(--text-faint);
  }
  .mi {
    display: grid;
    grid-template-columns: 52px minmax(0, 1fr);
    gap: 8px;
  }
  .ml {
    display: flex;
    flex-direction: column;
    justify-content: space-around;
    padding-bottom: 14px;
    font-size: 0.7rem;
    text-align: right;
    color: var(--text-dim);
  }
  .ml b {
    font-weight: var(--weight-semibold);
  }
  .adds {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 10px;
    margin-top: 5px;
    padding-top: 5px;
    border-top: 1px dashed color-mix(in srgb, var(--text) 10%, transparent);
    font-size: 0.7rem;
    font-weight: var(--weight-normal);
    color: var(--text-faint);
  }
  .adds b {
    font-family: var(--font-mono);
    font-size: 0.72rem;
  }
  .tried {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .th {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .th .label {
    margin: 0;
  }
  .tr {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto 120px;
    gap: 10px;
    align-items: center;
    padding: 4px 6px;
    margin-inline: -6px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    font: inherit;
    font-size: 0.8rem;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .tr:hover {
    background: color-mix(in srgb, var(--text) 5%, transparent);
  }
  .tr.here {
    background: var(--accent-fill-weak);
  }
  .tn {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tv {
    font-family: var(--font-mono);
    font-size: 0.76rem;
    color: var(--text-dim);
  }
  .sheet {
    position: sticky;
    top: 12px;
    display: flex;
  }
  .nf {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px 10px;
    margin-top: -4px;
  }
  .nf input {
    font: inherit;
    color: var(--text);
    background: transparent;
    border: 1px solid transparent;
    border-radius: 6px;
    padding: 2px 6px;
    margin-left: -6px;
  }
  .nf input:hover,
  .nf input:focus {
    border-color: var(--border);
    background: var(--bg);
    outline: none;
  }
  .nf .nmi {
    width: 100%;
    max-width: 300px;
    font-family: var(--font-display);
    font-weight: var(--weight-bold);
    font-size: 2rem;
    line-height: 1.1;
    color: color-mix(in srgb, var(--hue) 55%, var(--text-strong));
  }
  .mod {
    font-family: var(--font-mono);
    font-size: 0.78rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--warn);
  }
  .danger {
    color: var(--bad);
  }
  .error {
    margin: 0;
  }
  @media (max-width: 1100px) {
    .lo {
      grid-template-columns: minmax(0, 1fr);
    }
    .sheet {
      position: static;
    }
  }
</style>
