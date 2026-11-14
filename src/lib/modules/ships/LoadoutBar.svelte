<script lang="ts">
  // The open ship's loadout, in the page bar: one control for which one is
  // open and everything done to it. The button says which (Stock, a copy
  // you own as Saved or Fitted now, or a loadout saved here) and how far
  // the edits are from it. The menu lists them: Stock, then each copy you
  // own under a heading that says what it is ("Pledge, Area 18 hangar",
  // "Rental, ends 16 Nov 14:00") with its Saved and Fitted now beneath and
  // the way to the same ship in My Items, then the loadouts saved here with
  // rename and delete, and Revert / Save as new / Save at the foot while
  // there are edits. A copy with nothing fitted to read keeps its Fitted
  // now line, off, with the reason. The game's loadouts are read-only (the
  // inventory My Items last read): a try-out against one saves as a new
  // loadout. Names are typed in place, no dialog. Opening another loadout
  // over unsaved changes asks first; deleting one asks too. Esc or a click
  // outside closes.
  import { untrack } from "svelte";
  import { goto } from "$app/navigation";
  import { ConfirmDialog } from "@veelume/ui";
  import { ArrowUpRight, Check, ChevronDown, Pencil, Target, Trash2 } from "lucide-svelte";
  import { copyHeadings, FITTED, myItemsShipHref, NO_READ, SAVED } from "$lib/ownedShips";
  import { addLoadoutPlan, loadPlans, plansStore } from "$lib/state/plans.svelte";
  import { settingsStore } from "$lib/state/settings.svelte";
  import {
    changesOf,
    deleteLoadout,
    inGame,
    openLoadout,
    renameLoadout,
    saveLoadout,
    shipsLoadouts,
    shipsOwned,
    shipsUi,
  } from "$lib/state/ships.svelte";

  let { channel, shipName }: { channel: string; shipName: string } = $props();

  const ship = $derived(shipsUi.ship);
  const saved = $derived(shipsLoadouts.of(ship));
  const owned = $derived(shipsOwned.of(ship));
  const changes = $derived(changesOf(ship));

  const ago = (t: number) => {
    const h = (Date.now() / 1000 - t) / 3600;
    return h < 1 ? "just now" : h < 24 ? `${Math.floor(h)} h ago` : `${Math.floor(h / 24)} d ago`;
  };
  /** Your copies of the open ship, each under what it is. Saved is always
   *  there (stock until it was changed); Fitted now only with a read of the
   *  copy, else the reason it has none. */
  const copies = $derived.by(() => {
    const ships = owned?.ships ?? [];
    const heads = copyHeadings(ships);
    return ships.map((o, i) => ({
      urn: o.urn,
      head: heads[i],
      saved: `insured:${o.urn}`,
      fitted: o.live ? `live:${o.urn}` : null,
      why: o.noRead ? NO_READ[o.noRead] : "",
      href: myItemsShipHref(o.urn, o.ownership === "bricked"),
    }));
  });
  const read = $derived(owned?.readAt ? `read ${ago(owned.readAt)}` : "");
  /** My Items is a module of its own: no way there while it is off. */
  const myItems = $derived(settingsStore.current?.enabled_modules.includes("items") ?? false);

  /** Plans is a module of its own too: its mark shows only while it is on. */
  const plans = $derived(settingsStore.current?.enabled_modules.includes("plans") ?? false);
  $effect(() => {
    if (plans && plansStore.plans === null) void loadPlans();
  });

  /** "Stock" is the menu's value for no base. */
  const STOCK = "stock";
  /** Every loadout that can be open, by the name the button gives it: one
   *  copy needs no more than "Saved"; several say which. */
  const options = $derived([
    { value: STOCK, label: "Stock" },
    ...copies.flatMap((c) => {
      const which = copies.length > 1 ? `, ${c.head}` : "";
      return [
        { value: c.saved, label: `${SAVED}${which}` },
        ...(c.fitted ? [{ value: c.fitted, label: `${FITTED}${which}` }] : []),
      ];
    }),
    ...saved.map((s) => ({ value: s.id, label: s.name })),
  ]);
  /** What the draft is measured against: a saved loadout or one in the
   *  game; null: stock. */
  const based = $derived.by(() => {
    const id = shipsUi.basedOn[ship];
    const o = options.find((x) => x.value === id);
    return id && o ? { id, name: o.label, game: inGame(id) } : null;
  });
  const current = $derived(based?.id ?? STOCK);

  let open = $state(false);
  let el = $state<HTMLElement>();
  let naming = $state(false);
  let newName = $state("");
  /** The saved loadout being renamed in its row. */
  let renaming = $state<string | null>(null);
  let renameTo = $state("");
  let error = $state("");
  let asking = $state<{ kind: "open"; id: string | null; label: string } | { kind: "delete"; id: string; name: string } | null>(null);

  function close() {
    open = false;
    naming = false;
    renaming = null;
    error = "";
  }

  function pick(value: string) {
    close();
    const id = value === STOCK ? null : value;
    if (id === (based?.id ?? null)) return;
    if (changes > 0) asking = { kind: "open", id, label: options.find((o) => o.value === value)?.label ?? "it" };
    else void openLoadout(id);
  }

  // A loadout asked for from My Items ("Open in Ships"): opened like a
  // pick from the menu, so unsaved changes are asked about first. A copy
  // the inventory no longer has is dropped without a word.
  $effect(() => {
    const w = shipsUi.wanted;
    if (!w || w.ship !== ship || !owned) return;
    untrack(() => {
      shipsUi.wanted = null;
      if (options.some((o) => o.value === w.id)) pick(w.id);
    });
  });

  function startNaming() {
    newName = based && !based.game ? `${based.name} 2` : `${shipName} loadout`;
    error = "";
    renaming = null;
    naming = true;
  }

  async function save(asNew: boolean) {
    const name = asNew ? newName.trim() : (based?.name ?? "");
    if (!name) return;
    error = (await saveLoadout(channel, name, asNew)) ?? "";
    if (!error) close();
  }

  function revert() {
    const id = based?.id ?? null;
    close();
    void openLoadout(id);
  }

  function startRename(id: string, name: string) {
    renameTo = name;
    error = "";
    naming = false;
    renaming = id;
  }

  async function rename() {
    const id = renaming;
    const name = renameTo.trim();
    if (!id || !name) return;
    error = (await renameLoadout(id, name)) ?? "";
    if (!error) renaming = null;
  }

  function askDelete(id: string, name: string) {
    close();
    asking = { kind: "delete", id, name };
  }

  function confirm() {
    const a = asking;
    asking = null;
    if (a?.kind === "open") void openLoadout(a.id);
    else if (a?.kind === "delete") void deleteLoadout(a.id);
  }

  /** Esc steps back: out of a name being typed first, then out of the menu. */
  function key(e: KeyboardEvent) {
    if (e.key !== "Escape" || !open) return;
    if (renaming) renaming = null;
    else if (naming) naming = false;
    else close();
    error = "";
    e.preventDefault(); // the ship sheet under the menu stays open
  }

  function outside(e: MouseEvent) {
    const t = e.target as HTMLElement;
    if (open && el && t.isConnected && !el.contains(t)) close();
  }
</script>

<svelte:window onkeydown={key} onclick={outside} />

<div class="lo">
  <div class="menu" bind:this={el}>
    <button
      type="button"
      class="trigger"
      class:on={open}
      aria-haspopup="true"
      aria-expanded={open}
      aria-label="Loadout: {based?.name ?? 'Stock'}"
      onclick={() => (open ? close() : (open = true))}
    >
      <span class="cur">{based?.name ?? "Stock"}</span>
      {#if changes > 0}<span class="hue-chip h-loot">{changes} unsaved</span>{/if}
      <ChevronDown size={15} />
    </button>
    {#if open}
      <div class="pop" role="dialog" aria-label="Loadouts">
        <div class="list">
          <div class="row" class:sel={current === STOCK}>
            <button type="button" class="name" onclick={() => pick(STOCK)}><span>Stock</span></button>
          </div>
          {#each copies as c (c.urn)}
            <div class="copy" role="group" aria-label={c.head}>
              <p class="head">{c.head}</p>
              <div class="row" class:sel={c.saved === current}>
                <button type="button" class="name" onclick={() => pick(c.saved)}><span>{SAVED}</span></button>
              </div>
              {#if c.fitted}
                {@const id = c.fitted}
                <div class="row" class:sel={id === current}>
                  <button type="button" class="name" onclick={() => pick(id)}>
                    <span>{FITTED}</span>{#if read}<small>{read}</small>{/if}
                  </button>
                </div>
              {:else}
                <div class="row off">
                  <button type="button" class="name" disabled><span>{FITTED}</span><small>{c.why}</small></button>
                </div>
              {/if}
              {#if myItems}
                <a class="jump" href={c.href} onclick={close}>Show in My Items<ArrowUpRight size={13} /></a>
              {/if}
            </div>
          {/each}
          {#if saved.length}<hr />{/if}
          {#each saved as s (s.id)}
            <div class="row" class:sel={s.id === current}>
              {#if renaming === s.id}
                <!-- svelte-ignore a11y_autofocus -->
                <input
                  class="input"
                  bind:value={renameTo}
                  aria-label="New name for {s.name}"
                  autofocus
                  onkeydown={(e) => e.key === "Enter" && void rename()}
                />
                <button type="button" class="icon" title="Rename" aria-label="Rename to the typed name" disabled={!renameTo.trim()} onclick={() => void rename()}>
                  <Check size={14} />
                </button>
              {:else}
                <button type="button" class="name" onclick={() => pick(s.id)}><span>{s.name}</span></button>
                {#if plans}
                  {@const planned = !!plansStore.loadoutOf(s.id)}
                  <button
                    type="button"
                    class="icon"
                    class:planned
                    title={planned ? "In your plans: open Plans" : "Add to plan: what it takes to have this fitted"}
                    aria-label={planned ? `${s.name} is in your plans: open Plans` : `Add ${s.name} to your plans`}
                    onclick={() => (planned ? void goto("/plans") : void addLoadoutPlan(s.id))}
                  >
                    <Target size={14} />
                  </button>
                {/if}
                <button type="button" class="icon" title="Rename" aria-label="Rename {s.name}" onclick={() => startRename(s.id, s.name)}>
                  <Pencil size={14} />
                </button>
                <button type="button" class="icon bad" title="Delete" aria-label="Delete {s.name}" onclick={() => askDelete(s.id, s.name)}>
                  <Trash2 size={14} />
                </button>
              {/if}
            </div>
          {/each}
        </div>
        {#if changes > 0}
          <div class="foot">
            {#if naming}
              <!-- svelte-ignore a11y_autofocus -->
              <input
                class="input"
                bind:value={newName}
                aria-label="Name of the new loadout"
                autofocus
                onkeydown={(e) => e.key === "Enter" && void save(true)}
              />
              <div class="btns">
                <button type="button" class="btn" onclick={() => (naming = false)}>Cancel</button>
                <button type="button" class="btn primary" disabled={!newName.trim()} onclick={() => void save(true)}>Save</button>
              </div>
            {:else}
              <span class="count">{changes} unsaved change{changes === 1 ? "" : "s"}</span>
              <div class="btns">
                <button type="button" class="btn" onclick={revert}>{based ? "Revert" : "Back to stock"}</button>
                <button type="button" class="btn" class:primary={!based || based.game} onclick={startNaming}>Save as new</button>
                {#if based && !based.game}<button type="button" class="btn primary" onclick={() => void save(false)}>Save</button>{/if}
              </div>
            {/if}
          </div>
        {/if}
        {#if error}<p class="err">{error}</p>{/if}
      </div>
    {/if}
  </div>
</div>

<ConfirmDialog
  open={asking !== null}
  onclose={() => (asking = null)}
  onconfirm={confirm}
  title={asking?.kind === "delete" ? `Delete ${asking.name}?` : `Open ${asking?.kind === "open" ? asking.label : ""}?`}
  description={asking?.kind === "delete"
    ? asking.id === based?.id
      ? "The saved loadout goes. The parts on the ship now stay, as changes from stock."
      : "The saved loadout goes. The parts on the ship now stay as they are."
    : `Your ${changes} unsaved change${changes === 1 ? "" : "s"} will be lost.`}
  confirmLabel={asking?.kind === "delete" ? "Delete" : "Open without saving"}
  destructive
/>

<style>
  .lo {
    display: flex;
    justify-content: center;
  }
  .menu {
    position: relative;
    width: 300px;
  }
  /* Fixed width with the name giving way: the unsaved count appears
   * without moving anything on the row. */
  .trigger {
    all: unset;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 7px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-raised);
    color: var(--text-dim);
    cursor: pointer;
  }
  .trigger:hover,
  .trigger.on {
    border-color: var(--accent-border);
  }
  .trigger:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .cur {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    color: var(--text-strong);
    font-weight: var(--weight-medium);
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .pop {
    position: absolute;
    z-index: 20;
    top: calc(100% + 6px);
    /* A little wider than the button: a copy's heading and the reason
     * beside a Fitted now line need the room. */
    left: -20px;
    right: -20px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 6px;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--bg-raised);
    box-shadow: var(--shadow-panel);
    animation: rise 120ms var(--easing);
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 1px;
    max-height: min(420px, calc(100vh - 220px));
    overflow-y: auto;
  }
  hr {
    margin: 4px 6px;
    border: none;
    border-top: 1px solid var(--border);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 2px;
    padding-right: 4px;
    border-radius: var(--radius);
    color: var(--text);
  }
  .row:hover,
  .row:focus-within {
    background: color-mix(in srgb, var(--text) 5%, transparent);
  }
  .row.sel {
    background: var(--accent-fill-weak);
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .name {
    all: unset;
    box-sizing: border-box;
    flex: 1;
    min-width: 0;
    padding: 7px 10px;
    border-radius: var(--radius);
    cursor: pointer;
  }
  .name span {
    display: block;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  /* A copy you own: what it is, then its two loadouts and the way to it
   * in My Items, set in from the heading. */
  .copy {
    display: flex;
    flex-direction: column;
    gap: 1px;
    margin-top: 4px;
    padding-top: 6px;
    border-top: 1px solid var(--border);
  }
  .head {
    margin: 0;
    padding: 2px 10px 3px;
    font-size: var(--text-small);
    line-height: 1.35;
    color: var(--text-dim);
  }
  .copy .name {
    display: flex;
    align-items: baseline;
    gap: 10px;
    padding-left: 20px;
  }
  .copy .name span {
    flex: none;
  }
  /* Beside the loadout's name, as its own quieter element: when it was
   * read, or why there is nothing to read. */
  .name small {
    flex: 1;
    min-width: 0;
    font-size: var(--text-small);
    font-weight: var(--weight-regular, 400);
    color: var(--text-faint);
    text-align: right;
  }
  .row.off:hover {
    background: none;
  }
  .row.off .name {
    color: var(--text-faint);
    cursor: default;
  }
  .jump {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 5px 10px 5px 20px;
    border-radius: var(--radius);
    font-size: var(--text-small);
    color: var(--text-dim);
    text-decoration: none;
  }
  .jump:hover {
    color: var(--accent);
    background: color-mix(in srgb, var(--text) 5%, transparent);
  }
  .jump:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .row.sel .name {
    color: var(--text-strong);
    font-weight: var(--weight-medium);
  }
  /* Faded rather than absent until the row is hovered, so a row's width and
   * its name's ellipsis stay put. */
  .icon {
    all: unset;
    flex: none;
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: var(--radius);
    color: var(--text-faint);
    cursor: pointer;
    opacity: 0;
  }
  .row:hover .icon,
  .row.sel .icon,
  .row:focus-within .icon {
    opacity: 1;
  }
  .icon:hover:not(:disabled) {
    color: var(--text-strong);
    background: color-mix(in srgb, var(--text) 8%, transparent);
  }
  .row .icon.planned {
    opacity: 1;
    color: var(--accent);
  }
  .icon.bad:hover {
    color: var(--bad);
  }
  .icon:focus-visible {
    outline: 2px solid var(--accent);
  }
  .row .icon:disabled {
    cursor: default;
    opacity: 0.4;
  }
  .row .input {
    flex: 1;
    min-width: 0;
    margin: 2px 2px 2px 4px;
    font-size: var(--text-small);
    padding-block: 4px;
  }
  .foot {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 8px 6px 4px;
    border-top: 1px solid var(--border);
  }
  .foot .input {
    width: 100%;
    font-size: var(--text-small);
    padding-block: 4px;
  }
  .count {
    font-size: var(--text-small);
    color: var(--text-dim);
  }
  .btns {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
  }
  .err {
    margin: 0;
    padding: 0 6px 4px;
    font-size: var(--text-small);
    color: var(--bad);
  }
</style>
