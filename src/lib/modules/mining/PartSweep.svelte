<script lang="ts">
  // The part picker measured on every rock (design pass 5): every option for
  // the opened part as a comfort zone, with what it changes against the
  // loadout as it is now — clean cracks per 100 rocks you'd meet (Everyday)
  // and how much it breaks at all (Reach) — grouped by whether it cracks
  // more, about the same or fewer cleanly, best first in each. Options fill
  // in as their solves finish (a few seconds each; the solves queue behind
  // the sheet's own and drop when the sweep closes): a bar shows how far,
  // a waiting card whether it is measuring now or queued. Without a part on
  // now (a new loadout) the figures are the options' own. Every module is
  // an option: an active one is measured switched on (sc-mining takes a
  // fitted active module as on for the attempt; the calculator switches it
  // on again while its uses last), one that doesn't change cracking (FLTR)
  // lands in About the same. Laid over the list like the part picker; Esc
  // or a click outside closes.
  import { untrack, type Snippet } from "svelte";
  import { X } from "lucide-svelte";
  import type { LaserInputView, LoadoutView, MiningCatalogView } from "$lib/bindings";
  import { comfortKey, comfortOf, dropComfort, miningStore, requestComfort } from "$lib/state/mining.svelte";
  import { binsOf, scoresOf, type ComfortFilter, type ComfortRow } from "./comfort";
  import ComfortZone from "./ComfortZone.svelte";
  import Pending from "./Pending.svelte";
  import { FX_COLUMNS, familyOf, fxOf, laserShort } from "./format";
  import { target as parse, withPart } from "./parts";

  let {
    catalog,
    channel,
    target,
    lasers,
    loadoutOf,
    rows,
    filter,
    top,
    onpick,
    onclose,
    views,
  }: {
    catalog: MiningCatalogView;
    channel: string;
    /** `L<head>` or `S<head>:<slot>`. */
    target: string;
    /** The heads as they are now (the try-out, else the saved loadout). */
    lasers: LaserInputView[];
    /** The heads as a loadout to measure. */
    loadoutOf: (lasers: LaserInputView[]) => LoadoutView;
    rows: ComfortRow[];
    filter: ComfortFilter;
    /** The list's chart scale, so a sweep reads beside it. */
    top: number;
    onpick: (next: LaserInputView[]) => void;
    onclose: () => void;
    /** A switch to another view of the same choice (the part picker's stats). */
    views?: Snippet;
  } = $props();

  let el = $state<HTMLElement>();
  const t = $derived(parse(target));
  const current = $derived(lasers[t.head] ?? null);
  const currentLaser = $derived(catalog.lasers.find((l) => l.id === current?.laser) ?? null);
  const currentId = $derived(t.laser ? (current?.laser ?? null) : (current?.modules[t.slot] ?? null));
  const nameOf = (id: string | null) =>
    id === null
      ? "Empty slot"
      : t.laser
        ? laserShort(catalog.lasers.find((l) => l.id === id)?.name ?? "?")
        : (catalog.modules.find((m) => m.id === id)?.name ?? "?").replace(/\s+Module$/i, "");

  // The options: lasers that fit the mount; every module, the ones that
  // change cracking first (passive, then active) so they measure first.
  const modules = $derived(
    catalog.modules
      .map((m) => ({ m, fx: fxOf(m.modifiers, m.powerMultiplier), ...familyOf(m.name) }))
      .sort((a, b) => a.family.localeCompare(b.family) || a.tier - b.tier),
  );
  const cracking = (fx: ReturnType<typeof fxOf>) => FX_COLUMNS.some((c) => fx[c.key] !== 0);
  const options = $derived<(string | null)[]>(
    t.laser
      ? catalog.lasers.filter((l) => !currentLaser || l.size === currentLaser.size).map((l) => l.id)
      : [
          null,
          ...modules.filter((r) => !r.m.active && cracking(r.fx)).map((r) => r.m.id),
          ...modules.filter((r) => r.m.active && cracking(r.fx)).map((r) => r.m.id),
          ...modules.filter((r) => !cracking(r.fx)).map((r) => r.m.id),
        ],
  );

  /** A card's second line. A laser: its size and slots, and how many it
   *  leaves empty (it is measured with the modules on now, as many as fit,
   *  so slots to spare can still lift it). An active module: its uses and
   *  how it is measured. A module that doesn't change cracking: that. */
  function subOf(id: string | null): { line: string; warn: string } | null {
    if (t.laser) {
      const l = catalog.lasers.find((x) => x.id === id);
      if (!l) return null;
      const free = l.moduleSlots - Math.min(current?.modules.length ?? 0, l.moduleSlots);
      const slots = `${l.moduleSlots} module slot${l.moduleSlots === 1 ? "" : "s"}`;
      return { line: `S${l.size} · ${l.moduleSlots ? slots : "no module slots"}`, warn: free ? `${free} empty` : "" };
    }
    const r = modules.find((x) => x.m.id === id);
    if (!r) return null;
    if (!cracking(r.fx)) return { line: "no effect on cracking", warn: "" };
    if (!r.m.active) return null;
    const uses = r.m.uses !== null || r.m.durationS !== null
      ? `${r.m.uses ?? "?"} × ${r.m.durationS !== null ? `${Math.round(r.m.durationS)} s` : "?"}`
      : "";
    return { line: `active${uses ? ` · ${uses}` : ""} · measured switched on`, warn: "" };
  }
  const loadoutFor = (id: string | null) => loadoutOf(withPart(catalog, lasers, target, id));
  const now = $derived(loadoutOf(lasers));

  // Ask for every option's solve; a closed sweep drops the ones still
  // waiting. Untracked: asking reads the cache, and a finished solve must
  // not re-ask the rest.
  $effect(() => {
    const ids = options;
    const at = now;
    const ch = channel;
    return untrack(() => {
      const mine = new Set(ids.map((id) => comfortKey(ch, loadoutFor(id))));
      requestComfort(ch, at, true);
      for (const id of ids) requestComfort(ch, loadoutFor(id));
      return () => dropComfort((key) => !mine.has(key));
    });
  });

  const base = $derived.by(() => {
    const c = comfortOf(channel, now);
    return c ? scoresOf(c, rows, filter) : null;
  });
  const cards = $derived(
    options
      .map((id) => {
        const lo = loadoutFor(id);
        const c = comfortOf(channel, lo);
        return {
          id,
          key: comfortKey(channel, lo),
          s: c ? scoresOf(c, rows, filter) : null,
          bins: c ? binsOf(c, rows, filter) : null,
        };
      })
      .sort((a, b) => {
        if (!a.s || !b.s) return a.s ? -1 : b.s ? 1 : 0;
        return b.s.everyday - a.s.everyday || b.s.reach - a.s.reach;
      }),
  );
  const done = $derived(cards.filter((c) => c.s).length);
  // Grouped by what they do to clean cracks (Everyday, to the point).
  type Card = (typeof cards)[number];
  const groups = $derived.by(() => {
    const step = (c: Card) => (base && c.s ? Math.round((c.s.everyday - base.everyday) * 100) : 0);
    const out: { title: string; cards: Card[] }[] = [];
    const add = (title: string, list: Card[]) => list.length && out.push({ title, cards: list });
    const measured = cards.filter((c) => c.s);
    if (!base) add("Measured", measured);
    else {
      add("Now", measured.filter((c) => c.id === currentId));
      const rest = measured.filter((c) => c.id !== currentId);
      add("More clean cracks", rest.filter((c) => step(c) > 0));
      add("About the same", rest.filter((c) => step(c) === 0));
      add("Fewer clean cracks", rest.filter((c) => step(c) < 0));
    }
    add("Still measuring", cards.filter((c) => !c.s));
    return out;
  });
  const sgn = (x: number) => {
    const r = Math.round(x * 100);
    return `${r > 0 ? "+" : r < 0 ? "−" : "±"}${Math.abs(r)}`;
  };
  const tone = (x: number) => (Math.round(x * 100) > 0 ? "up" : Math.round(x * 100) < 0 ? "dn" : "z");

  function outside(e: MouseEvent) {
    const target = e.target as HTMLElement;
    // A click that swapped this in for another view lands here detached.
    if (el && target.isConnected && !el.contains(target) && !target.closest("[data-part]")) onclose();
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onclose()} onclick={outside} />

<div class="sweep" bind:this={el} role="dialog" aria-label={t.laser ? "Every laser, measured" : "Every module, measured"}>
  <div class="ph">
    <span class="ttl"
      >{t.laser ? "Laser" : `Slot ${t.slot + 1}`}<small
        >{current?.who ? `${current.who}'s ` : ""}{currentLaser ? laserShort(currentLaser.name) : ""}</small
      ></span
    >
    <span class="faint">{done < cards.length ? `measuring ${done} of ${cards.length}` : "best first in each group"}</span>
    <span class="grow"></span>
    {@render views?.()}
    <button type="button" class="x" onclick={onclose} aria-label="Close"><X size={15} /></button>
  </div>
  {#if done < cards.length}
    <span class="progress" role="progressbar" aria-valuemin={0} aria-valuemax={cards.length} aria-valuenow={done}
      ><i style:width="{(done / Math.max(1, cards.length)) * 100}%"></i></span
    >
  {/if}
  {#each groups as g (g.title)}
    <p class="gh">{g.title}<small>{g.cards.length}</small></p>
    <div class="grid">
      {#each g.cards as c (c.id ?? "")}
        <button type="button" class="card" class:cur={c.id === currentId} onclick={() => onpick(withPart(catalog, lasers, target, c.id))}>
          <span class="sn">
            <span class="nm"
              >{nameOf(c.id)}{#if c.id === currentId}<small>now</small>{/if}{#if subOf(c.id)}{@const sl = subOf(c.id)!}<span
                  class="sl">{sl.line}{#if sl.warn}<em> · {sl.warn}</em>{/if}</span
                >{/if}</span
            >
            {#if c.s && base && c.id !== currentId}
              <span class="mono"
                ><span class={tone(c.s.everyday - base.everyday)}>{sgn(c.s.everyday - base.everyday)}</span> ·
                <span class={tone(c.s.reach - base.reach)}>{sgn(c.s.reach - base.reach)}</span></span
              >
            {:else if c.s}
              <span class="mono">{Math.round(c.s.everyday * 100)} · {Math.round(c.s.reach * 100)}%</span>
            {/if}
          </span>
          {#if c.bins}
            <ComfortZone bins={c.bins} {top} height={72} axis />
          {:else}
            <Pending height={86} label={miningStore.comfortRunning === c.key ? "measuring…" : "queued"} />
          {/if}
        </button>
      {/each}
    </div>
  {/each}
  <p class="hint">
    First figure: clean cracks per 100 rocks you'd meet, second: reach (breaks at all){base ? ", both against the part on now" : ""}.
    {#if t.laser}Each laser is measured with the modules on now, as many as it has slots for; empty slots can still lift it.{/if}
    {#if t.laser && currentLaser}Other sizes don't fit this mount.{/if}
    {#if !t.laser}Active modules are measured switched on for the whole attempt, as if you fire them as the rock fights back.{/if}
  </p>
</div>

<style>
  .sweep {
    display: flex;
    flex-direction: column;
    gap: 8px;
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
  }
  .ttl {
    font-weight: var(--weight-semibold);
  }
  .ttl small {
    margin-left: 6px;
    font-weight: var(--weight-normal);
    color: var(--text-faint);
  }
  .grow {
    flex: 1;
  }
  .faint {
    color: var(--text-faint);
    font-size: var(--text-small);
  }
  .x {
    display: inline-flex;
    padding: 2px;
    border: 0;
    background: none;
    color: var(--text-faint);
    cursor: pointer;
  }
  .progress {
    display: block;
    height: 3px;
    border-radius: 2px;
    overflow: hidden;
    background: color-mix(in srgb, var(--text) 8%, transparent);
  }
  .progress i {
    display: block;
    height: 100%;
    background: var(--accent);
    transition: width 0.3s;
  }
  .gh {
    display: flex;
    align-items: baseline;
    gap: 8px;
    margin: 6px 0 0;
    font-family: var(--font-mono);
    font-size: 0.68rem;
    letter-spacing: var(--label-tracking);
    text-transform: var(--label-transform);
    color: var(--text-dim);
  }
  .gh small {
    color: var(--text-faint);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 10px;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 9px 11px 6px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
    background: var(--bg);
    font: inherit;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .card:hover {
    border-color: color-mix(in srgb, var(--accent) 50%, var(--border));
  }
  .card.cur {
    border-color: var(--accent);
    box-shadow: inset 0 0 0 1px var(--accent);
  }
  .sn {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    font-size: 0.8rem;
    font-weight: var(--weight-semibold);
  }
  .nm small {
    margin-left: 6px;
    font-weight: var(--weight-normal);
    color: var(--text-faint);
  }
  .sl {
    display: block;
    margin-top: 1px;
    font-family: var(--font-mono);
    font-size: 0.68rem;
    font-weight: var(--weight-normal);
    color: var(--text-faint);
  }
  .sl em {
    font-style: normal;
    color: var(--warn);
  }
  .mono {
    font-family: var(--font-mono);
    font-size: 0.76rem;
    white-space: nowrap;
  }
  .up {
    color: var(--good);
  }
  .dn {
    color: var(--bad);
  }
  .z {
    color: var(--text-faint);
  }
  .hint {
    margin: 0;
    font-size: 0.74rem;
    color: var(--text-faint);
  }
</style>
