<script lang="ts">
  import type { Snippet } from "svelte";

  // CatalogRow — the unified catalog row used by every Starlume catalog
  // (Items, Resources, Missions, Manufacturers). One anatomy:
  //
  //   [ gutter ] [ caret ] [ title (+ meta) ] ……… [ actions ] [ right ]
  //
  // Expansion is always an inline accordion: the `children` render below the
  // row when `open`. Open state is controlled by the parent (so it can persist
  // in browse state) via `open` + `ontoggle`. See docs/frontend.md.
  interface Props {
    title: string;
    gutter?: Snippet;
    meta?: Snippet;
    right?: Snippet;
    actions?: Snippet;
    expandable?: boolean;
    open?: boolean;
    dim?: boolean;
    /** Left-pad in px for nested rows. */
    indent?: number;
    ontoggle?: () => void;
    /** Expansion content — shown only when open. */
    children?: Snippet;
  }

  let {
    title,
    gutter,
    meta,
    right,
    actions,
    expandable = false,
    open = false,
    dim = false,
    indent = 0,
    ontoggle,
    children,
  }: Props = $props();

  const canExpand = $derived(expandable && children != null);

  function toggle() {
    if (canExpand) ontoggle?.();
  }
</script>

<div class="row-wrap">
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div
    class="row"
    class:can-expand={canExpand}
    style:padding-left="{10 + indent}px"
    role={canExpand ? "button" : undefined}
    tabindex={canExpand ? 0 : undefined}
    aria-expanded={canExpand ? open : undefined}
    onclick={toggle}
    onkeydown={(e) => {
      if (canExpand && (e.key === "Enter" || e.key === " ")) {
        e.preventDefault();
        toggle();
      }
    }}
  >
    {#if gutter}<span class="gutter">{@render gutter()}</span>{/if}
    {#if expandable}
      <span class="caret" class:open>{canExpand ? (open ? "▾" : "▸") : ""}</span>
    {/if}
    <span class="main">
      <span class="title" class:dim>{title}</span>
      {#if meta}<span class="meta">{@render meta()}</span>{/if}
    </span>
    {#if right || actions}
      <span class="end">
        {#if actions}<span class="actions">{@render actions()}</span>{/if}
        {#if right}{@render right()}{/if}
      </span>
    {/if}
  </div>
  {#if canExpand && open && children}
    <div class="expansion">{@render children()}</div>
  {/if}
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 10px;
    border-radius: 6px;
    cursor: default;
    transition: background var(--transition-fast);
  }
  .row.can-expand {
    cursor: pointer;
  }
  .row.can-expand:hover,
  .row.can-expand:focus-visible {
    outline: none;
    background: linear-gradient(90deg, var(--bg-raised), transparent 85%);
  }

  .gutter {
    width: 40px;
    flex: 0 0 auto;
    display: flex;
    justify-content: flex-start;
  }
  .caret {
    width: 10px;
    flex: 0 0 auto;
    font-size: 0.7rem;
    color: var(--text-faint);
  }
  .caret.open {
    color: var(--accent);
  }

  .main {
    flex: 1;
    min-width: 0;
  }
  .title {
    display: block;
    font-size: 0.9rem;
    font-weight: var(--weight-semibold);
    color: var(--text-strong);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .title.dim {
    color: var(--text-dim);
  }
  .meta {
    display: flex;
    gap: 6px;
    align-items: center;
    flex-wrap: wrap;
    margin-top: 3px;
    color: var(--text-dim);
    font-size: 0.72rem;
  }

  .end {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: 0 0 auto;
    justify-content: flex-end;
  }
  .actions {
    display: flex;
    gap: 9px;
    color: var(--text-faint);
  }
</style>
