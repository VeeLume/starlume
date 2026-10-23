<script lang="ts">
  // Ships: one ship, in two tabs, each meant to read in one screen with no
  // scrolling. Build assumes the ship is known: the part cards (alike parts
  // folded into one line) beside the power panel (sc-vehicles' network as
  // the ship's power screen shows it) and the figures parts and pips
  // change. Ship is the hull itself: picture, role, structure, armour,
  // flight, logistics, charts. Reads the cooked game data only: no network.
  import { onDestroy, onMount, untrack } from "svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { Segmented, Select, createBrowseState, type SelectOption } from "@veelume/ui";
  import type { SystemView } from "$lib/bindings";
  import NoGameData from "$lib/components/catalog/NoGameData.svelte";
  import PowerPanel from "$lib/modules/ships/PowerPanel.svelte";
  import ShipFigures from "$lib/modules/ships/ShipFigures.svelte";
  import SlotCards, { SYSTEMS, change, type Entry, type Fig } from "$lib/modules/ships/SlotCards.svelte";
  import { shieldResist } from "$lib/modules/ships/figures";
  import { dataStore, ensureChannel } from "$lib/state/data.svelte";
  import AccelRose from "$lib/modules/ships/AccelRose.svelte";
  import LoadoutBar from "$lib/modules/ships/LoadoutBar.svelte";
  import PartPicker from "$lib/modules/ships/PartPicker.svelte";
  import QtChart from "$lib/modules/ships/QtChart.svelte";
  import ShipMenu from "$lib/modules/ships/ShipMenu.svelte";
  import ShipPrices from "$lib/modules/ships/ShipPrices.svelte";
  import ShipsHome from "$lib/modules/ships/ShipsHome.svelte";
  import { crewParts, crewTitle } from "$lib/modules/ships/seats";
  import { ArrowLeft } from "lucide-svelte";
  import type { SlotView } from "$lib/bindings";
  import {
    closePicker,
    closeShip,
    listenForShipChanges,
    loadImages,
    loadPlaces,
    refreshImages,
    shipsImages,
    loadHome,
    loadPage,
    loadShipPrices,
    loadShips,
    openShip,
    shipsHome,
    openPicker,
    pick,
    resetPower,
    setLevel,
    shipsCandidates,
    shipsPicker,
    shipsPrices,
    shipsStore,
    shipsUi,
    toggleOff,
  } from "$lib/state/ships.svelte";

  function findSlot(slots: SlotView[], path: string[]): SlotView | null {
    for (const s of slots) {
      if (s.path.join("/") === path.join("/")) return s;
      const below = findSlot(s.children, path);
      if (below) return below;
    }
    return null;
  }

  /** Where a port is, as the game labels it: the deepest port label along
   *  the path that says more than "Slot" (a gun goes by its gimbal). */
  function placeOf(slots: SlotView[], path: string[]): string | null {
    let found: string | null = null;
    let level = slots;
    for (let i = 0; i < path.length; i++) {
      const s = level.find((x) => x.path.join("/") === path.slice(0, i + 1).join("/"));
      if (!s) break;
      if (s.label && s.label !== "Slot") found = s.label;
      level = s.children;
    }
    return found;
  }

  let ready = $state(false);
  /** Build or Ship, in the URL like any view mode (frontend rule 7). */
  const browse = createBrowseState({ tab: { kind: "one", default: "build" }, layout: { kind: "one", default: "grid" } });
  const tab = $derived(browse.values.tab === "ship" ? "ship" : "build");
  /** Layout trial: Build as the grid, or the holo in the middle with the
   *  fighting parts left of it and the running parts right. */
  const layout = $derived(browse.values.layout === "holo" ? "holo" : "grid");
  const FIGHT: Entry[][] = [[["weapons", "turrets"], "missiles", "countermeasures", "shields"]];
  const RUN: SystemView[][] = [["powerPlants", "coolers", "quantum", "radar", "lifeSupport", "fuel", "docked"]];
  let hover = $state<SystemView | null>(null);
  const channel = $derived(dataStore.channel);
  const ships = $derived(shipsStore.ships);
  const page = $derived(shipsStore.page);

  // The ship picker lists base hulls under their maker; a hull's editions
  // (Mk I / II, Executive, Wikelo specials) are the second picker.
  const spaceships = $derived((ships ?? []).filter((s) => !s.ground));
  const row = $derived(ships?.find((s) => s.guid === shipsUi.ship) ?? null);
  const baseOf = $derived(row?.base ?? row?.guid ?? "");
  const editions = $derived(spaceships.filter((s) => s.guid === baseOf || s.base === baseOf));
  const editionOptions = $derived<SelectOption[]>(
    editions.map((s) => ({ value: s.guid, label: s.guid === baseOf ? "Standard" : s.name, detail: s.reward ? "reward" : undefined })),
  );
  const shipName = $derived(row?.name ?? "");

  const guns = $derived(new Map((page?.guns ?? []).map((g) => [g.path, g])));

  $effect(() => {
    const c = channel;
    if (c)
      untrack(() => {
        void loadShips(c).then(() => loadImages(c));
        void loadPlaces(c);
        refreshImages();
      });
  });

  // The home page refreshes whenever it shows (recently opened moves).
  $effect(() => {
    const c = channel;
    if (c && !shipsUi.ship) untrack(() => void loadHome(c));
  });

  // Recompute when the ship, its edits or the power settings change.
  $effect(() => {
    const c = channel;
    const key = JSON.stringify([shipsUi.ship, shipsUi.edits[shipsUi.ship], shipsUi.mode, shipsUi.off[shipsUi.ship], shipsUi.levels[shipsUi.ship]]);
    if (!c || !shipsUi.ship || !key) return;
    untrack(() => void loadPage(c));
  });

  // Where the open ship is sold and rented (cached UEX prices, no network).
  $effect(() => {
    const c = channel;
    const ship = shipsUi.ship;
    if (c && ship) untrack(() => void loadShipPrices(c, ship));
  });

  const powered = $derived(
    new Set([...(page?.power?.columns.flatMap((c) => c.items) ?? []), ...(page?.power?.plants.map((p) => p.path) ?? [])]),
  );


  const fmt = (n: number | null | undefined, d = 0) =>
    n === null || n === undefined ? "—" : n.toLocaleString("en-US", { maximumFractionDigits: d });
  const mmss = (t: number) => `${Math.floor(t / 60)}:${String(Math.round(t % 60)).padStart(2, "0")}`;
  /** A trip time's change in whole seconds; shorter is better. */
  const secs = (now: number, before: number) => {
    const c = change(Math.round(now), Math.round(before), -1);
    return c && { ...c, text: `${c.text} s` };
  };

  /** Holo layout: the ship-level figures each system's header line adds
   *  (the old figures panel, handed out to the panels they belong to). */
  /** The factory loadout under the same power, while the ship has edits. */
  const stockPage = $derived(shipsStore.stock && page && shipsStore.stock.ship === page.ship ? shipsStore.stock : null);
  const stock = $derived(stockPage ? { slots: stockPage.slots, guns: new Map(stockPage.guns.map((g) => [g.path, g])) } : null);

  const lines = $derived.by(() => {
    const out: Partial<Record<SystemView, Fig[]>> = {};
    if (!page) return out;
    const s = page.summary;
    const was = stockPage?.summary ?? null;
    if (s.missiles) {
      const m = s.missiles;
      out.missiles = [
        ...(m.maxArmed !== null ? [{ v: `${m.maxArmed}`, u: "per salvo" }] : []),
        ...(m.cooldown !== null ? [{ v: fmt(m.cooldown, 1), u: "s apart" }] : []),
      ];
    }
    const regen = page.power?.shieldRegen ?? s.shieldRegenMax;
    const regenWas = stockPage ? (stockPage.power?.shieldRegen ?? stockPage.summary.shieldRegenMax) : null;
    out.shields = [
      ...(regen !== null ? [{ v: fmt(regen), u: "HP/s", change: change(regen, regenWas) }] : []),
      ...(s.shield?.face ? [{ v: "", u: s.shield.face.toLowerCase() }] : []),
    ];
    if (s.quantum) {
      const q = s.quantum;
      out.quantum = [
        { v: mmss(q.times[64][1]), u: "for 32 Gm", change: was?.quantum ? secs(q.times[64][1], was.quantum.times[64][1]) : null },
        ...(q.rangeGm ? [{ v: fmt(q.rangeGm), u: "Gm range" }] : []),
      ];
    }
    return out;
  });

  let unlisten: UnlistenFn | undefined;
  onMount(() => {
    void (async () => {
      unlisten = await listenForShipChanges(() => dataStore.channel);
      await ensureChannel();
      ready = true;
    })();
  });
  onDestroy(() => unlisten?.());
</script>

<div class="page">
  {#if ready && !channel}
    <NoGameData />
  {:else if shipsStore.shipsError}
    <p class="error">{shipsStore.shipsError}</p>
  {:else if ships && channel}
    <!-- With no ship open the start page searches and filters every ship
         itself; the picker is for switching once one is open. -->
    {#if shipsUi.ship}
      <div class="bar">
        <button type="button" class="btn subtle" onclick={closeShip} title="Back to your ships and new ones"><ArrowLeft size={14} /> Ships</button>
        <ShipMenu ships={spaceships.filter((s) => !s.base)} value={baseOf} onpick={openShip} />
        {#if editions.length > 1}
          <Select label="Edition" options={editionOptions} value={shipsUi.ship} onchange={openShip} />
        {/if}
        <LoadoutBar {channel} {shipName} />
        <Segmented
          options={[
            { value: "build", label: "Build" },
            { value: "ship", label: "Ship" },
          ]}
          value={tab}
          onchange={(v) => browse.set("tab", v)}
        />
        <span class="grow"></span>
        {#if tab === "build"}
          <Segmented
            options={[
              { value: "grid", label: "Grid" },
              { value: "holo", label: "Holo" },
            ]}
            value={layout}
            onchange={(v) => browse.set("layout", v)}
          />
        {/if}
      </div>
    {/if}
    {#if !shipsUi.ship}
      <ShipsHome ships={ships} home={shipsHome.view} onopen={openShip} />
    {:else if shipsStore.pageError}
      <p class="error">{shipsStore.pageError}</p>
    {:else if page && page.ship === shipsUi.ship}
      {@const s = page.summary}
      {@const picture = shipsImages.byGuid.get(shipsUi.ship)}
      {#snippet power()}
        {#if page.power}
          <PowerPanel
            power={page.power}
            aimAssist={s.aimAssist}
            crossSection={s.crossSection}
            resist={shieldResist(s.shield)}
            onlevel={setLevel}
            ontoggle={toggleOff}
            onmode={(m) => (shipsUi.mode = m)}
            onreset={resetPower}
          />
        {/if}
      {/snippet}
      {#snippet cards(columns?: Entry[][], withLines = false)}
        <SlotCards
          slots={page.slots}
          off={shipsUi.off[shipsUi.ship] ?? []}
          {powered}
          {guns}
          {columns}
          lines={withLines ? lines : null}
          stock={withLines ? stock : null}
          selected={shipsPicker.open?.targets[0].join("/") ?? null}
          onpick={(ss) => channel && void openPicker(channel, ss.map((s) => s.path))}
          ontoggle={toggleOff}
          onhover={(s) => (hover = s)}
          weaponPool={page.power?.columns.find((c) => c.itemType === "WeaponGun") ?? null}
        />
      {/snippet}
      {#if tab === "build" && shipsPicker.open}
        {@const targets = shipsPicker.open.targets}
        {@const at = findSlot(page.slots, targets[0])}
        <div class="over">
          <PartPicker
            title={placeOf(page.slots, targets[0]) ?? at?.name ?? "Part"}
            sub={at?.name ? `fitted: ${at.name}` : "empty"}
            list={shipsCandidates.list}
            error={shipsCandidates.error}
            count={targets.length}
            coolantAsked={page.power?.coolantAsked ?? null}
            tour={page.summary.quantum?.tour ?? null}
            onpick={(item) => pick(targets, item)}
            onclose={closePicker}
          />
        </div>
      {/if}
      {#if tab === "build" && layout === "holo"}
        <!-- The ship in the middle over its power screen; the parts that
             fight on its left, the parts that run it on its right. -->
        {@const glow = SYSTEMS.find((x) => x.id === hover)?.hue ?? null}
        <div class="around">
          <div class="side left">{@render cards(FIGHT, true)}</div>
          <div class="core">
            <div class="holo framed" class:lit={glow} style:--glow={glow ?? "var(--hue-sky)"}>
              {#if picture}<img class="shot" src={picture} alt="" />{/if}
            </div>
            {@render power()}
          </div>
          <div class="side right">{@render cards(RUN, true)}</div>
        </div>
      {:else if tab === "build"}
        <!-- Power and the figures it changes first, then the parts. -->
        <div class="ship build">
          <aside class="numbers" aria-label="Power and figures">
            {@render power()}
            <div class="figures">
              <ShipFigures {page} view="build" />
            </div>
          </aside>
          <div class="list">
            {@render cards()}
          </div>
        </div>
      {:else}
        <div class="ship">
          <div class="parts">
            <section class="head" aria-label="Ship">
              <div class="id">
                <span class="meta">
                  {#if row?.manufacturerCode}<b class="code">{row.manufacturerCode}</b>{/if}
                  {#each [row?.career, row?.role].filter(Boolean) as m (m)}<span>{m}</span>{/each}
                  <span class="crew" title={crewTitle(s.crew)}><b>{s.crew.crew} crew</b>{#each crewParts(s.crew) as c (c)}<i>{c}</i>{/each}</span>
                </span>
                <h1>{shipName}</h1>
              </div>
              <div class="holo">
                {#if picture}<img class="shot" src={picture} alt="" />{/if}
              </div>
            </section>
            <div class="charts">
              {#if s.flight}
                <section class="panel" aria-label="Flight">
                  <h3 class="label">Flight</h3>
                  <AccelRose normal={s.flight.acceleration} boosted={s.flight.accelerationBoosted} />
                </section>
              {/if}
              {#if s.quantum}
                <section class="panel" aria-label="Quantum travel">
                  <h3 class="label">Quantum travel</h3>
                  <QtChart drive={s.quantum} />
                </section>
              {/if}
              <section class="panel" aria-label="Where to get it">
                <h3 class="label">Where to get it</h3>
                <ShipPrices prices={shipsPrices.of(shipsUi.ship)} />
              </section>
            </div>
          </div>
          <aside class="numbers" aria-label="Figures">
            <div class="figures">
              <ShipFigures {page} view="ship" />
            </div>
          </aside>
        </div>
      {/if}
    {:else}
      <p class="hint">Reading the ship…</p>
    {/if}
  {:else if ready}
    <NoGameData waiting />
  {/if}
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: 16px;
    height: 100%;
    min-height: 0;
    overflow: auto;
  }
  /* On a narrow window the bar wraps rather than scrolling the page
   * sideways. */
  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 10px;
  }
  /* Icon and label on one line (preflight makes the svg a block). */
  .bar .btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    white-space: nowrap;
  }
  /* Two columns in the one page scroll: the parts (holo, then cards) and
   * the numbers (power, then figures). */
  .ship {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 470px;
    gap: 16px;
    align-items: start;
  }
  .ship.build {
    grid-template-columns: 360px minmax(0, 1fr);
  }
  .parts,
  .numbers {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
  }
  .figures {
    padding: 14px 18px;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: color-mix(in srgb, var(--bg-raised) 60%, transparent);
  }
  .head {
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 16px;
    border: 1px solid color-mix(in srgb, var(--hue-sky) 30%, var(--border));
    border-radius: var(--radius-lg);
    background: radial-gradient(
      120% 90% at 50% 35%,
      color-mix(in srgb, var(--hue-sky) 10%, var(--bg-sunk)),
      var(--bg-sunk) 70%
    );
  }
  h1 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.6rem;
    color: var(--text-strong);
  }
  .id {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-height: 64px;
  }
  .grow {
    flex: 1;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 12px;
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .crew {
    display: inline-flex;
    flex-wrap: wrap;
    gap: 4px;
    align-items: center;
  }
  .crew b {
    font-weight: 600;
    color: var(--text-dim);
    margin-right: 2px;
  }
  .crew i {
    font-style: normal;
    font-size: 0.7rem;
    padding: 0 6px;
    border-radius: var(--radius-chip);
    border: 1px solid var(--border);
    color: var(--text-dim);
  }
  .code {
    font: 700 0.72rem var(--font-mono);
    color: var(--hue-sand);
  }
  /* The holo window: the ship, at a fixed height so the cards start in
   * view under it. */
  .holo {
    position: relative;
    height: clamp(220px, 32vh, 380px);
  }
  .shot {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
    mask-image: radial-gradient(closest-side, black 70%, transparent);
    opacity: 0.92;
    pointer-events: none;
  }
  /* Holo layout: the ship and its power screen in the middle, the parts
   * around it. The middle column is capped so the parts keep their room. */
  .page {
    container-type: inline-size;
  }
  .around {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(400px, 520px) minmax(0, 1fr);
    grid-template-areas: "left core right";
    gap: 16px;
    align-items: start;
  }
  .side {
    min-width: 0;
  }
  .left {
    grid-area: left;
  }
  .right {
    grid-area: right;
  }
  .core {
    grid-area: core;
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
  }
  /* Too narrow for three columns: the ship and power side by side on
   * top, the two halves of the parts under them. */
  @container (width < 1400px) {
    .around {
      grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
      grid-template-areas:
        "core core"
        "left right";
    }
    .core {
      display: grid;
      grid-template-columns: minmax(0, 1fr) 380px;
    }
  }
  /* The holo window on Build: framed like the Ship tab's head; the system
   * under the pointer tints its glow (colour only, nothing moves). */
  .holo.framed {
    border: 1px solid color-mix(in srgb, var(--glow) 30%, var(--border));
    border-radius: var(--radius-lg);
    background: radial-gradient(
      120% 90% at 50% 45%,
      color-mix(in srgb, var(--glow) 10%, var(--bg-sunk)),
      var(--bg-sunk) 70%
    );
    transition:
      border-color 160ms var(--easing),
      background 160ms var(--easing);
  }
  .holo.framed.lit {
    border-color: color-mix(in srgb, var(--glow) 55%, var(--border));
  }
  /* A store picture is a full scene that hides the background, so the
   * glow lies over it, around the edge. */
  .holo.framed::after {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: inherit;
    pointer-events: none;
    box-shadow: inset 0 0 0 transparent;
    transition: box-shadow 160ms var(--easing);
  }
  .holo.framed.lit::after {
    box-shadow: inset 0 0 56px color-mix(in srgb, var(--glow) 45%, transparent);
  }
  .hint {
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .error {
    color: var(--bad);
  }
  .charts {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.4fr);
    gap: 16px;
  }
  .panel {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px 16px;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: color-mix(in srgb, var(--bg-raised) 45%, transparent);
  }
  .panel h3 {
    margin: 0;
  }
  /* The picker lies over the cards (the mining picker's pattern). */
  .list {
    position: relative;
  }
  /* Over the window, not the page: wherever the card sits, the picker
   * opens in view, its list scrolling inside it; a light veil keeps the
   * cards readable behind it. */
  .over {
    position: fixed;
    inset: 0;
    z-index: 30;
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding: 72px 24px 24px;
    background: color-mix(in srgb, var(--bg-sunk) 35%, transparent);
    animation: veil 140ms var(--easing);
  }
  @keyframes veil {
    from {
      background: transparent;
    }
  }
</style>
