<script lang="ts">
  // Plans: the goals you set and what is left to do for them (design pass 5
  // on the Starlume Design canvas; decision 'One unified plan', 2026-10-21).
  // The page leads with actions: the plan in view with its stages, a next
  // session to fly, then the open steps. Every step, stage and figure is
  // derived Rust-side from what the account holds; nothing is ticked here.
  // Which plan is in view and which session choice is open are page-local.
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { Button, ConfirmDialog, Dialog, Picker, Segmented } from "@veelume/ui";
  import { ArrowUpRight, Plus } from "lucide-svelte";
  import type { BlueprintRowView, PlanBoard, StepKind, Trip } from "$lib/bindings";
  import NoGameData from "$lib/components/catalog/NoGameData.svelte";
  import NowCard from "$lib/components/NowCard.svelte";
  import PlanGlyph, { type GlyphShape } from "$lib/modules/plans/PlanGlyph.svelte";
  import {
    CHOICES,
    KIND_TEXT,
    TIMELINE_ROWS,
    ago,
    amount,
    ledgerBars,
    stepFigure,
    stopsText,
    timeText,
    timeline,
  } from "$lib/modules/plans/model";
  import { getBlueprints } from "$lib/state/catalog.svelte";
  import { craftingStore, loadMarks } from "$lib/state/crafting.svelte";
  import { dataStore, ensureChannel } from "$lib/state/data.svelte";
  import {
    addCraftPlan,
    addLoadoutPlan,
    focusPlan,
    loadBoard,
    loadChoices,
    loadPlans,
    plansStore,
    removePlan,
    setCraftPlan,
    watchPlans,
  } from "$lib/state/plans.svelte";
  import { openShip, shipsUi } from "$lib/state/ships.svelte";

  let ready = $state(false);
  let now = $state(Date.now());
  let tab = $state<"active" | "done">("active");
  let picked = $state<string | null>(null);
  let choice = $state<Trip["choice"]>("most");
  let adding = $state(false);
  let removing = $state<PlanBoard | null>(null);
  let blueprints = $state<BlueprintRowView[]>([]);

  const channel = $derived(dataStore.channel);
  const board = $derived(plansStore.board(channel));

  onMount(() => {
    const stop = watchPlans(() => dataStore.channel);
    const tick = setInterval(() => (now = Date.now()), 60_000);
    void (async () => {
      await Promise.all([ensureChannel(), loadPlans(), loadMarks()]);
      ready = true;
    })();
    return () => {
      stop();
      clearInterval(tick);
    };
  });

  // A channel appears during a cook, and changes with the picker.
  $effect(() => {
    const c = channel;
    if (!c) return;
    void loadBoard(c);
    void loadChoices(c);
    void getBlueprints(c).then((rows) => {
      if (typeof rows !== "string") blueprints = rows;
    });
  });

  const shown = $derived((board?.plans ?? []).filter((p) => (tab === "done" ? p.done : !p.done)));
  const plan = $derived(shown.find((p) => p.id === picked) ?? shown[0] ?? null);
  const target = $derived(board?.targets.find((t) => t.plan === plan?.id) ?? null);
  const goal = $derived(plansStore.plans?.find((p) => p.id === plan?.id)?.goal ?? null);
  const isFocus = $derived(!!plan && board?.plans[0]?.id === plan.id);
  const steps = $derived((board?.steps ?? []).filter((s) => plan && s.plans.includes(plan.id)));
  const session = $derived(board?.sessions.find((s) => s.choice === choice) ?? null);
  const rows = $derived(session ? timeline(session) : []);
  const bars = $derived(ledgerBars(board?.ledger ?? []));
  const names = $derived(new Map((board?.plans ?? []).map((p) => [p.id, p.name])));
  const counts = $derived({
    active: (board?.plans ?? []).filter((p) => !p.done).length,
    done: (board?.plans ?? []).filter((p) => p.done).length,
  });

  const PLAN_LOOK: Record<PlanBoard["kind"], { shape: GlyphShape; chip: string }> = {
    loadout: { shape: "ship", chip: "LOADOUT" },
    craft: { shape: "craft", chip: "CRAFT PROJECT" },
    unknown: { shape: "blueprint", chip: "PLAN" },
  };
  const STEP_SHAPE: Record<StepKind, GlyphShape> = {
    fit: "crate",
    buy: "crate",
    mine: "mine",
    gather: "mine",
    craft: "craft",
    earn: "mission",
    find: "find",
  };

  // What "New plan" offers: your saved loadouts and the items you marked
  // as wanted on Crafting, minus what is already planned.
  type Offer = { key: string; label: string; detail: string; add: () => Promise<boolean> };
  const offers = $derived.by<Offer[]>(() => {
    const loadouts = plansStore.choices
      .filter((c) => !plansStore.loadoutOf(c.id))
      .map((c) => ({
        key: `loadout:${c.id}`,
        label: `${c.ship}, ${c.name}`,
        detail: "Saved loadout, to have fitted on the ship",
        add: () => addLoadoutPlan(c.id),
      }));
    const wanted = blueprints
      .filter((b) => craftingStore.wantsItem(b.guid) && !plansStore.craftOf(b.guid))
      .map((b) => ({
        key: `craft:${b.guid}`,
        label: b.name,
        detail: "Wanted item, to craft",
        add: () => addCraftPlan(b.guid),
      }));
    return [...loadouts, ...wanted];
  });

  async function add(offer: Offer) {
    adding = false;
    if (!(await offer.add())) return;
    const [kind, id] = offer.key.split(/:(.*)/s);
    tab = "active";
    picked = (kind === "loadout" ? plansStore.loadoutOf(id) : plansStore.craftOf(id))?.id ?? null;
  }

  function open() {
    if (target?.ship && target.loadout) {
      openShip(target.ship);
      shipsUi.wanted = { ship: target.ship, id: target.loadout };
      void goto("/ships?tab=build");
    } else if (target?.family) {
      void goto(`/crafting?sel=${encodeURIComponent(target.family)}`);
    }
  }

  function retarget(field: "count" | "quality", raw: string) {
    if (!plan || goal?.kind !== "craft") return;
    const value = Math.round(Number(raw));
    if (!Number.isFinite(value)) return;
    const count = field === "count" ? Math.max(1, value) : goal.count;
    const quality = field === "quality" ? Math.min(1000, Math.max(0, value)) : goal.quality;
    void setCraftPlan(plan.id, count, quality);
  }

  async function remove() {
    const gone = removing;
    removing = null;
    if (gone) await removePlan(gone.id);
  }
</script>

<div class="plans">
  <header class="bar">
    <Segmented
      options={[
        { value: "active", label: board ? `Active ${counts.active}` : "Active" },
        { value: "done", label: board ? `Done ${counts.done}` : "Done" },
      ]}
      value={tab}
      onchange={(v) => (tab = v as typeof tab)}
    />
    <span class="grow"></span>
    {#if board}
      {#if board.inventoryAt != null}
        <span class="dim small">Read from the game {ago(board.inventoryAt, now)}</span>
      {:else}
        <a class="dim small" href="/items">Your inventory was never read, so nothing you hold is counted</a>
      {/if}
    {/if}
    <Button onclick={() => (adding = true)}><Plus size={15} /> New plan</Button>
  </header>

  {#if ready && !channel}
    <NoGameData />
  {:else if board}
    {#if board.plans.length === 0}
      <section class="empty card">
        <h2>No plans yet</h2>
        <p class="dim">
          A plan is a goal: a saved loadout to have fitted, or an item to have crafted. The steps come from what you
          hold in the game, so there is nothing to tick off.
        </p>
        <div class="empty-actions">
          <Button onclick={() => (adding = true)}><Plus size={15} /> New plan</Button>
          <Button variant="outline" href="/ships">Save a loadout on Ships</Button>
          <Button variant="outline" href="/crafting">Pick a recipe on Crafting</Button>
        </div>
      </section>
    {:else}
      <div class="now-row">
        {#if plan}
          {@const look = PLAN_LOOK[plan.kind]}
          <NowCard
            size="wide"
            hue={plan.kind === "craft" ? "var(--hue-rose)" : "var(--hue-sky)"}
            chip={isFocus && !plan.done ? "FOCUS" : look.chip}
            version={plan.title}
            name={plan.subtitle}
            date={plan.done ? "done" : plan.waiting ? "waiting" : isFocus ? look.chip.toLowerCase() : ""}
          >
            {#snippet art()}<PlanGlyph shape={look.shape} large />{/snippet}
            {#if plan.missing}
              <p class="dim">{plan.note}. Remove the plan, or bring its goal back.</p>
            {:else}
              <ol class="stepper" aria-label="Stages" style:--n={plan.stages.length}>
                {#each plan.stages as s (s.name)}
                  <li class={s.state} aria-current={s.state === "current" ? "step" : undefined}>
                    <i></i><span>{s.name}<small>{s.sub}</small></span>
                  </li>
                {/each}
              </ol>
              <div class="stat-tiles">
                {#each plan.tiles as t (t.label)}
                  <div class="stat-tile" title="{t.label}: {t.value}"><b>{t.value}</b><span>{t.label}</span></div>
                {/each}
              </div>
            {/if}
            {#snippet action()}
              {#if goal?.kind === "craft" && !plan.done}
                <label class="field"
                  >Count
                  <input
                    class="input"
                    type="number"
                    min="1"
                    value={goal.count}
                    onchange={(e) => retarget("count", e.currentTarget.value)}
                  /></label
                >
                <label class="field"
                  >Quality
                  <input
                    class="input"
                    type="number"
                    min="0"
                    max="1000"
                    step="50"
                    value={goal.quality}
                    onchange={(e) => retarget("quality", e.currentTarget.value)}
                  /></label
                >
              {/if}
              <span class="grow"></span>
              <Button variant="ghost" onclick={() => (removing = plan)}>Remove</Button>
              {#if !isFocus && !plan.done && !plan.missing}
                <Button variant="outline" onclick={() => void focusPlan(plan.id)}>Make it the focus</Button>
              {/if}
              {#if target?.ship || target?.family}
                <Button variant="outline" onclick={open}>
                  {target.ship ? "Open ship" : "Open recipe"} <ArrowUpRight size={14} />
                </Button>
              {/if}
            {/snippet}
          </NowCard>
        {:else}
          <NowCard size="wide" version={tab === "done" ? "Nothing finished yet" : "Everything is done"} muted>
            <p class="dim">
              {tab === "done"
                ? "A plan moves here once the game shows its goal reached."
                : "Every plan has reached its goal. Add a new one, or look at Done."}
            </p>
          </NowCard>
        {/if}

        <NowCard
          hue="var(--hue-quality)"
          chip="NEXT SESSION"
          version={session ? stopsText(session) : "No trip"}
          name={session ? timeText(session) : ""}
          date={board.from ? `from ${board.from}` : "last place not known"}
        >
          <div class="pills" role="group" aria-label="What the session should go for">
            {#each CHOICES as c (c.id)}
              <button type="button" class="pill" class:active={choice === c.id} aria-pressed={choice === c.id} onclick={() => (choice = c.id)}
                >{c.label}</button
              >
            {/each}
          </div>
          <p class="why">{session?.why ?? ""}</p>
          <ol class="tl" style:--rows={TIMELINE_ROWS}>
            {#each rows as row (row.key)}
              {#if row.type === "leg"}
                <li class="leg"><span class="t">{row.time}</span><span class="dot"></span><span class="dim">{row.text}</span></li>
              {:else}
                <li class="stop" title={row.full}>
                  <span class="t"></span><span class="dot"></span><span class="what"><b>{row.place}</b> {row.text}</span>
                </li>
              {/if}
            {/each}
          </ol>
          <p class="drive dim small">
            {board.driveShip
              ? `Quantum times are for the ${board.driveShip}, the ship of your first loadout plan.`
              : "No loadout plan names a ship, so jumps carry distances only."}
          </p>
        </NowCard>
      </div>

      <div class="cols">
        <section class="steps" aria-label="Next steps">
          <h2 class="sec-h">Next steps{#if plan}<span class="for">for {plan.name}</span>{/if}</h2>
          {#if steps.length === 0}
            <p class="dim none">{plan ? (plan.missing ? plan.note : "Nothing left to do: the game shows this goal reached.") : ""}</p>
          {:else}
            <div class="rows">
              {#each steps as s (s.id)}
                <div class="step k-{s.kind}" class:blocked={s.blocked}>
                  <PlanGlyph shape={STEP_SHAPE[s.kind]} />
                  <span class="text">
                    <span class="step-title">{s.title}</span>
                    <span class="step-sub">
                      <span class="hue-chip" class:wait={s.blocked}>{s.blocked ? "blocked" : KIND_TEXT[s.kind]}</span>
                      <span>{s.detail}</span>
                      {#if s.plans.length > 1}
                        {#each s.plans as p (p)}<span class="tag">{names.get(p) ?? p}</span>{/each}
                      {/if}
                    </span>
                  </span>
                  <span class="pright"><span class="mono">{stepFigure(s)}</span><small>{s.at?.name ?? ""}</small></span>
                </div>
              {/each}
            </div>
          {/if}
        </section>

        <aside class="sheet" aria-label="Your plans">
          <section class="panel">
            <h2 class="sec-h">{tab === "done" ? "Done" : "Your plans"}<span class="grow"></span><span class="mono">{shown.length}</span></h2>
            <div class="plan-list">
              {#each shown as p (p.id)}
                <button type="button" class="plan-row kind-{p.kind}" class:sel={p.id === plan?.id} aria-pressed={p.id === plan?.id} onclick={() => (picked = p.id)}>
                  <PlanGlyph shape={PLAN_LOOK[p.kind].shape} />
                  <span class="plan-text">
                    <span class="plan-head"><b>{p.name}</b><span class="mono dim">{Math.round(p.progress * 100)}%</span></span>
                    <span class="track"><i style:width="{Math.round(p.progress * 100)}%"></i></span>
                    <span class="plan-note" class:waits={p.waiting}>{p.note}</span>
                  </span>
                </button>
              {/each}
            </div>
          </section>

          <section class="panel">
            <h2 class="sec-h">Materials across plans</h2>
            {#if board.ledger.length === 0}
              <p class="dim none">No open plan needs materials.</p>
            {/if}
            {#each board.ledger as m, i (`${m.material}:${m.minQuality}:${m.bulk}`)}
              <div class="mat" title={m.plans.map((p) => names.get(p) ?? p).join("\n")}>
                <span class="mat-name"><span>{m.material}</span><span class="dim mono">{m.minQuality > 1 ? `Q${m.minQuality}+` : "any quality"}</span></span>
                <span class="need"><i class="wanted" style:width="{bars[i].need}%"></i><i class="held" style:width="{bars[i].held}%"></i></span>
                <span class="mat-figure mono"><span class={bars[i].state}>{amount(m, m.held)}</span><span class="unit">/ {amount(m, m.need)} {m.bulk ? "SCU" : "pcs"}</span></span>
              </div>
            {/each}
          </section>
        </aside>
      </div>
    {/if}
  {:else if plansStore.error}
    <p class="error">{plansStore.error}</p>
  {:else if ready}
    <NoGameData waiting />
  {/if}
</div>

<Dialog open={adding} onclose={() => (adding = false)} title="New plan" class="max-w-lg!">
  <div class="new">
    {#if offers.length}
      <Picker items={offers} key={(o) => o.key} label={(o) => o.label} detail={(o) => o.detail} onpick={([o]) => void add(o)} class="pick" />
    {:else}
      <p class="dim new-none">
        Nothing to plan yet. A plan starts from a loadout you saved on Ships, or from an item you marked as wanted on
        Crafting.
      </p>
    {/if}
    <div class="new-foot">
      <Button variant="outline" href="/ships">Save a loadout on Ships</Button>
      <Button variant="outline" href="/crafting">Pick a recipe on Crafting</Button>
    </div>
  </div>
</Dialog>

<ConfirmDialog
  open={!!removing}
  onclose={() => (removing = null)}
  onconfirm={() => void remove()}
  title="Remove this plan?"
  description={removing ? `${removing.name} leaves your plans. Nothing you own or saved elsewhere changes.` : ""}
  confirmLabel="Remove"
  destructive
/>

<style>
  .plans {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 36px;
  }
  .grow {
    flex: 1;
  }
  .small {
    font-size: var(--text-small);
  }

  /* One hue a kind (the session page's idiom): role names here, the hue
   * tokens stay app.css's. */
  .k-fit,
  .k-buy {
    --hue: var(--hue-apricot);
  }
  .k-mine,
  .k-gather {
    --hue: var(--hue-quality);
  }
  .k-craft,
  .kind-craft {
    --hue: var(--hue-rose);
  }
  .k-earn,
  .k-find {
    --hue: var(--hue-sand);
  }
  .kind-loadout,
  .kind-unknown {
    --hue: var(--hue-sky);
  }

  .now-row {
    display: flex;
    flex-wrap: wrap;
    gap: 14px;
    align-items: stretch;
  }

  .stepper {
    display: grid;
    grid-template-columns: repeat(var(--n, 4), minmax(0, 1fr));
    max-width: 680px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .stepper li {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 5px;
    font-size: var(--text-small);
    color: var(--text-faint);
    text-align: center;
  }
  .stepper li::before {
    content: "";
    position: absolute;
    top: 5px;
    right: 50%;
    width: 100%;
    height: 2px;
    background: var(--border);
  }
  .stepper li:first-child::before {
    display: none;
  }
  .stepper li.done::before,
  .stepper li.current::before {
    background: color-mix(in srgb, var(--hue) 55%, var(--border));
  }
  .stepper i {
    position: relative;
    /* Above the next stage's line, which starts at this dot's centre. */
    z-index: 1;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--bg-raised);
    border: 2px solid var(--border);
  }
  .stepper li.done i {
    background: color-mix(in srgb, var(--hue) 70%, transparent);
    border-color: transparent;
  }
  .stepper li.done {
    color: var(--text-dim);
  }
  .stepper li.current i {
    background: var(--hue);
    border-color: var(--hue);
    box-shadow: 0 0 0 4px color-mix(in srgb, var(--hue) 22%, transparent);
  }
  .stepper li.current {
    font-weight: var(--weight-semibold);
    color: color-mix(in srgb, var(--hue) 80%, var(--text));
  }
  .stepper small {
    display: block;
    font-weight: var(--weight-regular, 400);
    font-family: var(--font-mono);
    color: var(--text-faint);
  }
  .stat-tiles {
    max-width: 680px;
  }
  .field {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--text-small);
    color: var(--text-dim);
  }
  .field .input {
    width: 76px;
  }

  /* The session card keeps its height whatever the choice: the sentence
   * and the timeline reserve their tallest case. */
  .why {
    margin: 0;
    min-height: 2.9em;
    font-size: 0.9rem;
    line-height: 1.45;
  }
  .tl {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
    height: calc(var(--rows) * 1.65rem);
  }
  .tl li {
    display: grid;
    grid-template-columns: 62px 10px minmax(0, 1fr);
    gap: 10px;
    align-items: center;
    height: 1.65rem;
    font-size: 0.88rem;
  }
  .tl .t {
    font-family: var(--font-mono);
    font-size: var(--text-small);
    color: var(--text-faint);
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .tl .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--hue);
  }
  .tl .leg .dot {
    width: 5px;
    height: 5px;
    margin: 0 2px;
    background: var(--text-faint);
  }
  .tl .what {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--text-dim);
  }
  .tl b {
    font-weight: var(--weight-medium);
    color: color-mix(in srgb, var(--hue) 45%, var(--text-strong));
  }
  .drive {
    margin: 0;
    min-height: 2.6em;
  }

  .cols {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 440px;
    gap: 24px;
    align-items: start;
  }
  @container (max-width: 980px) {
    .cols {
      grid-template-columns: minmax(0, 1fr);
    }
  }
  .plans {
    container-type: inline-size;
  }
  .sec-h {
    display: flex;
    align-items: baseline;
    gap: 8px;
    margin: 0 0 8px;
    font-family: var(--font-mono);
    font-size: var(--text-small);
    font-weight: var(--weight-medium);
    text-transform: var(--label-transform);
    letter-spacing: var(--label-tracking);
    color: var(--text-dim);
  }
  .sec-h .for {
    font-family: var(--font-sans);
    font-weight: 400;
    text-transform: none;
    letter-spacing: 0;
    color: var(--text-faint);
  }
  .none {
    margin: 0;
    padding: 9px 6px;
  }
  .step {
    display: grid;
    grid-template-columns: 40px minmax(0, 1fr) auto;
    gap: 12px;
    align-items: center;
    padding: 9px 6px;
    border-bottom: 1px solid var(--border);
  }
  .step.blocked {
    opacity: 0.6;
  }
  .text {
    min-width: 0;
  }
  .step-title {
    display: block;
    font-weight: var(--weight-semibold);
    color: var(--text-strong);
  }
  .step-sub {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin-top: 3px;
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .hue-chip.wait {
    --hue: var(--bad);
  }
  .pright {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 2px;
    min-width: 88px;
    text-align: right;
  }
  .pright .mono {
    word-break: normal;
    white-space: nowrap;
    color: var(--text);
  }
  .pright small {
    font-size: var(--text-small);
    color: var(--text-faint);
  }

  .sheet {
    position: sticky;
    top: 16px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    min-width: 0;
  }
  .panel {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 14px;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--bg-raised);
  }
  .panel .sec-h {
    margin: 0;
  }
  .plan-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .plan-row {
    all: unset;
    box-sizing: border-box;
    display: grid;
    grid-template-columns: 40px minmax(0, 1fr);
    gap: 12px;
    align-items: center;
    padding: 8px 6px;
    border-radius: var(--radius);
    cursor: pointer;
  }
  .plan-row:hover {
    background: var(--accent-fill-faint);
  }
  .plan-row:focus-visible {
    outline: 2px solid var(--accent-line);
  }
  .plan-row.sel {
    background: var(--accent-fill-weak);
  }
  .plan-text {
    display: flex;
    flex-direction: column;
    gap: 5px;
    min-width: 0;
  }
  .plan-head {
    display: flex;
    gap: 8px;
    align-items: baseline;
  }
  .plan-head b {
    flex: 1;
    min-width: 0;
    font-weight: var(--weight-semibold);
    color: var(--text-strong);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .track {
    position: relative;
    height: 4px;
    border-radius: 2px;
    background: var(--bg-sunk);
    overflow: hidden;
  }
  .track i {
    position: absolute;
    inset: 0 auto 0 0;
    border-radius: 2px;
    background: color-mix(in srgb, var(--hue) 70%, transparent);
  }
  .plan-note {
    font-size: var(--text-small);
    color: var(--text-faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .plan-note.waits {
    color: var(--warn);
  }

  .mat {
    display: grid;
    grid-template-columns: 116px minmax(0, 1fr) 128px;
    gap: 10px;
    align-items: center;
    font-size: 0.86rem;
  }
  .mat-name {
    display: flex;
    flex-direction: column;
    min-width: 0;
    color: var(--text-strong);
  }
  .mat-name span {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .need {
    position: relative;
    height: 10px;
    border-radius: 3px;
    background: var(--bg-sunk);
    overflow: hidden;
  }
  .need i {
    position: absolute;
    inset: 0 auto 0 0;
    border-radius: 3px;
  }
  .need .wanted {
    background: color-mix(in srgb, var(--text) 18%, transparent);
  }
  .need .held {
    background: var(--hue-quality);
  }
  .mat-figure {
    text-align: right;
    font-size: 0.82rem;
    word-break: normal;
    white-space: nowrap;
  }
  .mat-figure .good {
    color: var(--good);
  }
  .mat-figure .warn {
    color: var(--warn);
  }
  .mat-figure .bad {
    color: var(--bad);
  }
  .unit {
    margin-left: 4px;
    color: var(--text-faint);
  }

  .empty {
    display: flex;
    flex-direction: column;
    gap: 10px;
    max-width: 640px;
    padding: 20px 22px;
  }
  .empty h2,
  .empty p {
    margin: 0;
  }
  .empty-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 4px;
  }
  .new {
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .new :global(.pick) {
    max-height: 46vh;
  }
  .new-none {
    margin: 0;
    padding: 16px;
  }
  .new-foot {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 8px;
    padding: 12px 16px 16px;
    border-top: 1px solid var(--border);
  }
</style>
