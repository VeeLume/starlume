<script lang="ts">
  // The open ship's loadout, in the page bar: one control for which one is
  // open and everything done to it. The button says which (Stock, your
  // ship's Insured or Live loadout in the game, or a saved one) and how far
  // the edits are from it. The menu lists them, with rename and delete on
  // each saved one, and Revert / Save as new / Save at its foot while there
  // are edits. The game's loadouts are read-only (the inventory My Items
  // last read): a try-out against one saves as a new loadout. Names are
  // typed in place, no dialog. Opening another loadout over unsaved changes
  // asks first; deleting one asks too. Esc or a click outside closes.
  import { ConfirmDialog } from "@veelume/ui";
  import { Check, ChevronDown, Pencil, Trash2 } from "lucide-svelte";
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
  /** Your copies' loadouts in the game: Insured always (stock until it was
   *  changed), Live when the copy is in a stash. Several copies: each named
   *  by where it is. */
  const gameOptions = $derived.by(() => {
    const ships = owned?.ships ?? [];
    return ships.flatMap((o, i) => {
      const which = ships.length > 1 ? ` · ${o.place ?? `copy ${i + 1}`}` : "";
      const read = owned?.readAt ? `, read ${ago(owned.readAt)}` : "";
      return [
        { value: `insured:${o.urn}`, label: `Insured${which}` },
        ...(o.live ? [{ value: `live:${o.urn}`, label: `Live${which}${read}` }] : []),
      ];
    });
  });

  /** "Stock" is the menu's value for no base. */
  const STOCK = "stock";
  const fixed = $derived([{ value: STOCK, label: "Stock" }, ...gameOptions]);
  const options = $derived([...fixed, ...saved.map((s) => ({ value: s.id, label: s.name }))]);
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
          {#each fixed as o (o.value)}
            <div class="row" class:sel={o.value === current}>
              <button type="button" class="name" onclick={() => pick(o.value)}><span>{o.label}</span></button>
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
    left: 0;
    right: 0;
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
