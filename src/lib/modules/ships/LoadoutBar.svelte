<script lang="ts">
  // The open ship's loadout, in the page bar (design note "Ship loadout
  // page: design direction", pass 6): which one is open (Stock or a saved
  // one), how far the edits are from it, and Revert / Save as new / Save on
  // the same row, as mining's try-out does. Save as new names the loadout
  // in place, no dialog. Opening another loadout over unsaved changes asks
  // first; deleting one asks too.
  import { ConfirmDialog, Select } from "@veelume/ui";
  import {
    changesOf,
    deleteLoadout,
    openLoadout,
    saveLoadout,
    shipsLoadouts,
    shipsUi,
    swapsFromStock,
  } from "$lib/state/ships.svelte";

  let { channel, shipName }: { channel: string; shipName: string } = $props();

  const ship = $derived(shipsUi.ship);
  const saved = $derived(shipsLoadouts.of(ship));
  const based = $derived(saved.find((s) => s.id === shipsUi.basedOn[ship]) ?? null);
  const changes = $derived(changesOf(ship));
  const swaps = $derived(swapsFromStock(ship));

  /** "Stock" is the select's value for no saved loadout. */
  const STOCK = "stock";
  const options = $derived([{ value: STOCK, label: "Stock" }, ...saved.map((s) => ({ value: s.id, label: s.name }))]);

  let naming = $state(false);
  let newName = $state("");
  let error = $state("");
  let asking = $state<{ kind: "open"; id: string | null; label: string } | { kind: "delete" } | null>(null);

  function pick(value: string) {
    const id = value === STOCK ? null : value;
    if (id === (based?.id ?? null)) return;
    if (changes > 0) asking = { kind: "open", id, label: id ? (saved.find((s) => s.id === id)?.name ?? "it") : "Stock" };
    else void openLoadout(id);
  }

  function startNaming() {
    newName = based ? `${based.name} 2` : `${shipName} loadout`;
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
  <Select label="Loadout" {options} value={based?.id ?? STOCK} onchange={pick} />
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
      <button type="button" class="btn" onclick={startNaming}>Save as new</button>
      <button type="button" class="btn primary" onclick={() => void save(false)}>Save</button>
    {:else}
      <button type="button" class="btn subtle" onclick={() => (asking = { kind: "delete" })}>Delete</button>
    {/if}
  {:else if changes > 0}
    <span class="hue-chip h-loot">{swaps} changed from stock</span>
    <button type="button" class="btn" onclick={() => void openLoadout(null)}>Back to stock</button>
    <button type="button" class="btn primary" onclick={startNaming}>Save as new</button>
  {/if}
  {#if error}<span class="err">{error}</span>{/if}
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
  .lo {
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
