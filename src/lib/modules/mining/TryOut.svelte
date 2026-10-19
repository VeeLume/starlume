<script lang="ts">
  // A try-out against the saved loadout: the saved one and the try-out as
  // outcome bars (the change beside the try-out's number), then Revert /
  // Save as new / Save on one quiet row. With `minus` the bars read as what
  // a crew mate adds beside your ship. Without a saved loadout (`saved`
  // false) only the try-out and Save as new. `figure` replaces the bars
  // (Loadouts measured on every rock draws the try-out's comfort zone).
  import type { Snippet } from "svelte";
  import type { EvalView } from "$lib/bindings";
  import OutcomeBar from "./OutcomeBar.svelte";

  let {
    was,
    now,
    changes,
    saved = true,
    minus = null,
    saveLabel = "Save",
    name,
    onrevert,
    onsave,
    onsaveas,
    figure = undefined,
  }: {
    was: EvalView | null | undefined;
    now: EvalView | null | undefined;
    changes: number;
    /** There is a saved loadout to compare with and write into. */
    saved?: boolean;
    /** A crew mate's: subtracted, the bars show what they add. */
    minus?: number | null;
    saveLabel?: string;
    /** Suggested name for Save as new. */
    name: string;
    onrevert: () => void;
    onsave: () => void;
    onsaveas: (name: string) => Promise<string | null>;
    /** In place of the outcome bars. */
    figure?: Snippet;
  } = $props();

  let naming = $state(false);
  let newName = $state("");
  let error = $state("");

  const pct = (e: EvalView | null | undefined) =>
    e === undefined ? undefined : e?.difficulty ? Math.round(e.difficulty.controlled * 100) - (minus ?? 0) : null;
  const a = $derived(pct(was));
  const b = $derived(pct(now));
  const delta = $derived(a !== null && a !== undefined && b !== null && b !== undefined ? b - a : 0);
  const fig = (v: number | null | undefined) => (v === undefined ? "…" : v === null ? "—" : minus !== null ? `+${v}` : `${v}`);

  function startNaming() {
    newName = name;
    error = "";
    naming = true;
  }
  async function saveAs() {
    error = (await onsaveas(newName)) ?? "";
    if (!error) naming = false;
  }
</script>

{#snippet bar(e: EvalView | null | undefined, v: number | null | undefined)}
  {#if minus !== null}
    <span class="add"><i style:width="{Math.max(0, v ?? 0)}%"></i></span>
  {:else}
    <OutcomeBar d={e?.difficulty ?? null} height={8} />
  {/if}
{/snippet}

<div class="try">
  {#if figure}{@render figure()}{:else}
  <div class="cmp">
    {#if saved}
      <span class="k">saved</span>{@render bar(was, a)}<b>{fig(a)}</b>
    {/if}
    <span class="k">try-out</span>{@render bar(now, b)}<b
      >{fig(b)}{#if delta}<small class:up={delta > 0} class:dn={delta < 0}>{delta > 0 ? "+" : "−"}{Math.abs(delta)}</small>{/if}</b
    >
  </div>
  {/if}
  <div class="acts">
    {#if naming}
      <!-- svelte-ignore a11y_autofocus -->
      <input
        class="input"
        bind:value={newName}
        aria-label="Name of the new loadout"
        autofocus
        onkeydown={(e) => e.key === "Enter" && void saveAs()}
      />
      <span class="grow"></span>
      <button type="button" class="btn" onclick={() => (naming = false)}>Cancel</button>
      <button type="button" class="btn primary" disabled={!newName.trim()} onclick={() => void saveAs()}>Save as new</button>
    {:else}
      <span class="faint">{saved ? `${changes} change${changes === 1 ? "" : "s"}` : "Not saved yet"}</span>
      <span class="grow"></span>
      {#if saved}<button type="button" class="btn" onclick={onrevert}>Revert</button>{/if}
      <button type="button" class="btn" class:primary={!saved} onclick={startNaming}>Save as new</button>
      {#if saved}<button type="button" class="btn primary" onclick={onsave}>{saveLabel}</button>{/if}
    {/if}
  </div>
  {#if error}<p class="error">{error}</p>{/if}
</div>

<style>
  .try {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px 0;
    border-block: 1px solid color-mix(in srgb, var(--text) 8%, transparent);
  }
  .cmp {
    display: grid;
    grid-template-columns: 52px minmax(0, 1fr) 64px;
    gap: 5px 10px;
    align-items: center;
  }
  .k {
    font-size: 0.66rem;
    letter-spacing: 0.06em;
    text-transform: var(--label-transform);
    color: var(--text-faint);
  }
  .add {
    display: block;
    height: 8px;
    border-radius: 5px;
    overflow: hidden;
    background: color-mix(in srgb, var(--text) 6%, transparent);
  }
  .add i {
    display: block;
    height: 100%;
    background: color-mix(in srgb, var(--good) 70%, transparent);
  }
  b {
    font-family: var(--font-mono);
    font-size: 0.86rem;
    text-align: right;
    white-space: nowrap;
  }
  small {
    font-size: 0.7rem;
    margin-left: 4px;
  }
  .up {
    color: var(--good);
  }
  .dn {
    color: var(--bad);
  }
  .acts {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--text-small);
  }
  .acts .btn {
    padding: 3px 10px;
    white-space: nowrap;
  }
  .acts .input {
    width: 12rem;
    font-size: var(--text-small);
    padding-block: 3px;
  }
  .grow {
    flex: 1;
  }
  .faint {
    color: var(--text-faint);
  }
  .error {
    margin: 0;
  }
</style>
