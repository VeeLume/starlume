<script lang="ts">
  // A place to browse into: a location, an area, a container, or a ship
  // (ships get their loadout state on the tile).
  import { Clock } from "lucide-svelte";
  import { offSaved } from "../model/loadout";
  import { stowed, units, visible, type Node } from "../model/tree";
  import Tag from "./Tag.svelte";
  import { fmtN, fmtTime, nodeIcon, SOURCE } from "./ui";

  let { node, on, bricked, onclick }: { node: Node; on: boolean; bricked: boolean; onclick: () => void } = $props();

  const Icon = $derived(nodeIcon(node));
  const ship = $derived(node.type === "ship" ? node.ship : null);
  const off = $derived(ship ? offSaved(ship) : null);
  const inside = $derived(
    node.type === "ship"
      ? node.virtual
        ? 0
        : stowed(node, (x) => visible(x, bricked)).reduce((s, x) => s + x.item.stack, 0)
      : units(node, (x) => visible(x, bricked)),
  );
  const carried = $derived(node.kids.filter((k) => k.type === "ship"));
  const preview = $derived(
    node.type === "ship"
      ? ""
      : node.kids
          .filter((k) => visible(k, bricked))
          .slice(0, 3)
          .map((k) => k.name)
          .join(", "),
  );
</script>

<button type="button" class="tile" class:on class:ship={!!ship} {onclick}>
  <span class="top">
    <Icon size={15} />
    <span class="name">{node.name}</span>
    {#if ship?.kind.size}<span class="size">S{ship.kind.size}</span>{/if}
  </span>
  {#if preview}<span class="sub">{preview}</span>{/if}
  <span class="meta">
    {#if ship}
      {#if ship.rental_expiry}<span class="ends"><Clock size={10} />ends {fmtTime(ship.rental_expiry)}</span>{/if}
      {#if ship.ownership === "bricked"}<Tag label="bricked" tone="destructive" />{/if}
      {#if off != null}<Tag label={off ? `${off} off saved` : "as saved"} tone={off ? "warning" : "success"} />{/if}
      {#if inside > 0}<span>{fmtN(inside)} stored</span>{/if}
      {#if carried.length}<span>carries {carried.map((c) => c.name.replace(/^\S+\s/, "")).join(", ")}</span>{/if}
      {#if SOURCE[ship.source]}<span>{SOURCE[ship.source]}</span>{/if}
    {:else}
      <span class="num">{fmtN(inside)} inside</span>
      {#if node.type === "item" && node.item.ownership === "rented"}<Tag label="rented" tone="warning" />{/if}
    {/if}
  </span>
</button>

<style>
  .tile {
    display: flex;
    flex-direction: column;
    gap: 5px;
    min-width: 0;
    min-height: 58px;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: color-mix(in srgb, var(--bg-raised) 55%, transparent);
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
    transition:
      background 90ms,
      border-color 90ms;
  }
  .tile:hover {
    background: var(--bg-raised);
  }
  .tile.on {
    border-color: var(--accent);
    background: var(--accent-fill-faint);
  }
  .top {
    display: flex;
    gap: 7px;
    align-items: flex-start;
    color: var(--text-dim);
  }
  .name {
    flex: 1;
    min-width: 0;
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--text-strong);
    line-height: 1.3;
  }
  .size {
    font-family: var(--font-mono);
    font-size: 0.68rem;
    color: var(--text-faint);
  }
  .sub {
    font-size: 0.7rem;
    color: var(--text-dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 7px;
    margin-top: auto;
    font-size: 0.68rem;
    color: var(--text-faint);
  }
  .ends {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    color: var(--warn);
  }
  .num {
    font-family: var(--font-mono);
  }
</style>
