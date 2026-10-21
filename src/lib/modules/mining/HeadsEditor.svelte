<script lang="ts">
  // A loadout's heads, editable in place: the ship card's and the Loadouts
  // sheet's. Each head is the laser as a tile (name, size, power, range, its
  // own modifiers stacked like a module's), then its module slots as tiles.
  // A tile opens the part picker (`onopen` with `L<head>` or
  // `S<head>:<slot>`); the parent places the picker. On a ship with several
  // heads (a MOLE) each head names who works it. A head or slot that differs
  // from `base` (the saved loadout) carries an amber dot. `structure` adds a
  // head and removes one (the Loadouts sheet). `ontoggle` switches a head
  // on and off (the ship card: a MOLE seat nobody is in today); one stays on.
  import { Plus, X } from "lucide-svelte";
  import type { Snippet } from "svelte";
  import { Switch } from "@veelume/ui";
  import type { LaserInputView, MiningCatalogView } from "$lib/bindings";
  import Chevron from "$lib/components/Chevron.svelte";
  import FxLines from "./FxLines.svelte";
  import { fxOf, laserShort } from "./format";

  let {
    catalog,
    lasers,
    base = null,
    open = "",
    onopen,
    onchange = undefined,
    structure = false,
    slotNote = undefined,
    ontoggle = undefined,
  }: {
    catalog: MiningCatalogView;
    lasers: LaserInputView[];
    /** The saved lasers, to mark what changed; null: nothing to compare. */
    base?: LaserInputView[] | null;
    /** The open picker's key. */
    open?: string;
    onopen: (key: string) => void;
    /** Seat names, and heads added or removed. */
    onchange?: (next: LaserInputView[]) => void;
    structure?: boolean;
    /** Under a saved module's tile (head, slot): what it adds, on Loadouts
     *  measured on every rock. */
    slotNote?: Snippet<[number, number]>;
    /** Switch head `i` off (true) or on. */
    ontoggle?: (i: number, off: boolean) => void;
  } = $props();

  const laserById = $derived(new Map(catalog.lasers.map((l) => [l.id, l])));
  const moduleById = $derived(new Map(catalog.modules.map((m) => [m.id, m])));
  /** Several heads on one ship: each names who works it. */
  const seated = $derived(lasers.length > 1);
  const seat = (l: LaserInputView, i: number) => l.who ?? (i === 0 ? "You" : `Head ${i + 1}`);
  const firing = $derived(lasers.filter((l) => !l.off).length);
  const laserChanged = (i: number) => base !== null && base[i]?.laser !== lasers[i].laser;
  const slotChanged = (i: number, s: number) => base !== null && (base[i]?.modules[s] ?? null) !== (lasers[i].modules[s] ?? null);

  function rename(i: number, who: string) {
    onchange?.(lasers.map((l, j) => (j === i ? { ...l, who: who.trim() || null } : l)));
  }
  function addHead() {
    const last = lasers.at(-1);
    onchange?.([...lasers, { laser: last?.laser ?? "", modules: [], owner: last?.owner ?? 0, who: null, crew: null, distanceM: null, off: false }]);
  }
  function removeHead(i: number) {
    onchange?.(lasers.filter((_, j) => j !== i));
  }
</script>

<div class="heads">
  {#each lasers as l, i (i)}
    {@const laser = laserById.get(l.laser)}
    <div class="head" class:off={l.off}>
      {#if seated}
        <div class="seat">
          {#if onchange}
            <input value={l.who ?? ""} placeholder={seat(l, i)} aria-label="Who works this head" onchange={(e) => rename(i, e.currentTarget.value)} />
          {:else}<span>{seat(l, i)}</span>{/if}
          {#if ontoggle}
            <span class="fire" title={!l.off && firing === 1
                ? "One head stays on"
                : l.off
                  ? "Not on the rock: nobody in the seat, no power, or the beam off"
                  : "On the rock: its beam hits"}>
              <span class="faint">{l.off ? "off" : "on"}</span>
              <Switch
                checked={!l.off}
                disabled={!l.off && firing === 1}
                label="{seat(l, i)} fires"
                onchange={(on) => ontoggle(i, !on)}
              />
            </span>
          {/if}
          {#if structure}
            <button type="button" class="rm" onclick={() => removeHead(i)} title="Remove this head" aria-label="Remove head"><X size={13} /></button>
          {/if}
        </div>
      {/if}
      <button type="button" class="laser" class:on={open === `L${i}`} class:chg={laserChanged(i)} data-part onclick={() => onopen(`L${i}`)}>
        <span class="l">
          <b>{laser ? laserShort(laser.name) : "Unknown laser"}</b>
          {#if laser}
            <small
              >S{laser.size} · {Math.round(laser.power).toLocaleString("en-US")}{#if laser.optimalRangeM !== null && laser.maxRangeM !== null}
                · {laser.optimalRangeM.toFixed(0)}–{laser.maxRangeM.toFixed(0)} m{/if}</small
            >
          {/if}
        </span>
        {#if laser}<FxLines fx={fxOf(laser.modifiers)} stacked />{/if}
        <span class="car"><Chevron open={open === `L${i}`} /></span>
      </button>
      {#if laser && laser.moduleSlots > 0}
        <div class="slots">
          {#each Array.from({ length: laser.moduleSlots }, (_, s) => s) as s (s)}
            {@const m = l.modules[s] ? moduleById.get(l.modules[s]) : null}
            <button
              type="button"
              class="slot"
              class:empty={!m}
              class:on={open === `S${i}:${s}`}
              class:chg={slotChanged(i, s)}
              data-part
              onclick={() => onopen(`S${i}:${s}`)}
            >
              <b>{m ? m.name.replace(/\s+Module$/i, "") : "Empty"}{#if m?.active}<span class="act">active</span>{/if}</b>
              {#if m}<FxLines fx={fxOf(m.modifiers, m.powerMultiplier)} stacked />{:else}<span class="faint">slot {s + 1}</span>{/if}
              {#if m && slotNote && !slotChanged(i, s)}{@render slotNote(i, s)}{/if}
            </button>
          {/each}
        </div>
      {/if}
    </div>
  {:else}
    <button type="button" class="laser empty" class:on={open === "L0"} data-part onclick={() => onopen("L0")}>
      <span class="l"><b>Pick a laser</b><small>the ship's mining laser, then its modules</small></span>
      <span class="car"><Chevron open={open === "L0"} /></span>
    </button>
  {/each}
  {#if structure && lasers.length}
    <button type="button" class="add" onclick={addHead}><Plus size={13} /> Head on this ship</button>
  {/if}
</div>

<style>
  .heads {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .head {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .head + .head {
    padding-top: 10px;
    border-top: 1px solid color-mix(in srgb, var(--text) 8%, transparent);
  }
  .seat {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 0.82rem;
    font-weight: var(--weight-semibold);
  }
  .fire {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-left: auto;
    font-family: var(--font-mono);
    font-size: 0.7rem;
    font-weight: var(--weight-medium);
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .fire + .rm {
    margin-left: 0;
  }
  /* A head holding fire: still editable, dimmed out of the stats. */
  .head.off .laser,
  .head.off .slots {
    opacity: 0.45;
  }
  .seat input {
    font: inherit;
    color: var(--text);
    background: transparent;
    border: 1px solid transparent;
    border-radius: 6px;
    padding: 1px 6px;
    margin-left: -6px;
    width: 10rem;
  }
  .seat input:hover,
  .seat input:focus {
    border-color: var(--border);
    background: var(--bg);
    outline: none;
  }
  .rm {
    display: inline-flex;
    margin-left: auto;
    padding: 2px;
    border: 1px solid transparent;
    border-radius: 4px;
    background: none;
    color: var(--text-faint);
    cursor: pointer;
  }
  .rm:hover {
    color: var(--bad);
    border-color: var(--danger-border);
  }
  .laser,
  .slot {
    position: relative;
    font: inherit;
    color: var(--text);
    text-align: left;
    cursor: pointer;
    border-radius: 7px;
    border: 1px solid var(--border);
  }
  .laser {
    width: 100%;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto 14px;
    gap: 12px;
    align-items: center;
    padding: 7px 10px;
    font-size: 0.72rem;
    background: color-mix(in srgb, var(--hue-sky) 7%, var(--bg));
  }
  .laser.empty {
    border-style: dashed;
    background: transparent;
    font-size: inherit;
  }
  .l {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .l b {
    font-family: var(--font-display);
    font-size: 1.15rem;
    line-height: 1.15;
    color: color-mix(in srgb, var(--hue-sky) 60%, var(--text-strong));
  }
  .empty .l b {
    font-family: inherit;
    font-size: 0.9rem;
    color: var(--text-dim);
  }
  .l small {
    font-family: var(--font-mono);
    font-size: 0.7rem;
    color: var(--text-faint);
  }
  .laser :global(.fx) {
    align-items: flex-end;
  }
  .car {
    color: var(--text-faint);
  }
  .slots {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(112px, 1fr));
    gap: 6px;
  }
  .slot {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    padding: 6px 9px;
    font-size: 0.68rem;
    background: color-mix(in srgb, var(--bg) 70%, transparent);
  }
  .slot b {
    font-size: 0.84rem;
    font-weight: var(--weight-semibold);
    padding-right: 10px;
  }
  .slot.empty {
    border-style: dashed;
    background: transparent;
  }
  .slot.empty b {
    color: var(--text-faint);
    font-weight: var(--weight-medium);
  }
  .act {
    margin-left: 6px;
    font-family: var(--font-mono);
    font-size: 0.6rem;
    font-weight: var(--weight-medium);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--hue-rose);
  }
  .laser:hover,
  .slot:hover {
    border-color: color-mix(in srgb, var(--accent) 50%, var(--border));
  }
  .laser.on,
  .slot.on {
    border-color: var(--accent);
    box-shadow: inset 0 0 0 1px var(--accent);
  }
  .chg::after {
    content: "";
    position: absolute;
    top: 7px;
    right: 7px;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--warn);
  }
  .faint {
    color: var(--text-faint);
  }
  .add {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    align-self: flex-start;
    padding: 0;
    border: 0;
    background: none;
    font: inherit;
    font-size: var(--text-small);
    color: var(--accent);
    cursor: pointer;
  }
</style>
