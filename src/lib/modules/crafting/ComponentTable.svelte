<script lang="ts">
  // One kind of ship component as the shops shelve it (UX board sketches 3
  // and 8): sizes across, class and grade down, one recipe per row (sizes
  // only scale the amounts). What the field and the view tab leave out dims
  // in place; nothing moves.
  import { componentGrid } from "./browse";
  import type { Family } from "./model";
  import { lookOf } from "./shelf";

  let {
    kind,
    list,
    keep,
    klass,
    selected,
    onpick,
  }: {
    kind: string;
    /** Every blueprint of the kind: the table's shape never changes. */
    list: Family[];
    /** What the view tab and the typed text leave in. */
    keep: (f: Family) => boolean;
    /** The class word in the field, if any: other rows dim. */
    klass: string;
    selected: string | null;
    onpick: (f: Family) => void;
  } = $props();

  const grid = $derived(componentGrid(list));
  const look = $derived(lookOf(kind));
  const Icon = $derived(look.icon);
  const owned = $derived(list.filter((f) => f.ownedCount > 0).length);
  const amounts = (size: number) => {
    const f = list.find((x) => x.variants[0].size === size);
    return f ? f.variants[0].materials.map((m) => (m.scu != null ? m.scu.toFixed(2) : `${m.count ?? 0}`)).join(" · ") : "";
  };
  /** Rows, each marked when it opens a class (the class name shows once). */
  const rows = $derived(grid.rows.map((r, i) => ({ ...r, first: i === 0 || grid.rows[i - 1].klass !== r.klass })));
</script>

<section class="table" style="--hue: {look.hue}; --cols: {grid.sizes.length}">
  <div class="shelf">
    <Icon size={15} />
    <h2>{kind}</h2>
    <span class="count">{owned} of {list.length} owned</span>
  </div>
  <div class="grid">
    <div class="h lbl">Class · grade</div>
    {#each grid.sizes as s (s)}
      <div class="h"><span class="size">Size {s}</span><span class="amt">{amounts(s)} SCU</span></div>
    {/each}
    <div class="h lbl rec">One recipe per row</div>
    {#each rows as r (r.key)}
      {@const first = r.first}
      {@const off = (!!klass && r.klass !== klass) || r.cells.every((c) => c.every((f) => !keep(f)))}
      <div class="c cls" class:first class:off>
        <span>{first ? r.klass || "Unclassed" : ""}</span><b>{r.grade}</b>
      </div>
      {#each r.cells as cell, i (i)}
        <div class="c" class:first class:off>
          {#each cell as f (f.key)}
            <button
              type="button"
              class="name"
              class:own={f.ownedCount > 0}
              class:dim={!keep(f)}
              class:on={selected === f.key}
              title={f.ownedCount > 0 ? `${f.title}: you own this blueprint` : f.title}
              onclick={() => onpick(f)}>{f.title}</button
            >
          {/each}
        </div>
      {/each}
      <div class="c rec" class:first class:off>{r.recipe.join(" · ")}</div>
    {/each}
  </div>
</section>

<style>
  .table {
    margin-bottom: 18px;
  }
  .shelf {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 12px 6px 6px;
    padding-bottom: 6px;
    border-bottom: 1px solid color-mix(in srgb, var(--hue) 30%, var(--border));
    color: color-mix(in srgb, var(--hue) 75%, var(--text-strong));
  }
  .shelf h2 {
    margin: 0;
    font-family: var(--font-display);
    font-size: 1.12rem;
    font-weight: var(--weight-bold);
    color: var(--text-strong);
  }
  .count {
    margin-left: auto;
    font-family: var(--font-mono);
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .grid {
    display: grid;
    grid-template-columns: 120px repeat(var(--cols), minmax(0, 1fr)) minmax(200px, 1.5fr);
    font-size: var(--text-control);
  }
  .h,
  .c {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0 10px;
    min-height: 30px;
    padding: 4px 10px;
    border-bottom: 1px solid color-mix(in srgb, var(--border) 60%, transparent);
  }
  .h {
    flex-direction: column;
    align-items: flex-start;
    justify-content: flex-end;
    gap: 0;
    border-bottom-color: var(--border);
  }
  .lbl {
    justify-content: flex-end;
    font-family: var(--font-mono);
    font-size: var(--text-badge);
    letter-spacing: var(--label-tracking);
    text-transform: uppercase;
    color: var(--text-faint);
  }
  .size {
    font-weight: var(--weight-semibold);
    color: var(--text-strong);
  }
  .amt {
    font-family: var(--font-mono);
    font-size: var(--text-badge);
    color: var(--text-faint);
  }
  .c.first {
    border-top: 1px solid color-mix(in srgb, var(--text) 14%, var(--border));
  }
  .c.off {
    opacity: 0.32;
  }
  .cls {
    gap: 0 8px;
    font-weight: var(--weight-semibold);
    color: var(--text-strong);
  }
  .cls b {
    font-family: var(--font-mono);
    color: var(--text-dim);
  }
  .rec {
    border-left: 1px solid var(--border);
    font-size: var(--text-small);
    color: var(--text-dim);
  }
  .name {
    padding: 1px 4px;
    margin: 0 -4px;
    border: 0;
    border-radius: var(--radius-chip);
    background: transparent;
    color: var(--text);
    font: inherit;
    cursor: pointer;
  }
  .name:hover {
    background: color-mix(in srgb, var(--text) 7%, transparent);
  }
  .name.own {
    color: var(--good);
    font-weight: var(--weight-semibold);
  }
  .name.dim {
    opacity: 0.35;
  }
  .name.on {
    background: color-mix(in srgb, var(--accent) 18%, transparent);
    color: var(--text-strong);
  }
</style>
