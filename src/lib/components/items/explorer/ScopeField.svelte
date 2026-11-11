<script lang="ts">
  // My Items' scope field (decision 'My Items opens on the few you care
  // about, and one field is its only scope line', 2026-11-11): the place, the
  // kind and a sub kind as words over the shared WordField, then free text.
  // The place word opens up / inside / beside, as the breadcrumb's crumbs
  // did, and the whole tree from there. Focused, it offers where to go
  // (field.ts); typing filters the page as it did.
  import type { SvelteSet } from "svelte/reactivity";
  import { Popup } from "@veelume/ui";
  import WordField, { type FieldGroup, type FieldRow, type FieldWord } from "$lib/components/WordField.svelte";
  import { SUBCATEGORIES, subcategoryOf, type Category } from "../model/classify";
  import { kindsIn, placeNote, suggest, type Suggestion } from "../model/field";
  import { areasBelow } from "../model/gear";
  import { ancestors, type ItemNode, type Node, type PlaceNode, type Tree } from "../model/tree";
  import TreeNode from "./TreeNode.svelte";
  import { nodeIcon, type View } from "./ui";

  let {
    tree,
    view,
    at,
    q,
    cat,
    sub,
    crafted,
    kindsFrom,
    here,
    all,
    areas,
    craftedCount,
    open,
    count,
    kidsOf,
    isAt,
    ongo,
    onopen,
    onkind,
    oncrafted,
    onq,
  }: {
    tree: Tree;
    view: View;
    at: Node;
    q: string;
    cat: string;
    sub: string;
    crafted: boolean;
    kindsFrom: ItemNode[];
    here: ItemNode[];
    all: ItemNode[];
    areas: { area: PlaceNode; n: number }[];
    craftedCount: number;
    /** The place tree's open branches (page-local). */
    open: SvelteSet<string>;
    count: (n: Node) => number;
    kidsOf: (n: Node) => Node[];
    isAt: (n: Node) => boolean;
    /** Each of these also clears the typed text, in one URL write (two
     *  writes in a row would undo each other). */
    ongo: (n: Node) => void;
    /** Go to a thing: its container, with it picked. */
    onopen: (n: ItemNode) => void;
    onkind: (cat: string, sub: string) => void;
    oncrafted: (on: boolean) => void;
    onq: (q: string) => void;
  } = $props();

  let treeOpen = $state(false);
  const items = $derived(view === "items");
  const root = $derived(at.type === "place" && at.level === "root");

  // A place that holds nothing of its own offers the areas below it instead
  // of its next level (the storage is one pick away, not three).
  const atFlat = $derived(at.type === "place" && (at.level === "root" || at.level === "system" || at.level === "body"));
  const placeRow = (n: Node, note = ""): FieldRow => ({
    key: n.id,
    label: n.name,
    note,
    n: count(n),
    icon: nodeIcon(n),
    cur: n === at,
    pick: () => ongo(n),
  });
  const placeMenu = $derived.by<FieldGroup[]>(() => {
    const up = ancestors(at).filter((x) => x !== at);
    const beside = at.parent && !atFlat ? kidsOf(at.parent).filter((x) => x !== at) : [];
    const inside = atFlat ? areasBelow(at, count).slice(0, 8).map((a) => a.area as Node) : kidsOf(at);
    return [
      { title: "Up", rows: up.map((n) => placeRow(n)) },
      { title: atFlat ? "Biggest here" : "Inside", rows: inside.map((n) => placeRow(n, atFlat ? placeNote(n) : "")) },
      { title: "Beside", rows: beside.map((n) => placeRow(n)) },
      { rows: [{ key: "tree", label: "All places, as a tree…", pick: () => (treeOpen = true) }] },
    ];
  });

  const words = $derived.by<FieldWord[]>(() => {
    const out: FieldWord[] = [
      {
        key: "place",
        label: root ? "Everywhere" : at.type === "place" && at.level === "area" && at.parent ? `${at.name} · ${at.parent.name}` : at.name,
        menu: placeMenu,
        remove: root ? undefined : () => ongo(tree.root),
        removeLabel: "Look everywhere",
      },
    ];
    if (crafted) out.push({ key: "crafted", label: "Crafted", remove: () => oncrafted(false) });
    if (cat && items) {
      out.push({
        key: "kind",
        label: cat,
        menu: [{ rows: kindsIn(kindsFrom).map((k) => ({ key: k.cat, label: k.cat, n: k.n, cur: k.cat === cat, pick: () => onkind(k.cat, "") })) }],
        remove: () => onkind("", ""),
      });
      if (sub) {
        const subs = (SUBCATEGORIES[cat as Category] ?? []).map((s) => ({
          s,
          n: kindsFrom.filter((x) => subcategoryOf(x, cat as Category) === s).reduce((t, x) => t + x.item.stack, 0),
        }));
        out.push({
          key: "sub",
          label: sub,
          menu: [{ rows: subs.map((x) => ({ key: x.s, label: x.s, n: x.n, cur: x.s === sub, off: !x.n, pick: () => onkind(cat, x.s) })) }],
          remove: () => onkind(cat, ""),
        });
      }
    }
    return out;
  });

  const pickOf = (s: Suggestion) =>
    s.type === "crafted"
      ? () => oncrafted(true)
      : s.type === "place"
        ? () => ongo(s.node)
        : s.type === "kind"
          ? () => onkind(s.cat, s.sub)
          : () => onopen(s.node);
  const keyOf = (s: Suggestion) =>
    s.type === "place" || s.type === "thing" ? s.node.id : s.type === "kind" ? `${s.cat}/${s.sub}` : s.type;
  const suggestions = $derived.by<FieldGroup[]>(() => {
    const g = suggest(q, { tree, kindsFrom, here, all, count, areas, crafted: crafted ? 0 : craftedCount });
    return (items ? g : g.filter((x) => x.title === "Places")).map((x) => ({
      title: x.title,
      rows: x.items.map((s) => ({ key: keyOf(s), label: s.label, note: s.note, n: s.n, pick: pickOf(s) })),
    }));
  });

  /** Drop the last word: the sub kind, the kind, Crafted, then the place. */
  function onback() {
    if (sub) onkind(cat, "");
    else if (cat) onkind("", "");
    else if (crafted) oncrafted(false);
    else if (!root) ongo(tree.root);
  }
</script>

<span class="wrap">
  <WordField
    {words}
    {q}
    placeholder={view === "ships" ? "Find a ship…" : view === "res" ? "Find a material…" : "A thing, a kind, a place…"}
    {suggestions}
    {onq}
    {onback}
  />
  <Popup open={treeOpen} onclose={() => (treeOpen = false)} label="All places" class="places-pop">
    <div role="tree">
      <TreeNode
        node={tree.root}
        depth={0}
        {open}
        forceOpen={false}
        {isAt}
        {count}
        {kidsOf}
        ongo={(n) => ((treeOpen = false), ongo(n))}
      />
    </div>
  </Popup>
</span>

<style>
  .wrap {
    position: relative;
    flex: 1;
    min-width: 0;
    display: flex;
  }
  .wrap :global(.places-pop) {
    width: 300px;
    max-height: 60vh;
    overflow-y: auto;
    padding: 6px;
    border-radius: var(--radius-lg);
    background: var(--bg-raised);
    border: 1px solid var(--border);
    box-shadow: var(--shadow-panel);
  }
</style>
