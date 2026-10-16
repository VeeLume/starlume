<script lang="ts">
  // The ship(s) on the rock: lasers with their module slots, which ship each
  // is on (lasers on one ship split the instability wave — a MOLE is one
  // ship), and the gadget. Any laser can be picked: which lasers fit which
  // ship is not in the game data model yet. A crew mate's lasers (brought along
  // from their loadout) carry their name and come off with the ×.
  import { Plus, X } from "lucide-svelte";
  import type { LaserInputView, LoadoutView, MiningCatalogView } from "$lib/bindings";
  import Monogram from "$lib/components/Monogram.svelte";

  let {
    catalog,
    loadout,
    onchange,
  }: {
    catalog: MiningCatalogView;
    loadout: LoadoutView;
    onchange: (next: LoadoutView) => void;
  } = $props();

  const laserById = $derived(new Map(catalog.lasers.map((l) => [l.id, l])));
  const ships = $derived(Math.max(1, ...loadout.lasers.filter((l) => !l.who).map((l) => l.owner + 2)));

  function setLaser(i: number, patch: Partial<LaserInputView>) {
    const lasers = loadout.lasers.map((l, j) => (j === i ? { ...l, ...patch } : l));
    onchange({ ...loadout, lasers });
  }

  function pickLaser(i: number, id: string) {
    const slots = laserById.get(id)?.moduleSlots ?? 0;
    setLaser(i, { laser: id, modules: loadout.lasers[i].modules.slice(0, slots) });
  }

  function pickModule(i: number, slot: number, id: string) {
    const modules = [...loadout.lasers[i].modules];
    if (id) modules[slot] = id;
    else modules.splice(slot, 1);
    setLaser(i, { modules: modules.filter(Boolean) });
  }

  function addLaser() {
    const last = loadout.lasers.at(-1);
    onchange({
      ...loadout,
      lasers: [
        ...loadout.lasers,
        { laser: last?.laser ?? catalog.lasers[0]?.id ?? "", modules: [], owner: last?.owner ?? 0, who: null },
      ],
    });
  }

  function removeLaser(i: number) {
    onchange({ ...loadout, lasers: loadout.lasers.filter((_, j) => j !== i) });
  }
</script>

<div class="lasers">
  {#each loadout.lasers as l, i (i)}
    {@const laser = laserById.get(l.laser)}
    <div class="laser">
      <div class="head">
        {#if l.who}<span class="who"><Monogram name={l.who} size={22} />{l.who}</span>{/if}
        <select class="select grow" value={l.laser} onchange={(e) => pickLaser(i, e.currentTarget.value)} aria-label="Laser">
          {#if !laser}<option value={l.laser}>Unknown laser</option>{/if}
          {#each catalog.lasers as opt (opt.id)}
            <option value={opt.id}>{opt.name}</option>
          {/each}
        </select>
        {#if loadout.lasers.length > 1 && !l.who}
          <select
            class="select"
            value={String(l.owner)}
            onchange={(e) => setLaser(i, { owner: Number(e.currentTarget.value) })}
            aria-label="Ship"
            title="Lasers on one ship split the instability wave; a MOLE is one ship"
          >
            {#each Array.from({ length: ships }, (_, n) => n) as n (n)}
              <option value={String(n)}>Ship {n + 1}</option>
            {/each}
          </select>
        {/if}
        <button class="btn subtle icon" onclick={() => removeLaser(i)} title="Remove laser" aria-label="Remove laser">
          <X size={14} />
        </button>
      </div>
      {#if laser}
        <div class="slots">
          {#each Array.from({ length: laser.moduleSlots }, (_, s) => s) as s (s)}
            <select
              class="select"
              value={l.modules[s] ?? ""}
              onchange={(e) => pickModule(i, s, e.currentTarget.value)}
              aria-label="Module slot {s + 1}"
            >
              <option value="">— empty slot —</option>
              {#each catalog.modules as m (m.id)}
                <option value={m.id}>{m.name}{m.active ? " (active)" : ""}</option>
              {/each}
            </select>
          {/each}
          {#if laser.moduleSlots === 0}<span class="dim small">no module slots</span>{/if}
        </div>
        <p class="dim small facts">
          {laser.power.toFixed(0)} power · S{laser.size}
          {#if laser.optimalRangeM !== null && laser.maxRangeM !== null}
            · {laser.optimalRangeM.toFixed(0)}–{laser.maxRangeM.toFixed(0)} m{/if}
        </p>
      {/if}
    </div>
  {/each}
  <button class="btn subtle add" onclick={addLaser}><Plus size={14} /> Add laser</button>
</div>

<label class="gadget">
  <span class="label">Gadget</span>
  <select
    class="select"
    value={loadout.gadget ?? ""}
    onchange={(e) => onchange({ ...loadout, gadget: e.currentTarget.value || null })}
  >
    <option value="">No gadget</option>
    {#each catalog.gadgets as g (g.id)}
      <option value={g.id}>{g.name}</option>
    {/each}
  </select>
</label>

<style>
  .lasers {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .laser {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding-bottom: var(--space-2);
    border-bottom: 1px solid var(--border-soft);
  }
  .head,
  .slots {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }
  .grow {
    flex: 1;
    max-width: none;
  }
  .who {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--text-small);
    font-weight: var(--weight-semibold);
  }
  .slots .select {
    flex: 1 1 140px;
  }
  .icon {
    padding: 4px 6px;
    display: inline-flex;
  }
  .add {
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .small {
    font-size: var(--text-small);
  }
  .facts {
    margin: 0;
  }
  .gadget {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin-top: var(--space-2);
  }
</style>
