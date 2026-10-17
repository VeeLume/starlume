<script lang="ts">
  // The ship(s) of a loadout, in the card language: one block per ship,
  // each laser as a picker with its figures (power, range, slots — so
  // choosing compares them) and its module slots as tiles with their
  // effects. A picker opens its list in place: lasers with their figures,
  // modules with their effects (active ones tagged), searchable. A ship
  // with several lasers (a MOLE) names who works each head. Which lasers
  // fit which ship is not in the game data model yet, so any laser can be
  // picked. The gadget belongs to the rock (the calculator's gadget card).
  import { Plus, X } from "lucide-svelte";
  import type { LaserInputView, LoadoutView, MiningCatalogView } from "$lib/bindings";
  import { effects, laserShort, operatorNames } from "./format";

  let {
    catalog,
    loadout,
    onchange,
  }: {
    catalog: MiningCatalogView;
    loadout: LoadoutView;
    onchange: (next: LoadoutView) => void;
  } = $props();

  /** The open list: a laser picker (`laser:i`) or a slot (`slot:i:s`). */
  let openList = $state("");
  let q = $state("");

  const laserById = $derived(new Map(catalog.lasers.map((l) => [l.id, l])));
  const moduleById = $derived(new Map(catalog.modules.map((m) => [m.id, m])));
  const owners = $derived([...new Set(loadout.lasers.map((l) => l.owner))].sort((a, b) => a - b));
  const seatNames = $derived(operatorNames(loadout.lasers));
  const query = $derived(q.trim().toLowerCase());
  const lasersShown = $derived(catalog.lasers.filter((l) => l.name.toLowerCase().includes(query)));
  const modulesShown = $derived(catalog.modules.filter((m) => m.name.toLowerCase().includes(query)));

  function toggle(key: string) {
    openList = openList === key ? "" : key;
    q = "";
  }

  function setLaser(i: number, patch: Partial<LaserInputView>) {
    onchange({ ...loadout, lasers: loadout.lasers.map((l, j) => (j === i ? { ...l, ...patch } : l)) });
  }

  function pickLaser(i: number, id: string) {
    const slots = laserById.get(id)?.moduleSlots ?? 0;
    setLaser(i, { laser: id, modules: loadout.lasers[i].modules.slice(0, slots) });
    openList = "";
  }

  function pickModule(i: number, slot: number, id: string | null) {
    const modules = [...loadout.lasers[i].modules];
    if (id) modules[slot] = id;
    else modules.splice(slot, 1);
    setLaser(i, { modules: modules.filter(Boolean) });
    openList = "";
  }

  function addLaser(owner: number) {
    const last = [...loadout.lasers].reverse().find((l) => l.owner === owner);
    onchange({
      ...loadout,
      lasers: [
        ...loadout.lasers,
        { laser: last?.laser ?? catalog.lasers[0]?.id ?? "", modules: [], owner, who: null, crew: null },
      ],
    });
  }

  function addShip() {
    const owner = Math.max(-1, ...loadout.lasers.map((l) => l.owner)) + 1;
    addLaser(owner);
  }

  function removeLaser(i: number) {
    onchange({ ...loadout, lead: null, lasers: loadout.lasers.filter((_, j) => j !== i) });
  }
</script>

<div class="ships">
  {#each owners as owner, n (owner)}
    {@const onShip = loadout.lasers.map((l, i) => ({ l, i })).filter((x) => x.l.owner === owner)}
    <section class="ship">
      <div class="sh">
        <span class="label">{owners.length > 1 ? `Ship ${n + 1}` : "Ship"}</span>
        <span class="grow"></span>
        <button class="link" onclick={() => addLaser(owner)}><Plus size={13} /> Laser on this ship</button>
      </div>
      {#each onShip as { l, i } (i)}
        {@const laser = laserById.get(l.laser)}
        <div class="head">
          {#if onShip.length > 1}
            <label class="seat">
              <span class="label">Head {onShip.findIndex((x) => x.i === i) + 1}</span>
              <input
                class="input"
                value={l.who ?? ""}
                placeholder={seatNames[i]}
                aria-label="Who works this head"
                onchange={(e) => setLaser(i, { who: e.currentTarget.value.trim() || null })}
              />
            </label>
          {/if}
          <div class="pickrow">
            <button class="laserpick" class:open={openList === `laser:${i}`} onclick={() => toggle(`laser:${i}`)}>
              <span class="ln"
                ><b>{laser ? laserShort(laser.name) : "Unknown laser"}</b><span
                  >{laser ? `Mining Laser S${laser.size}` : ""}</span
                ></span
              >
              {#if laser}
                <span class="lf">
                  <span><b>{Math.round(laser.power).toLocaleString("en-US")}</b>power</span>
                  {#if laser.optimalRangeM !== null && laser.maxRangeM !== null}
                    <span><b>{laser.optimalRangeM.toFixed(0)}–{laser.maxRangeM.toFixed(0)} m</b>range</span>
                  {/if}
                  <span><b>{laser.moduleSlots}</b>slots</span>
                </span>
              {/if}
              <span class="car">▾</span>
            </button>
            {#if loadout.lasers.length > 1}
              <button class="x" onclick={() => removeLaser(i)} title="Remove this laser" aria-label="Remove laser"
                ><X size={14} /></button
              >
            {/if}
          </div>
          {#if openList === `laser:${i}`}
            <div class="list">
              <input class="input" bind:value={q} placeholder="Find a laser" aria-label="Find a laser" />
              {#each lasersShown as opt (opt.id)}
                <button class="lr" class:cur={opt.id === l.laser} onclick={() => pickLaser(i, opt.id)}>
                  <b>{laserShort(opt.name)}</b>
                  <span class="faint">S{opt.size}</span>
                  <span class="mono">{Math.round(opt.power).toLocaleString("en-US")}</span>
                  <span class="faint"
                    >{opt.optimalRangeM !== null && opt.maxRangeM !== null
                      ? `${opt.optimalRangeM.toFixed(0)}–${opt.maxRangeM.toFixed(0)} m`
                      : ""}</span
                  >
                  <span class="faint">{opt.moduleSlots} slots</span>
                </button>
              {/each}
            </div>
          {/if}
          {#if laser && laser.moduleSlots > 0}
            <div class="slots">
              {#each Array.from({ length: laser.moduleSlots }, (_, s) => s) as s (s)}
                {@const m = l.modules[s] ? moduleById.get(l.modules[s]) : null}
                <button class="sl" class:empty={!m} class:on={openList === `slot:${i}:${s}`} onclick={() => toggle(`slot:${i}:${s}`)}>
                  <span class="sn">Slot {s + 1}</span>
                  <b>{m?.name ?? "Empty"}</b>
                  <span class="fx">
                    {#if m}
                      {#each effects(m.modifiers, m.powerMultiplier).slice(0, 2) as fx (fx.text)}
                        <span class="hue-chip" style:--hue={fx.good ? "var(--good)" : "var(--warn)"}>{fx.text}</span>
                      {/each}
                    {:else}<span class="faint">pick a module</span>{/if}
                  </span>
                </button>
              {/each}
            </div>
            {#each Array.from({ length: laser.moduleSlots }, (_, s) => s) as s (s)}
              {#if openList === `slot:${i}:${s}`}
                <div class="list">
                  <input class="input" bind:value={q} placeholder="Find a module" aria-label="Find a module" />
                  {#if l.modules[s]}
                    <button class="mr" onclick={() => pickModule(i, s, null)}><b class="faint">Empty slot</b></button>
                  {/if}
                  {#each modulesShown as m (m.id)}
                    <button class="mr" class:cur={m.id === l.modules[s]} onclick={() => pickModule(i, s, m.id)}>
                      <b>{m.name}</b>
                      {#if m.active}<span class="tag">active</span>{:else}<span></span>{/if}
                      <span class="fx">
                        {#each effects(m.modifiers, m.powerMultiplier).slice(0, 3) as fx (fx.text)}
                          <span class="hue-chip" style:--hue={fx.good ? "var(--good)" : "var(--warn)"}>{fx.text}</span>
                        {/each}
                      </span>
                    </button>
                  {/each}
                </div>
              {/if}
            {/each}
          {/if}
        </div>
      {/each}
    </section>
  {/each}
  <button class="addship" onclick={addShip}><Plus size={14} /> Another ship</button>
</div>

<style>
  .ships {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .ship {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
    border-radius: var(--radius-lg);
    background: linear-gradient(135deg, color-mix(in srgb, var(--hue-sky) 9%, var(--bg)), var(--bg) 60%);
    border: 1px solid color-mix(in srgb, var(--hue-sky) 25%, var(--border));
  }
  .sh {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .sh .label {
    margin: 0;
  }
  .grow {
    flex: 1;
  }
  .link {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 0;
    border: 0;
    background: none;
    font: inherit;
    font-size: var(--text-small);
    color: var(--accent);
    cursor: pointer;
  }
  .head {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .head + .head {
    padding-top: 10px;
    border-top: 1px solid color-mix(in srgb, var(--text) 8%, transparent);
  }
  .seat {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .seat .label {
    margin: 0;
    flex: none;
  }
  .seat .input {
    flex: 1;
    min-width: 0;
  }
  .pickrow {
    display: flex;
    gap: 6px;
    align-items: stretch;
  }
  .laserpick {
    flex: 1;
    min-width: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto 16px;
    gap: 14px;
    align-items: center;
    padding: 10px 12px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
    background: var(--bg-raised);
    font: inherit;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .laserpick:hover,
  .laserpick.open {
    border-color: var(--accent-border);
  }
  .ln {
    display: flex;
    align-items: baseline;
    gap: 10px;
    min-width: 0;
  }
  .ln b {
    font-family: var(--font-display);
    font-size: 1.5rem;
    color: color-mix(in srgb, var(--hue-sky) 55%, var(--text-strong));
    white-space: nowrap;
  }
  .ln span {
    font-size: var(--text-small);
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .lf {
    display: flex;
    gap: 14px;
  }
  .lf span {
    display: flex;
    flex-direction: column;
    font-size: 0.62rem;
    color: var(--text-faint);
    text-transform: var(--label-transform);
    letter-spacing: 0.08em;
  }
  .lf b {
    font-family: var(--font-mono);
    font-size: 0.86rem;
    color: var(--text);
    text-transform: none;
    letter-spacing: 0;
  }
  .car {
    color: var(--text-faint);
  }
  .x {
    display: inline-flex;
    align-items: center;
    padding: 0 8px;
    border: 1px solid transparent;
    border-radius: var(--radius);
    background: none;
    color: var(--text-faint);
    cursor: pointer;
  }
  .x:hover {
    color: var(--bad);
    border-color: var(--danger-border);
  }
  .slots {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 8px;
  }
  .sl {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 8px 10px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
    background: var(--bg-raised);
    font: inherit;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .sl.on {
    border-color: var(--accent);
    box-shadow: inset 0 0 0 1px var(--accent);
  }
  .sl.empty {
    border-style: dashed;
    background: transparent;
  }
  .sl.empty b {
    color: var(--text-faint);
    font-weight: var(--weight-medium);
  }
  .sn {
    font-size: 0.62rem;
    color: var(--text-faint);
    text-transform: var(--label-transform);
    letter-spacing: 0.08em;
  }
  .fx {
    display: flex;
    flex-wrap: wrap;
    gap: 3px;
    font-size: 0.72rem;
  }
  .fx .hue-chip {
    font-size: 0.62rem;
    padding: 0 5px;
  }
  .list {
    display: flex;
    flex-direction: column;
    max-height: 300px;
    overflow: auto;
    padding: 6px;
    border-radius: var(--radius);
    border: 1px solid var(--accent);
    background: var(--bg-raised);
  }
  .list .input {
    margin-bottom: 4px;
  }
  .lr,
  .mr {
    display: grid;
    gap: 8px;
    align-items: center;
    padding: 6px 8px;
    border: 0;
    border-radius: 6px;
    background: none;
    font: inherit;
    font-size: 0.86rem;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .lr {
    grid-template-columns: minmax(0, 1fr) 30px 60px 70px 56px;
  }
  .mr {
    grid-template-columns: 120px auto minmax(0, 1fr);
  }
  .lr:hover,
  .mr:hover {
    background: color-mix(in srgb, var(--text) 5%, transparent);
  }
  .lr.cur,
  .mr.cur {
    background: var(--accent-fill-weak);
  }
  .mono {
    font-family: var(--font-mono);
    font-size: 0.8rem;
  }
  .faint {
    color: var(--text-faint);
    font-size: 0.78rem;
  }
  .tag {
    font-size: 0.62rem;
  }
  .addship {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    min-height: 40px;
    border: 1px dashed var(--border);
    border-radius: var(--radius-lg);
    background: transparent;
    font: inherit;
    font-size: 0.875rem;
    color: var(--text-dim);
    cursor: pointer;
  }
  .addship:hover {
    border-color: var(--accent);
    color: var(--text);
  }
  @media (max-width: 560px) {
    .lf {
      display: none;
    }
    .laserpick {
      grid-template-columns: minmax(0, 1fr) 16px;
    }
  }
</style>
