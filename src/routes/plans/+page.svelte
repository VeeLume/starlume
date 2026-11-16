<script lang="ts">
  // Plans: the goals you set and what is left to do for them (design pass 5
  // on the Starlume Design canvas; decision 'One unified plan', 2026-10-21).
  // The page leads with actions: the plan in view with its stages, a next
  // session to fly, then the open steps. Every step, stage and figure is
  // derived Rust-side from what the account holds; nothing is ticked here.
  // A plan is a saved loadout, one craft, or a project: a named group of
  // crafts served as one plan (Hearth's projects). The order of the list is
  // who the ledger serves first; a parked plan keeps its place and reserves
  // nothing. A craft's minimum quality is set per material, at the
  // qualities that material comes in ($lib/quality).
  // Which plan is in view and which session choice is open are page-local.
  import { onMount, untrack } from "svelte";
  import { goto } from "$app/navigation";
  import { Button, ConfirmDialog, Dialog, Picker, Segmented } from "@veelume/ui";
  import { ArrowDown, ArrowUp, ArrowUpRight, Plus } from "lucide-svelte";
  import type { BlueprintRowView, PlanBoard, PlanCraft, PlanSlot, StepKind, Trip } from "$lib/bindings";
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
  import { snapUp, stopsOf } from "$lib/quality";
  import { getBlueprints } from "$lib/state/catalog.svelte";
  import { craftingStore, loadMarks } from "$lib/state/crafting.svelte";
  import { dataStore, ensureChannel } from "$lib/state/data.svelte";
  import {
    addCraftPlan,
    addLoadoutPlan,
    addProject,
    focusPlan,
    loadBoard,
    loadChoices,
    loadPlans,
    movePlan,
    parkPlan,
    planCraft,
    plansStore,
    removeCraft,
    removePlan,
    renamePlan,
    setPlanNotes,
    updateCraft,
    watchPlans,
  } from "$lib/state/plans.svelte";
  import { settingsStore } from "$lib/state/settings.svelte";
  import { openShip, shipsUi } from "$lib/state/ships.svelte";

  let ready = $state(false);
  let now = $state(Date.now());
  let tab = $state<"active" | "done">("active");
  let picked = $state<string | null>(null);
  let choice = $state<Trip["choice"]>("most");
  let adding = $state(false);
  /** The project a craft is being picked for. */
  let addingTo = $state<PlanBoard | null>(null);
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
    // Only the channel re-runs this: the catalog cache `getBlueprints`
    // reads is replaced with every dataset that lands.
    untrack(() => {
      void loadBoard(c);
      void loadChoices(c);
      void getBlueprints(c).then((rows) => {
        if (typeof rows !== "string") blueprints = rows;
      });
    });
  });

  /** Ships, Crafting and My Items are modules of their own: no way there
   *  while one is off. */
  const on = (id: string) => settingsStore.current?.enabled_modules.includes(id) ?? false;

  const shown = $derived((board?.plans ?? []).filter((p) => (tab === "done" ? p.done : !p.done)));
  const plan = $derived(shown.find((p) => p.id === picked) ?? shown[0] ?? null);
  const target = $derived(board?.targets.find((t) => t.plan === plan?.id) ?? null);
  /** The plan as stored: its own name (the board's is the shown one). */
  const stored = $derived(plansStore.plans?.find((p) => p.id === plan?.id) ?? null);
  const isFocus = $derived(!!plan && board?.plans[0]?.id === plan.id);
  const steps = $derived((board?.steps ?? []).filter((s) => plan && s.plans.includes(plan.id)));
  const session = $derived(board?.sessions.find((s) => s.choice === choice) ?? null);
  const rows = $derived(session ? timeline(session, board?.from ?? null) : []);
  const bars = $derived(ledgerBars(board?.ledger ?? []));
  const names = $derived(new Map((board?.plans ?? []).map((p) => [p.id, p.name])));
  const counts = $derived({
    active: (board?.plans ?? []).filter((p) => !p.done).length,
    done: (board?.plans ?? []).filter((p) => p.done).length,
  });

  const PLAN_LOOK: Record<PlanBoard["kind"], { shape: GlyphShape; chip: string }> = {
    loadout: { shape: "ship", chip: "LOADOUT" },
    craft: { shape: "craft", chip: "CRAFT" },
    project: { shape: "blueprint", chip: "PROJECT" },
    unknown: { shape: "blueprint", chip: "PLAN" },
  };
  const STEP_SHAPE: Record<StepKind, GlyphShape> = {
    fit: "crate",
    buy: "crate",
    mine: "mine",
    refine: "mine",
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

  /** A typed whole number within `min..max`, or null when it is none. */
  function typed(raw: string, min: number, max: number): number | null {
    const value = Math.round(Number(raw));
    return raw.trim() === "" || !Number.isFinite(value) ? null : Math.min(max, Math.max(min, value));
  }

  const minimums = (c: PlanCraft) => c.slots.map((s) => s.min);

  function setCount(c: PlanCraft, raw: string) {
    const count = typed(raw, 1, 9999);
    if (plan && count != null) void updateCraft(plan.id, c.blueprint, count, minimums(c));
    return count ?? c.count;
  }

  /** One quality for every material of a craft: the shortcut. */
  function setAll(c: PlanCraft, raw: string) {
    const quality = typed(raw, 0, 1000);
    if (plan && quality != null) void updateCraft(plan.id, c.blueprint, c.count, [], quality);
  }

  function setSlot(c: PlanCraft, index: number, quality: number) {
    if (!plan) return;
    const slots = minimums(c);
    slots[index] = quality;
    void updateCraft(plan.id, c.blueprint, c.count, slots);
  }

  /** What a slot's minimum can be set to, and where it stands now. */
  function slotChoice(s: PlanSlot) {
    const stops = stopsOf(s.bands, s.floor);
    return { stops, now: snapUp(stops, s.min) };
  }

  function stopLabel(s: PlanSlot, value: number, first: boolean) {
    if (first && s.floor <= 1) return "any quality";
    const band = s.bands.find((b) => b.value === value);
    return band ? `Q${value}, band ${band.from} to ${band.to}` : `Q${value}`;
  }

  const slotAmount = (s: PlanSlot) => `${amount(s, s.amount)} ${s.bulk ? "SCU" : "pcs"}`;

  // What "Add a craft" offers a project: the items you marked as wanted on
  // Crafting that it does not have yet.
  const craftOffers = $derived(
    blueprints.filter((b) => craftingStore.wantsItem(b.guid) && !addingTo?.crafts.some((c) => c.blueprint === b.guid)),
  );

  async function addToProject(blueprint: string) {
    const project = addingTo;
    addingTo = null;
    if (project) await planCraft(project.id, blueprint, 1, []);
  }

  async function newProject() {
    adding = false;
    const id = await addProject("New project");
    if (id) {
      tab = "active";
      picked = id;
    }
  }

  /** Move a plan one place up or down among the ones shown. */
  function shift(id: string, by: -1 | 1) {
    const at = shown.findIndex((p) => p.id === id);
    const before = by < 0 ? shown[at - 1] : shown[at + 2];
    if (at < 0 || (by < 0 && !before)) return;
    void movePlan(id, before?.id ?? null);
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
        {#if on("items")}
          <a class="dim small" href="/items">Your inventory was never read, so nothing you hold is counted</a>
        {:else}
          <span class="dim small">Your inventory was never read, so nothing you hold is counted</span>
        {/if}
      {/if}
    {/if}
    <Button onclick={() => (adding = true)}><Plus size={15} /> New plan</Button>
  </header>

  {#if board && plansStore.error}<p class="error">{plansStore.error}</p>{/if}

  {#if ready && !channel}
    <NoGameData />
  {:else if board}
    {#if board.plans.length === 0}
      <section class="empty card">
        <h2>No plans yet</h2>
        <p class="dim">
          A plan is a goal: a saved loadout to have fitted, an item to have crafted, or a project of several crafts.
          The steps come from what you hold in the game, so there is nothing to tick off.
        </p>
        <div class="empty-actions">
          <Button onclick={() => (adding = true)}><Plus size={15} /> New plan</Button>
          {#if on("ships")}<Button variant="outline" href="/ships">Save a loadout on Ships</Button>{/if}
          {#if on("crafting")}<Button variant="outline" href="/crafting">Pick a recipe on Crafting</Button>{/if}
        </div>
      </section>
    {:else}
      <div class="now-row">
        {#if plan}
          {@const look = PLAN_LOOK[plan.kind]}
          <NowCard
            size="wide"
            hue={plan.kind === "craft" || plan.kind === "project" ? "var(--hue-rose)" : "var(--hue-sky)"}
            chip={plan.parked ? "PARKED" : isFocus && !plan.done ? "FOCUS" : look.chip}
            version={plan.title}
            name={plan.subtitle}
            date={plan.done ? "done" : plan.parked ? look.chip.toLowerCase() : plan.waiting ? "waiting" : isFocus ? look.chip.toLowerCase() : ""}
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
            <!-- Every plan has a name and notes of its own, here for all
                 kinds so the card is one height. -->
            <div class="meta">
              <label class="field"
                >Name
                <input
                  class="input"
                  value={stored?.name ?? ""}
                  placeholder={plan.kind === "project" ? "Project" : `${plan.title}, ${plan.subtitle}`}
                  onchange={(e) => void renamePlan(plan.id, e.currentTarget.value)}
                /></label
              >
              <label class="field"
                >Notes
                <input
                  class="input"
                  value={plan.notes}
                  placeholder="What it is for"
                  onchange={(e) => void setPlanNotes(plan.id, e.currentTarget.value)}
                /></label
              >
            </div>
            {#snippet action()}
              <span class="grow"></span>
              <Button variant="ghost" onclick={() => (removing = plan)}>Remove</Button>
              {#if !plan.done && !plan.missing}
                <Button
                  variant="ghost"
                  title={plan.parked
                    ? "It reserves materials and parts again, at its place in the order"
                    : "It stays in the list and reserves nothing, so the plans behind it get the stock"}
                  onclick={() => void parkPlan(plan.id, !plan.parked)}>{plan.parked ? "Take it up again" : "Park"}</Button
                >
              {/if}
              {#if !isFocus && !plan.done && !plan.missing}
                <Button variant="outline" onclick={() => void focusPlan(plan.id)}>Make it the focus</Button>
              {/if}
              {#if (target?.ship && on("ships")) || (target?.family && on("crafting"))}
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
              <li title={row.full}>
                <span class="n">{row.n}</span><span class="what"><b>{row.what}</b><span>{row.where}</span></span>
              </li>
            {/each}
          </ol>
        </NowCard>
      </div>

      <div class="cols">
        <section class="steps" aria-label="Next steps">
          <h2 class="sec-h">Next steps{#if plan}<span class="for">for {plan.name}</span>{/if}</h2>
          {#if steps.length === 0}
            <p class="dim none">
              {!plan
                ? ""
                : plan.missing
                  ? plan.note
                  : plan.parked
                    ? "Parked: it reserves nothing and has no steps until you take it up again."
                    : plan.kind === "project" && plan.crafts.length === 0
                      ? "No crafts in this project yet. Add one below, or from a recipe on Crafting."
                      : "Nothing left to do: the game shows this goal reached."}
            </p>
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

          {#if plan && (plan.kind === "project" || plan.crafts.length > 0)}
            <!-- What the plan crafts, each with the minimum quality of
                 every material. All of it is always shown: nothing here
                 opens or closes. -->
            <h2 class="sec-h crafts-h">
              {plan.kind === "project" ? "Crafts in this project" : "Minimum quality per material"}
              <span class="grow"></span>
              {#if plan.kind === "project"}
                <Button variant="outline" onclick={() => (addingTo = plan)}><Plus size={14} /> Add a craft</Button>
              {/if}
            </h2>
            {#each plan.crafts as c (c.blueprint)}
              <div class="craft" class:missing={c.missing}>
                <div class="craft-head">
                  <b>{c.name}</b>
                  <span class="dim small">{c.crafted} of {c.count} crafted</span>
                  <span class="grow"></span>
                  {#if !c.missing}
                    <label class="field"
                      >Count
                      <input
                        class="input"
                        type="number"
                        min="1"
                        value={c.count}
                        onchange={(e) => (e.currentTarget.value = String(setCount(c, e.currentTarget.value)))}
                      /></label
                    >
                    <label class="field" title="One minimum for every material; each can then be set on its own below"
                      >All at least
                      <input
                        class="input"
                        type="number"
                        min="0"
                        max="1000"
                        step="50"
                        value={c.quality ?? ""}
                        placeholder="mixed"
                        onchange={(e) => setAll(c, e.currentTarget.value)}
                      /></label
                    >
                    {#if c.family && on("crafting")}
                      <Button variant="ghost" href="/crafting?sel={encodeURIComponent(c.family)}">Recipe <ArrowUpRight size={14} /></Button>
                    {/if}
                  {/if}
                  {#if plan.kind === "project"}
                    <Button variant="ghost" onclick={() => void removeCraft(plan.id, c.blueprint)}>Take out</Button>
                  {/if}
                </div>
                <div class="slots">
                  {#each c.slots as s, i (i)}
                    {@const choice = slotChoice(s)}
                    <label class="slot">
                      <span class="slot-name">{s.name ?? "Material"}</span>
                      <span class="slot-mat"
                        ><i class:held={s.held} title={s.held ? "Your stock covers it" : "Not covered by your stock"}></i>{s.material}<span
                          class="dim mono">{slotAmount(s)}</span
                        ></span
                      >
                      <span class="slot-moves">{s.moves.length ? `moves ${s.moves.join(", ")}` : "moves no stat"}</span>
                      {#if choice.stops.length}
                        <select class="select" value={choice.now} onchange={(e) => setSlot(c, i, Number(e.currentTarget.value))}>
                          {#each choice.stops as v, n (v)}
                            <option value={v}>{stopLabel(s, v, n === 0)}</option>
                          {/each}
                        </select>
                      {:else}
                        <!-- No bands in the game data for this material. -->
                        <input
                          class="input"
                          type="number"
                          min={s.floor}
                          max="1000"
                          value={s.min}
                          onchange={(e) => setSlot(c, i, typed(e.currentTarget.value, s.floor, 1000) ?? s.min)}
                        />
                      {/if}
                    </label>
                  {/each}
                </div>
              </div>
            {/each}
          {/if}
        </section>

        <aside class="sheet" aria-label="Your plans">
          <section class="panel">
            <h2 class="sec-h">{tab === "done" ? "Done" : "Your plans"}<span class="grow"></span><span class="mono">{shown.length}</span></h2>
            <div class="plan-list">
              {#each shown as p, n (p.id)}
                <div class="plan-row kind-{p.kind}" class:sel={p.id === plan?.id} class:parked={p.parked}>
                  <button type="button" class="plan-pick" aria-pressed={p.id === plan?.id} onclick={() => (picked = p.id)}>
                    <PlanGlyph shape={PLAN_LOOK[p.kind].shape} />
                    <span class="plan-text">
                      <span class="plan-head"><b>{p.name}</b><span class="mono dim">{Math.round(p.progress * 100)}%</span></span>
                      <span class="track"><i style:width="{Math.round(p.progress * 100)}%"></i></span>
                      <span class="plan-note" class:waits={p.waiting}>{p.note}</span>
                    </span>
                  </button>
                  <!-- The order is who gets the stock first. -->
                  {#if tab === "active"}
                    <span class="order">
                      <button type="button" disabled={n === 0} title="Serve it earlier" aria-label="Move {p.name} up" onclick={() => shift(p.id, -1)}
                        ><ArrowUp size={13} /></button
                      >
                      <button
                        type="button"
                        disabled={n === shown.length - 1}
                        title="Serve it later"
                        aria-label="Move {p.name} down"
                        onclick={() => shift(p.id, 1)}><ArrowDown size={13} /></button
                      >
                    </span>
                  {/if}
                </div>
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
      <Button variant="outline" onclick={() => void newProject()}>New project</Button>
      <span class="grow"></span>
      {#if on("ships")}<Button variant="outline" href="/ships">Save a loadout on Ships</Button>{/if}
      {#if on("crafting")}<Button variant="outline" href="/crafting">Pick a recipe on Crafting</Button>{/if}
    </div>
  </div>
</Dialog>

<Dialog open={!!addingTo} onclose={() => (addingTo = null)} title="Add a craft to {addingTo?.name ?? 'the project'}" class="max-w-lg!">
  <div class="new">
    {#if craftOffers.length}
      <Picker
        items={craftOffers}
        key={(b) => b.guid}
        label={(b) => b.name}
        detail={() => "Wanted item, to craft"}
        onpick={([b]) => void addToProject(b.guid)}
        class="pick"
      />
    {:else}
      <p class="dim new-none">
        Nothing to add from here: this lists the items you marked as wanted on Crafting. Any recipe there can be added
        to a project from its own sheet.
      </p>
    {/if}
    <div class="new-foot">
      {#if on("crafting")}<Button variant="outline" href="/crafting">Pick a recipe on Crafting</Button>{/if}
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
  .k-refine,
  .k-gather {
    --hue: var(--hue-quality);
  }
  .k-craft,
  .kind-craft,
  .kind-project {
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
  .meta {
    display: grid;
    grid-template-columns: minmax(0, 2fr) minmax(0, 3fr);
    gap: 12px;
    max-width: 680px;
  }
  .meta .field {
    min-width: 0;
  }
  .meta .input {
    flex: 1;
    width: auto;
    min-width: 0;
  }

  .crafts-h {
    align-items: center;
    min-height: 32px;
    margin-top: 22px;
  }
  .craft {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px 6px;
    border-bottom: 1px solid var(--border);
  }
  .craft.missing {
    opacity: 0.6;
  }
  .craft-head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 12px;
  }
  .craft-head b {
    font-weight: var(--weight-semibold);
    color: var(--text-strong);
  }
  .slots {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(210px, 1fr));
    gap: 8px;
  }
  .slot {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-sunk);
  }
  .slot-name {
    font-family: var(--font-mono);
    font-size: var(--text-small);
    color: var(--text-faint);
    text-transform: var(--label-transform);
    letter-spacing: var(--label-tracking);
  }
  .slot-mat {
    display: flex;
    align-items: baseline;
    gap: 6px;
    font-weight: var(--weight-medium);
    color: var(--text);
  }
  .slot-mat i {
    flex: 0 0 auto;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--bad);
    transform: translateY(-1px);
  }
  .slot-mat i.held {
    background: var(--good);
  }
  .slot-mat .mono {
    font-size: var(--text-small);
    font-weight: 400;
  }
  /* The stat this material moves: the reason to set its minimum high or
   * leave it low. */
  .slot-moves {
    font-size: var(--text-small);
    color: color-mix(in srgb, var(--hue-rose) 55%, var(--text));
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .slot .select,
  .slot .input {
    width: 100%;
    max-width: none;
    margin-top: 3px;
  }

  /* The session card keeps its height whatever the choice: the sentence
   * and the timeline reserve their tallest case. */
  .why {
    margin: 0;
    min-height: 4.35em;
    font-size: 0.9rem;
    line-height: 1.45;
  }
  .tl {
    display: flex;
    flex-direction: column;
    margin: 0;
    padding: 0;
    list-style: none;
    height: calc(var(--rows) * 3.3rem);
  }
  .tl li {
    display: grid;
    grid-template-columns: 22px minmax(0, 1fr);
    gap: 10px;
    align-items: center;
    height: 3.3rem;
    font-size: 0.88rem;
  }
  .tl .n {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: color-mix(in srgb, var(--hue) 16%, transparent);
    color: color-mix(in srgb, var(--hue) 45%, var(--text-strong));
    font-family: var(--font-mono);
    font-size: var(--text-small);
  }
  .tl .what {
    display: flex;
    flex-direction: column;
    min-width: 0;
    line-height: 1.4;
    color: var(--text-dim);
  }
  .tl .what > * {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tl b {
    font-weight: var(--weight-medium);
    color: var(--text-strong);
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
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    border-radius: var(--radius);
  }
  .plan-pick {
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
  .plan-pick:focus-visible {
    outline: 2px solid var(--accent-line);
  }
  .plan-row.sel {
    background: var(--accent-fill-weak);
  }
  .plan-row.parked .plan-pick {
    opacity: 0.6;
  }
  .order {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding-right: 4px;
  }
  .order button {
    all: unset;
    display: grid;
    place-items: center;
    width: 22px;
    height: 18px;
    border-radius: var(--radius-chip);
    color: var(--text-faint);
    cursor: pointer;
  }
  .order button:hover:not(:disabled) {
    color: var(--text-strong);
    background: var(--accent-fill);
  }
  .order button:focus-visible {
    outline: 2px solid var(--accent-line);
  }
  .order button:disabled {
    opacity: 0.3;
    cursor: default;
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
