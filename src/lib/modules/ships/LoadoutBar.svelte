<script lang="ts">
  // The open ship's loadout, in the page bar (design note "Ship loadout
  // page: design direction", pass 6): which one is open (Stock, your ship's
  // Insured or Live loadout in the game, or a saved one), how far the edits
  // are from it, and Revert / Save as new / Save on the same row, as
  // mining's try-out does. The game's loadouts are read-only (the inventory
  // My Items last read): a try-out against one saves as a new loadout. Save
  // as new names the loadout in place, no dialog. Opening another loadout
  // over unsaved changes asks first; deleting one asks too.
  import { ConfirmDialog, Select } from "@veelume/ui";
  import {
    changesOf,
    deleteLoadout,
    inGame,
    openLoadout,
    saveLoadout,
    shipsLoadouts,
    shipsOwned,
    shipsUi,
    swapsFromStock,
  } from "$lib/state/ships.svelte";

  let { channel, shipName }: { channel: string; shipName: string } = $props();

  const ship = $derived(shipsUi.ship);
  const saved = $derived(shipsLoadouts.of(ship));
  const owned = $derived(shipsOwned.of(ship));
  const changes = $derived(changesOf(ship));
  const swaps = $derived(swapsFromStock(ship));

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

  /** "Stock" is the select's value for no base. */
  const STOCK = "stock";
  const options = $derived([{ value: STOCK, label: "Stock" }, ...gameOptions, ...saved.map((s) => ({ value: s.id, label: s.name }))]);
  /** What the draft is measured against: a saved loadout or one in the
   *  game; null: stock. */
  const based = $derived.by(() => {
    const id = shipsUi.basedOn[ship];
    const o = options.find((x) => x.value === id);
    return id && o ? { id, name: o.label, game: inGame(id) } : null;
  });

  let naming = $state(false);
  let newName = $state("");
  let error = $state("");
  let asking = $state<{ kind: "open"; id: string | null; label: string } | { kind: "delete" } | null>(null);

  function pick(value: string) {
    const id = value === STOCK ? null : value;
    if (id === (based?.id ?? null)) return;
    if (changes > 0) asking = { kind: "open", id, label: options.find((o) => o.value === value)?.label ?? "it" };
    else void openLoadout(id);
  }

  function startNaming() {
    newName = based && !based.game ? `${based.name} 2` : `${shipName} loadout`;
    error = "";
    naming = true;
  }

  async function save(asNew: boolean) {
    const name = asNew ? newName.trim() : (based?.name ?? "");
    if (!name) return;
    error = (await saveLoadout(channel, name, asNew)) ?? "";
    if (!error) naming = false;
  }

  function confirm() {
    const a = asking;
    asking = null;
    if (a?.kind === "open") void openLoadout(a.id);
    else if (a?.kind === "delete" && based) void deleteLoadout(based.id);
  }
</script>

<div class="lo">
  <div class="pick"><Select label="Loadout" {options} value={based?.id ?? STOCK} onchange={pick} /></div>
  <div class="acts">
    {#if naming}
      <!-- svelte-ignore a11y_autofocus -->
      <input
        class="input"
        bind:value={newName}
        aria-label="Name of the new loadout"
        autofocus
        onkeydown={(e) => {
          if (e.key === "Enter") void save(true);
          if (e.key === "Escape") naming = false;
        }}
      />
      <button type="button" class="btn" onclick={() => (naming = false)}>Cancel</button>
      <button type="button" class="btn primary" disabled={!newName.trim()} onclick={() => void save(true)}>Save</button>
    {:else if based}
      {#if changes > 0}
        <span class="hue-chip h-loot">{changes} unsaved change{changes === 1 ? "" : "s"}</span>
        <button type="button" class="btn" onclick={() => void openLoadout(based.id)}>Revert</button>
        <button type="button" class="btn" class:primary={based.game} onclick={startNaming}>Save as new</button>
        {#if !based.game}<button type="button" class="btn primary" onclick={() => void save(false)}>Save</button>{/if}
      {:else if !based.game}
        <button type="button" class="btn subtle" onclick={() => (asking = { kind: "delete" })}>Delete</button>
      {/if}
    {:else if changes > 0}
      <span class="hue-chip h-loot">{swaps} changed from stock</span>
      <button type="button" class="btn" onclick={() => void openLoadout(null)}>Back to stock</button>
      <button type="button" class="btn primary" onclick={startNaming}>Save as new</button>
    {/if}
    {#if error}<span class="err">{error}</span>{/if}
  </div>
</div>

<ConfirmDialog
  open={asking !== null}
  onclose={() => (asking = null)}
  onconfirm={confirm}
  title={asking?.kind === "delete" ? `Delete ${based?.name ?? "this loadout"}?` : `Open ${asking?.kind === "open" ? asking.label : ""}?`}
  description={asking?.kind === "delete"
    ? "The saved loadout goes. The parts on the ship now stay, as changes from stock."
    : `Your ${changes} unsaved change${changes === 1 ? "" : "s"} will be lost.`}
  confirmLabel={asking?.kind === "delete" ? "Delete" : "Open without saving"}
  destructive
/>

<style>
  /* The select centred, its actions beside it in a column reserved at their
   * widest, so they appear without moving the select or the ship picker. */
  .lo {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(26rem, 1fr);
    align-items: center;
    column-gap: 8px;
  }
  .pick {
    grid-column: 2;
  }
  .acts {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .input {
    width: 14rem;
    font-size: var(--text-small);
    padding-block: 4px;
  }
  .err {
    font-size: var(--text-small);
    color: var(--bad);
  }
</style>
