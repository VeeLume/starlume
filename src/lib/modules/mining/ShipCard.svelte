<script lang="ts">
  // Your ship on the rock, in News's card: the laser large, its figures as
  // tiles, each module and the gadget with its effects as chips (amber
  // hurts, green helps), and the crew mates brought along as support. The
  // saved loadouts pick from the top.
  import { X } from "lucide-svelte";
  import type { LoadoutView, MiningCatalogView } from "$lib/bindings";
  import Monogram from "$lib/components/Monogram.svelte";
  import NowCard from "$lib/components/NowCard.svelte";
  import { miningStore, removeSupport, setLoadout } from "$lib/state/mining.svelte";
  import { effects, int, lasersLine, namesOf, ownLasers, sameLoadout } from "./format";

  let { catalog, loadout, onedit }: { catalog: MiningCatalogView; loadout: LoadoutView; onedit: () => void } =
    $props();

  const names = $derived(namesOf(catalog));
  const laserById = $derived(new Map(catalog.lasers.map((l) => [l.id, l])));
  const moduleById = $derived(new Map(catalog.modules.map((m) => [m.id, m])));
  const gadget = $derived(catalog.gadgets.find((g) => g.id === loadout.gadget) ?? null);

  const own = $derived(ownLasers(loadout));
  const first = $derived(own.length ? (laserById.get(own[0].laser) ?? null) : null);
  const power = $derived(own.reduce((s, l) => s + (laserById.get(l.laser)?.power ?? 0), 0));
  const crew = $derived(
    [...new Set(loadout.lasers.flatMap((l) => (l.who === null ? [] : [l.who])))].map((who) => ({
      who,
      line: lasersLine({ lasers: loadout.lasers.filter((l) => l.who === who), gadget: null }, names),
    })),
  );

  const mine = $derived((miningStore.state?.saved ?? []).filter((s) => s.owner === null));
  const current = $derived(mine.find((s) => sameLoadout(s.loadout, { ...loadout, lasers: own })) ?? null);
</script>

<NowCard
  hue="var(--hue-sky)"
  chip="● YOUR SHIP"
  version={own.length ? lasersLine({ lasers: own, gadget: null }, names) : "No laser"}
  name={first ? `Mining Laser S${first.size}` : ""}
>
  {#snippet top()}
    {#if mine.length}
      <select
        class="select pick"
        value={current?.id ?? ""}
        onchange={(e) => {
          const s = mine.find((x) => x.id === e.currentTarget.value);
          if (s) void setLoadout(s.loadout);
        }}
        aria-label="Saved loadout"
      >
        {#if !current}<option value="" disabled>Unsaved</option>{/if}
        {#each mine as s (s.id)}<option value={s.id}>{s.name}</option>{/each}
      </select>
    {/if}
  {/snippet}
  {#if first}
    <div class="stat-tiles">
      <div class="stat-tile"><b>{int(power)}</b><span>power</span></div>
      <div class="stat-tile">
        <b>{first.optimalRangeM === null ? "—" : `${first.optimalRangeM.toFixed(0)} m`}</b><span>optimal</span>
      </div>
      <div class="stat-tile"><b>{first.maxRangeM === null ? "—" : `${first.maxRangeM.toFixed(0)} m`}</b><span>max</span></div>
    </div>
  {/if}
  <div class="mods">
    {#each own as l, i (i)}
      {#if own.length > 1}<div class="laser">{names.get(l.laser) ?? "Unknown laser"}</div>{/if}
      {#each l.modules as id, s (s)}
        {@const m = moduleById.get(id)}
        <div class="m">
          <b>{m?.name ?? "Unknown module"}</b>
          {#if m}
            {#each effects(m.modifiers, m.powerMultiplier) as fx (fx.text)}
              <span class="hue-chip" style:--hue={fx.good ? "var(--good)" : "var(--warn)"}>{fx.text}</span>
            {/each}
          {/if}
        </div>
      {/each}
    {/each}
    <div class="m" class:faint={!gadget}>
      <b>{gadget?.name ?? "No gadget"}</b>
      {#if gadget}
        {#each effects(gadget.modifiers) as fx (fx.text)}
          <span class="hue-chip" style:--hue={fx.good ? "var(--good)" : "var(--warn)"}>{fx.text}</span>
        {/each}
      {/if}
    </div>
  </div>
  {#if crew.length}
    <div class="crew">
      {#each crew as c (c.who)}
        <div class="mate">
          <Monogram name={c.who} size={24} />
          <b>{c.who}</b>
          <span class="dim">{c.line}</span>
          <button
            class="btn subtle icon"
            onclick={() => void removeSupport(c.who)}
            title="Take {c.who} off the rock"
            aria-label="Remove {c.who}"><X size={14} /></button
          >
        </div>
      {/each}
    </div>
  {/if}
  {#snippet action()}
    <button class="btn" onclick={onedit}>Edit ship</button>
  {/snippet}
</NowCard>

<style>
  .pick {
    font-size: var(--text-small);
    padding-block: 2px;
  }
  .mods {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .laser {
    font-size: var(--text-small);
    color: var(--text-faint);
    margin-top: 2px;
  }
  .m {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
    font-size: 0.86rem;
  }
  .m b {
    font-weight: var(--weight-semibold);
    margin-right: 4px;
    min-width: 76px;
  }
  .m.faint b {
    color: var(--text-faint);
    font-weight: var(--weight-normal);
  }
  .crew {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding-top: 8px;
    border-top: 1px solid color-mix(in srgb, var(--text) 8%, transparent);
  }
  .mate {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.86rem;
  }
  .mate .dim {
    flex: 1;
    min-width: 0;
  }
  .icon {
    padding: 4px 6px;
    display: inline-flex;
  }
</style>
