<script lang="ts">
  // The place picked in the tree: its name large and the ship rocks that
  // spawn there with their share and, with a loadout, the ship's class
  // split and good share (easy and manageable rolls). A place under several spawn
  // tables lists each by name; a table the place shares with others says
  // where else it applies instead of showing a file name.
  import type { AreaView, ClassSharesView, PlaceNodeView } from "$lib/bindings";
  import NowCard from "$lib/components/NowCard.svelte";
  import ClassBar from "./ClassBar.svelte";
  import { oreName, pct } from "./format";
  import RockArt from "./RockArt.svelte";

  let {
    node,
    parent,
    areas,
    area,
    rockType,
    classOf,
    ship,
    onpick,
  }: {
    node: PlaceNodeView;
    /** The planet a moon or place sits under. */
    parent: PlaceNodeView | null;
    areas: Map<string, AreaView>;
    area: string;
    rockType: string;
    classOf: (area: string, rockType: string) => ClassSharesView | null;
    ship: string | null;
    onpick: (area: string, rockType: string) => void;
  } = $props();

  const good = (c: ClassSharesView | null) => (c ? Math.round((c.easy + c.manageable) * 100) : null);
  const groups = $derived(
    node.areas.flatMap((id) => {
      const a = areas.get(id);
      if (!a) return [];
      const others = a.places.filter((p) => p.id !== node.id).map((p) => p.name);
      return [{ a, others }];
    }),
  );
  const withClasses = $derived(ship !== null && groups.some((g) => g.a.rocks.some((r) => classOf(g.a.id, r.rockType))));
  const where = $derived(
    node.kind === "field"
      ? `${node.count} places`
      : parent
        ? node.kind === "body"
          ? `moon of ${parent.name}`
          : `at ${parent.name}`
        : "",
  );
  const listed = (names: string[]) =>
    names.length > 3 ? `${names.slice(0, 3).join(", ")} +${names.length - 3}` : names.join(", ");
</script>

<NowCard hue="var(--hue-sand)" chip="◆ PLACE" version={node.name} date={where}>
  {#snippet art()}<RockArt />{/snippet}
  <div class="head label" class:cls={withClasses}>
    <span>Ship rocks here</span><span>share</span>{#if withClasses}<span>Your {ship}</span><span>good</span>{/if}
  </div>
  {#each groups as g (g.a.id)}
    {#if groups.length > 1 && !g.a.fromRecord}<p class="area">{g.a.name}</p>{/if}
    <div class="rocks">
      {#each g.a.rocks as r (r.rockType)}
        {@const c = classOf(g.a.id, r.rockType)}
        <button
          class="rk"
          class:cls={withClasses}
          class:on={g.a.id === area && r.rockType === rockType}
          disabled={!r.rollable}
          title={r.rollable ? undefined : "No rock model in the game files"}
          onclick={() => onpick(g.a.id, r.rockType)}
        >
          <b>{oreName(r.name)}</b>
          <span class="n">{pct(r.share)}</span>
          {#if withClasses}
            {#if c}<ClassBar shares={c} />{:else}<span></span>{/if}
            <span class="n g">{good(c) ?? "—"}</span>
          {/if}
        </button>
      {/each}
    </div>
    {#if g.others.length && node.kind !== "field"}
      <p class="also">Same spawn table at {listed(g.others)}</p>
    {/if}
  {/each}
</NowCard>

<style>
  .head,
  .rk {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 54px;
    gap: 12px;
    align-items: center;
  }
  .head.cls,
  .rk.cls {
    grid-template-columns: minmax(0, 1fr) 54px minmax(70px, 1fr) 30px;
  }
  .head {
    margin: 4px 0 0;
    font-size: 0.66rem;
    color: var(--text-faint);
  }
  .head span:nth-child(2),
  .head span:nth-child(4) {
    text-align: right;
  }
  .area {
    margin: 6px 0 0;
    font-size: 0.9rem;
    font-weight: var(--weight-semibold);
  }
  .rocks {
    display: flex;
    flex-direction: column;
  }
  .rk {
    padding: 9px 8px;
    margin-inline: -8px;
    border: 0;
    border-top: 1px solid color-mix(in srgb, var(--text) 7%, transparent);
    border-radius: 6px;
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
  .n {
    font-family: var(--font-mono);
    font-size: 0.8rem;
    text-align: right;
    color: var(--text-dim);
  }
  .n.g {
    color: var(--good);
    font-weight: var(--weight-semibold);
  }
  .also {
    margin: 2px 0 0;
    font-size: var(--text-small);
    color: var(--text-faint);
  }
</style>
