<script lang="ts">
  // The place picked in the tree: its name large and the ship rocks that
  // spawn there with their share. A place under several spawn areas lists
  // each; an area the place shares with others says with which.
  import type { AreaView, PlaceNodeView } from "$lib/bindings";
  import NowCard from "$lib/components/NowCard.svelte";
  import { pct } from "./format";
  import RockArt from "./RockArt.svelte";

  let {
    node,
    parent,
    areas,
    area,
    rockType,
    onpick,
  }: {
    node: PlaceNodeView;
    /** The planet a moon or place sits under. */
    parent: PlaceNodeView | null;
    areas: Map<string, AreaView>;
    area: string;
    rockType: string;
    onpick: (area: string, rockType: string) => void;
  } = $props();

  const groups = $derived(
    node.areas.flatMap((id) => {
      const a = areas.get(id);
      if (!a) return [];
      const others = a.places.filter((p) => p.id !== node.id).map((p) => p.name);
      const top = Math.max(0.0001, ...a.rocks.map((r) => r.share));
      return [{ a, others, top }];
    }),
  );
  const heads = $derived(groups.length > 1 || groups.some((g) => g.others.length > 0 && node.kind !== "field"));
  const where = $derived(
    node.kind === "field"
      ? `${node.count} places`
      : parent
        ? node.kind === "body"
          ? `moon of ${parent.name}`
          : `at ${parent.name}`
        : "",
  );
  const shared = (names: string[]) =>
    names.length > 3 ? `Shared with ${names.slice(0, 3).join(", ")} +${names.length - 3}` : `Shared with ${names.join(", ")}`;
</script>

<NowCard hue="var(--hue-sand)" chip="◆ PLACE" version={node.name} date={where}>
  {#snippet art()}<RockArt />{/snippet}
  <p class="label">Ship rocks here</p>
  {#each groups as g (g.a.id)}
    {#if heads}
      <div class="head">
        <b>{g.a.name}</b>
        {#if g.others.length && node.kind !== "field"}<span>{shared(g.others)}</span>{/if}
      </div>
    {/if}
    <div class="rocks">
      {#each g.a.rocks as r (r.rockType)}
        <button
          class="rk"
          class:on={g.a.id === area && r.rockType === rockType}
          disabled={!r.rollable}
          title={r.rollable ? undefined : "No rock model in the game files"}
          onclick={() => onpick(g.a.id, r.rockType)}
        >
          <b>{r.name}</b>
          <span class="track"><i style:width="{(r.share / g.top) * 100}%"></i></span>
          <span class="n">{pct(r.share)}</span>
        </button>
      {/each}
    </div>
  {/each}
</NowCard>

<style>
  .label {
    margin: 4px 0 0;
  }
  .head {
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding-top: 6px;
  }
  .head b {
    font-size: 0.9rem;
  }
  .head span {
    font-size: var(--text-small);
    color: var(--text-faint);
  }
  .rocks {
    display: flex;
    flex-direction: column;
  }
  .rk {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(60px, 1.2fr) 44px;
    gap: 12px;
    align-items: center;
    padding: 9px 8px;
    margin-inline: -8px;
    border: 0;
    border-radius: 6px;
    border-top: 1px solid color-mix(in srgb, var(--text) 7%, transparent);
    background: none;
    font: inherit;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .rk:first-child {
    border-top-color: transparent;
  }
  .rk:hover:not(:disabled) {
    background: color-mix(in srgb, var(--text) 4%, transparent);
  }
  .rk.on {
    background: var(--accent-fill);
  }
  .rk:disabled {
    color: var(--text-faint);
    cursor: default;
  }
  .rk b {
    font-size: 0.95rem;
    font-weight: var(--weight-semibold);
  }
  .track {
    display: block;
    height: 4px;
    border-radius: 2px;
    background: color-mix(in srgb, var(--text) 8%, transparent);
    overflow: hidden;
  }
  .track i {
    display: block;
    height: 100%;
    background: color-mix(in srgb, var(--hue-sand) 65%, transparent);
  }
  .n {
    font-family: var(--font-mono);
    font-size: 0.8rem;
    text-align: right;
    color: var(--text-dim);
  }
</style>
