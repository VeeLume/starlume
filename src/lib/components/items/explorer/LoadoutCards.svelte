<script lang="ts">
  // A ship's loadout as cards, grouped like the community calculators: one
  // section per component group, one card per hardpoint (mount › part),
  // each line the part fitted now and, where it differs, what is saved
  // (and where that saved part is now). Clicking a line selects the part.
  import { Check, CircleQuestionMark, CornerDownRight, TriangleAlert } from "lucide-svelte";
  import type { ItemKindView } from "$lib/bindings";
  import { LOADOUT_SECTIONS, loadoutOf, type LoadoutLine } from "../model/loadout";
  import { ancestors, container, isBricked, pathLabel, type Node, type ShipNode, type Tree } from "../model/tree";
  import ChipButton from "./ChipButton.svelte";
  import Tag from "./Tag.svelte";
  import { SECTION_ICON } from "./ui";

  let {
    ship,
    tree,
    selectedGuid,
    onselect,
    ongo,
  }: {
    ship: ShipNode;
    tree: Tree;
    selectedGuid: string | null;
    onselect: (kind: ItemKindView) => void;
    ongo: (n: Node) => void;
  } = $props();

  let onlyDiffs = $state(false);
  let showStock = $state(true);

  const loadout = $derived(loadoutOf(ship.ship));
  const keepLine = (l: LoadoutLine) => !onlyDiffs || l.status === "off" || (showStock && l.custom);
  const sections = $derived(
    LOADOUT_SECTIONS.map((s) => ({
      s,
      hardpoints: loadout.hardpoints
        .filter((h) => h.section === s)
        .map((h) => ({ ...h, lines: h.lines.filter(keepLine) }))
        .filter((h) => h.lines.length),
    })).filter((x) => x.hardpoints.length),
  );

  /** Where a saved part sits now, outside this ship. */
  function whereNow(kind: ItemKindView) {
    const m = new Map<string, { c: Node; n: number }>();
    for (const x of tree.byKind.get(kind.guid) ?? []) {
      if (isBricked(x) || ancestors(x).includes(ship)) continue;
      const c = container(x);
      if (!c) continue;
      const e = m.get(c.id) ?? { c, n: 0 };
      e.n += x.item.stack;
      m.set(c.id, e);
    }
    return [...m.values()];
  }
  /** "On you › … › Backpack": start and end of a long path. */
  const shortPath = (c: Node) => {
    const p = pathLabel(c).split(" › ");
    return p.length > 2 ? `${p[0]} › … › ${p.at(-1)}` : p.join(" › ");
  };
</script>

{#if ship.virtual}
  <p class="dim">Fitted parts become readable once the ship is in a stash.</p>
{:else}
  <div class="bar">
    {#if loadout.compared}
      <Tag label={loadout.off ? `${loadout.off} fitted ≠ saved` : "Fitted matches saved"} tone={loadout.off ? "warning" : "success"} />
      <Tag label={`${loadout.custom} saved ≠ stock`} />
      <span class="grow"></span>
      <ChipButton on={showStock} onclick={() => (showStock = !showStock)}>Stock</ChipButton>
      <ChipButton on={onlyDiffs} onclick={() => (onlyDiffs = !onlyDiffs)}>Only differences</ChipButton>
    {:else}
      <span class="dim">Fitted now. Saved and stock loadouts need “Loadouts” on in Settings → Online.</span>
    {/if}
  </div>
  {#if !sections.length}<p class="dim">No differences.</p>{/if}
  <div class="cols">
    {#each sections as { s, hardpoints } (s)}
      {@const Icon = SECTION_ICON[s]}
      <section class="sec">
        <h4><Icon size={13} />{s}<span class="num">{hardpoints.reduce((n, h) => n + h.n, 0)}</span></h4>
        {#each hardpoints as h (h.key)}
          <div class="hp">
            {#if h.mount}
              <div class="mount">
                <span class="size">{h.mount.size ? `S${h.mount.size}` : ""}</span><span class="grow">{h.mount.name}</span>
                {#if h.n > 1}<span class="num">×{h.n}</span>{/if}
              </div>
            {/if}
            {#each h.lines as l, k (k)}
              {@const n = h.mount ? l.n : l.n * h.n}
              {@const empty = !l.fitted && l.status === "off"}
              {@const where = l.status === "off" && l.saved ? whereNow(l.saved) : []}
              <button
                type="button"
                class="line {l.status}"
                class:sel={!!l.part && l.part.guid === selectedGuid}
                style:padding-left="{8 + Math.max(0, l.depth - 1) * 14}px"
                onclick={() => l.part && onselect(l.part)}
              >
                <span class="size">{l.part?.size ? `S${l.part.size}` : ""}</span>
                <span class="lbody">
                  <span class="lname">
                    {#if l.status === "unknown"}<span class="dim">{l.saved?.name ?? "Empty"}</span>
                    {:else if empty}<em class="emptied">Empty</em>
                    {:else}{l.fitted?.name}{/if}
                    {#if n > 1}<span class="num dim">{` ×${n}`}</span>{/if}
                  </span>
                  {#if l.status === "off"}
                    <span class="diff warn">
                      <span>Saved: {l.saved?.name ?? "Empty"}</span>
                      {#each where.slice(0, 2) as w (w.c.id)}
                        <span
                          class="where"
                          role="link"
                          tabindex="-1"
                          title={pathLabel(w.c)}
                          onclick={(e) => {
                            e.stopPropagation();
                            ongo(w.c);
                          }}
                          onkeydown={() => {}}><CornerDownRight size={11} />{shortPath(w.c)}{w.n > 1 ? ` ×${w.n}` : ""}</span
                        >
                      {/each}
                    </span>
                  {/if}
                  {#if l.status === "unknown"}<span class="diff dim">Saved; fitted state not in the inventory data</span>{/if}
                  {#if showStock && l.custom}<span class="diff dim">Stock: {l.stock?.name ?? "Empty"}</span>{/if}
                </span>
                <span class="mark" title={l.status === "ok" ? "Fitted as saved" : l.status === "off" ? "Differs from saved" : "Unknown"}>
                  {#if l.status === "ok"}<Check size={13} />{:else if l.status === "off"}<TriangleAlert size={13} />{:else}<CircleQuestionMark
                      size={13}
                    />{/if}
                </span>
              </button>
            {/each}
          </div>
        {/each}
      </section>
    {/each}
  </div>
{/if}

<style>
  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin: 10px 0;
  }
  .grow {
    flex: 1;
  }
  .dim {
    color: var(--text-dim);
    font-size: 0.8rem;
  }
  .cols {
    columns: 240px;
    column-gap: 12px;
  }
  .sec {
    break-inside: avoid;
    margin-bottom: 14px;
  }
  h4 {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0 0 6px;
    font-family: var(--font-mono);
    font-size: 0.62rem;
    font-weight: 400;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-dim);
  }
  h4 .num {
    color: var(--text-faint);
    letter-spacing: 0;
  }
  .hp {
    break-inside: avoid;
    margin-bottom: 6px;
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: color-mix(in srgb, var(--bg-raised) 45%, transparent);
  }
  .mount {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 6px 8px;
    border-bottom: 1px solid var(--border-soft);
    font-size: 0.74rem;
    color: var(--text-dim);
  }
  .line {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    width: 100%;
    padding: 7px 8px;
    border: 0;
    border-left: 3px solid transparent;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .line + .line {
    border-top: 1px solid var(--border-soft);
  }
  .line:hover {
    background: var(--bg-raised);
  }
  .line.ok {
    border-left-color: color-mix(in srgb, var(--good) 55%, transparent);
  }
  .line.off {
    border-left-color: var(--warn);
    background: color-mix(in srgb, var(--warn) 6%, transparent);
  }
  .line.unknown {
    border-left-color: var(--border);
  }
  .line.sel {
    box-shadow: inset 0 0 0 1px var(--accent);
  }
  .size {
    flex: 0 0 auto;
    min-width: 26px;
    font-family: var(--font-mono);
    font-size: 0.64rem;
    font-weight: 700;
    text-align: center;
    color: var(--text-dim);
  }
  .lbody {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .lname {
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--text-strong);
  }
  .emptied {
    font-weight: 500;
    color: var(--warn);
  }
  .diff {
    display: flex;
    flex-direction: column;
    gap: 1px;
    margin-top: 2px;
    font-size: 0.72rem;
  }
  .diff.warn {
    color: var(--warn);
  }
  .where {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--text-dim);
    cursor: pointer;
  }
  .where:hover {
    color: var(--text);
  }
  .mark {
    flex: 0 0 auto;
    padding-top: 1px;
  }
  .ok .mark {
    color: var(--good);
  }
  .off .mark {
    color: var(--warn);
  }
  .unknown .mark {
    color: var(--text-faint);
  }
  .num {
    font-family: var(--font-mono);
  }
</style>
