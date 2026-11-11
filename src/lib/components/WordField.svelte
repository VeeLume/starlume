<script lang="ts" module>
  // Shared by My Items and Crafting (decision 'My Items opens on the few you
  // care about, and one field is its only scope line', 2026-11-11): a
  // page's scope as words, then free text. A candidate for the kit once a
  // third page wants it.
  export interface FieldRow {
    key: string;
    label: string;
    note?: string;
    n?: number;
    /** A lucide icon component. */
    icon?: unknown;
    cur?: boolean;
    off?: boolean;
    pick: () => void;
  }
  export interface FieldGroup {
    title?: string;
    rows: FieldRow[];
  }
  export interface FieldWord {
    key: string;
    label: string;
    /** Its neighbours, opened by the word. */
    menu?: FieldGroup[];
    /** Drops the word; without it the word stays. */
    remove?: () => void;
    removeLabel?: string;
  }
</script>

<script lang="ts">
  // The field: words that each open their neighbours with counts (× drops
  // one), then free text. Focused, it shows `suggestions`; ↑↓ pick, Enter
  // or Tab takes the pick, Esc closes, Backspace on empty text drops the
  // last word (`onback`). Picks are the caller's: each row carries its own.
  import type { Component } from "svelte";
  import { Popup } from "@veelume/ui";
  import { X } from "lucide-svelte";
  import Chevron from "$lib/components/Chevron.svelte";

  let {
    words,
    q,
    placeholder,
    suggestions,
    onq,
    onback,
  }: {
    words: FieldWord[];
    q: string;
    placeholder: string;
    suggestions: FieldGroup[];
    onq: (q: string) => void;
    onback: () => void;
  } = $props();

  let input = $state<HTMLInputElement>();
  let focused = $state(false);
  let listOpen = $state(false);
  let hi = $state(-1);
  let menu = $state<string | null>(null);

  const groups = $derived(suggestions.filter((g) => g.rows.length));
  const flat = $derived(groups.flatMap((g) => g.rows));
  const showList = $derived(focused && listOpen && flat.length > 0);
  const fmtN = (n: number) => n.toLocaleString("en-US");
  const iconOf = (r: FieldRow) => r.icon as Component<{ size?: number }> | undefined;

  function take(r: FieldRow) {
    if (r.off) return;
    menu = null;
    listOpen = false;
    hi = -1;
    r.pick();
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
        take(flat[hi]);
      } else listOpen = false;
    } else if (e.key === "Escape") {
      if (showList) {
        e.stopPropagation();
        listOpen = false;
        hi = -1;
      } else input?.blur();
    } else if (e.key === "Backspace" && !q) {
      onback();
    }
  }
</script>

{#snippet rows(list: FieldGroup[], option: boolean)}
  {#each list as g, gi (g.title ?? gi)}
    {#if g.title}<span class="wf-group">{g.title}</span>{/if}
    {#each g.rows as r (r.key)}
      {@const Icon = iconOf(r)}
      {#if option}
        {@const i = flat.indexOf(r)}
        <!-- mousedown keeps the focus in the input, so the pick lands -->
        <div
          class="row"
          class:on={i === hi}
          class:off={r.off}
          role="option"
          tabindex="-1"
          aria-selected={i === hi}
          onmousedown={(e) => e.preventDefault()}
          onclick={() => take(r)}
          onkeydown={() => {}}
          onmouseenter={() => (hi = i)}
        >
          <span class="wf-ic">{#if Icon}<Icon size={14} />{/if}</span>
          <span class="wf-label">{r.label}{#if r.note}<span class="wf-note">· {r.note}</span>{/if}</span>
          <span class="wf-n">{r.n != null ? fmtN(r.n) : ""}</span>
        </div>
      {:else}
        <button type="button" class="row" class:cur={r.cur} class:off={r.off} disabled={r.off} role="menuitem" onclick={() => take(r)}>
          <span class="wf-ic">{#if Icon}<Icon size={14} />{/if}</span>
          <span class="wf-label">{r.label}{#if r.note}<span class="wf-note">· {r.note}</span>{/if}</span>
          <span class="wf-n">{r.n != null ? fmtN(r.n) : ""}</span>
        </button>
      {/if}
    {/each}
  {/each}
{/snippet}

<div class="field" class:focus={focused || !!menu}>
  {#each words as w (w.key)}
    <span class="anchor">
      <span class="word">
        {#if w.menu}
          <button
            type="button"
            class="w-open"
            aria-haspopup="menu"
            aria-expanded={menu === w.key}
            onclick={() => ((listOpen = false), (menu = menu === w.key ? null : w.key))}
          >
            {w.label}<Chevron open={menu === w.key} size={13} />
          </button>
        {:else}
          <span class="w-label">{w.label}</span>
        {/if}
        {#if w.remove}
          <button type="button" class="w-x" aria-label={w.removeLabel ?? `Drop ${w.label}`} title={w.removeLabel ?? `Drop ${w.label}`} onclick={w.remove}>
            <X size={12} />
          </button>
        {/if}
      </span>
      {#if w.menu}
        <Popup open={menu === w.key} onclose={() => (menu = null)} label={w.label} class="menu-pop">
          <div class="menu" role="menu" aria-label={w.label}>{@render rows(w.menu, false)}</div>
        </Popup>
      {/if}
    </span>
  {/each}

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
      aria-controls="word-field-list"
      aria-autocomplete="list"
      onfocus={() => ((focused = true), (listOpen = true), (menu = null))}
      onblur={() => ((focused = false), (hi = -1))}
      oninput={(e) => (onq(e.currentTarget.value), (listOpen = true), (hi = -1))}
      {onkeydown}
    />
    {#if showList}
      <div class="list" id="word-field-list" role="listbox" aria-label="Suggestions">{@render rows(groups, true)}</div>
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
  .wf-group {
    padding: 8px 8px 3px;
    font-family: var(--font-mono);
    font-size: var(--text-badge);
    letter-spacing: var(--label-tracking);
    text-transform: uppercase;
    color: var(--text-faint);
  }
  .row {
    display: grid;
    grid-template-columns: 14px minmax(0, 1fr) auto;
    gap: 8px;
    align-items: center;
    padding: 6px 8px;
    border: 0;
    border-radius: var(--radius);
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: var(--text-control);
    text-align: left;
    cursor: pointer;
  }
  button.row {
    color: var(--text-dim);
  }
  button.row:hover {
    background: color-mix(in srgb, var(--text) 5%, transparent);
    color: var(--text);
  }
  .row.on {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    color: var(--text-strong);
  }
  .row.cur {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    color: var(--text-strong);
  }
  .row.off {
    opacity: 0.4;
    cursor: default;
  }
  .wf-ic {
    display: grid;
    place-items: center;
    color: var(--text-dim);
  }
  .wf-label {
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .wf-note {
    margin-left: 0.4em;
    color: var(--text-dim);
  }
  .wf-n {
    font-family: var(--font-mono);
    font-size: var(--text-small);
    color: var(--text-faint);
  }
</style>
