<script lang="ts">
  // Ships: one ship on one page, meant to read in one screen with no
  // scrolling. Build assumes the ship is known: the part cards (alike parts
  // folded into one line) beside the power panel (sc-vehicles' network as
  // the ship's power screen shows it) and the figures parts and pips
  // change. The hull itself (role, structure, armour, flight, logistics,
  // charts) opens out of the holo. Reads the cooked game data only: no
  // network.
  import { onDestroy, onMount, untrack } from "svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { Select, createBrowseState, type SelectOption } from "@veelume/ui";
  import type { SystemView } from "$lib/bindings";
  import NoGameData from "$lib/components/catalog/NoGameData.svelte";
  import PowerPanel from "$lib/modules/ships/PowerPanel.svelte";
  import SlotCards, { SYSTEMS, change, type Entry, type Fig } from "$lib/modules/ships/SlotCards.svelte";
  import { shieldResist } from "$lib/modules/ships/figures";
  import { dataStore, ensureChannel } from "$lib/state/data.svelte";
  import { settingsStore } from "$lib/state/settings.svelte";
  import LoadoutBar from "$lib/modules/ships/LoadoutBar.svelte";
  import PartPicker from "$lib/modules/ships/PartPicker.svelte";
  import ShipDetails from "$lib/modules/ships/ShipDetails.svelte";
  import ShipHolo from "$lib/modules/ships/ShipHolo.svelte";
  import ShipMenu from "$lib/modules/ships/ShipMenu.svelte";
  import ShipsHome from "$lib/modules/ships/ShipsHome.svelte";
  import type { SlotView } from "$lib/bindings";
  import {
    closePicker,
    listenForShipChanges,
    loadImages,
    loadMarket,
    loadOwned,
    loadPlaces,
    refreshImages,
    shipsImages,
    loadHome,
    loadPage,
    loadScene,
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
    shipsScene,
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
  /** The view, in the URL like any view mode (frontend rule 7): every ship,
   *  or the open ship's Build page. Build needs a ship open; the list keeps
   *  it open, so Build goes back to it. */
  const VIEWS = [
    ["ships", "Ships"],
    ["build", "Build"],
  ] as const;
  type View = (typeof VIEWS)[number][0];
  const browse = createBrowseState({ tab: { kind: "one", default: "build" } });
  const view = $derived<View>(!shipsUi.ship || browse.values.tab === "ships" ? "ships" : "build");
  /** A ship picked from the list or the picker opens on its Build page. */
  function openFromList(guid: string) {
    openShip(guid);
    browse.set("tab", "build");
  }
  /** The holo opened into the ship's stats: an expansion, so page-local
   *  (frontend rule 7). It stays open across hulls, to compare them. */
  let details = $state(false);
  let holoSlot = $state<HTMLElement>();
  const holo = $derived(settingsStore.current?.ship_holo ?? false);
  /** Build: the holo in the middle with the fighting parts left of it and
   *  the running parts right. */
  const FIGHT: Entry[][] = [[["weapons", "turrets"], "missiles", "countermeasures", "shields"]];
  const RUN: SystemView[][] = [["powerPlants", "coolers", "quantum", "radar", "lifeSupport", "fuel", "docked"]];
  let hover = $state<SystemView | null>(null);
  let hoverPorts = $state<string[] | null>(null);
  let hoverLine = $state<string[] | null>(null);
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
        void loadShips(c).then(() => {
          void loadImages(c);
          void loadMarket(c);
        });
        void loadPlaces(c);
        refreshImages();
      });
  });

  // The list refreshes whenever it shows (recently opened moves).
  $effect(() => {
    const c = channel;
    if (c && view === "ships") untrack(() => void loadHome(c));
  });

  // Recompute when the ship, its edits or the power settings change.
  $effect(() => {
    const c = channel;
    const key = JSON.stringify([shipsUi.ship, shipsUi.edits[shipsUi.ship], shipsUi.mode, shipsUi.off[shipsUi.ship], shipsUi.levels[shipsUi.ship]]);
    if (!c || !shipsUi.ship || !key) return;
    untrack(() => void loadPage(c));
  });

  // The holo draws what is fitted: again when the parts change (the store
  // skips an unchanged loadout, so a power change costs nothing). Once the
  // page is in, for the same reason as below.
  $effect(() => {
    const c = channel;
    const ship = shipsUi.ship;
    const edits = JSON.stringify(shipsUi.edits[ship]);
    if (c && ship && edits !== "" && holo && view === "build" && page?.ship === ship) untrack(() => void loadScene(c));
  });

  // Where the open ship is sold and rented (cached UEX prices), and your
  // copies' loadouts in the game (the cached inventory): no network. Once
  // its page is in: then the game data both read is loaded (asked any
  // sooner after a start, the commands failed and never retried).
  $effect(() => {
    const c = channel;
    const ship = shipsUi.ship;
    if (c && ship && page?.ship === ship)
      untrack(() => {
        void loadShipPrices(c, ship);
        void loadOwned(c, ship);
      });
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
    <!-- Mining's tab row: the views on the left; the open ship, its
         edition and its loadout on the right. The list searches and
         filters every ship itself; the ship select switches once one is
         open. -->
    <div class="tabs" role="tablist" aria-label="View">
      {#each VIEWS as [id, label] (id)}
        <button
          role="tab"
          class="tab"
          class:active={view === id}
          aria-selected={view === id}
          disabled={id !== "ships" && !shipsUi.ship}
          title={id !== "ships" && !shipsUi.ship ? "Pick a ship first" : undefined}
          onclick={() => browse.set("tab", id)}>{label}</button
        >
      {/each}
      <!-- The picker shows on every tab ("Pick a ship" with none open), so
           the greyed Build tab has its way in beside it. -->
      {#if shipsUi.ship}
        <div class="loadout">
          <LoadoutBar {channel} {shipName} />
        </div>
      {/if}
      <div class="open">
        <ShipMenu ships={spaceships.filter((s) => !s.base)} value={baseOf} onpick={openFromList} />
        {#if shipsUi.ship && editions.length > 1}
          <Select label="Edition" options={editionOptions} value={shipsUi.ship} onchange={openShip} />
        {/if}
      </div>
    </div>
    {#if view === "ships"}
      <ShipsHome ships={ships} home={shipsHome.view} onopen={openFromList} />
    {:else if shipsStore.pageError}
      <p class="error">{shipsStore.pageError}</p>
    {:else if page && page.ship === shipsUi.ship}
      {@const s = page.summary}
      {@const picture = shipsImages.byGuid.get(shipsUi.ship)}
      {@const glow = SYSTEMS.find((x) => x.id === hover)?.hue ?? null}
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
      {#snippet cards(columns: Entry[][])}
        <SlotCards
          slots={page.slots}
          off={shipsUi.off[shipsUi.ship] ?? []}
          {powered}
          {guns}
          {columns}
          {lines}
          {stock}
          selected={shipsPicker.open?.targets[0].join("/") ?? null}
          onpick={(ss) => channel && void openPicker(channel, ss.map((s) => s.path))}
          ontoggle={toggleOff}
          onhover={(s, ports, line) => {
            hover = s;
            hoverPorts = ports;
            hoverLine = line ? ports : null;
          }}
          weaponPool={page.power?.columns.find((c) => c.itemType === "WeaponGun") ?? null}
        />
      {/snippet}
      {#if view === "build" && shipsPicker.open}
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
      <!-- The ship in the middle over its power screen; the parts that
           fight on its left, the parts that run it on its right. The holo
           opens the ship's own stats over all of it. -->
      <div class="stage">
        <div class="around" class:under={details}>
          <div class="side left fades" inert={details}>{@render cards(FIGHT)}</div>
          <div class="core">
            <div class="holo-slot" bind:this={holoSlot}>
              <ShipHolo
                {picture}
                live={holo}
                scene={shipsScene.of(shipsUi.ship)}
                open={details}
                slot={holoSlot}
                hue={glow}
                lit={hoverPorts}
                focus={hoverLine}
                onclick={() => (details = !details)}
              />
            </div>
            <div class="fades" inert={details}>{@render power()}</div>
          </div>
          <div class="side right fades" inert={details}>{@render cards(RUN)}</div>
        </div>
        {#if details}
          <ShipDetails {page} {row} name={shipName} from={holoSlot} onclose={() => (details = false)} />
        {/if}
      </div>
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
  /* The page's gap spaces the tabs (app.css draws them, as on Mining). */
  .tabs {
    margin-bottom: 0;
    flex-wrap: wrap;
  }
  /* The open loadout in the middle of the tab row, taking the room between
   * the tabs and the ship. */
  .loadout {
    flex: 1 1 auto;
    align-self: center;
    margin-inline: 16px;
    padding-bottom: 6px;
  }
  /* The open ship and its edition at the tab row's end, on the tabs' line;
   * on a narrow window they wrap under the tabs rather than scrolling the
   * page sideways. */
  .open {
    margin-left: auto;
    align-self: center;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: flex-end;
    gap: 8px 10px;
    padding-bottom: 6px;
  }
  /* What the ship sheet lies over. */
  .stage {
    position: relative;
    flex: 1 0 auto;
  }
  /* The holo card, at a fixed height so the power screen starts in view
   * under it. It keeps its place while the holo is expanded. */
  .holo-slot {
    height: clamp(220px, 32vh, 380px);
  }
  /* Build: the ship and its power screen in the middle, the parts around
   * it. The middle column is capped so the parts keep their room. */
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
  /* The sheet can be shorter than the cards; they go, it doesn't stretch.
   * The holo stays: it is the sheet's centre. */
  .fades {
    transition: opacity 200ms var(--easing);
  }
  .around.under .fades {
    opacity: 0;
  }
  /* A long list of cards must not stretch the expanded holo past the
   * window and push the ship out of view. */
  .around.under {
    height: 0;
    overflow: hidden;
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
  .hint {
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .error {
    color: var(--bad);
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
