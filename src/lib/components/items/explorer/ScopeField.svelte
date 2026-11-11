<script lang="ts">
  // The scope field: the page's one scope line (decision 'My Items opens on
  // the few you care about, and one field is its only scope line',
  // 2026-11-11). It holds the place, the kind and a sub kind as words, then
  // free text. A word opens its neighbours with counts, as the breadcrumb's
  // crumbs did, and × or Backspace drops it. Focused, the field offers where
  // to go (field.ts); typing filters the page as it did.
  import { tick } from "svelte";
  import type { SvelteSet } from "svelte/reactivity";
  import { Popup } from "@veelume/ui";
  import { X } from "lucide-svelte";
  import Chevron from "$lib/components/Chevron.svelte";
  import { SUBCATEGORIES, subcategoryOf, type Category } from "../model/classify";
  import { kindsIn, placeNote, suggest, type Suggestion } from "../model/field";
  import { areasBelow } from "../model/gear";
  import { ancestors, type ItemNode, type Node, type PlaceNode, type Tree } from "../model/tree";
  import TreeNode from "./TreeNode.svelte";
  import { fmtN, nodeIcon, type View } from "./ui";

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
    /** Each of these also clears the typed text, in one URL write. */
    ongo: (n: Node) => void;
    /** Go to a thing: its container, with it picked. */
    onopen: (n: ItemNode) => void;
    onkind: (cat: string, sub: string) => void;
    oncrafted: (on: boolean) => void;
    onq: (q: string) => void;
  } = $props();

  let input = $state<HTMLInputElement>();
  let focused = $state(false);
  let listOpen = $state(false);
  let hi = $state(-1);
  /** Which word's menu is open: "place" | "kind" | "sub" | "tree". */
  let menu = $state<string | null>(null);

  const items = $derived(view === "items");
  const groups = $derived.by(() => {
    const g = suggest(q, { tree, kindsFrom, here, all, count, areas, crafted: crafted ? 0 : craftedCount });
    return items ? g : g.filter((x) => x.title === "Places");
  });
  const flat = $derived(groups.flatMap((g) => g.items));
  const showList = $derived(focused && listOpen && flat.length > 0);

  const root = $derived(at.type === "place" && at.level === "root");
  const placeLabel = $derived(
    root ? "Everywhere" : at.type === "place" && at.level === "area" && at.parent ? `${at.name} · ${at.parent.name}` : at.name,
  );
  const placeholder = $derived(
    view === "ships" ? "Find a ship…" : view === "res" ? "Find a material…" : "A thing, a kind, a place…",
  );

  /** A pick replaces the typed text (the handlers clear it in the same URL
   *  write: two writes in a row would undo each other). */
  function pick(s: Suggestion) {
    if (s.type === "crafted") oncrafted(true);
    else if (s.type === "place") ongo(s.node);
    else if (s.type === "kind") onkind(s.cat, s.sub);
    else onopen(s.node);
    hi = -1;
    listOpen = false;
  }

  /** Drop the last word: the sub kind, the kind, Crafted, then the place. */
  function dropLast() {
    if (sub) onkind(cat, "");
    else if (cat) onkind("", "");
    else if (crafted) oncrafted(false);
    else if (!root) ongo(tree.root);
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      listOpen = true;
      if (!flat.length) return;
      const d = e.key === "ArrowDown" ? 1 : -1;
      hi = hi < 0 ? (d > 0 ? 0 : flat.length - 1) : (hi + d + flat.length) % flat.length;
    } else if (e.key === "Enter" || (e.key === "Tab" && hi >= 0 && showList)) {
      if (hi >= 0 && showList) {
        e.preventDefault();
        pick(flat[hi]);
      } else listOpen = false;
    } else if (e.key === "Escape") {
      if (showList) {
        e.stopPropagation();
        listOpen = false;
        hi = -1;
      } else input?.blur();
    } else if (e.key === "Backspace" && !q) {
      dropLast();
    }
  }

  async function openMenu(m: string) {
    listOpen = false;
    menu = menu === m ? null : m;
    await tick();
  }
  function goFromMenu(n: Node) {
    menu = null;
    ongo(n);
  }

  // A word's neighbours: up the path, beside it, inside it. A place that
  // holds nothing of its own offers the areas below it instead of its next
  // level (the storage is one pick away, not three).
  const atFlat = $derived(at.type === "place" && (at.level === "root" || at.level === "system" || at.level === "body"));
  const up = $derived(ancestors(at).filter((x) => x !== at));
  const beside = $derived(at.parent && !atFlat ? kidsOf(at.parent).filter((x) => x !== at) : []);
  const inside = $derived(atFlat ? areasBelow(at, count).slice(0, 8).map((a) => a.area as Node) : kidsOf(at));
  const kinds = $derived(kindsIn(kindsFrom));
  const subs = $derived(
    cat
      ? (SUBCATEGORIES[cat as Category] ?? []).map((s) => ({
          s,
          n: kindsFrom
            .filter((x) => subcategoryOf(x, cat as Category) === s)
            .reduce((t, x) => t + x.item.stack, 0),
        }))
      : [],
  );
</script>

{#snippet placeRow(n: Node, note = "")}
  {@const Icon = nodeIcon(n)}
  <button type="button" class="menu-row" class:cur={n === at} role="menuitem" onclick={() => goFromMenu(n)}>
    <Icon size={14} /><span class="menu-name">{n.name}{#if note}<span class="o-note">· {note}</span>{/if}</span><span
      class="menu-n">{fmtN(count(n))}</span
    >
  </button>
{/snippet}

<div class="field" class:focus={focused || !!menu}>
  <span class="anchor">
    <span class="word">
      <button type="button" class="w-open" aria-haspopup="menu" aria-expanded={menu === "place"} onclick={() => openMenu("place")}>
        {placeLabel}<Chevron open={menu === "place"} size={13} />
      </button>
      {#if !root}
        <button type="button" class="w-x" aria-label="Look everywhere" title="Look everywhere" onclick={() => ongo(tree.root)}>
          <X size={12} />
        </button>
      {/if}
    </span>
    <Popup open={menu === "place"} onclose={() => (menu = null)} label="Places" class="menu-pop">
      <div class="menu" role="menu" aria-label="Places">
        {#if up.length}
          <span class="menu-g">Up</span>
          {#each up as n (n.id)}{@render placeRow(n)}{/each}
        {/if}
        {#if inside.length}
          <span class="menu-g">{atFlat ? "Biggest here" : "Inside"}</span>
          {#each inside as n (n.id)}{@render placeRow(n, atFlat ? placeNote(n) : "")}{/each}
        {/if}
        {#if beside.length}
          <span class="menu-g">Beside</span>
          {#each beside as n (n.id)}{@render placeRow(n)}{/each}
        {/if}
        <button type="button" class="menu-row more" role="menuitem" onclick={() => (menu = "tree")}>
          <span></span><span class="menu-name">All places, as a tree…</span><span></span>
        </button>
      </div>
    </Popup>
    <Popup open={menu === "tree"} onclose={() => (menu = null)} label="All places" class="places-pop">
      <div role="tree">
        <TreeNode node={tree.root} depth={0} {open} forceOpen={false} {isAt} {count} {kidsOf} ongo={goFromMenu} />
      </div>
    </Popup>
  </span>

  {#if crafted}
    <span class="word">
      <span class="w-label">Crafted</span>
      <button type="button" class="w-x" aria-label="Drop Crafted" title="Drop Crafted" onclick={() => oncrafted(false)}>
        <X size={12} />
      </button>
    </span>
  {/if}

  {#if cat && items}
    <span class="anchor">
      <span class="word">
        <button type="button" class="w-open" aria-haspopup="menu" aria-expanded={menu === "kind"} onclick={() => openMenu("kind")}>
          {cat}<Chevron open={menu === "kind"} size={13} />
        </button>
        <button type="button" class="w-x" aria-label="Drop {cat}" title="Drop {cat}" onclick={() => onkind("", "")}>
          <X size={12} />
        </button>
      </span>
      <Popup open={menu === "kind"} onclose={() => (menu = null)} label="Kinds" class="menu-pop">
        <div class="menu" role="menu" aria-label="Kinds">
          {#each kinds as k (k.cat)}
            <button
              type="button"
              class="menu-row"
              class:cur={k.cat === cat}
              role="menuitem"
              onclick={() => ((menu = null), onkind(k.cat, ""))}
            >
              <span></span><span class="menu-name">{k.cat}</span><span class="menu-n">{fmtN(k.n)}</span>
            </button>
          {/each}
        </div>
      </Popup>
    </span>
  {/if}

  {#if cat && sub && items}
    <span class="anchor">
      <span class="word">
        <button type="button" class="w-open" aria-haspopup="menu" aria-expanded={menu === "sub"} onclick={() => openMenu("sub")}>
          {sub}<Chevron open={menu === "sub"} size={13} />
        </button>
        <button type="button" class="w-x" aria-label="Drop {sub}" title="Drop {sub}" onclick={() => onkind(cat, "")}>
          <X size={12} />
        </button>
      </span>
      <Popup open={menu === "sub"} onclose={() => (menu = null)} label="{cat} kinds" class="menu-pop">
        <div class="menu" role="menu" aria-label="{cat} kinds">
          {#each subs as x (x.s)}
            <button
              type="button"
              class="menu-row"
              class:cur={x.s === sub}
              class:off={!x.n}
              role="menuitem"
              disabled={!x.n}
              onclick={() => ((menu = null), onkind(cat, x.s))}
            >
              <span></span><span class="menu-name">{x.s}</span><span class="menu-n">{fmtN(x.n)}</span>
            </button>
          {/each}
        </div>
      </Popup>
    </span>
  {/if}

  <span class="anchor grow">
    <input
      bind:this={input}
      class="text"
      type="search"
      value={q}
      {placeholder}
      aria-label={placeholder}
      role="combobox"
      aria-expanded={showList}
      aria-controls="scope-list"
      aria-autocomplete="list"
      onfocus={() => ((focused = true), (listOpen = true), (menu = null))}
      onblur={() => ((focused = false), (hi = -1))}
      oninput={(e) => (onq(e.currentTarget.value), (listOpen = true), (hi = -1))}
      {onkeydown}
    />
    {#if showList}
      <div class="list" id="scope-list" role="listbox" aria-label="Suggestions">
        {#each groups as g (g.title)}
          <span class="menu-g">{g.title}</span>
          {#each g.items as s (s.type + s.label + (s.type === "place" || s.type === "thing" ? s.node.id : s.type === "kind" ? s.cat : ""))}
            {@const i = flat.indexOf(s)}
            <!-- mousedown keeps the focus in the input, so the pick lands -->
            <div
              class="opt"
              class:on={i === hi}
              role="option"
              tabindex="-1"
              aria-selected={i === hi}
              onmousedown={(e) => e.preventDefault()}
              onclick={() => pick(s)}
              onkeydown={() => {}}
              onmouseenter={() => (hi = i)}
            >
              <span class="o-label">{s.label}{#if s.note}<span class="o-note">· {s.note}</span>{/if}</span>
              <span class="menu-n">{fmtN(s.n)}</span>
            </div>
          {/each}
        {/each}
      </div>
    {/if}
  </span>
</div>

<style>
  .field {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 38px;
    padding: 4px 6px;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--bg-raised);
    transition: border-color var(--transition-fast);
  }
  .field.focus {
    border-color: var(--accent);
  }
  .anchor {
    position: relative;
    display: inline-flex;
  }
  .grow {
    flex: 1;
    min-width: 120px;
  }
  .word {
    display: inline-flex;
    align-items: center;
    height: 28px;
    border-radius: var(--radius-chip);
    background: color-mix(in srgb, var(--text) 9%, transparent);
    color: var(--text-strong);
    font-size: var(--text-control);
    white-space: nowrap;
  }
  .w-open,
  .w-label {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 100%;
    padding: 0 6px 0 9px;
    border: 0;
    background: transparent;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  .w-label {
    cursor: default;
  }
  .w-open :global(.chev) {
    color: var(--text-faint);
  }
  .w-open:hover {
    color: var(--text-strong);
    background: color-mix(in srgb, var(--text) 6%, transparent);
    border-radius: var(--radius-chip);
  }
  .w-x {
    display: grid;
    place-items: center;
    height: 100%;
    padding: 0 7px 0 3px;
    border: 0;
    background: transparent;
    color: var(--text-faint);
    cursor: pointer;
  }
  .w-x:hover {
    color: var(--text);
  }
  .text {
    flex: 1;
    width: 100%;
    min-width: 0;
    height: 28px;
    padding: 0 6px;
    border: 0;
    outline: none;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: var(--text-control);
  }
  .text::placeholder {
    color: var(--text-faint);
  }
  .list {
    position: absolute;
    top: calc(100% + 8px);
    left: -6px;
    z-index: 20;
    width: min(620px, 80vw);
    max-height: 60vh;
    overflow-y: auto;
    padding: 6px;
    display: flex;
    flex-direction: column;
    gap: 1px;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--bg-raised);
    box-shadow: var(--shadow-panel);
  }
  .opt {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 12px;
    align-items: center;
    padding: 6px 8px;
    border-radius: var(--radius);
    color: var(--text);
    font-size: var(--text-control);
    cursor: pointer;
  }
  .opt.on {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    color: var(--text-strong);
  }
  .o-label {
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .o-note {
    margin-left: 0.4em;
    color: var(--text-dim);
  }
  .menu-g {
    padding: 8px 8px 3px;
    font-family: var(--font-mono);
    font-size: var(--text-badge);
    letter-spacing: var(--label-tracking);
    text-transform: uppercase;
    color: var(--text-faint);
  }
  .anchor :global(.places-pop) {
    width: 300px;
    max-height: 60vh;
    overflow-y: auto;
    padding: 6px;
    border-radius: var(--radius-lg);
    background: var(--bg-raised);
    border: 1px solid var(--border);
    box-shadow: var(--shadow-panel);
  }
  .anchor :global(.menu-pop) {
    min-width: 260px;
    max-height: 60vh;
    overflow-y: auto;
    padding: 4px;
    border-radius: var(--radius-lg);
    background: var(--bg-raised);
    border: 1px solid var(--border);
    box-shadow: var(--shadow-panel);
  }
  .menu {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .menu-row {
    display: grid;
    grid-template-columns: 14px minmax(0, 1fr) auto;
    gap: 8px;
    align-items: center;
    padding: 6px 8px;
    border: 0;
    border-radius: var(--radius);
    background: transparent;
    color: var(--text-dim);
    font: inherit;
    font-size: var(--text-control);
    text-align: left;
    cursor: pointer;
  }
  .menu-row:hover {
    background: color-mix(in srgb, var(--text) 5%, transparent);
    color: var(--text);
  }
  .menu-row.cur {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    color: var(--text-strong);
  }
  .menu-row.off {
    opacity: 0.4;
    cursor: default;
  }
  .menu-row.more .menu-name {
    color: var(--text-faint);
  }
  .menu-name {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .menu-n {
    font-family: var(--font-mono);
    font-size: var(--text-small);
    color: var(--text-faint);
  }
</style>
