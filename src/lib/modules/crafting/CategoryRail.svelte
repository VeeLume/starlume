<script lang="ts">
  // The catalog's categories, the way My Items' tree sits beside its cards:
  // item types down the side, the open type's own filters nested under it.
  // It keeps to its column, so narrowing never moves the list.
  import Chevron from "$lib/components/Chevron.svelte";
  import { Layers } from "lucide-svelte";
  import { lookOf } from "./shelf";

  type Count = [string, number];
  type Field = "kind" | "weight" | "class" | "grade";

  let {
    all,
    tops,
    kinds,
    weights,
    classes,
    grades,
    values,
    ontype,
    ontoggle,
  }: {
    all: number;
    tops: Count[];
    kinds: Count[];
    weights: Count[];
    classes: Count[];
    grades: Count[];
    values: Record<"type" | Field, string>;
    ontype: (t: string) => void;
    ontoggle: (field: Field, v: string) => void;
  } = $props();

  const fmtN = (n: number) => n.toLocaleString("en-US");
  const open = $derived(kinds.length > 1 || weights.length > 1 || classes.length > 1 || grades.length > 1);
</script>

{#snippet leaf(field: Field, list: Count[])}
  {#each list as [v, n] (v)}
    <button type="button" class="row leaf" class:on={values[field] === v} aria-pressed={values[field] === v} onclick={() => ontoggle(field, v)}>
      <span class="name">{v}</span><span class="n">{fmtN(n)}</span>
    </button>
  {/each}
{/snippet}

<nav class="rail" aria-label="Categories">
  <button type="button" class="row top" class:on={!values.type} aria-pressed={!values.type} onclick={() => ontype("")}>
    <span class="caret"></span><span class="ico" style="--hue: var(--text-dim)"><Layers size={14} /></span>
    <span class="name">All</span><span class="n">{fmtN(all)}</span>
  </button>
  {#each tops as [t, n] (t)}
    {@const look = lookOf(t)}
    {@const Icon = look.icon}
    {@const active = values.type === t}
    <button type="button" class="row top" class:on={active} aria-pressed={active} aria-expanded={active} onclick={() => ontype(active ? "" : t)}>
      <span class="caret"><Chevron dir="right" open={active} size={13} /></span>
      <span class="ico" style="--hue: {look.hue}"><Icon size={14} /></span>
      <span class="name">{t}</span><span class="n">{fmtN(n)}</span>
    </button>
    {#if active && open}
      <div class="nest">
        {#if kinds.length > 1}{@render leaf("kind", kinds)}{/if}
        {#if weights.length > 1}
          <div class="label">Weight</div>
          {@render leaf("weight", weights)}
        {/if}
        {#if classes.length > 1}
          <div class="label">Class</div>
          {@render leaf("class", classes)}
        {/if}
        {#if grades.length > 1}
          <div class="label">Grade</div>
          <div class="grades">
            {#each grades as [g, c] (g)}
              <button
                type="button"
                class="grade"
                class:on={values.grade === g}
                aria-pressed={values.grade === g}
                title="Grade {g}: {fmtN(c)}"
                onclick={() => ontoggle("grade", g)}>{g}</button
              >
            {/each}
          </div>
        {/if}
      </div>
    {/if}
  {/each}
</nav>

<style>
  .rail {
    flex: 0 0 186px;
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-height: 0;
    padding: 6px;
    overflow-y: auto;
    scrollbar-width: thin;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--bg-raised);
  }
  .row {
    all: unset;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 5px 6px;
    border-radius: 6px;
    font-size: 0.82rem;
    color: var(--text);
    cursor: pointer;
  }
  .row:hover {
    background: color-mix(in srgb, var(--text) 6%, transparent);
  }
  .row:focus-visible,
  .grade:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -1px;
  }
  .row.on {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    color: var(--text-strong);
  }
  .caret {
    flex: 0 0 11px;
    display: grid;
    place-items: center;
    color: var(--text-faint);
  }
  .ico {
    flex: 0 0 auto;
    display: grid;
    place-items: center;
    color: color-mix(in srgb, var(--hue) 75%, var(--text-strong));
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .n {
    font-family: var(--font-mono);
    font-size: 0.68rem;
    color: var(--text-faint);
  }
  .on .n {
    color: var(--text-dim);
  }
  .nest {
    display: flex;
    flex-direction: column;
    gap: 1px;
    margin: 1px 0 5px 11px;
    padding-left: 8px;
    border-left: 1px solid var(--border);
  }
  .leaf {
    padding: 3px 6px;
    font-size: 0.78rem;
    color: var(--text-dim);
  }
  .label {
    margin: 7px 0 2px 6px;
    font-family: var(--font-mono);
    font-size: var(--text-badge);
    color: var(--text-faint);
    text-transform: var(--label-transform);
    letter-spacing: var(--label-tracking);
  }
  .grades {
    display: flex;
    gap: 3px;
    padding: 0 6px;
  }
  .grade {
    all: unset;
    flex: 1;
    box-sizing: border-box;
    height: 24px;
    display: grid;
    place-items: center;
    border: 1px solid var(--border);
    border-radius: var(--radius-chip);
    font-family: var(--font-mono);
    font-size: 0.74rem;
    color: var(--text-dim);
    cursor: pointer;
  }
  .grade:hover {
    border-color: color-mix(in srgb, var(--text) 25%, var(--border));
    color: var(--text);
  }
  .grade.on {
    color: var(--accent);
    border-color: var(--accent-border);
    background: var(--accent-fill);
  }
</style>
