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
  import { Button, ConfirmDialog, Dialog, Picker } from "@veelume/ui";
  import { ArrowDown, ArrowUp, ArrowUpRight, Plus } from "lucide-svelte";
  import type { BlueprintRowView, PartChoice, PartSource, PlanBoard, PlanCraft, PlanSlot, RepFilter, StepKind, TrackKind } from "$lib/bindings";
  import NoGameData from "$lib/components/catalog/NoGameData.svelte";
  import NowCard from "$lib/components/NowCard.svelte";
  import PlanGlyph, { type GlyphShape } from "$lib/modules/plans/PlanGlyph.svelte";
  import { KIND_TEXT, TRACKS, ago, amount, focusView, ledgerBars, stepFigure, type Focus } from "$lib/modules/plans/model";
  import { snapUp, stopsOf } from "$lib/quality";
  import { getBlueprints } from "$lib/state/catalog.svelte";
  import { craftingStore, loadMarks } from "$lib/state/crafting.svelte";
  import { dataStore, ensureChannel } from "$lib/state/data.svelte";
  import {
    addCraftPlan,
    addLoadoutPlan,
    addProject,
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
    setPartSource,
    setPlanNotes,
    setPrefer,
    updateCraft,
    watchPlans,
  } from "$lib/state/plans.svelte";
  import { settingsStore } from "$lib/state/settings.svelte";
  import { openShip, shipsUi } from "$lib/state/ships.svelte";

  let ready = $state(false);
  let now = $state(Date.now());
  let tab = $state<"active" | "done">("active");
  let picked = $state<string | null>(null);
  /** What the session is for: a kind of work across your plans, or one
   *  plan to get ready; `null`: the kind of work the board suggests. */
  let focus = $state<Focus | null>(null);
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
  /** The kinds of work the board has, in the card's order. */
  const tracksShown = $derived(TRACKS.filter((t) => board?.tracks.tracks.some((x) => x.kind === t.id)));
  /** The plans the card offers as a focus: the open ones. */
  const focusPlans = $derived((board?.plans ?? []).filter((p) => !p.done && !p.parked && !p.missing));
  const shownFocus = $derived<Focus>(
    focus ??
      (board?.tracks.next
        ? { kind: "activity", track: board.tracks.next }
        : { kind: "activity", track: tracksShown[0]?.id ?? "shopping" }),
  );
  /** The trips the focus flies: the board's for a kind of work, the
   *  plan's own for a plan. */
  const focusTracks = $derived(
    shownFocus.kind === "plan"
      ? (board?.planTracks.find((p) => p.plan === shownFocus.plan)?.tracks.tracks ?? [])
      : (board?.tracks.tracks ?? []),
  );
  const view = $derived(focusView(board?.steps ?? [], focusTracks, shownFocus, board?.from ?? ""));
  const names = $derived(new Map((board?.plans ?? []).map((p) => [p.id, p.name])));
  /** The plan the page shows the controls of: the focus when it is a
   *  plan; on Done, the one picked there. */
  const detail = $derived(
    shownFocus.kind === "plan"
      ? (board?.plans.find((p) => p.id === shownFocus.plan) ?? null)
      : tab === "done"
        ? plan
        : null,
  );
  const target = $derived(board?.targets.find((t) => t.plan === detail?.id) ?? null);
  /** The plan as stored: its own name (the board's is the shown one). */
  const stored = $derived(plansStore.plans?.find((p) => p.id === detail?.id) ?? null);
  /** A plan's name where room is short (the Focus pills, step tags,
   *  Components rows): your own name for it, else the ship or item
   *  without its maker ("Constellation Mk V Centaurus"). The full name is
   *  the tooltip. */
  function short(id: string): string {
    const p = board?.plans.find((x) => x.id === id);
    if (!p) return names.get(id) ?? id;
    const own = plansStore.plans?.find((x) => x.id === id)?.name;
    if (own) return own;
    if (p.kind !== "loadout") return p.kind === "project" ? p.name : p.title;
    const words = p.title.split(" ");
    return words.length > 1 ? words.slice(1).join(" ") : p.title;
  }
  const focusName = $derived(
    shownFocus.kind === "plan"
      ? `to get ${names.get(shownFocus.plan) ?? "the plan"} ready`
      : `for ${(TRACKS.find((t) => t.id === shownFocus.track)?.label ?? "").toLowerCase()} across your plans`,
  );
  const FOCUS_HUE: Record<TrackKind, string> = {
    shopping: "var(--hue-sky)",
    mining: "var(--hue-quality)",
    refining: "var(--hue-sky)",
    missions: "var(--hue-sand)",
    looting: "var(--hue-apricot)",
    crafting: "var(--hue-rose)",
  };
  /** The Components panel's sources: id, the Prefer label, a row's. */
  const SOURCES: [PartSource, string, string][] = [
    ["store", "Store", "Store"],
    ["market", "Player market", "Market"],
    ["craft", "Crafting", "Craft"],
    ["loot", "Looting", "Loot"],
  ];
  /** What a part's chosen source asks: the total for its count and where,
   *  the blueprint's state for a craft, the best odds and place for loot. */
  function partFigure(c: PartChoice): { value: string; detail: string } | null {
    const o = c.options.find((x) => x.source === c.chosen);
    if (!o) return null;
    if (o.price != null) return { value: `${Math.round(o.price * c.count).toLocaleString("en-US")} aUEC`, detail: o.detail };
    if (o.source === "loot") {
      // "4.0% at Ruin Station": the odds lead, the place is the detail.
      const cut = o.detail.indexOf(" at ");
      return cut < 0 ? { value: o.detail, detail: "" } : { value: o.detail.slice(0, cut), detail: o.detail.slice(cut + 4) };
    }
    return { value: "craft", detail: o.detail };
  }

  /** A rep step's missions in Contracts: its faction, open to you now (or
   *  opening at the step's rank), sorted by rep per minute. */
  function repLink(f: RepFilter): string {
    const q = new URLSearchParams({ faction: f.faction, sort: "repmin" });
    if (f.rank) q.set("rank", f.rank);
    else q.set("status", "open");
    return `/contracts?${q}`;
  }

  /** Picking a plan, in the list or on the card, makes it the focus. */
  function pickPlan(id: string) {
    picked = id;
    focus = { kind: "plan", plan: id };
  }
  const bars = $derived(ledgerBars(board?.ledger ?? []));
  const counts = $derived({
    active: (board?.plans ?? []).filter((p) => !p.done).length,
    done: (board?.plans ?? []).filter((p) => p.done).length,
  });

  const PLAN_LOOK: Record<PlanBoard["kind"], { shape: GlyphShape; chip: string }> = {
    loadout: { shape: "ship", chip: "LOADOUT" },
    craft: { shape: "craft", chip: "CRAFT" },
    project: { shape: "blueprint", chip: "PROJECT" },
    rank: { shape: "mission", chip: "RANK" },
    mission: { shape: "mission", chip: "MISSION" },
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
    loot: "find",
    rep: "mission",
    run: "mission",
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
    if (detail && count != null) void updateCraft(detail.id, c.blueprint, count, minimums(c));
    return count ?? c.count;
  }

  /** One quality for every material of a craft: the shortcut. */
  function setAll(c: PlanCraft, raw: string) {
    const quality = typed(raw, 0, 1000);
    if (detail && quality != null) void updateCraft(detail.id, c.blueprint, c.count, [], quality);
  }

  function setSlot(c: PlanCraft, index: number, quality: number) {
    if (!detail) return;
    const slots = minimums(c);
    slots[index] = quality;
    void updateCraft(detail.id, c.blueprint, c.count, slots);
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
  <!-- Active and Done are the page's views: the tab row Ships uses (a
       Segmented is an input, not navigation; Valerie, 2026-11-17). -->
  <header class="tabs" role="tablist" aria-label="Plans">
    {#each [["active", "Active", counts.active], ["done", "Done", counts.done]] as const as [id, label, n] (id)}
      <button type="button" role="tab" class="tab" class:active={tab === id} aria-selected={tab === id} onclick={() => (tab = id)}
        >{label}{#if board}<span class="n">{n}</span>{/if}</button
      >
    {/each}
    <span class="grow"></span>
    {#if board}
      {#if board.inventoryAt != null}
        <span class="dim small side">Read from the game {ago(board.inventoryAt, now)}</span>
      {:else}
        {#if on("items")}
          <a class="dim small side" href="/items">Your inventory was never read, so nothing you hold is counted</a>
        {:else}
          <span class="dim small side">Your inventory was never read, so nothing you hold is counted</span>
        {/if}
      {/if}
    {/if}
    <span class="side"><Button onclick={() => (adding = true)}><Plus size={15} /> New plan</Button></span>
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
      <div class="cols">
        <section class="lead" aria-label="Next session and next steps">
          <!-- What the session is for (pass 6): a kind of work across your
               plans, or one plan to get ready. Distance leads; every time
               names its source; mining and refining have none (a dash). -->
          <div class="session">
          <NowCard
            size="wide"
            hue={shownFocus.kind === "plan" ? "var(--hue-rose)" : FOCUS_HUE[shownFocus.track]}
            chip="NEXT SESSION"
            version={view.card.distance}
            name={view.card.what}
            date={board.from ? `from ${board.from}` : "last place not known"}
          >
            {#snippet above()}
              <div class="pick">
                <span class="pick-label">Activity</span>
                <div class="opts" role="radiogroup" aria-label="Activity">
                  {#each tracksShown as t (t.id)}
                    {@const on = shownFocus.kind === "activity" && shownFocus.track === t.id}
                    <button
                      type="button"
                      role="radio"
                      class="opt"
                      class:on
                      style:--hue={FOCUS_HUE[t.id]}
                      aria-checked={on}
                      onclick={() => (focus = { kind: "activity", track: t.id })}>{t.label}</button
                    >
                  {/each}
                </div>
                <span class="pick-label">Focus</span>
                <div class="opts" role="radiogroup" aria-label="Focus">
                  {#each focusPlans as p (p.id)}
                    {@const on = shownFocus.kind === "plan" && shownFocus.plan === p.id}
                    <button
                      type="button"
                      role="radio"
                      class="opt"
                      class:on
                      style:--hue="var(--hue-rose)"
                      aria-checked={on}
                      title={p.name}
                      onclick={() => pickPlan(p.id)}>{short(p.id)}</button
                    >
                  {/each}
                </div>
              </div>
            {/snippet}
            <div class="stat-tiles focus-tiles">
              {#each view.card.tiles as t (t.label)}
                <div class="stat-tile"><b>{t.value}</b><span>{t.label}</span></div>
              {/each}
            </div>
            <p class="route-line" title={view.card.route}>{view.card.route}</p>
          </NowCard>
          </div>

          <h2 class="sec-h steps-h">Next steps<span class="for">{focusName}</span></h2>
          {#if view.groups.length === 0}
            {@const focused = shownFocus.kind === "plan" ? board.plans.find((p) => p.id === shownFocus.plan) : null}
            <p class="dim none">
              {focused?.missing
                ? focused.note
                : focused?.parked
                  ? "Parked: it reserves nothing and has no steps until you take it up again."
                  : focused?.kind === "project" && focused.crafts.length === 0
                  ? "No crafts in this project yet. Add one below, or from a recipe on Crafting."
                  : focused
                    ? "Nothing left to do: the game shows this goal reached."
                    : "Nothing of this kind to do."}
            </p>
          {/if}
          <!-- The focus's steps by session and by run: a shopping run, a
               mining load ending at its refinery, then what waits. -->
          {#each view.groups as g (g.key)}
            <div class="grp-h"><b>{g.title}</b><span class="grow"></span><span class="mono">{g.figure}</span></div>
            {#each g.runs as r (r.key)}
              <div class="run-h"><span>{r.title}</span><span class="grow"></span><span class="mono">{r.figure}</span></div>
              <div class="rows">
                {#each r.steps as { step: s, other } (s.id)}
                  <div class="step k-{s.kind}" class:blocked={s.blocked} class:other>
                    <span class="glyph"><PlanGlyph shape={STEP_SHAPE[s.kind]} /></span>
                    <span class="text">
                      <span class="step-title">{s.title}</span>
                      <span class="step-sub">
                        <span class="hue-chip" class:wait={s.blocked}>{s.blocked ? "waits" : KIND_TEXT[s.kind]}</span>
                        <span>{s.detail}</span>
                        {#if s.kind === "earn" && s.id.startsWith("earn:") && on("contracts")}
                          <!-- The step's id carries the blueprint; Contracts scopes its catalog to it. -->
                          <a class="earn-lk" href="/contracts?bp={encodeURIComponent(s.id.slice(5))}">Missions</a>
                        {/if}
                        {#if s.rep && on("contracts")}
                          <!-- The faction's missions open now (or at the rank), most rep a minute first. -->
                          <a class="earn-lk" href={repLink(s.rep)}>Missions</a>
                        {/if}
                        {#if s.kind !== "earn" && s.mission && on("contracts")}
                          <!-- Contracts finds the mission by its title and opens it. -->
                          <a class="earn-lk" href="/contracts?q={encodeURIComponent(s.mission.title)}&open={encodeURIComponent(s.mission.id)}">Mission</a>
                        {/if}
                        {#if shownFocus.kind === "plan" ? other : s.plans.length > 1}
                          {#each s.plans as p (p)}<span class="tag" title={names.get(p) ?? p}>{short(p)}</span>{/each}
                        {/if}
                      </span>
                    </span>
                    <span class="pright"><span class="mono">{stepFigure(s)}</span><small>{s.at?.name ?? ""}</small></span>
                  </div>
                {/each}
              </div>
            {/each}
          {/each}



          {#if detail && (detail.kind === "project" || detail.crafts.length > 0)}
            <!-- What the detail crafts, each with the minimum quality of
                 every material. All of it is always shown: nothing here
                 opens or closes. -->
            <h2 class="sec-h crafts-h">
              {detail.kind === "project" ? "Crafts in this project" : "Minimum quality per material"}
              <span class="grow"></span>
              {#if detail.kind === "project"}
                <Button variant="outline" onclick={() => (addingTo = detail)}><Plus size={14} /> Add a craft</Button>
              {/if}
            </h2>
            {#each detail.crafts as c (c.blueprint)}
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
                  {#if detail.kind === "project"}
                    <Button variant="ghost" onclick={() => void removeCraft(detail.id, c.blueprint)}>Take out</Button>
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
                <div class="plan-row kind-{p.kind}" class:sel={p.id === (detail?.id ?? (shownFocus.kind === "plan" ? shownFocus.plan : null))} class:parked={p.parked}>
                  <button type="button" class="plan-pick" aria-pressed={p.id === detail?.id} onclick={() => pickPlan(p.id)}>
                    <span class="glyph"><PlanGlyph shape={PLAN_LOOK[p.kind].shape} /></span>
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

          {#if detail}
            <!-- The plan in focus: rename, note, park, remove, open its goal. -->
            <section class="panel" aria-label="This plan">
              <h2 class="sec-h">This plan<span class="grow"></span><span class="for">{PLAN_LOOK[detail.kind].chip.toLowerCase()}{detail.parked ? ", parked" : ""}</span></h2>
              {#if detail.missing}
                <p class="dim">{detail.note}. Remove the plan, or bring its goal back.</p>
              {/if}
              <label class="this-field"
                >Name
                <input
                  class="input"
                  value={stored?.name ?? ""}
                  placeholder={detail.kind === "project" ? "Project" : `${detail.title}, ${detail.subtitle}`}
                  onchange={(e) => void renamePlan(detail.id, e.currentTarget.value)}
                /></label
              >
              <label class="this-field"
                >Notes
                <input
                  class="input"
                  value={detail.notes}
                  placeholder="What it is for"
                  onchange={(e) => void setPlanNotes(detail.id, e.currentTarget.value)}
                /></label
              >
              <div class="this-actions">
                <Button variant="ghost" onclick={() => (removing = detail)}>Remove</Button>
                {#if !detail.done && !detail.missing}
                  <Button
                    variant="ghost"
                    title={detail.parked
                      ? "It reserves materials and parts again, at its place in the order"
                      : "It stays in the list and reserves nothing, so the plans behind it get the stock"}
                    onclick={() => void parkPlan(detail.id, !detail.parked)}>{detail.parked ? "Take it up again" : "Park"}</Button
                  >
                {/if}
                <span class="grow"></span>
                {#if (target?.ship && on("ships")) || (target?.family && on("crafting"))}
                  <Button variant="outline" onclick={open}>
                    {target.ship ? "Open ship" : "Open recipe"} <ArrowUpRight size={14} />
                  </Button>
                {/if}
              </div>
            </section>
          {/if}

          {#if board.parts.length}
            <!-- Where each part the plans still need comes from: one choice
                 for all, and each part's own over it (decision 'one source
                 per need with a switch'). A source a part lacks is off. -->
            <section class="panel">
              <h2 class="sec-h">Components across plans</h2>
              <div class="src-pref" role="radiogroup" aria-label="Prefer">
                <span class="pick-label">Prefer</span>
                {#each SOURCES as [id, label] (id)}
                  <button type="button" role="radio" class="src-btn" class:on={board.prefer === id} aria-checked={board.prefer === id} onclick={() => void setPrefer(id)}
                    >{label}</button
                  >
                {/each}
              </div>
              {#each board.parts as c (c.item)}
                <div class="part">
                  <span class="part-text"
                    ><span class="part-name" title={c.name}>{c.count > 1 ? `${c.count} × ${c.name}` : c.name}</span><span
                      class="part-plans"
                      title={c.plans.map((p) => names.get(p) ?? p).join(", ")}>{c.plans.map(short).join(", ")}</span
                    ></span
                  >
                  <span class="src" role="radiogroup" aria-label="{c.name} from">
                    {#each SOURCES as [id, , short] (id)}
                      {@const has = c.options.some((o) => o.source === id)}
                      <button
                        type="button"
                        role="radio"
                        class="src-btn"
                        class:on={c.chosen === id}
                        aria-checked={c.chosen === id}
                        disabled={!has}
                        onclick={() => void setPartSource(c.item, id)}>{short}</button
                      >
                    {/each}
                  </span>
                  {#if partFigure(c)}
                    {@const f = partFigure(c)}
                    <span class="pright"><span class="mono">{f?.value}</span><small>{f?.detail}</small></span>
                  {/if}
                </div>
              {/each}
            </section>
          {/if}

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
  /* Hues by role, as the pass 6 mock: a shop sky, mining teal, the
   * refinery sky, missions sand, loot apricot, crafts and plans rose. */
  .k-fit,
  .k-buy,
  .k-refine {
    --hue: var(--hue-sky);
  }
  .k-mine,
  .k-gather {
    --hue: var(--hue-quality);
  }
  .k-loot {
    --hue: var(--hue-apricot);
  }
  .k-craft,
  .kind-craft,
  .kind-project {
    --hue: var(--hue-rose);
  }
  .k-earn,
  .k-rep,
  .k-run,
  .k-find,
  .kind-rank,
  .kind-mission {
    --hue: var(--hue-sand);
  }
  .kind-loadout {
    --hue: var(--hue-rose);
  }
  .kind-unknown {
    --hue: var(--hue-sky);
  }

  /* The left column: the session card, then its steps. */
  .lead {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  /* A block, so the card is as tall as its content (a NowCard is a
   * flex item sized for a row); one height for every focus. */
  .session {
    display: grid;
  }
  .session :global(.now .main) {
    min-height: 250px;
  }
  .steps-h {
    margin-top: 22px;
  }
  /* A row's thumbnail: the glyph on a tile in its hue. */
  .glyph {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border-radius: var(--radius);
    background: linear-gradient(135deg, color-mix(in srgb, var(--hue) 16%, var(--bg-sunk)), var(--bg-sunk));
    border: 1px solid color-mix(in srgb, var(--hue) 22%, var(--border));
    box-sizing: border-box;
  }
  .glyph :global(svg) {
    width: 30px;
    height: 30px;
    opacity: 0.9;
  }
  /* The plan in focus, in the side column under Your plans. */
  .this-field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: var(--text-small);
    color: var(--text-dim);
  }
  .this-actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
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

  /* The tab row's other pieces sit on its middle, like the ship picker on
   * Ships'. */
  .tabs .side {
    align-self: center;
    margin: 0 0 6px 10px;
  }

  /* The session card keeps its height whatever the focus: the tiles and
   * the route reserve their line each. */
  .pick {
    display: grid;
    grid-template-columns: 150px minmax(0, 1fr);
    gap: 8px 12px;
    align-items: center;
  }
  .pick-label {
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  /* What the session is for: square options (an input, not chips), the
   * picked one in its focus's hue. */
  .opts {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .opt {
    all: unset;
    box-sizing: border-box;
    cursor: pointer;
    max-width: 260px;
    padding: 7px 11px;
    border: 1px solid var(--border);
    border-radius: var(--radius-chip);
    font-size: var(--text-small);
    line-height: 1;
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .opt:hover {
    color: var(--text);
  }
  .opt:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .opt.on {
    background: color-mix(in srgb, var(--hue) 16%, transparent);
    border-color: color-mix(in srgb, var(--hue) 42%, transparent);
    color: var(--text-strong);
    font-weight: var(--weight-semibold);
  }
  .focus-tiles {
    max-width: 760px;
    min-height: 3.6rem;
  }
  .route-line {
    margin: 0;
    min-height: 1.4em;
    font-family: var(--font-mono);
    font-size: var(--text-small);
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* Components across plans: Prefer, then each part on two lines (name
   * and plans, its sources under them), its figure beside both. */
  .src-pref {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .src-pref .pick-label {
    margin-right: 4px;
  }
  .src-btn {
    all: unset;
    box-sizing: border-box;
    cursor: pointer;
    font-size: var(--text-small);
    line-height: 1;
    padding: 5px 9px;
    border-radius: var(--radius-chip);
    border: 1px solid var(--border);
    color: var(--text-dim);
    white-space: nowrap;
  }
  .src-btn:hover:not(:disabled) {
    color: var(--text);
  }
  .src-btn.on {
    background: var(--accent-fill-weak);
    border-color: var(--accent-border);
    color: var(--text-strong);
    font-weight: var(--weight-semibold);
  }
  .src-btn:disabled {
    opacity: 0.35;
    cursor: default;
  }
  .part {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 6px 12px;
    align-items: start;
    padding: 8px 0;
    border-top: 1px solid var(--border);
    font-size: 0.86rem;
  }
  .part-text {
    grid-column: 1;
    grid-row: 1;
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
  }
  /* The name keeps most of the line; the plans give way first. */
  .part-plans {
    flex: 1 1 0;
    min-width: 0;
    font-size: var(--text-small);
    color: var(--text-faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .part-name {
    flex: 0 1 auto;
    max-width: 75%;
    color: var(--text-strong);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .part .src {
    grid-column: 1;
    grid-row: 2;
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .part .pright {
    grid-column: 2;
    grid-row: 1 / span 2;
    align-self: center;
  }

  /* Next steps by session and by run. */
  .grp-h {
    display: flex;
    align-items: baseline;
    gap: 10px;
    margin: 20px 0 2px;
    padding: 0 6px;
  }
  .grp-h b {
    font-size: 1rem;
    font-weight: var(--weight-semibold);
    color: var(--text-strong);
  }
  .grp-h .mono,
  .run-h .mono {
    font-size: var(--text-small);
    color: var(--text-dim);
  }
  /* A run's heading starts where its steps' titles do. */
  .run-h {
    display: flex;
    align-items: baseline;
    gap: 10px;
    margin-top: 10px;
    padding: 6px 6px 4px 58px;
    font-size: var(--text-small);
    color: var(--text-dim);
    border-bottom: 1px solid var(--border);
  }
  .run-h .mono {
    color: var(--text-faint);
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
    margin: 8px 0 10px;
    font-family: var(--font-display);
    font-size: 0.75rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: var(--label-tracking);
    color: var(--text-faint);
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
  /* Another plan's mining in the same load: there, but not this plan's. */
  .step.other .step-title {
    color: var(--text-dim);
    font-weight: var(--weight-medium);
  }
  .step.other :global(svg) {
    opacity: 0.6;
  }
  .text {
    min-width: 0;
  }
  .earn-lk {
    color: var(--accent);
    text-decoration: none;
  }
  .earn-lk:hover {
    text-decoration: underline;
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
    gap: 12px;
    padding: 14px 16px;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: color-mix(in srgb, var(--bg-raised) 55%, transparent);
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
