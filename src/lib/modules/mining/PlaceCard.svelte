<script lang="ts">
  // The place picked in the tree: its name large and the ship rocks that
  // spawn there with their share, with a loadout the ship's class split and
  // good share (easy and manageable rolls), and each rock's mass range on
  // the shared scale. A Lagrange point (design pass 4) says whose point it
  // is and its rest stop, and lists its tables by what they spawn at: its
  // own (Pyro, with where else the same table spawns), the gas pockets, the
  // table every mining claim carries, the yield tables with their claims by
  // name. A place under several spawn tables lists each by name; a table the
  // place shares with others says where else it applies.
  import type { AreaView, ClassSharesView, PlaceNodeView } from "$lib/bindings";
  import NowCard from "$lib/components/NowCard.svelte";
  import { oreName, pct } from "./format";
  import MassAxis from "./MassAxis.svelte";
  import RockArt from "./RockArt.svelte";
  import RockRow from "./RockRow.svelte";

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
    /** The planet a moon, place or Lagrange point sits under. */
    parent: PlaceNodeView | null;
    areas: Map<string, AreaView>;
    area: string;
    rockType: string;
    classOf: (area: string, rockType: string) => ClassSharesView | null;
    ship: string | null;
    onpick: (area: string, rockType: string) => void;
  } = $props();

  const isPoint = $derived(node.kind === "lpoint");
  /** `HUR L1` → `L1`. */
  const short = (name: string) => name.split(" ").pop() ?? name;
  const listed = (names: string[]) =>
    names.length > 3 ? `${names.slice(0, 3).join(", ")} +${names.length - 3}` : names.join(", ");

  type Group = { title: string | null; detail: string | null; claims: boolean; list: { a: AreaView; also: string }[] };
  const groups = $derived.by((): Group[] => {
    const of = (ids: string[]) =>
      ids.flatMap((id) => {
        const a = areas.get(id);
        if (!a) return [];
        // Where else the table spawns: a point's siblings by their L, others by name.
        const others = a.places.filter((p) => p.id !== node.id);
        const code = node.name.split(" ")[0];
        const also = isPoint
          ? others.length
            ? `Same spawn table at ${listed(others.map((p) => (p.name.startsWith(`${code} `) ? short(p.name) : p.name)))}`
            : ""
          : others.length && node.kind !== "field"
            ? `Same spawn table at ${listed(others.map((p) => p.name))}`
            : "";
        return [{ a, also }];
      });
    if (isPoint)
      return node.groups.map((g) => ({
        title: g.title,
        detail: g.detail,
        claims: g.title !== null && g.title.includes("claims"),
        // Only the point's own table says where else it spawns: a pocket's
        // or claim's table is everywhere already.
        list: of(g.areas).map((x) => ({ ...x, also: g.title === null ? x.also : "" })),
      }));
    const list = of(node.areas);
    return list.map((x) => ({
      title: list.length > 1 && !x.a.fromRecord ? x.a.name : null,
      detail: null,
      claims: false,
      list: [x],
    }));
  });
  const withClasses = $derived(
    ship !== null && groups.some((g) => g.list.some((x) => x.a.rocks.some((r) => classOf(x.a.id, r.rockType)))),
  );
  const where = $derived(
    node.kind === "field"
      ? `${node.count} places`
      : isPoint
        ? `Lagrange point${parent ? ` of ${parent.name}` : ""}`
        : parent
          ? node.kind === "body"
            ? `moon of ${parent.name}`
            : `at ${parent.name}`
          : "",
  );
</script>

<NowCard hue="var(--hue-sand)" chip={isPoint ? "◆ LAGRANGE POINT" : "◆ PLACE"} version={node.name} date={where}>
  {#snippet art()}<RockArt />{/snippet}
  {#if isPoint}
    <p class="stop">
      {#if node.restStop}rest stop <b>{node.restStop}</b>{:else}<span class="faint">no rest stop</span>{/if}
      {#if node.count}<span class="faint"> · {node.count} mining claims</span>{/if}
    </p>
  {/if}
  <div class="head label" class:cls={withClasses}>
    <span>Ship rocks</span><span>share</span>{#if withClasses}<span>Your {ship}</span><span>good</span>{/if}
  </div>
  <MassAxis />
  {#each groups as g, gi (gi)}
    {#if g.title}
      <p class="gh" class:claims={g.claims}>{g.title}{#if g.detail}<small>{g.detail}</small>{/if}</p>
    {/if}
    {#each g.list as x (x.a.id)}
      <div class="rocks">
        {#each x.a.rocks as r (r.rockType)}
          <RockRow
            name={oreName(r.name)}
            figure={pct(r.share)}
            classes={classOf(x.a.id, r.rockType)}
            {withClasses}
            mass={r.mass}
            on={x.a.id === area && r.rockType === rockType}
            disabled={!r.rollable}
            title={r.rollable ? undefined : "No rock model in the game files"}
            onclick={() => onpick(x.a.id, r.rockType)}
          />
        {/each}
      </div>
      {#if x.also}<p class="also">{x.also}</p>{/if}
    {/each}
  {/each}
</NowCard>

<style>
  .stop {
    margin: -4px 0 0;
    font-size: var(--text-small);
    color: var(--text-dim);
  }
  .stop b {
    color: var(--text);
    font-weight: var(--weight-semibold);
  }
  .head {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 54px;
    gap: 12px;
    align-items: center;
    margin: 4px 0 0;
    font-size: 0.66rem;
    color: var(--text-faint);
  }
  .head.cls {
    grid-template-columns: minmax(0, 1fr) 54px minmax(70px, 1fr) 30px;
  }
  .head span:nth-child(2),
  .head span:nth-child(4) {
    text-align: right;
  }
  .gh {
    display: flex;
    flex-direction: column;
    gap: 1px;
    margin: 10px 0 0;
    font-size: 0.9rem;
    font-weight: var(--weight-semibold);
  }
  .gh small {
    font-size: var(--text-small);
    font-weight: normal;
    color: var(--text-faint);
  }
  .gh.claims small {
    line-height: 1.5;
  }
  .rocks {
    display: flex;
    flex-direction: column;
  }
  .also {
    margin: 2px 0 0;
    font-size: var(--text-small);
    color: var(--text-faint);
  }
</style>
