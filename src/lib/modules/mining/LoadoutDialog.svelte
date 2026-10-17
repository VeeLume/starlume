<script lang="ts">
  // A loadout in the editor. `live` edits your part of the calculator's
  // (every change solves at once; crew mates brought along stay as they
  // are) and offers to save it; otherwise it edits a saved loadout, yours
  // or a crew mate's, as a draft until Save.
  import { untrack } from "svelte";
  import { Dialog } from "@veelume/ui";
  import type { LoadoutView, MiningCatalogView, SavedLoadoutView } from "$lib/bindings";
  import { deleteLoadout, miningStore, saveLoadout, setLoadout } from "$lib/state/mining.svelte";
  import { ownLasers, sameLoadout } from "./format";
  import LoadoutEditor from "./LoadoutEditor.svelte";

  let {
    open,
    onclose,
    catalog,
    live = false,
    saved = null,
    crew = false,
  }: {
    open: boolean;
    onclose: () => void;
    catalog: MiningCatalogView;
    /** Edit the calculator's loadout. */
    live?: boolean;
    /** The saved loadout to edit; `null` for a new one. */
    saved?: SavedLoadoutView | null;
    /** A new loadout is a crew mate's. */
    crew?: boolean;
  } = $props();

  const empty = (): LoadoutView => ({
    lasers: catalog.lasers[0] ? [{ laser: catalog.lasers[0].id, modules: [], owner: 0, who: null, crew: null }] : [],
    gadget: null,
    lead: null,
  });

  let draft = $state<LoadoutView>({ lasers: [], gadget: null, lead: null });
  let name = $state("");
  let owner = $state("");
  let error = $state("");

  const current = $derived(miningStore.state?.current ?? { lasers: [], gadget: null, lead: null });
  const own = $derived<LoadoutView>({ ...current, lasers: ownLasers(current) });
  const match = $derived(
    (miningStore.state?.saved ?? []).find((s) => s.owner === null && sameLoadout(s.loadout, own)) ?? null,
  );
  const isCrew = $derived(saved ? saved.owner !== null : crew);

  /** Your edited lasers with the crew mates' kept, on ships after yours. */
  function withCrew(next: LoadoutView): LoadoutView {
    const crewLasers = current.lasers.filter((l) => l.crew !== null);
    const after = Math.max(-1, ...next.lasers.map((l) => l.owner)) + 1;
    const base = Math.min(...crewLasers.map((l) => l.owner), Infinity);
    return {
      ...next,
      gadget: current.gadget,
      lead: null,
      lasers: [...next.lasers, ...crewLasers.map((l) => ({ ...l, owner: after + (l.owner - base) }))],
    };
  }

  // A fresh draft each time it opens.
  $effect(() => {
    if (!open) return;
    untrack(() => {
      error = "";
      if (live) {
        name = match?.name ?? "";
        return;
      }
      draft = saved ? structuredClone($state.snapshot(saved.loadout)) : empty();
      name = saved?.name ?? "";
      owner = saved?.owner ?? "";
    });
  });

  async function save() {
    if (live) {
      error = (await saveLoadout(match?.id ?? null, name, null, own)) ?? "";
    } else {
      if (isCrew && !owner.trim()) {
        error = "Whose loadout is it?";
        return;
      }
      error = (await saveLoadout(saved?.id ?? null, name, isCrew ? owner : null, draft)) ?? "";
    }
    if (!error) onclose();
  }

  async function remove() {
    if (saved) await deleteLoadout(saved.id);
    onclose();
  }
</script>

<Dialog
  {open}
  {onclose}
  title={live ? "Your ship" : saved ? `Edit ${saved.name}` : isCrew ? "Crew loadout" : "Loadout"}
  class="max-w-xl!"
>
  <form
    class="body"
    onsubmit={(e) => {
      e.preventDefault();
      void save();
    }}
  >
    <div class="fields">
      {#if isCrew && !live}
        <label><span class="label">Player</span><input class="input" bind:value={owner} placeholder="Juno" /></label>
      {/if}
      <label>
        <span class="label">Name</span>
        <input class="input" bind:value={name} placeholder={isCrew ? "Prospector" : "Golem"} />
      </label>
    </div>
    {#if live}
      <LoadoutEditor {catalog} loadout={own} onchange={(next) => void setLoadout(withCrew(next))} />
    {:else}
      <LoadoutEditor {catalog} loadout={draft} onchange={(next) => (draft = next)} />
    {/if}
    {#if error}<p class="error">{error}</p>{/if}
    <div class="foot">
      {#if saved}<button type="button" class="btn subtle danger" onclick={() => void remove()}>Delete</button>{/if}
      <span class="grow"></span>
      <button type="button" class="btn subtle" onclick={onclose}>{live ? "Done" : "Cancel"}</button>
      <button type="submit" class="btn primary" disabled={!name.trim()}>{live && match ? "Save" : live ? "Save as loadout" : "Save"}</button>
    </div>
  </form>
</Dialog>

<style>
  .body {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    padding: 12px 16px 16px;
    overflow: auto;
  }
  .fields {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-3);
  }
  .fields label {
    flex: 1 1 160px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .foot {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .grow {
    flex: 1;
  }
  .danger {
    color: var(--bad);
  }
  .error {
    margin: 0;
  }
</style>
