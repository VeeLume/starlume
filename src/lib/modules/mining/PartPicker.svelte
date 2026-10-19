<script lang="ts">
  // The part picker, opened from a head or slot tile and laid over the
  // column beside it, so the ship, the outcome and How to crack it stay in
  // view. A slot lists the modules: passive, then active (uses × duration),
  // families in tier order (Focus, II, III), figures in aligned columns, and
  // each option's crack on the calculator's rock against what's in the slot
  // now; modules that don't change cracking (FLTR, XTR) in one line. A head
  // lists the lasers by size. One scroll, the page's; search filters, Esc or
  // a click outside closes.
  import type { Snippet } from "svelte";
  import { X } from "lucide-svelte";
  import type { EvalView, LaserInputView, LoadoutView, MiningCatalogView } from "$lib/bindings";
  import { compareOnRock } from "$lib/state/mining.svelte";
  import CrackCell from "./CrackCell.svelte";
  import { FX_COLUMNS, familyOf, fxOf, helps, laserShort, signed } from "./format";
  import { withPart } from "./parts";

  let {
    catalog,
    channel,
    target,
    lasers,
    build,
    minus = null,
    onpick,
    onclose,
    views,
  }: {
    catalog: MiningCatalogView;
    channel: string;
    /** `L<head>` or `S<head>:<slot>`. */
    target: string;
    /** The heads being edited. */
    lasers: LaserInputView[];
    /** The whole setup on the rock for some heads (crew mates, gadget). */
    build: (lasers: LaserInputView[]) => LoadoutView;
    /** Subtracted from every crack: a crew mate's options read as what they add. */
    minus?: number | null;
    onpick: (next: LaserInputView[]) => void;
    onclose: () => void;
    /** A switch to another view of the same choice (Loadouts' sweep). */
    views?: Snippet;
  } = $props();

  let q = $state("");
  let el = $state<HTMLElement>();
  let cracks = $state<Map<string, number | null> | null>(null);

  const isLaser = $derived(target.startsWith("L"));
  const head = $derived(Number(target.slice(1).split(":")[0]));
  const slot = $derived(isLaser ? -1 : Number(target.split(":")[1]));
  const current = $derived(lasers[head] ?? null);
  const query = $derived(q.trim().toLowerCase());
  const laserName = $derived(current ? laserShort(catalog.lasers.find((l) => l.id === current.laser)?.name ?? "") : "");

  /** The heads with this option in place. */
  const withOption = (id: string | null): LaserInputView[] => withPart(catalog, lasers, target, id);

  const modules = $derived.by(() => {
    const rows = catalog.modules.map((m) => ({ m, fx: fxOf(m.modifiers, m.powerMultiplier), ...familyOf(m.name) }));
    const sort = (a: (typeof rows)[number], b: (typeof rows)[number]) => a.family.localeCompare(b.family) || a.tier - b.tier;
    const cracking = rows.filter((r) => FX_COLUMNS.some((c) => r.fx[c.key] !== 0));
    const families = (active: boolean) => {
      const by = new Map<string, typeof rows>();
      for (const r of cracking.filter((r) => r.m.active === active).sort(sort)) by.set(r.family, [...(by.get(r.family) ?? []), r]);
      return [...by.values()];
    };
    return {
      passive: families(false),
      active: families(true),
      none: rows.filter((r) => !cracking.includes(r)).sort(sort),
    };
  });
  const lasersBySize = $derived.by(() => {
    const sizes = [...new Set(catalog.lasers.map((l) => l.size))].sort((a, b) => a - b);
    return sizes.map((size) => ({ size, lasers: catalog.lasers.filter((l) => l.size === size).sort((a, b) => a.name.localeCompare(b.name)) }));
  });
  const options = $derived<(string | null)[]>(
    isLaser ? catalog.lasers.map((l) => l.id) : [null, ...catalog.modules.map((m) => m.id)],
  );
  const currentId = $derived(isLaser ? (current?.laser ?? null) : (current?.modules[slot] ?? null));

  // Each option's crack on the rock: one comparison for the open target.
  $effect(() => {
    const opts = options;
    const loadouts = opts.map((id) => build(withOption(id)));
    cracks = null;
    let live = true;
    void compareOnRock(channel, loadouts).then((evals) => {
      if (!live || !evals) return;
      const pct = (e: EvalView | null) => (e?.difficulty ? Math.round(e.difficulty.controlled * 100) : null);
      cracks = new Map(opts.map((id, k) => [id ?? "", pct(evals[k])]));
    });
    return () => (live = false);
  });

  const crackOf = (id: string | null) => {
    if (!cracks) return undefined;
    const v = cracks.get(id ?? "");
    return v === undefined || v === null ? null : v - (minus ?? 0);
  };
  const deltaOf = (id: string | null) => {
    const a = crackOf(id);
    const b = crackOf(currentId);
    return a === null || a === undefined || b === null || b === undefined || id === currentId ? null : a - b;
  };
  const shown = (name: string) => !query || name.toLowerCase().includes(query);

  function pick(id: string | null) {
    onpick(withOption(id));
  }

  function outside(e: MouseEvent) {
    const t = e.target as HTMLElement;
    // A click that swapped this in for another view lands here detached.
    if (el && t.isConnected && !el.contains(t) && !t.closest("[data-part]")) onclose();
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onclose()} onclick={outside} />

{#snippet cell(key: (typeof FX_COLUMNS)[number]["key"], v: number)}
  <span class="v" class:good={v !== 0 && helps(key, v)} class:bad={v !== 0 && !helps(key, v)} class:z={v === 0}
    >{v === 0 ? "·" : signed(v)}</span
  >
{/snippet}

<div class="pick" bind:this={el} role="dialog" aria-label={isLaser ? "Pick a laser" : "Pick a module"}>
  <div class="ph">
    <span class="ttl">{isLaser ? "Laser" : `Slot ${slot + 1}`}<small>{current?.who ? `${current.who}'s ${laserName}` : laserName}</small></span>
    <!-- svelte-ignore a11y_autofocus -->
    <input class="input" bind:value={q} placeholder={isLaser ? "Find a laser" : "Find a module"} aria-label="Find" autofocus />
    <span class="grow"></span>
    {@render views?.()}
    <button type="button" class="x" onclick={onclose} aria-label="Close"><X size={15} /></button>
  </div>

  {#if isLaser}
    <div class="row h lz"><span>Laser</span><span>power</span><span>range</span><span>resist.</span><span>instab.</span><span>window</span><span>{minus === null ? "crack here" : "adds"}</span></div>
    {#each lasersBySize as g (g.size)}
      <div class="grp"><p class="label">Size {g.size}</p></div>
      <div class="fam">
        {#each g.lasers.filter((l) => shown(l.name)) as l (l.id)}
          {@const fx = fxOf(l.modifiers)}
          <button type="button" class="row lz" class:cur={l.id === currentId} onclick={() => pick(l.id)}>
            <span class="nm"><b>{laserShort(l.name)}</b><small>{l.moduleSlots} slot{l.moduleSlots === 1 ? "" : "s"}</small></span>
            <span class="v">{Math.round(l.power).toLocaleString("en-US")}</span>
            <span class="v">{l.optimalRangeM !== null && l.maxRangeM !== null ? `${l.optimalRangeM.toFixed(0)}–${l.maxRangeM.toFixed(0)} m` : ""}</span>
            {@render cell("resistance", fx.resistance)}{@render cell("instability", fx.instability)}{@render cell("window", fx.window)}
            <CrackCell value={crackOf(l.id)} delta={deltaOf(l.id)} adds={minus !== null} />
          </button>
        {/each}
      </div>
    {/each}
  {:else}
    <div class="row h"><span>Module</span>{#each FX_COLUMNS as c (c.key)}<span>{c.label}</span>{/each}<span>{minus === null ? "crack here" : "adds"}</span></div>
    {#if !query}
      <button type="button" class="row" class:cur={currentId === null} onclick={() => pick(null)}>
        <span class="nm"><b class="faint">Empty slot</b></span>{#each FX_COLUMNS as c (c.key)}<span class="v z">·</span>{/each}
        <CrackCell value={crackOf(null)} delta={deltaOf(null)} adds={minus !== null} />
      </button>
    {/if}
    {#each [["Passive", "always on", modules.passive], ["Active", "switched on while mining, uses × duration", modules.active]] as const as [label, sub, fams] (label)}
      <div class="grp"><p class="label">{label}</p><small>{sub}</small></div>
      {#each fams as fam (fam[0].family)}
        <div class="fam">
          {#each fam.filter((r) => shown(r.short)) as r (r.m.id)}
            <button type="button" class="row" class:cur={r.m.id === currentId} onclick={() => pick(r.m.id)}>
              <span class="nm"
                ><b>{r.short}</b>{#if r.m.active && (r.m.uses !== null || r.m.durationS !== null)}<small
                    >{r.m.uses ?? "?"} × {r.m.durationS !== null ? `${Math.round(r.m.durationS)} s` : "?"}</small
                  >{/if}</span
              >
              {#each FX_COLUMNS as c (c.key)}{@render cell(c.key, r.fx[c.key])}{/each}
              <CrackCell value={crackOf(r.m.id)} delta={deltaOf(r.m.id)} adds={minus !== null} />
            </button>
          {/each}
        </div>
      {/each}
    {/each}
    {#if modules.none.length}
      <p class="none">No effect on cracking: {modules.none.map((r) => r.short).join(", ")}.</p>
    {/if}
  {/if}
</div>

<style>
  .pick {
    display: flex;
    flex-direction: column;
    padding: 12px 16px 14px;
    border-radius: var(--radius-lg);
    background: var(--bg-raised);
    border: 1px solid color-mix(in srgb, var(--accent) 55%, var(--border));
    box-shadow: 0 24px 60px color-mix(in srgb, var(--bg-sunk) 85%, transparent);
  }
  .ph {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    margin-bottom: 8px;
  }
  .ttl {
    font-weight: var(--weight-semibold);
  }
  .ttl small {
    margin-left: 6px;
    font-weight: var(--weight-normal);
    color: var(--text-faint);
  }
  .ph .input {
    flex: 0 1 190px;
    font-size: var(--text-small);
    padding-block: 4px;
  }
  .grow {
    flex: 1;
  }
  .x {
    display: inline-flex;
    padding: 2px;
    border: 0;
    background: none;
    color: var(--text-faint);
    cursor: pointer;
  }
  .row {
    display: grid;
    grid-template-columns: minmax(120px, 1.4fr) repeat(6, minmax(44px, 0.55fr)) 104px;
    gap: 0 8px;
    align-items: center;
    width: calc(100% + 16px);
    padding: 4px 8px;
    margin-inline: -8px;
    border: 0;
    border-radius: 6px;
    background: none;
    font: inherit;
    font-size: 0.84rem;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .row.lz {
    grid-template-columns: minmax(120px, 1.4fr) 56px 72px repeat(3, minmax(44px, 0.55fr)) 104px;
  }
  .row:hover {
    background: color-mix(in srgb, var(--text) 5%, transparent);
  }
  .row.cur {
    background: var(--accent-fill-weak);
    box-shadow: inset 0 0 0 1px var(--accent-border);
  }
  .row.h {
    cursor: default;
    padding-block: 0 4px;
    font-size: 0.64rem;
    letter-spacing: 0.03em;
    text-transform: var(--label-transform);
    color: var(--text-faint);
  }
  .row.h:hover {
    background: none;
  }
  .row.h span:not(:first-child) {
    text-align: right;
  }
  .nm {
    display: flex;
    align-items: baseline;
    gap: 6px;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
  }
  .nm b {
    font-weight: var(--weight-semibold);
  }
  .nm small {
    font-family: var(--font-mono);
    font-size: 0.68rem;
    color: var(--text-faint);
  }
  .v {
    font-family: var(--font-mono);
    font-size: 0.78rem;
    text-align: right;
    white-space: nowrap;
  }
  .good {
    color: var(--good);
  }
  .bad {
    color: var(--bad);
  }
  .z {
    color: var(--text-faint);
    opacity: 0.55;
  }
  .faint {
    color: var(--text-faint);
    font-weight: var(--weight-medium);
  }
  .grp {
    display: flex;
    align-items: baseline;
    gap: 10px;
    margin: 10px 0 2px;
  }
  .grp .label {
    margin: 0;
  }
  .grp small {
    font-size: 0.72rem;
    color: var(--text-faint);
  }
  .fam {
    padding-block: 2px;
    border-top: 1px solid color-mix(in srgb, var(--text) 7%, transparent);
  }
  .none {
    margin: 10px 0 0;
    font-size: 0.78rem;
    color: var(--text-faint);
  }
</style>
